use std::io::{self, BufWriter, Write};
use std::path::{MAIN_SEPARATOR, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use lumo_core::{EntryKind, list_entries};

#[derive(Parser)]
#[command(
    name = "lumo",
    version,
    about = "Lumo File Explorer",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the contents of a directory
    Ls {
        /// Directory to list (defaults to the current directory)
        path: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Ls { path } => ls(path.unwrap_or_else(|| PathBuf::from("."))),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        // A closed pipe (e.g. `lumo ls | head`) is not an error worth reporting.
        Err(err) if is_broken_pipe(&err) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("lumo: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn ls(path: PathBuf) -> anyhow::Result<()> {
    let entries = list_entries(&path)?;

    let mut out = BufWriter::new(io::stdout().lock());
    for entry in entries {
        let name = entry.name.to_string_lossy();
        if entry.kind == EntryKind::Directory {
            writeln!(out, "{name}{MAIN_SEPARATOR}")
        } else {
            writeln!(out, "{name}")
        }
        .context("failed to write to stdout")?;
    }
    out.flush().context("failed to write to stdout")?;
    Ok(())
}

fn is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|cause| cause.downcast_ref::<io::Error>())
        .any(|io_err| io_err.kind() == io::ErrorKind::BrokenPipe)
}
