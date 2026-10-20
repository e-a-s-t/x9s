# Development

**Status: Devbox environment configured; Ratatui UI and Kubernetes integration are not implemented.**

## Install and enter Devbox

Install Devbox using the [official installation instructions](https://www.jetify.com/docs/devbox/installing_devbox/) for your operating system. This repository pins the Rust compiler, Cargo, rustfmt, Clippy, rust-analyzer, cargo-edit, and jq in `devbox.json`; `devbox.lock` records the resolved environment. Devbox/Nix is the source of the project toolchain. Do not install a competing project toolchain through rustup.

```bash
git clone <repository>
cd x9s

# Enter an interactive environment
devbox shell

# Build and run
cargo build
cargo run
```

Devbox supports the configured macOS Apple Silicon and Intel, and Linux x86_64 and ARM64 systems where the pinned Nix packages are available. No Docker or globally installed Rust toolchain is required.

## Commands

Run the equivalent commands through Devbox without entering an interactive shell:

| Command | Action |
| --- | --- |
| `devbox run build` | Debug build (`cargo build`) |
| `devbox run release` | Optimized build (`cargo build --release`) |
| `devbox run run` | Run x9s (`cargo run`) |
| `devbox run test` | Run tests (`cargo test`) |
| `devbox run fmt` | Format Rust source (`cargo fmt`) |
| `devbox run fmt-check` | Check formatting (`cargo fmt --check`) |
| `devbox run lint` | Run Clippy with warnings denied |
| `devbox run check` | Run `cargo check` |
| `devbox run ci` | Run formatting check, Clippy, and tests; stop on the first failure |

For the complete local validation, matching the intended GitHub Actions command, run:

```bash
devbox run ci
```

The CI script runs `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, then `cargo test`.

## Toolchain and dependencies

Devbox package versions are pinned in `devbox.json`, and the lockfile should be committed with environment changes. To update the Rust toolchain or a development utility, edit its package version in `devbox.json`, regenerate the lockfile with `devbox update`, then review the resolved diff and run `devbox run ci`. Keep rustc, Cargo, rustfmt, and Clippy on the same compatible Rust release. `rust-toolchain.toml` is not used; Devbox is the toolchain source of truth.

Application crate dependencies belong in `Cargo.toml` and are resolved by Cargo into `Cargo.lock`; update them using Cargo and review/test the resulting changes. Devbox packages are for tools needed to develop or build the project, not a replacement for application dependencies.

## Version management

`Cargo.toml` is the authoritative application version. `devbox run version` reads the `x9s` package version using Cargo metadata. Bump commands use cargo-edit to update the `x9s` package version and keep `Cargo.lock` synchronized:

```bash
devbox run version
devbox run patch
devbox run minor
devbox run major
```

Each bump reports the old and new versions and leaves changes in the working tree for review. For example, a patch bump of `0.1.0` produces `0.1.1`; a minor bump of `0.1.1` produces `0.2.0`; a major bump of `0.2.0` produces `1.0.0`. These commands do not commit, tag, push, or publish. Keep version bumping separate from release publishing.

## Planned dependencies and conventions

Ratatui is the mandatory TUI framework and Crossterm handles terminal input and events. Other planned dependencies include kube-rs, Tokio, Serde, serde_json, serde_yaml, tracing, anyhow/thiserror. Add dependencies only as required by working features; do not prepopulate unused modules.

Keep modules aligned to responsibilities: `kubernetes/`, `crossplane/`, `model/`, `ui/`. Prefer typed domain models at boundaries; use dynamic objects for provider resources. Add unit tests for parsers and fixture-based classification tests without cluster access. Keep network operations asynchronous. Update docs in the same change as behavior.

For the UI, follow Ratatui's application patterns: maintain app state independently, handle input with Crossterm, compose screens from Ratatui widgets and layouts, and render in the terminal draw loop. Do not perform Kubernetes calls or other blocking work during rendering; run I/O asynchronously and deliver results back to the UI.

The project is intended to use GitHub for source hosting and code review. See [GitHub CI and delivery](github.md) for the proposed Actions and release process.

## First task

Implement Phase 1 incrementally: kubeconfig connection, discovery, classification, managed-resource table, condition parsing, selection/details/YAML/events, filtering, and robust error handling. See [roadmap](roadmap.md) and [test cases](testcases.md).
