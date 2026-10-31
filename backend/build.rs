use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DEP_CATENARY_OSR_BRIDGE_LIB_DIR");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }

    // osr-ffi publishes the completed CMake output directory via Cargo
    // dependency metadata (requires links = "catenary_osr_bridge").
    let native_dir = PathBuf::from(
        env::var_os("DEP_CATENARY_OSR_BRIDGE_LIB_DIR")
            .expect("osr-ffi must publish DEP_CATENARY_OSR_BRIDGE_LIB_DIR"),
    );
    let native_lib = native_dir.join("libcatenary_osr_bridge.so");
    assert!(native_lib.is_file(), "missing native bridge at {}", native_lib.display());

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    // OUT_DIR = target/<profile>/build/osr-tester-<hash>/out
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("unexpected Cargo OUT_DIR structure");
    let bundled_dir = profile_dir.join("lib");
    fs::create_dir_all(&bundled_dir).expect("create native library bundle directory");
    fs::copy(&native_lib, bundled_dir.join("libcatenary_osr_bridge.so"))
        .expect("copy native OSR bridge beside executable");

    // This applies to the final binary, unlike a dependency's rustc-link-arg.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/lib");
    // Preserve cargo run even if tools move or remove the bundle directory.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", native_dir.display());
}
