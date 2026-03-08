use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=FSCRIPT_RELEASE_TAG");
    println!("cargo:rerun-if-env-changed=FSCRIPT_BUILD_DATE");
    println!("cargo:rerun-if-env-changed=FSCRIPT_BUILD_TARGET");
    println!("cargo:rerun-if-env-changed=FSCRIPT_BUILD_PROFILE");
    println!("cargo:rerun-if-env-changed=FSCRIPT_GIT_SHA");

    set_env(
        "FSCRIPT_RELEASE_TAG",
        env::var("FSCRIPT_RELEASE_TAG").unwrap_or_else(|_| env::var("CARGO_PKG_VERSION").unwrap()),
    );
    set_env(
        "FSCRIPT_BUILD_DATE",
        env::var("FSCRIPT_BUILD_DATE").unwrap_or_else(|_| "unknown".to_string()),
    );
    set_env(
        "FSCRIPT_BUILD_TARGET",
        env::var("FSCRIPT_BUILD_TARGET").unwrap_or_else(|_| env::var("TARGET").unwrap()),
    );
    set_env(
        "FSCRIPT_BUILD_PROFILE",
        env::var("FSCRIPT_BUILD_PROFILE").unwrap_or_else(|_| env::var("PROFILE").unwrap()),
    );
    set_env(
        "FSCRIPT_GIT_SHA",
        env::var("FSCRIPT_GIT_SHA").unwrap_or_else(|_| "unknown".to_string()),
    );
}

fn set_env(name: &str, value: String) {
    println!("cargo:rustc-env={name}={value}");
}
