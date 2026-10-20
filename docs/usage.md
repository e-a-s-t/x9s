# Usage

The default command starts the interactive TUI. The CLI commands are non-interactive, do not initialize Ratatui, and can run without a terminal. All operations are read-only and use the shared Kubernetes and Crossplane Core implementation.

## Commands

```text
x9s                                      # Start the TUI
x9s tui                                  # Start the TUI explicitly
x9s get managed                          # List confirmed Managed Resources as a table
x9s get managed --output json            # List as JSON
x9s get managed --namespace crossplane-system
x9s get managed --all-namespaces
x9s diagnose                             # Show discovery diagnostics
x9s diagnose --output json
x9s --context kind-local-dev             # Select context for the TUI
x9s get managed --context kind-local-dev
x9s diagnose --context kind-local-dev
x9s --help
x9s --version
```

`--context` selects a kubeconfig context for that invocation and does not change the user's active context. Without an override, x9s uses the current kubeconfig context. `--namespace` filters namespaced resources; cluster-scoped resources remain included. The default listing covers all namespaces. Empty resource results are successful when discovery completed.

Use `devbox run run` for the default TUI, or pass command-line arguments through Devbox:

```bash
devbox run -- cargo run -- --help
devbox run -- cargo run -- --version
devbox run -- cargo run -- get managed --output json
devbox run -- cargo run -- diagnose --output json
```

The version comes from Cargo package metadata. CLI parsing errors use clap's normal usage message and exit code.

## Output and exit codes

Table output is intended for people. JSON mode is intended for scripts and agents: stdout contains only the requested JSON data, with no logs, progress output, or terminal escape sequences. Errors are written to stderr as structured JSON in JSON mode. Diagnostics include the chosen context, Kubernetes connectivity, discovered CRD and classification counts, classification reasons, instance counts, and discovery/list errors.

| Code | Meaning |
| --- | --- |
| `0` | Operation completed successfully. An empty list is success if discovery completed. |
| `1` | Fatal runtime error, including Kubernetes connectivity or initial CRD-discovery failure. |
| `2` | Invalid CLI arguments (clap default). |
| `3` | Partial Kubernetes discovery or resource listing. Successfully discovered resources are preserved in the output. |

Connectivity failures never produce an empty successful result. See [agent usage](agents.md) for machine integration examples and JSON behavior.

## Discovery diagnostics

For the TUI, discovery logs can be enabled without writing over the interface by setting `X9S_LOG_FILE`:

```bash
X9S_LOG_FILE=/tmp/x9s-discovery.log RUST_LOG=debug x9s
```

The file is append-only. When configured, the default log level is `info`; `RUST_LOG` overrides the filter. Without a log file, warning-level events go to a sink. Logs contain context, discovery and classification totals, Possible classification reasons, API matching, and per-type listing outcomes. Avoid sharing kubeconfig contents, tokens, Secret values, or private infrastructure identifiers.

## TUI resource list

The table shows Kind, Name, Namespace, Synced, Ready, and Age. Cluster-scoped resources show `-` for namespace. Condition labels distinguish True, False, Unknown, and Missing; status is not conveyed by color alone. The list supports keyboard selection and scrolling. Refresh preserves the selected resource when it remains present. On refresh failure, x9s reports the error and retains the last successful list.

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next resource |
| `k` / `Up` | Select the previous resource |
| `Enter` | Open selected resource details |
| `Esc` | Close details or leave filter entry |
| `/` | Enter a case-insensitive filter for kind, name, or namespace |
| `r` | Refresh resources |
| `q` | Quit |
| `Ctrl+C` | Exit |

The details view shows API version, kind, name, namespace, UID, creation time, external name when available, Ready and Synced status, and available condition reasons/messages. Missing fields are shown as unavailable rather than inferred.

## Scope

Only confirmed Managed Resources appear in the default list. Possible Managed Resources are retained internally but are not exposed as a separate command or TUI toggle. YAML inspection, Kubernetes Events, relationship navigation, and dedicated screens for other Crossplane families are not implemented. See [Crossplane compatibility](crossplane.md).
