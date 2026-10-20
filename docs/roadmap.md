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

The original Phase 1 interface was a flat confirmed Managed Resource list. Phase 2 extends the TUI while keeping the application read-only and preserving the CLI's confirmed-MR contract.

Build, formatting, lint, unit tests, CLI checks, and TUI startup/exit have been verified. Live-cluster access remains environment-dependent and must only be claimed after a successful connection.

## Phase 2 — ProviderConfig navigation and Crossplane relationships (implemented with limitations)

- Dynamic ProviderConfig discovery and association scoped by provider ownership metadata.
- Global typed relationship graph for composition, ownership, explicit references, selectors, and ProviderConfig links.
- ProviderConfig selector, resource tree/flat views, relationship navigation, history, filtering, command mode, details, sanitized YAML, Events, and semantic theme.
- Remaining discovery and graph constraints are documented in [architecture](architecture.md).

## Phase 3 — Advanced operations (requires explicit scope and approval)

- Any resource editing or reconciliation actions, with safety controls.
- Destructive operations only after a separate design and explicit confirmation requirements.
- Provider log correlation and richer dashboards.

Never describe planned capabilities as implemented in user documentation.
