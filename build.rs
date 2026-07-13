use std::env;

fn main() {
    println!(
        "cargo:rustc-env=BSP_TO_GLB_BUILD_TARGET={}",
        env::var("TARGET").expect("Cargo should set TARGET")
    );
    println!(
        "cargo:rustc-env=BSP_TO_GLB_BUILD_PROFILE={}",
        env::var("PROFILE").expect("Cargo should set PROFILE")
    );
    println!("cargo:rerun-if-env-changed=BSP_TO_GLB_GIT_SHA");
    if let Ok(commit) = env::var("BSP_TO_GLB_GIT_SHA") {
        if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            panic!("BSP_TO_GLB_GIT_SHA must be a 40- or 64-character hexadecimal object ID");
        }
        println!("cargo:rustc-env=BSP_TO_GLB_SOURCE_COMMIT={commit}");
    }
}
