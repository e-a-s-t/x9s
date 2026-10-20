# GitHub CI and Delivery

**Status: proposed process. No GitHub Actions workflow or Homebrew formula is configured in this repository.**

GitHub is the intended source code hosting and development platform. Pull requests are used for code review, and GitHub Issues are used for bug reports and feature requests. Do not assume a repository URL or pre-existing GitHub configuration.

## Proposed GitHub Actions workflow

Run CI for pull requests and pushes to the repository's primary branch. Use a controlled stable Rust toolchain and cache Cargo dependencies when practical. CI should run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Build optimized binaries for Linux and macOS and attach the platform-specific artifacts to the CI run. Windows is a future target and should be added after terminal compatibility is validated.

When a version tag is pushed, build release binaries for the supported Linux and macOS targets and publish the artifacts in a corresponding GitHub Release. No cluster credentials are needed for these checks or builds. Any signing or publishing credentials must be deliberately configured; none are assumed here.

## Distribution

GitHub Releases are the source for versioned release binaries. Homebrew distribution is planned through the [`e-a-s-t/homebrew-tap`](https://github.com/e-a-s-t/homebrew-tap) tap. The maintainer updates the tap formula for each release so it references the published version and matching artifact checksums. This tap update is a manual release step; cross-repository Actions automation is not part of the current proposal.

Users will be able to install from Homebrew once the formula is available:

```bash
brew tap e-a-s-t/tap
brew install x9s
```

The install commands describe the planned distribution path and may not work until the formula is published.
