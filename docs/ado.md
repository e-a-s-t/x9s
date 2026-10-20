# Azure DevOps

**Status: proposed pipeline only. No pipeline or service connection has been configured.**

## CI expectations

On pull requests and main-branch changes, run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

Use pinned/controlled toolchain versions and cache Cargo registry/build artifacts when practical. Publish test results and binaries as build artifacts. Start with Linux; expand to macOS and Windows only after verifying terminal compatibility.

## Release expectations

Tag-triggered release builds, versioned binaries, checksums, and release notes. Do not assume an Azure DevOps organization, project, agent pool, artifact feed, or signing credentials. Define those explicitly before writing a deploy/publish pipeline. No cluster credentials should be needed for unit-test CI.
