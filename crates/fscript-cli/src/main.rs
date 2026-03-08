use camino::Utf8PathBuf;
use clap::{CommandFactory, Parser, Subcommand};

const RELEASE_VERSION: &str = env!("FSCRIPT_RELEASE_TAG");
const BUILD_DATE: &str = env!("FSCRIPT_BUILD_DATE");
const BUILD_TARGET: &str = env!("FSCRIPT_BUILD_TARGET");
const BUILD_PROFILE: &str = env!("FSCRIPT_BUILD_PROFILE");
const GIT_SHA: &str = env!("FSCRIPT_GIT_SHA");

#[derive(Debug, Parser)]
#[command(name = "fscript")]
#[command(about = "FScript compiler and tooling")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Typecheck and validate a source file.
    Check { path: Utf8PathBuf },
    /// Run an FScript entrypoint.
    Run { path: Utf8PathBuf },
    /// Compile an FScript entrypoint to a native executable.
    Compile {
        input: Utf8PathBuf,
        output: Utf8PathBuf,
    },
    /// Show version and build information.
    Version,
}

fn main() {
    if let Err(error) = try_main() {
        eprintln!("{}", error.render_pretty());
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), fscript_driver::DriverError> {
    if let Some(path) = direct_entry_file() {
        let summary = fscript_driver::run_file(&path)?;
        if let Some(value) = summary.last_value {
            println!("{value}");
        }
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Check { path }) => {
            let summary = fscript_driver::check_file(&path)?;
            println!("checked {} ({} tokens)", summary.path, summary.token_count);
        }
        Some(Command::Run { path }) => {
            let summary = fscript_driver::run_file(&path)?;
            if let Some(value) = summary.last_value {
                println!("{value}");
            }
        }
        Some(Command::Compile { input, output }) => {
            fscript_driver::compile_file(&input, &output)?;
        }
        Some(Command::Version) => {
            print_version();
        }
        None => {
            let mut command = Cli::command();
            if let Err(error) = command.print_help() {
                eprintln!("failed to print CLI help: {error}");
                std::process::exit(1);
            }
            println!();
        }
    }

    Ok(())
}

fn print_version() {
    print!("{}", version_output());
}

fn version_output() -> String {
    format!(
        "fscript\nversion: {RELEASE_VERSION}\nbuild date: {BUILD_DATE}\ntarget: {BUILD_TARGET}\nprofile: {BUILD_PROFILE}\ncommit: {GIT_SHA}\n"
    )
}

fn direct_entry_file() -> Option<Utf8PathBuf> {
    let mut args = std::env::args_os();
    let _program_name = args.next();
    let first = args.next()?;

    if args.next().is_some() {
        return None;
    }

    let path = Utf8PathBuf::from_path_buf(first.into()).ok()?;

    (path.extension() == Some("fs")).then_some(path)
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;
    use insta::assert_snapshot;

    use super::{BUILD_DATE, BUILD_PROFILE, BUILD_TARGET, Cli, GIT_SHA, RELEASE_VERSION};

    #[test]
    fn snapshots_cli_help_output() {
        let mut command = Cli::command();
        let mut help = Vec::new();
        command
            .write_long_help(&mut help)
            .expect("help output should render");

        assert_snapshot!(
            "cli_help",
            String::from_utf8(help).expect("help output should be utf-8")
        );
    }

    #[test]
    fn version_output_contains_build_metadata() {
        let output = super::version_output();

        assert!(output.contains("fscript"));
        assert!(output.contains(&format!("version: {RELEASE_VERSION}")));
        assert!(output.contains(&format!("build date: {BUILD_DATE}")));
        assert!(output.contains(&format!("target: {BUILD_TARGET}")));
        assert!(output.contains(&format!("profile: {BUILD_PROFILE}")));
        assert!(output.contains(&format!("commit: {GIT_SHA}")));
    }
}
