# x9s

**Crossplane resources at your fingertips.**

x9s is a Rust-based Crossplane resource explorer scaffold. Its planned terminal UI will use Ratatui and Crossterm, with Kubernetes discovery through kube-rs rather than generated provider-specific clients. GitHub is the intended development and release platform.

## Status

**Starter scaffold only.** Kubernetes connectivity and the TUI are not implemented yet. The executable supports `--version`.

## Start developing

Install [Devbox](https://www.jetify.com/docs/devbox/installing_devbox/) for your platform, then enter the project environment. Devbox supplies the pinned Rust toolchain and development utilities declared by this repository.

```bash
git clone <repository>
cd x9s

devbox shell

cargo build
cargo run
```

Or use the project commands:

```bash
devbox run build
devbox run test
devbox run lint
devbox run ci
```

Read [development instructions](docs/development.md) for setup, toolchain updates, and version management. See the [documentation index](docs/README.md) and [roadmap](docs/roadmap.md) for project status.

The planned Homebrew distribution path is the [`e-a-s-t/homebrew-tap`](https://github.com/e-a-s-t/homebrew-tap) tap. The formula may not yet be available; release details are in the [GitHub delivery plan](docs/github.md).

## Design principles

- Read-only first; use current kubeconfig.
- Crossplane v1/v2, namespaced and cluster-scoped resources.
- No hardcoded provider resource kinds or cluster-side installation.
- Useful troubleshooting without leaking credentials.
