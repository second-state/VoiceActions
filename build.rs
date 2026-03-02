fn main() {
    // Set rpath so the binary finds bundled shared libraries at runtime
    // without requiring LD_LIBRARY_PATH / DYLD_LIBRARY_PATH.
    match std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default().as_str() {
        "macos" => {
            // @executable_path — finds libwasmedge.0.dylib alongside the binary
            println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
        }
        "linux" => {
            // $ORIGIN/lib — finds bundled .so files in the lib/ subdirectory
            println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/lib");
        }
        _ => {}
    }
}
