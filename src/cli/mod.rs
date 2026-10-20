pub mod args;
mod diagnose;
mod get;
mod output;

use std::io::Write as _;
use std::process::ExitCode;

use anyhow::Error;
use clap::Parser;
use serde::Serialize;

use crate::core::{DiscoveryError, Kubernetes};
use args::{Cli, Command, ResourceCommand};
use output::OutputFormat;

#[derive(Serialize)]
struct ErrorOutput<'a> {
    code: &'a str,
    message: &'a str,
}

pub async fn run() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    let json_requested = arguments_request_json(&arguments[1..]);
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            if json_requested && code == 2 {
                let output = ErrorOutput {
                    code: "invalid_arguments",
                    message: &error.to_string(),
                };
                let _ = serde_json::to_writer(std::io::stderr().lock(), &output);
                eprintln!();
            } else {
                let _ = error.print();
            }
            return ExitCode::from(code as u8);
        }
    };
    let command = cli.command.unwrap_or(Command::Tui {});
    match command {
        Command::Tui {} => crate::tui::ui::run(cli.context.as_deref())
            .await
            .map(|_| ExitCode::SUCCESS)
            .unwrap_or_else(|error| fatal(error, false)),
        command => {
            let format = command.output_format();
            match Kubernetes::connect(cli.context.as_deref()).await {
                Ok(client) => match client.list_managed_resources().await {
                    Ok(report) => {
                        let partial = report.is_partial();
                        match command {
                            Command::Get {
                                command:
                                    ResourceCommand::Managed {
                                        namespace,
                                        all_namespaces,
                                        output,
                                    },
                            } => get::write(&report, namespace.as_deref(), all_namespaces, output),
                            Command::Diagnose { .. } => diagnose::write(&report, format),
                            Command::Tui {} => unreachable!(),
                        }
                        .map_err(|error| fatal(error, format == OutputFormat::Json))
                        .map(|_| report_exit_code(partial))
                        .unwrap_or_else(|code| code)
                    }
                    Err(error) => fatal(error, format == OutputFormat::Json),
                },
                Err(error) => fatal(error, format == OutputFormat::Json),
            }
        }
    }
}

fn arguments_request_json(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| argument == "--output=json")
        || arguments
            .windows(2)
            .any(|pair| pair[0] == "--output" && pair[1] == "json")
}

fn report_exit_code(partial: bool) -> ExitCode {
    if partial {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}

impl Command {
    fn output_format(&self) -> OutputFormat {
        match self {
            Command::Get {
                command: ResourceCommand::Managed { output, .. },
            }
            | Command::Diagnose { output } => *output,
            Command::Tui {} => OutputFormat::Table,
        }
    }
}

fn fatal(error: Error, json: bool) -> ExitCode {
    if json {
        let output = ErrorOutput {
            code: "fatal_runtime_error",
            message: &format!("{error:#}"),
        };
        let _ = serde_json::to_writer(std::io::stderr().lock(), &output);
        eprintln!();
    } else {
        eprintln!("x9s: {error:#}");
    }
    ExitCode::from(1)
}

pub(crate) fn write_errors(errors: &[DiscoveryError], json: bool) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    for error in errors {
        if json {
            let _ = serde_json::to_writer(&mut lock, error);
            let _ = std::io::Write::write_all(&mut lock, b"\n");
        } else {
            let _ = writeln!(lock, "x9s: {}: {}", error.code, error.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_and_partial_reports_use_contract_exit_codes() {
        assert_eq!(report_exit_code(false), ExitCode::SUCCESS);
        assert_eq!(report_exit_code(true), ExitCode::from(3));
    }

    #[test]
    fn detects_json_mode_when_clap_reports_invalid_arguments() {
        assert!(arguments_request_json(&[
            "get".into(),
            "managed".into(),
            "--output=json".into(),
            "--bad".into(),
        ]));
        assert!(!arguments_request_json(&["get".into(), "managed".into()]));
    }
}
