# Architecture

**Status: Phase 1 implementation.**

## Development environment

Devbox pins the Rust toolchain and development utilities. Cargo manages application dependencies and package metadata; `Cargo.toml` is the version source of truth. See [development](development.md).

## Layers

1. **Kubernetes adapter:** initializes kube-rs from the current kubeconfig context, discovers served APIs and their scopes, and lists dynamic resources asynchronously.
2. **Crossplane interpretation:** classifies discovered resource families and Managed Resource candidates, interprets conditions, and extracts Crossplane metadata.
3. **Resource model:** normalizes API version, kind, name, scope, UID, creation time, external name, conditions, and the original Kubernetes object.
4. **Terminal UI:** manages list, filter, details, loading, and error state; Ratatui renders the views and Crossterm supplies terminal input and events.

The UI does not issue Kubernetes requests. Async operations deliver results to application state so rendering and keyboard input remain responsive. Malformed or absent conditions are handled as unknown/missing, not as success.

## Discovery and scope

Discovery uses installed Kubernetes API and CRD information rather than provider-specific kind lists. For each confirmed or possible Managed Resource CRD, the adapter resolves group, served version, kind, plural, LIST capability, and scope against Kubernetes API discovery. It uses kube-rs `pinned_group` discovery for each CRD group/version and caches that result for subsequent CRDs in the same version. A discovery failure is recorded for that group/version and skipped while other groups and versions continue. Matching resources use the discovered scope to construct the dynamic API request and are listed independently, so one failed list does not stop other types. Crossplane classification combines schema, ownership/metadata conventions, and available object-level evidence; see [Crossplane compatibility](crossplane.md). Only confirmed Managed Resources appear in the default list; possible candidates remain in the internal discovery result.

Discovery diagnostics include the active context, CRD and classification counts, possible-classification reasons, per-group/version discovery failures and dynamic API matching problems, per-type list outcomes, and normalized totals. Set `X9S_LOG_FILE` to append structured diagnostics to a file. `RUST_LOG` controls the filter; when no log file is configured, diagnostics use a warning-level sink and do not write over the terminal UI.

Kubernetes may return an empty string in a list response's `metadata.continue` field to indicate that pagination is complete. The CRD and resource paginators treat both an absent and empty continuation token as completion. Previously, the empty string was treated as another page token, causing x9s to repeatedly request the same page and remain in loading state.

The UI is initially Managed Resource focused. Other Crossplane families are classified internally, but do not have separate browser views in Phase 1. Partial API failures are shown as errors while retaining the last successful resource list where available.

## Security and scope

The application is read-only and requires no cluster-side installation. It follows kubeconfig authentication, TLS, RBAC, and namespace scope. It does not inspect Secret values or log credentials. YAML and Events views, relationship navigation, and resource modifications are outside Phase 1.

## Design decisions

- **D001:** Rust + Ratatui + Crossterm for the terminal UI.
- **D002:** kube-rs dynamic APIs for arbitrary installed resource types.
- **D003:** Read-only initial release.
- **D004:** Kubernetes transport, Crossplane interpretation, normalized model, and UI are separate responsibilities.
- **D005:** Conservative three-way classification; uncertain resources are not promoted into the default MR list.

## Terminal UI reference

Use the [official Ratatui documentation](https://ratatui.rs/) for framework guidance and current conventions. Do not introduce another TUI framework without a concrete need.
