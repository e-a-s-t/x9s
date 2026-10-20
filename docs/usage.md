# Usage

**Status: Phase 1 interface.**

Run x9s using the current kubeconfig context:

```bash
devbox run run
```

The application also supports `--help` and `--version`. The version is derived from Cargo package metadata at compile time. No in-cluster installation is required. The UI displays the active context, lists confirmed Managed Resources, and shows loading or API error state as applicable. See [Crossplane compatibility](crossplane.md) for how resources are classified.

For CLI checks from Devbox, pass arguments directly to Cargo:

```bash
devbox run -- cargo run -- --help
devbox run -- cargo run -- --version
```

## Discovery diagnostics

Discovery logs can be enabled without writing to the terminal UI by setting `X9S_LOG_FILE`:

```bash
X9S_LOG_FILE=/tmp/x9s-discovery.log RUST_LOG=debug devbox run run
```

The file is append-only. When a path is configured, the default log level is `info`; `RUST_LOG` overrides the filter. Without a log file, only warning-level events are enabled and sent to a sink. Logs report context, discovery/classification totals, Possible classification reasons, dynamic API matching, listing errors, and per-type instance counts. See [Support](support.md) for interpreting a discovery trace.

## Resource list

The table shows Kind, Name, Namespace, Synced, Ready, and Age. Cluster-scoped resources show `-` for namespace. Condition labels distinguish True, False, Unknown, and Missing; status is not conveyed by color alone. The list supports keyboard selection and scrolling, including larger result sets. Refresh attempts preserve the selected resource when it remains present. If a refresh fails, x9s reports the error and retains the last successful list.

## Keyboard controls

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

The details view shows API version, kind, name, namespace, UID, creation time, external name when available, Ready and Synced status, and condition reasons/messages when present. Missing fields are shown as unavailable rather than inferred.

## Scope

Only confirmed Managed Resources appear in the default list. Possible Managed Resources are retained internally but cannot yet be enabled from the UI. YAML inspection, Kubernetes Events, relationship navigation, and dedicated screens for other Crossplane families are not available in this phase. All Kubernetes operations are read-only.
