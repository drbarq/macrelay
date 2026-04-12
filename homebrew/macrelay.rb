class Macrelay < Formula
  desc "Open-source MCP server for native macOS apps — with menu bar manager"
  homepage "https://github.com/drbarq/macrelay"
  url "https://github.com/drbarq/macrelay/releases/download/v1.0.0/macrelay-macos-universal"
  version "1.0.0"
  sha256 "2d7fae6b08dc314cf25841068b4359cf06d5d017604b25a453fc09435e8d300a"
  license "MIT"

  # Menu bar management app (installed alongside the server)
  resource "menubar" do
    url "https://github.com/drbarq/macrelay/releases/download/v1.0.0/macrelay-menubar-macos-universal"
    sha256 "PLACEHOLDER_MENUBAR_SHA256"
  end

  depends_on :macos
  depends_on macos: :sonoma

  def install
    bin.install "macrelay-macos-universal" => "macrelay"

    resource("menubar").stage do
      bin.install "macrelay-menubar-macos-universal" => "macrelay-menubar"
    end
  end

  test do
    assert_match "macrelay", shell_output("#{bin}/macrelay --version 2>&1", 0)
  end
end
