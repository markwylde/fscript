fn main() {
    println!("cargo:rerun-if-env-changed=FSCRIPT_PROGRAM_IMAGE_PATH");

    let image_path = std::env::var("FSCRIPT_PROGRAM_IMAGE_PATH").unwrap_or_else(|_| {
        let path = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set"))
            .join("default-program-image.json");
        std::fs::write(
            &path,
            r#"{"entry":"<entry>","modules":{"<entry>":{"items":[],"exports":[]}}}"#,
        )
        .expect("default embedded program image should be writable");
        path.to_string_lossy().into_owned()
    });

    println!("cargo:rerun-if-changed={image_path}");
    println!("cargo:rustc-env=FSCRIPT_PROGRAM_IMAGE_PATH={image_path}");
}
