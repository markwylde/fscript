use camino::Utf8PathBuf;
use clap::{CommandFactory, Parser, Subcommand};

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

    use super::Cli;

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
}
