#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf 'release-version: %s\n' "$1" >&2
  exit 1
}

level="${1:-}"
case "$level" in
  major|minor|patch) ;;
  *) fail 'usage: scripts/release-version.sh major|minor|patch' ;;
esac

[[ "$(git branch --show-current)" == main ]] || fail 'must run on the main branch'
[[ -z "$(git status --porcelain)" ]] || fail 'working tree must be clean'

version="$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)"$/\1/p' Cargo.toml | head -n 1)"
[[ -n "$version" ]] || fail 'could not read a numeric x.y.z version from Cargo.toml'
IFS=. read -r major minor patch <<< "$version"
case "$level" in
  major) next="$((major + 1)).0.0" ;;
  minor) next="$major.$((minor + 1)).0" ;;
  patch) next="$major.$minor.$((patch + 1))" ;;
esac
tag="v$next"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && fail "tag $tag already exists"

# Used by isolated validation to exercise version calculations and safety guards
# without changing the manifest, committing, or creating a tag.
if [[ "${X9S_VERSION_BUMP_CHECK_ONLY:-0}" == 1 ]]; then
  printf '%s -> %s\n' "$version" "$next"
  exit 0
fi

cargo set-version --package x9s "$next"
updated="$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)"$/\1/p' Cargo.toml | head -n 1)"
[[ "$updated" == "$next" ]] || fail "Cargo.toml version is $updated, expected $next"
cargo check --workspace
locked="$(cargo metadata --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "x9s") | .version')"
[[ "$locked" == "$next" ]] || fail "Cargo metadata reports $locked, expected $next"

reported="$(cargo run --quiet -- --version)"
[[ "$reported" == "x9s $next" ]] || fail "x9s --version reported '$reported', expected 'x9s $next'"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

changed="$(git diff --name-only)"
while IFS= read -r file; do
  [[ -z "$file" || "$file" == Cargo.toml || "$file" == Cargo.lock ]] || fail "unexpected version update changed $file"
done <<< "$changed"
git add -- Cargo.toml Cargo.lock
git diff --cached --quiet && fail 'version update produced no staged changes'
git commit -m "chore(release): bump version to $next"
git tag -a "$tag" -m "Release $tag"
[[ "$(git rev-parse "$tag^{}")" == "$(git rev-parse HEAD)" ]] || fail "tag $tag does not reference the version commit"
printf 'Created version commit %s and annotated tag %s. Push manually after review.\n' "$(git rev-parse --short HEAD)" "$tag"
