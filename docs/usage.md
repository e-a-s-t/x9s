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

## TUI navigation

The TUI uses a persistent, responsive header with kubeconfig context details, shortcuts supported by the current view, and an X9s wordmark on wide terminals. Kubernetes version and metrics show `n/a` when not already available; the header does not poll the cluster for them. Resource tables keep Ready and Synced visible on compact terminals, while namespace and age are hidden to preserve room for names.

The initial screen lists discovered ProviderConfigs, All Providers, Unassigned resources, and Unresolved ProviderConfig references. ProviderConfig identities include API group, kind, namespace when applicable, and name. Counts include unique managed-resource instances. Selecting a ProviderConfig shows its associated managed resources; unresolved resources remain visible in All Providers and under Unresolved ProviderConfig. All Providers includes discovered infrastructure instances and exact referenced targets, not CRD definitions or Crossplane internals. Use `:system` for Crossplane diagnostic objects.

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next resource |
| `k` / `Up` | Select the previous resource |
| `Enter` | Open selected ProviderConfig/resource; in details, open the selected related resource |
| `Esc` | Return through navigation history |
| `/` | Filter the current ProviderConfig or resource view |
| `:` | Enter a command |
| `p` | Open the ProviderConfig selector |
| `t` | Toggle tree and flat resource views |
| `h` / `Left` | Collapse the selected tree node |
| `l` / `Right` | Expand the selected tree node |
| `y` | Show sanitized resource YAML from details |
| `d` | Describe the selected resource from details |
| `e` | Load Kubernetes Events from details |
| `r` | Refresh resources while preserving navigation state |
| `?` | Show help |
| `q` | Quit |
| `Ctrl+C` | Exit |

In details, `j`/`k` selects a related resource and `Enter` navigates to it. `/` filters ProviderConfigs by identity/status and resources by name, kind, namespace, or status. Commands are `:providers`, `:all`, `:tree`, `:flat`, `:system`, `:health`, and `:context`; use `:context <name>` to switch kubeconfig contexts.

The details view shows API identity, namespace, UID, external name when available, Ready and Synced conditions, and discovered relationships. YAML and describe output redact Secret payload fields and credential-like keys. Event listing is asynchronous and follows Kubernetes RBAC.

## Scope

The TUI lists actual infrastructure instances from discovered APIs; CRDs are never shown as instances. Unresolved ProviderConfig references remain accessible in `:all` and in their own selector entry. The CLI remains focused on confirmed Managed Resources. See [architecture](architecture.md) for discovery and graph limitations, and [Crossplane compatibility](crossplane.md) for classification behavior.
