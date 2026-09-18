use clap::error::ErrorKind;
use fileblade::backend;
use fileblade::public_cli::{self, RootCommand};
use fileblade::{AppError, AppResult, server};
use fileblade_output::{Format, Output};
use std::io;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    match public_cli::parse(std::env::args_os()) {
        Ok(cli) => {
            let output = Arc::new(Output::new(cli.output.into(), cli.quiet));
            if let RootCommand::Native(args) = cli.command {
                return fileblade::native::run(args, output);
            }
            match execute(cli.command, Arc::clone(&output)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) if broken_pipe(&error) => ExitCode::SUCCESS,
                Err(error) => {
                    let _ = output.error(&error.to_string());
                    ExitCode::FAILURE
                }
            }
        }
        Err(error) => clap_error(error),
    }
}

fn execute(command: RootCommand, output: Arc<Output>) -> AppResult<()> {
    if let RootCommand::ExecHex(args) = &command {
        return fileblade::drop_target::exec_hex(&args.values);
    }
    if fileblade::lease::selected_root()?.is_some() {
        if let RootCommand::Preferences(changes) = &command {
            let changing =
                changes.trash_retention_days.is_some() || changes.agent_management.is_some();
            let mut arguments = vec![
                if changing {
                    "preferences-set"
                } else {
                    "preferences-read"
                }
                .into(),
            ];
            if let Some(days) = changes.trash_retention_days {
                arguments.extend(["--trash-retention-days".into(), days.to_string().into()]);
            }
            if let Some(enabled) = changes.agent_management {
                arguments.extend(["--agent-management".into(), enabled.to_string().into()]);
            }
            return if fileblade::native::run_backend(arguments, output)? {
                Ok(())
            } else {
                Err(AppError::command("native preferences request failed"))
            };
        }
        if matches!(command, RootCommand::List(_)) {
            return Err(AppError::command(
                "native owner-unavailable: state writes must be admitted by the native authority",
            ));
        }
    }
    match command {
        RootCommand::CompanionMutate => {
            if fileblade::lease::selected_root()?.is_some() {
                return Err(AppError::command(
                    "native owner-unavailable: companion mutations must be admitted by the native authority",
                ));
            }
            let started = std::time::Instant::now();
            let result = fileblade::companion_mutations::stdin_request();
            let _ = fileblade::audit::record(&fileblade::audit::Event {
                via: "cli",
                actor: "companion",
                command: "companion-mutate",
                arguments: &[],
                outcome: &result,
                started,
            });
            output.machine(&result?)?;
            Ok(())
        }
        RootCommand::Backend { command } => {
            if fileblade::lease::selected_root()?.is_some() && server::native_mutating(&command) {
                return Err(AppError::command(
                    "native owner-unavailable: mutating backend commands must be admitted by the native authority",
                ));
            }
            let cancelled = AtomicBool::new(false);
            let mut progress = |value| {
                output.machine(&value)?;
                Ok(())
            };
            let raw = std::env::args_os()
                .skip(2)
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            let started = std::time::Instant::now();
            let result = backend::dispatch(command, &cancelled, &mut progress);
            let _ = fileblade::audit::record(&fileblade::audit::Event {
                via: "cli",
                actor: "cli",
                command: raw.first().map_or("", String::as_str),
                arguments: raw.get(1..).unwrap_or_default(),
                outcome: &result,
                started,
            });
            output.machine(&result?)?;
            Ok(())
        }
        RootCommand::Serve(options) => server::run(options, output),
        command => public_cli::run(command)?.emit(&output),
    }
}

fn clap_error(error: clap::Error) -> ExitCode {
    let kind = error.kind();
    let rendered = error.to_string();
    let arguments: Vec<String> = std::env::args_os()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    let json = arguments
        .iter()
        .any(|argument| argument == "--output=json" || argument == "-ojson")
        || arguments.windows(2).any(|pair| {
            matches!(pair[0].as_str(), "-o" | "--output") && pair[1].as_str() == "json"
        });
    let format = if json { Format::Json } else { Format::Text };
    let output = Output::new(format, false);
    if matches!(kind, ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        let _ = output.text(rendered.trim_end());
        ExitCode::SUCCESS
    } else {
        let _ = output.error(rendered.trim_end());
        ExitCode::from(2)
    }
}

fn broken_pipe(error: &AppError) -> bool {
    matches!(error, AppError::Io(io_error) if io_error.kind() == io::ErrorKind::BrokenPipe)
}
