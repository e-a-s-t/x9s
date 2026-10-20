use clap::{Parser, Subcommand};

pub use super::output::OutputFormat;

#[derive(Debug, Parser)]
#[command(
    name = "x9s",
    version,
    about = "Read-only Crossplane resource explorer"
)]
pub struct Cli {
    /// Kubernetes context to use (defaults to kubeconfig current context)
    #[arg(long, global = true)]
    pub context: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the interactive terminal interface
    Tui {},
    /// Query discovered resources
    Get {
        #[command(subcommand)]
        command: ResourceCommand,
    },
    /// Diagnose Kubernetes and Crossplane discovery
    Diagnose {
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        output: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
pub enum ResourceCommand {
    /// List Managed Resources
    Managed {
        /// Namespace to filter namespaced resources
        #[arg(long, conflicts_with = "all_namespaces")]
        namespace: Option<String>,
        /// Include namespaced resources from every namespace (the default)
        #[arg(long)]
        all_namespaces: bool,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        output: OutputFormat,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_tui_and_explicit_tui() {
        assert!(Cli::try_parse_from(["x9s"]).unwrap().command.is_none());
        assert!(matches!(
            Cli::try_parse_from(["x9s", "tui"]).unwrap().command,
            Some(Command::Tui {})
        ));
    }

    #[test]
    fn parses_list_output_namespace_and_context_options() {
        let cli = Cli::try_parse_from([
            "x9s",
            "get",
            "managed",
            "--output",
            "json",
            "--namespace",
            "crossplane-system",
            "--context",
            "kind-local-dev",
        ])
        .unwrap();
        assert_eq!(cli.context.as_deref(), Some("kind-local-dev"));
        assert!(matches!(
            cli.command,
            Some(Command::Get {
                command: ResourceCommand::Managed {
                    namespace: Some(ref namespace),
                    output: OutputFormat::Json,
                    ..
                }
            }) if namespace == "crossplane-system"
        ));
    }

    #[test]
    fn parses_diagnostics_and_rejects_conflicting_namespace_flags() {
        assert!(matches!(
            Cli::try_parse_from(["x9s", "diagnose", "--output", "json"])
                .unwrap()
                .command,
            Some(Command::Diagnose {
                output: OutputFormat::Json
            })
        ));
        let error = Cli::try_parse_from([
            "x9s",
            "get",
            "managed",
            "--namespace",
            "default",
            "--all-namespaces",
        ])
        .unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }
}
