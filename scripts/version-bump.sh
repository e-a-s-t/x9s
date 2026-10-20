#!/usr/bin/env bash
set -euo pipefail

level="${1:?usage: version-bump.sh patch|minor|major}"
case "$level" in
  patch|minor|major) ;;
  *) echo "unsupported version bump: $level" >&2; exit 2 ;;
esac

version() {
  cargo metadata --no-deps --format-version 1 \
    | jq -er '.packages[] | select(.name == "x9s") | .version'
}

old_version="$(version)"
cargo set-version --package x9s --bump "$level"
new_version="$(version)"
printf 'x9s version: %s -> %s\n' "$old_version" "$new_version"
