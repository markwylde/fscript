use std::process::Command;

use fscript_test_support::{example_path, normalize_snapshot, write_temp_file, write_temp_project};
use insta::assert_snapshot;

#[test]
fn snapshots_help_output() {
    let output = run_cli(["--help"]);

    assert!(output.status.success());
    assert_snapshot!(
        "help_output",
        normalize_snapshot(&String::from_utf8_lossy(&output.stdout))
    );
}

#[test]
fn version_command_reports_build_metadata() {
    let output = run_cli(["version"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fscript"));
    assert!(stdout.contains("version:"));
    assert!(stdout.contains("build date:"));
    assert!(stdout.contains("target:"));
    assert!(stdout.contains("profile:"));
    assert!(stdout.contains("commit:"));
}

#[test]
fn check_command_succeeds_end_to_end() {
    let output = run_cli(["check", example_path("hello_world.fs").as_str()]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("checked"));
}

#[test]
fn run_command_succeeds_end_to_end() {
    let output = run_cli(["run", example_path("hello_world.fs").as_str()]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello, fscript user\n"
    );
}

#[test]
fn direct_entry_mode_runs_a_single_fs_argument() {
    let path = write_temp_file(
        "cli-direct-entry",
        "person = 'world'\nmessage = 'hello, ' + person",
    );
    let output = run_cli([path.as_str()]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello, world\n");
}

#[test]
fn compile_command_emits_a_runnable_binary_end_to_end() {
    let project = write_temp_project(
        "cli-compile-success",
        &[
            (
                "main.fs",
                "import { greet } from './greeter.fs'\nmessage = greet('world')",
            ),
            (
                "greeter.fs",
                "export greet = (name: String): String => 'hello, ' + name",
            ),
        ],
    );
    let main_path = project.join("main.fs");
    let output_path = project.join("hello-bin");
    let compile = run_cli(["compile", main_path.as_str(), output_path.as_str()]);

    assert!(
        compile.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let binary = Command::new(&output_path)
        .output()
        .expect("compiled binary should run");
    assert!(binary.status.success());
    assert_eq!(String::from_utf8_lossy(&binary.stdout), "hello, world\n");
}

#[test]
fn snapshots_compile_failure_output_end_to_end() {
    let project = write_temp_project(
        "cli-compile-failure",
        &[("main.fs", "answer = if (true) { 1 }")],
    );
    let main_path = project.join("main.fs");
    let output_path = project.join("hello-bin");
    let output = run_cli(["compile", main_path.as_str(), output_path.as_str()]);

    assert!(!output.status.success());
    assert_snapshot!(
        "compile_failure_output",
        normalize_snapshot(&String::from_utf8_lossy(&output.stderr))
    );
}

fn run_cli<I, S>(args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fscript"))
        .args(args)
        .output()
        .expect("cli command should execute")
}
