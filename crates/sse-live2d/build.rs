//! Links Live2D Cubism Core (decision Q1). The library is proprietary and never shipped with
//! this repository: point `SSE_CUBISM_CORE_DIR` at the `Core/` directory of a Cubism SDK for
//! Native you downloaded and accepted the terms for.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=SSE_CUBISM_CORE_DIR");
    let Some(dir) = env::var_os("SSE_CUBISM_CORE_DIR").map(PathBuf::from) else {
        panic!(
            "SSE_CUBISM_CORE_DIR is not set. Download the Live2D Cubism SDK for Native, accept its \
             terms, and set SSE_CUBISM_CORE_DIR to its `Core` directory."
        );
    };
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let lib_dir = match (os.as_str(), arch.as_str()) {
        ("macos", "aarch64") => dir.join("lib/macos/arm64"),
        ("macos", "x86_64") => dir.join("lib/macos/x86_64"),
        ("windows", "x86_64") => dir.join("lib/windows/x86_64/143"),
        ("linux", "x86_64") => dir.join("lib/linux/x86_64"),
        other => panic!("no Cubism Core build known for {other:?}"),
    };
    assert!(
        lib_dir.is_dir(),
        "Cubism Core library directory not found: {}",
        lib_dir.display()
    );
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if os == "windows" {
        println!("cargo:rustc-link-lib=static=Live2DCubismCore_MT");
    } else {
        println!("cargo:rustc-link-lib=static=Live2DCubismCore");
    }
}
