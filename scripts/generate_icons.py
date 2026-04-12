#!/usr/bin/env python3
"""
Generate icon pack from MR2.png.

Steps:
1. Flood-fill remove outer dark background → transparent
2. Drop isolated pixel clusters (removes watermark/sparkle)
3. Crop tight to logo content bounding box
4. Pad to square canvas (centered)
5. Generate app iconset (10 sizes) → .icns
6. Generate plugin icons (64, 128, 256, 512)
7. Generate menu bar icons from mr2-white.png (white on transparent) at 18 and 36px
"""

import subprocess
import sys
from collections import deque
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import label

ASSETS = Path(__file__).parent.parent / "assets"
SRC = ASSETS / "macrelay_logo_source.png"

ICONSET_DIR = ASSETS / "macrelay.iconset"
ICNS_OUT = ASSETS / "macrelay.icns"

# App icon sizes: (logical_size, scale) → filename
ICONSET_SIZES = [
    (16, 1, "icon_16x16.png"),
    (16, 2, "icon_16x16@2x.png"),
    (32, 1, "icon_32x32.png"),
    (32, 2, "icon_32x32@2x.png"),
    (128, 1, "icon_128x128.png"),
    (128, 2, "icon_128x128@2x.png"),
    (256, 1, "icon_256x256.png"),
    (256, 2, "icon_256x256@2x.png"),
    (512, 1, "icon_512x512.png"),
    (512, 2, "icon_512x512@2x.png"),
]

PLUGIN_SIZES = [64, 128, 256, 512]
MENUBAR_SIZES = [18, 36]

# Padding added around the cropped logo before squaring (in pixels at source resolution)
CROP_PADDING = 30


def remove_background(img: Image.Image, threshold: int = 35) -> Image.Image:
    """
    Flood-fill from all four corners to make the outer dark background transparent.
    Only pixels where max(R,G,B) < threshold are considered background.
    Preserves inner dark areas (letter interiors) which are only reachable
    through non-dark border pixels.
    """
    rgba = img.convert("RGBA")
    pixels = np.array(rgba, dtype=np.uint8)
    h, w = pixels.shape[:2]

    visited = np.zeros((h, w), dtype=bool)
    queue = deque()

    for r, c in [(0, 0), (0, w - 1), (h - 1, 0), (h - 1, w - 1)]:
        if not visited[r, c]:
            queue.append((r, c))
            visited[r, c] = True

    removed = 0
    while queue:
        r, c = queue.popleft()
        R, G, B, A = pixels[r, c]
        if max(int(R), int(G), int(B)) < threshold:
            pixels[r, c, 3] = 0
            removed += 1
            for dr, dc in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
                nr, nc = r + dr, c + dc
                if 0 <= nr < h and 0 <= nc < w and not visited[nr, nc]:
                    visited[nr, nc] = True
                    queue.append((nr, nc))

    print(f"  Background removal: {removed:,} pixels made transparent")
    return Image.fromarray(pixels, "RGBA")


def drop_isolated_clusters(img: Image.Image) -> Image.Image:
    """
    Keep only the largest connected component of non-transparent pixels.
    Removes isolated artifacts like watermarks/sparkles that float in the
    former background area after flood-fill removal.
    """
    pixels = np.array(img, dtype=np.uint8)
    alpha_mask = pixels[:, :, 3] > 0

    labeled, num_features = label(alpha_mask)
    if num_features == 0:
        return img

    # Find the largest component (skip label 0 = transparent background)
    sizes = np.bincount(labeled.ravel())
    sizes[0] = 0  # ignore background
    largest_label = sizes.argmax()

    dropped = np.sum(alpha_mask & (labeled != largest_label))
    pixels[labeled != largest_label, 3] = 0

    print(f"  Isolated clusters removed: {dropped:,} pixels ({num_features - 1} extra components dropped)")
    return Image.fromarray(pixels, "RGBA")


