fn main() {
    // On macOS, set rpath to @executable_path so the binary finds
    // libwasmedge.0.dylib when placed alongside it — no need for
    // DYLD_LIBRARY_PATH at runtime.
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
    }
}
