# x9s

**Crossplane resources at your fingertips.**

x9s is a planned Rust-based Crossplane resource explorer for inspecting resources and their reconciliation state. Its terminal UI will use Ratatui with Crossterm, and it will use Kubernetes discovery through kube-rs rather than generated provider-specific clients. GitHub is the intended development and release platform.

## Status

**Starter scaffold only.** The initial executable is a placeholder; Kubernetes connectivity and TUI are not implemented yet.

## Start development

```bash
cargo run
```

Read [AGENTS.md](AGENTS.md), the [documentation index](docs/README.md), and the [roadmap](docs/roadmap.md). Start with Phase 1.

When releases are available, the planned Homebrew distribution path is the [`e-a-s-t/homebrew-tap`](https://github.com/e-a-s-t/homebrew-tap) tap. See the [GitHub delivery plan](docs/github.md).

## Design principles

- Read-only first; use current kubeconfig.
- Crossplane v1/v2, namespaced and cluster-scoped resources.
- No hardcoded provider resource kinds or cluster-side installation.
- Useful troubleshooting without leaking credentials.
