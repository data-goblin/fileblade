use clap::{Args as ClapArgs, Subcommand};
use fileblade_output::Output;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::ExitCode;
use std::sync::Arc;

mod backend;
pub use backend::run as run_backend;
mod drain;
mod filemanager1;
mod open;
pub mod roles;

#[derive(Clone, Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    Portal,
    Backend {
        #[arg(last = true, num_args = 1..)]
        arguments: Vec<OsString>,
    },
    Drain {
        #[arg(long, default_value_t = 30000, value_parser = clap::value_parser!(u64).range(1..=300000))]
        timeout_ms: u64,
        #[arg(long)]
        json: bool,
    },
    Ipc {
        #[arg(last = true, num_args = 2..)]
        arguments: Vec<OsString>,
    },
    Open {
        #[arg(required = true)]
        paths: Vec<String>,
    },
    Filemanager1,
    Roles(roles::Args),
}

pub fn run(args: Args, output: Arc<Output>) -> ExitCode {
    let result = match args.command {
        Command::Roles(args) => return roles::run(args, output),
        Command::Drain { timeout_ms, .. } => {
            let result = drain::run(timeout_ms);
            let code = match result["status"].as_str() {
                Some("drained" | "already_stopped") => 0,
                Some("busy") => 3,
                _ => 1,
            };
            return if output.machine(&result).is_ok() {
                ExitCode::from(code)
            } else {
                ExitCode::FAILURE
            };
        }
        Command::Backend { arguments } => match backend::run(arguments, Arc::clone(&output)) {
            Ok(true) => return ExitCode::SUCCESS,
            Ok(false) => return ExitCode::FAILURE,
            Err(error) => Err(error),
        },
        Command::Portal => crate::chooser::portal::serve(),
        Command::Open { paths } => open::run(&paths, false),
        Command::Filemanager1 => filemanager1::serve(),
        Command::Ipc { arguments } => crate::paths::app_root().and_then(|root| {
            Err(std::process::Command::new("qs")
                .args(["ipc", "-n", "-p"])
                .arg(root.join("app"))
                .args(["call", "--"])
                .args(arguments)
                .exec()
                .into())
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output.error(&error.to_string());
            ExitCode::FAILURE
        }
    }
}
