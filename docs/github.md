# GitHub CI and Delivery

**Status: Devbox commands are implemented. No GitHub Actions workflow, release automation, or Homebrew formula is configured in this repository.**

GitHub is the intended source code hosting and development platform. Pull requests are intended for code review, and GitHub Issues for bug reports and feature requests.

## CI

Future GitHub Actions workflows should install Devbox and run the same project validation used locally:

```bash
devbox run ci
```

This runs `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`, stopping if a step fails. No GitHub Actions workflow is added by the current tooling setup. Future CI may build optimized binaries for supported Linux and macOS targets; no cluster credentials are needed.

## Versioning and release preparation

`Cargo.toml` is the version source of truth. Use cargo-edit through the Devbox commands to inspect or increment the `x9s` version:

```bash
devbox run version
devbox run patch
devbox run minor
devbox run major
```

The bump updates `Cargo.toml` and `Cargo.lock`, prints the old and new versions, and leaves changes uncommitted. Review the actual version before preparing a release. For example, if the command reports `0.1.0 -> 0.1.1`:

```bash
# Increment version
devbox run patch

# Validate
devbox run ci

# Review changes
git diff

# Commit version changes (replace with the actual version)
git add Cargo.toml Cargo.lock
git commit -m "chore(release): bump version to 0.1.1"

# Create an annotated tag (replace with the actual version)
git tag -a v0.1.1 -m "Release v0.1.1"

# Push commit and tag
git push
git push origin v0.1.1
```

The example version and tag are illustrative; use the version printed by the bump command. Tags use a `v` prefix, such as `v0.1.0`, `v0.2.0`, or `v1.0.0`. Version bumping and publishing are separate operations. Bump commands never create commits or tags, push changes, or publish a release.

A future `devbox run publish` command is planned to verify a clean working tree, run validation, verify the Cargo version, create an annotated Git tag, push the commit and tag, and trigger the GitHub release workflow. It is not implemented.

## Distribution

GitHub Releases are the intended source for versioned release binaries. Homebrew distribution is planned through the [`e-a-s-t/homebrew-tap`](https://github.com/e-a-s-t/homebrew-tap) tap. The formula will need to reference the released version and matching artifact checksums; do not assume it exists yet.

Once the formula is available, the intended install command is:

```bash
brew install e-a-s-t/tap/x9s
```

The command may not work until the formula is published. Release and tap updates are future/manual steps; no publishing automation is configured.
