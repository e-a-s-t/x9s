# AGENTS.md

## Project
x9s is a Rust-based Crossplane resource explorer built with Ratatui and Crossterm, integrated with Kubernetes through kube-rs. The scaffold is not yet implemented; see the roadmap for planned phases.

## Rules
- Ratatui is the mandatory TUI framework; use Crossterm for terminal input and events.
- Use Rust, kube-rs, Tokio, Serde, and tracing as the project develops.
- Discover API resources dynamically; never hardcode provider kinds.
- Support Crossplane v1 and v2, namespaced and cluster-scoped resources.
- Read-only in Phase 1. No in-cluster components required.
- Respect kubeconfig, RBAC, TLS, and namespace scope.
- Never log credentials or Secret values.
- Keep Kubernetes API access, Crossplane interpretation, and UI separate.
- Prefer incremental, tested changes over premature abstraction.

## Documentation map
- [Architecture](docs/architecture.md): design and component boundaries.
- [Development](docs/development.md): local workflow and conventions.
- [GitHub](docs/github.md): proposed CI/CD and release process.
- [Usage](docs/usage.md): interface and commands.
- [Support](docs/support.md): troubleshooting and limitations.
- [Test cases](docs/testcases.md): verification and acceptance.
- [Crossplane](docs/crossplane.md): resource discovery and compatibility.
- [Roadmap](docs/roadmap.md): phases and scope.
- [Documentation index](docs/README.md).

## Workflow
1. Read relevant documentation and inspect existing code.
2. Make the smallest working change; add appropriate tests.
3. Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
4. Update relevant docs; distinguish planned features from implemented ones.
5. Report what changed, verification results, and remaining limitations.

Do not implement future phases without explicit instruction. Keep plans distinct from implemented functionality.
