# x9s

**Crossplane resources at your fingertips.**

x9s is a planned Rust terminal UI for inspecting Crossplane resources and their reconciliation state. Inspired by K9s, it will use Kubernetes discovery rather than generated provider-specific clients.

## Status

**Starter scaffold only.** The initial executable is a placeholder; Kubernetes connectivity and TUI are not implemented yet.

## Start development

```bash
cargo run
```

Read [AGENTS.md](AGENTS.md), the [documentation index](docs/README.md), and the [roadmap](docs/roadmap.md). Start with Phase 1.

## Design principles

- Read-only first; use current kubeconfig.
- Crossplane v1/v2, namespaced and cluster-scoped resources.
- No hardcoded provider resource kinds or cluster-side installation.
- Useful troubleshooting without leaking credentials.
