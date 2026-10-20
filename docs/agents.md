# Agent and Script Usage

The x9s CLI is non-interactive and intended for automation. It shares Kubernetes discovery and Crossplane classification with the TUI, uses kubeconfig authentication, and performs read-only operations. No TTY or MCP server is required.

## Commands

```bash
x9s get managed --output json
x9s get managed --namespace crossplane-system --output json
x9s get managed --all-namespaces --output json
x9s diagnose --output json
x9s get managed --context kind-local-dev --output json
```

`get managed` lists confirmed Managed Resources. JSON output is a deterministic JSON array of normalized resources, with stable field names, including a null namespace for cluster-scoped resources and explicit True, False, Unknown, or Missing condition status. It does not serialize the underlying Kubernetes object, Secret values, or credentials. An empty array is a successful result if discovery completed. `diagnose --output json` returns structured discovery data including the selected context, connectivity, discovered CRD/type counts, classification reasons, instance counts, and errors.

`--context` selects the kubeconfig context for a single invocation without changing the active context. `--namespace` filters namespaced resources while cluster-scoped resources remain in results. `--all-namespaces` explicitly requests all namespaces; with neither flag, listing also spans all namespaces.

## Output contract

- In JSON mode stdout contains only the requested JSON data. No logs, progress indicators, or ANSI sequences are mixed into it.
- Errors are emitted to stderr as structured JSON. On partial discovery/listing, successful resource results are preserved and the failure details are reported on stderr (and in diagnostics output where applicable).
- Kubernetes connectivity failure and failure to discover CRDs are fatal. They are never represented as an empty successful result.
- No interactive prompts are used. Operations do not create, update, or delete Kubernetes resources.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Successful operation, including a completed discovery with zero matching instances. |
| `1` | Fatal runtime error, such as Kubernetes connectivity or initial CRD discovery failure. |
| `2` | Invalid CLI arguments, using clap's default convention. |
| `3` | Partial Kubernetes discovery or resource listing. Successful discoveries are retained. |

## Integration examples

Capture the list as JSON and check the command status before consuming it:

```bash
x9s get managed --output json >managed.json
status=$?
case "$status" in
  0) jq '.[] | {kind, name, namespace, synced, ready}' managed.json ;;
  3) echo "Some resource types could not be listed; inspect stderr" >&2 ;;
  *) echo "x9s failed with exit code $status" >&2 ;;
esac
```

For context-specific diagnostics:

```bash
x9s diagnose --context kind-local-dev --output json >diagnostics.json
```

The CLI uses stderr for structured errors in JSON mode; redirect stdout and stderr separately when an agent needs to retain both the requested data and error details. See [usage](usage.md) for the complete command reference and TUI controls.
