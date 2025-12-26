fn main() {
    println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/CMakeLists.txt");
    println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/gt_bridge/gt_bridge.cc");
    println!("cargo:rerun-if-changed=../vendor/signalbackup-tools/gt_bridge/gt_bridge.h");

    let mut cmake_cfg = cmake::Config::new("../vendor/signalbackup-tools");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let deployment_target =
            std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "13.3".into());
        cmake_cfg.define("CMAKE_OSX_DEPLOYMENT_TARGET", deployment_target);
    }

    let dst = cmake_cfg
        .define("SIGNALBACKUP_BUILD_EXECUTABLE", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .build();

    let lib_dir = dst.join("lib");
    let lib_path = lib_dir.join("libsignalbackup_tools_static.a");
    println!("cargo:metadata=signalbackup_lib={}", lib_path.display());
    println!("cargo:metadata=signalbackup_lib_dir={}", lib_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=signalbackup_tools_static");
    // Force load all objects so static registrars are linked.
    if lib_path.exists() {
        println!("cargo:rustc-link-arg=-Wl,-force_load,{}", lib_path.display());
    }

    // C++ stdlib
    println!("cargo:rustc-link-lib=c++");

    // Crypto + sqlite
    println!("cargo:rustc-link-lib=crypto");
    println!("cargo:rustc-link-lib=sqlite3");

    // macOS frameworks used by signalbackup-tools
    println!("cargo:rustc-link-lib=framework=Security");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");

    println!("cargo:rerun-if-env-changed=OPENSSL_DIR");
    println!("cargo:rerun-if-env-changed=OPENSSL_LIB_DIR");
    println!("cargo:rerun-if-env-changed=OPENSSL_INCLUDE_DIR");

    if let Ok(dir) = std::env::var("OPENSSL_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", dir);
    }
    if let Ok(dir) = std::env::var("OPENSSL_DIR") {
        let lib = std::path::Path::new(&dir).join("lib");
        if lib.exists() {
            println!("cargo:rustc-link-search=native={}", lib.display());
        }
    }

    // Homebrew defaults (Apple Silicon + Intel)
    for path in [
        "/opt/homebrew/opt/openssl@3/lib",
        "/usr/local/opt/openssl@3/lib",
        "/opt/homebrew/opt/openssl/lib",
        "/usr/local/opt/openssl/lib",
    ] {
        if std::path::Path::new(path).exists() {
            println!("cargo:rustc-link-search=native={}", path);
        }
    }
}
