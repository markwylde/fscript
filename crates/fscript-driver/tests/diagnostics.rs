use fscript_driver::{DriverError, check_file, compile_file};
use fscript_test_support::{normalize_snapshot, write_temp_file, write_temp_project};
use insta::assert_snapshot;

#[test]
fn snapshots_parse_error_rendering() {
    let path = write_temp_file("driver-parse-error", "message 'hello'");
    let error = check_file(&path).expect_err("invalid syntax should fail");

    assert_snapshot!("parse_error", normalize_snapshot(&render_error(&error)));
}

#[test]
fn snapshots_type_error_rendering() {
    let path = write_temp_file(
        "driver-type-error",
        "greet = (name: String): Number => 'hello, ' + name",
    );
    let error = check_file(&path).expect_err("type mismatch should fail");

    assert_snapshot!("type_error", normalize_snapshot(&render_error(&error)));
}

#[test]
fn snapshots_compile_error_rendering() {
    let project = write_temp_project("driver-compile-error", &[("main.fs", "answer = 42")]);
    let error =
        compile_file(&project.join("main.fs"), &project).expect_err("tool failure should fail");

    assert_snapshot!("compile_error", normalize_snapshot(&render_error(&error)));
}

fn render_error(error: &DriverError) -> String {
    error.render_pretty()
}
