#!/usr/bin/env bash
# Fails if the server's tests and its release image would use different compilers.
# server/rust-toolchain.toml sets one and the Dockerfile's rust image sets the other,
# and a Dependabot update only changes the Dockerfile. Run by
# .github/workflows/repo-checks.yml.
#
# Usage: scripts/check-rust-toolchain.sh [repo-dir]
set -euo pipefail

cd "${1:-.}"

toolchain=$(sed -n 's/^channel = "\(.*\)"$/\1/p' server/rust-toolchain.toml)
image=$(sed -n 's/^FROM rust:\([^@ ]*\).*/\1/p' Dockerfile)
image=${image%%-*}  # 1.99.1-bookworm is Rust 1.99.1

if [[ -z "$toolchain" || -z "$image" ]]; then
  echo "Couldn't read the versions: toolchain [$toolchain], image [$image]" >&2
  exit 1
fi

if [[ "$toolchain" != "$image" ]]; then
  echo "server/rust-toolchain.toml says $toolchain but the Dockerfile uses rust:$image" >&2
  exit 1
fi

echo "The server builds with Rust $toolchain everywhere"
