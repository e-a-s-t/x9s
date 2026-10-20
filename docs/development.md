# Development

**Status: scaffold available; TUI not implemented.**

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

Ratatui, Crossterm, kube-rs, Tokio, Serde, serde_json, serde_yaml, tracing, anyhow/thiserror. Add only as required by working features; do not prepopulate unused modules.

## Conventions

Keep modules aligned to responsibilities: `kubernetes/`, `crossplane/`, `model/`, `ui/`. Prefer typed domain models at boundaries; use dynamic objects for provider resources. Add unit tests for parsers and fixture-based classification tests without cluster access. Keep network operations asynchronous. Update docs in the same change as behavior.

## First task

Implement Phase 1 incrementally: kubeconfig connection, discovery, classification, managed-resource table, condition parsing, selection/details/YAML/events, filtering, and robust error handling. See [roadmap](roadmap.md) and [test cases](testcases.md).
