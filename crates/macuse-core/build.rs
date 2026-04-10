fn main() {
    // Link macOS frameworks needed for permissions and accessibility
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
}