def crop_to_content(img: Image.Image, padding: int = CROP_PADDING) -> Image.Image:
    """
    Crop the image tight to the non-transparent content bounding box,
    then add a small uniform padding on all sides.
    """
    bbox = img.getbbox()  # (left, upper, right, lower)
    if bbox is None:
        return img

    w, h = img.size
    left  = max(0, bbox[0] - padding)
    top   = max(0, bbox[1] - padding)
    right = min(w, bbox[2] + padding)
    bottom = min(h, bbox[3] + padding)

    cropped = img.crop((left, top, right, bottom))
    print(f"  Cropped: {w}x{h} → {cropped.size[0]}x{cropped.size[1]} (bbox {bbox}, pad {padding})")
    return cropped


def pad_to_square(img: Image.Image) -> Image.Image:
    """Center the image on a square canvas (side = max dimension), transparent fill."""
    w, h = img.size
    side = max(w, h)
    square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    x = (side - w) // 2
    y = (side - h) // 2
    square.paste(img, (x, y), img)
    return square


def resize(img: Image.Image, size: int) -> Image.Image:
    return img.resize((size, size), Image.LANCZOS)


def prepare_source(path: Path, label: str = "") -> Image.Image:
    """Full pipeline: remove bg → drop isolated clusters → crop tight → pad square."""
    tag = f"[{label}] " if label else ""
    print(f"\n{tag}Processing {path.name}...")
    img = Image.open(path).convert("RGBA")
    img = remove_background(img, threshold=35)
    img = drop_isolated_clusters(img)
    img = crop_to_content(img, padding=CROP_PADDING)
    img = pad_to_square(img)
    print(f"  Final canvas: {img.size[0]}x{img.size[1]}")
    return img


def extract_white_template(img: Image.Image, brightness_threshold: int = 60) -> Image.Image:
    """
    From an already-prepared square image (mr2-white pipeline):
    remove interior dark pixels by global brightness threshold so only the
    white letterform remains on transparent background.
    """
    pixels = np.array(img, dtype=np.uint8)
    R = pixels[:, :, 0].astype(int)
    G = pixels[:, :, 1].astype(int)
    B = pixels[:, :, 2].astype(int)
    is_dark = np.maximum(np.maximum(R, G), B) < brightness_threshold
    pixels[is_dark, 3] = 0
    print(f"  Interior dark pixels removed: {is_dark.sum():,}")
    return Image.fromarray(pixels, "RGBA")


def main():
    # ── App icon source (MR2.png — coloured) ──────────────────────────────────
    square = prepare_source(SRC, "app icon")
    square_path = ASSETS / "macrelay_logo_square.png"
    square.save(square_path, "PNG")
    print(f"  Saved source: {square_path}")

    # ── App iconset ────────────────────────────────────────────────────────────
    print("\nGenerating app iconset...")
    ICONSET_DIR.mkdir(exist_ok=True)
    for logical, scale, filename in ICONSET_SIZES:
        px = logical * scale
        resize(square, px).save(ICONSET_DIR / filename, "PNG")
        print(f"  {filename} ({px}x{px})")

    print("\nCompiling .icns...")
    result = subprocess.run(
        ["iconutil", "-c", "icns", str(ICONSET_DIR), "-o", str(ICNS_OUT)],
        capture_output=True, text=True,
    )
    if result.returncode == 0:
        print(f"  Saved: {ICNS_OUT} ({ICNS_OUT.stat().st_size // 1024} KB)")
    else:
        print(f"  ERROR: {result.stderr}")
        sys.exit(1)

    # ── Plugin icons ───────────────────────────────────────────────────────────
    print("\nGenerating plugin icons...")
    for px in PLUGIN_SIZES:
        out = ASSETS / f"plugin_{px}.png"
        resize(square, px).save(out, "PNG")
        print(f"  plugin_{px}.png")

    # ── Menu bar icons (mr2-white.png) ─────────────────────────────────────────
    white_src = ASSETS / "macrelay_logo_white_source.png"
    white_square = prepare_source(white_src, "menu bar")
    template = extract_white_template(white_square)

    print("\nGenerating menu bar icons...")
    for px in MENUBAR_SIZES:
        out = ASSETS / f"menubar_v3_{px}.png"
        resize(template, px).save(out, "PNG")
        print(f"  menubar_v3_{px}.png ({px}x{px})")

    print("\nDone!")


if __name__ == "__main__":
    main()
