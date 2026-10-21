# GitHub CI and Delivery

**Status: GitHub Actions CI and tagged releases use the repository's pinned Devbox environment. Homebrew publishing is not configured.**

GitHub is the intended source code hosting and development platform. Pull requests are intended for code review, and GitHub Issues for bug reports and feature requests.

## CI

CI installs Devbox with Jetify's official GitHub Action and runs the same pinned toolchain used locally:

```bash
devbox run ci
```

CI runs formatting, Clippy, and tests through `devbox run -- cargo ...`; no separate Rust installation is used. No cluster credentials are needed.

## Versioning and release preparation

`Cargo.toml` is the version source of truth. Use cargo-edit through the Devbox commands to inspect or increment the `x9s` version:

```bash
devbox run version
devbox run patch
devbox run minor
devbox run major
```

Each bump requires a clean working tree on `main`, refuses an existing target tag, updates `Cargo.toml` and `Cargo.lock`, verifies the binary version, runs validation, stages only the version files, commits, and creates an annotated tag. For example:

```bash
# Create patch release
devbox run patch

# Review the release commit and tag
git log -1
git tag --list

# Publish after review (replace with the actual version)
git push origin main
git push origin v0.1.1
```

Use `devbox run minor` or `devbox run major` for those release levels. The tag triggers release validation, four native platform builds through Devbox, checksum generation, and GitHub Release publication. Release publication waits for every build to succeed.

## Distribution

GitHub Releases are the intended source for versioned release binaries. Homebrew distribution is planned through the [`e-a-s-t/homebrew-tap`](https://github.com/e-a-s-t/homebrew-tap) tap. The formula will need to reference the released version and matching artifact checksums; do not assume it exists yet.

Once the formula is available, the intended install command is:

```bash
brew install e-a-s-t/tap/x9s
```

The command may not work until the formula is published. Release and tap updates are future/manual steps; no publishing automation is configured.
