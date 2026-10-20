# x9s

**Crossplane resources at your fingertips.**

x9s is a read-only Rust terminal browser for Crossplane managed resources. It uses the current kubeconfig context, discovers Kubernetes APIs dynamically, and renders resources in a Ratatui interface. It does not install components in the cluster or modify Kubernetes resources.

## Quick start

Install [Devbox](https://www.jetify.com/docs/devbox/installing_devbox/) and use the repository's pinned Rust environment:

```bash
git clone <repository>
cd x9s
devbox run run
```

Or enter the environment and run the application directly:

```bash
devbox shell
cargo run
```

The browser lists confirmed Managed Resources by default. Use `/` to filter, `Enter` to inspect details, `r` to refresh, and `q` to exit. See [usage](docs/usage.md) for keyboard controls and [Crossplane compatibility](docs/crossplane.md) for discovery and classification behavior.

## Development

Use Devbox for build and validation commands. See [development](docs/development.md), the [documentation index](docs/README.md), and [roadmap](docs/roadmap.md) for project status and development guidance.

## Design principles

- Read-only access through the active kubeconfig context.
- Crossplane v1 and v2; discovered namespace and cluster scopes.
- No provider-specific kind lists or required cluster-side components.
- Conservative classification: only confirmed Managed Resources appear by default.
- Never display or log Kubernetes credentials or Secret values.
