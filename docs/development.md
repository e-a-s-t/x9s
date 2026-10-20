# Development

**Status: Devbox is the supported Rust environment for the Phase 1 application.**

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
| `devbox run -- cargo run -- <args>` | Run a CLI/TUI command with arguments |
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

## Discovery diagnostics

To capture discovery diagnostics without interfering with the Ratatui interface, set `X9S_LOG_FILE` to a writable file path before starting x9s. The file is opened in append mode and missing parent directories are created. For example:

```bash
X9S_LOG_FILE=/tmp/x9s-discovery.log RUST_LOG=debug devbox run run
```

When a log file is configured, the default filter is `info`; `RUST_LOG` overrides it. Without `X9S_LOG_FILE`, logging goes to a sink at `warn` level, and stdout remains dedicated to the TUI. Discovery records context and aggregate counts, classification reasons for Possible resources, API discovery mismatches, and per-type listing outcomes. Error events record Kubernetes error categories rather than request bodies or credentials. Never share kubeconfig contents, tokens, or Secret values when reporting an issue.

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

## Dependencies and conventions

Ratatui is the mandatory TUI framework and Crossterm handles terminal input and events. Kubernetes access uses kube-rs with Tokio; Serde/JSON support resource data, and tracing is used for diagnostics. Add dependencies only as required by working features; do not prepopulate unused modules.

Keep modules aligned to responsibilities: shared discovery and normalized types live in `core/`, terminal behavior in `tui/`, and argument parsing and output formatting in `cli/`. Prefer typed domain models at boundaries; use dynamic objects for provider resources. Add unit tests for parsers and fixture-based classification/normalization tests without cluster access, including custom Upjet groups, Crossplane v2, both resource scopes, and unrelated CRDs. Keep network operations asynchronous. Update docs in the same change as behavior.

For the UI, follow Ratatui's application patterns: maintain app state independently, handle input with Crossterm, compose screens from Ratatui widgets and layouts, and render in the terminal draw loop. Do not perform Kubernetes calls or other blocking work during rendering; run I/O asynchronously and deliver results back to the UI.

The project is intended to use GitHub for source hosting and code review. See [GitHub CI and delivery](github.md) for the proposed Actions and release process.

## Phase 1 behavior

The application defaults to the read-only TUI and also provides non-interactive `get managed` and `diagnose` commands. Both interfaces accept a kubeconfig context override and share Core discovery and normalization. Possible MRs are retained internally and excluded from the default list. Resource details, filtering, refresh, and condition parsing are part of the TUI. YAML and Events views are not implemented. See [usage](usage.md), [agent usage](agents.md), [Crossplane compatibility](crossplane.md), and [roadmap](roadmap.md).
