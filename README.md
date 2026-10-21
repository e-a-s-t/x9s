# x9s

**Crossplane resources at your fingertips.**

x9s is a read-only Rust Crossplane resource explorer with two interfaces: an interactive Ratatui TUI and a non-interactive CLI for scripts and agents. Both use the same Kubernetes discovery and Crossplane classification code. It does not install components in the cluster or modify Kubernetes resources.

## Quick start

Install [Devbox](https://www.jetify.com/docs/devbox/installing_devbox/) and use the repository's pinned Rust environment:

```bash
git clone <repository>
cd x9s
devbox run run
```

The default command starts the TUI. To list resources or inspect discovery from a script:

```bash
devbox run -- cargo run -- get managed --output json
devbox run -- cargo run -- diagnose --output json
```

See [usage](docs/usage.md) for commands, output formats, context selection, and exit codes, and [agent usage](docs/agents.md) for machine-oriented examples.

Or enter the environment and run the application directly:

```bash
devbox shell
cargo run
```

## Installation

Prebuilt releases are available for macOS on Apple Silicon and Intel, and Linux on
ARM64 and AMD64. Download the matching `.tar.gz` archive from the
[GitHub Releases](https://github.com/e-a-s-t/x9s/releases) page, verify it with
`SHA256SUMS`, then extract and place the `x9s` binary somewhere on your `PATH`.
Each archive contains a single `x9s` binary at its root. For example, on Linux
AMD64:

```bash
curl -LO https://github.com/e-a-s-t/x9s/releases/latest/download/x9s-linux-amd64.tar.gz
curl -LO https://github.com/e-a-s-t/x9s/releases/latest/download/SHA256SUMS
sha256sum --ignore-missing -c SHA256SUMS
tar -xzf x9s-linux-amd64.tar.gz
install -m 755 x9s "$HOME/.local/bin/x9s"
```

On macOS, use `shasum -a 256 -c SHA256SUMS` to check the checksum file.

## Releases for maintainers

Update the package version in `Cargo.toml` and `Cargo.lock`, commit the change,
then push a version tag such as `v0.1.1`. The release workflow runs formatting,
Clippy, and tests, builds all four platform archives, and publishes them with a
`SHA256SUMS` file and automatically generated release notes. A manual workflow
run must target a version tag. Do not publish a tag until its version matches
the package version.

The browser lists confirmed Managed Resources by default. Use `/` to filter, `Enter` to inspect details, `r` to refresh, and `q` to exit. See [usage](docs/usage.md) for keyboard controls and [Crossplane compatibility](docs/crossplane.md) for discovery and classification behavior.

## Development

Use Devbox for build and validation commands. See [development](docs/development.md), the [documentation index](docs/README.md), and [roadmap](docs/roadmap.md) for project status and development guidance.

## Design principles

- Read-only access through the selected kubeconfig context.
- Crossplane v1 and v2; discovered namespace and cluster scopes.
- No provider-specific kind lists or required cluster-side components.
- Conservative classification: only confirmed Managed Resources appear by default.
- Never display or log Kubernetes credentials or Secret values.
