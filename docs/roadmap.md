# Roadmap

## Phase 0 — Starter scaffold (complete)

Repository conventions, documentation, and Devbox environment.

## Phase 1 — Read-only resource browser (implemented)

- Current kubeconfig context and asynchronous Kubernetes access.
- Dynamic API discovery with namespace and cluster scopes.
- Conservative Crossplane family/MR classification, including Confirmed, Possible, and Not a Managed Resource results.
- Default list of confirmed MRs, with condition status and normalized metadata.
- Ratatui table, filtering, selection, details, refresh, and clear error/loading states.
- Fixture-based tests and documentation for supported behavior and limitations.

Phase 1 remains read-only. Possible MRs are held internally but are not selectable in the UI. YAML and Events views, relationship navigation, provider logs, and dedicated views for other resource families are not included.

Build, formatting, lint, unit tests, CLI checks, and TUI startup/exit have been verified. Live-cluster access remains environment-dependent and must only be claimed after a successful connection.

## Phase 2 — Crossplane intelligence

- XR and composed-resource navigation based on explicit references.
- Composition and ProviderConfig relationships.
- Provider health and reconciliation troubleshooting.

## Phase 3 — Advanced operations (requires explicit scope and approval)

- Any resource editing or reconciliation actions, with safety controls.
- Destructive operations only after a separate design and explicit confirmation requirements.
- Provider log correlation and richer dashboards.

Never describe planned capabilities as implemented in user documentation.
