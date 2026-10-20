# Roadmap

## Phase 0 — Starter scaffold (current)

Repository instructions, documentation, Cargo project, and placeholder executable. No Kubernetes or TUI functionality yet.

## Phase 1 — Read-only resource browser

- kubeconfig and context support
- Dynamic API discovery and Crossplane classification
- Namespaced and cluster-scoped managed resources
- Responsive Ratatui resource table and navigation
- Condition/health display
- Resource details, YAML, and Kubernetes Events
- Filtering and graceful handling of errors/RBAC restrictions
- Fixture-based tests and updated usage/support docs

**Done when:** `cargo build --release`, formatting, linting, and tests pass; the app can browse installed managed resources without modifying them.

## Phase 2 — Crossplane intelligence

- XR/composed-resource tree navigation
- Composition and ProviderConfig relationships
- Provider health and reconciliation troubleshooting
- Explicit reference-based navigation; no name guessing

## Phase 3 — Advanced operations (requires approval)

- Explicit reconcile actions and safe editing
- Destructive actions only with clear confirmation
- Provider log correlation, richer dashboards, context switching

Never represent planned capabilities as implemented in user documentation.
