# Development

**Status: scaffold available; Ratatui UI and Kubernetes integration are not implemented.**

## Prerequisites

Stable Rust toolchain (`rustup`, `cargo`), a Kubernetes kubeconfig for eventual integration testing, and access to a Crossplane cluster when testing live discovery.

## Commands

```bash
cargo run
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

## Planned dependencies

Ratatui is the mandatory TUI framework and Crossterm handles terminal input and events. Other planned dependencies include kube-rs, Tokio, Serde, serde_json, serde_yaml, tracing, anyhow/thiserror. Add dependencies only as required by working features; do not prepopulate unused modules.

## Conventions

Keep modules aligned to responsibilities: `kubernetes/`, `crossplane/`, `model/`, `ui/`. Prefer typed domain models at boundaries; use dynamic objects for provider resources. Add unit tests for parsers and fixture-based classification tests without cluster access. Keep network operations asynchronous. Update docs in the same change as behavior.

For the UI, follow Ratatui's application patterns: maintain app state independently, handle input with Crossterm, compose screens from Ratatui widgets and layouts, and render in the terminal draw loop. Do not perform Kubernetes calls or other blocking work during rendering; run I/O asynchronously and deliver results back to the UI.

The project is intended to use GitHub for source hosting and code review. See [GitHub CI and delivery](github.md) for the proposed Actions and release process.

## First task

Implement Phase 1 incrementally: kubeconfig connection, discovery, classification, managed-resource table, condition parsing, selection/details/YAML/events, filtering, and robust error handling. See [roadmap](roadmap.md) and [test cases](testcases.md).
