#!/usr/bin/env bash
# Build the Linux TUI binaries for all supported architectures.
#
# Uses cargo-zigbuild so the build works from any host (Linux runner or a Mac)
# and links against a fixed minimum glibc rather than whatever the build
# machine happens to have. Install with: pipx install cargo-zigbuild (plus zig).
set -e

cd tui

RELEASE_DIR="../releases"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

# Oldest glibc the binaries will run on (Debian 11, Ubuntu 20.04, RHEL 9)
GLIBC="${JOTTERY_MIN_GLIBC:-2.31}"

build() {
  local target="$1" name="$2"
  echo "=== Linux ${name} (bundled SQLite, glibc >= ${GLIBC}) ==="
  cargo zigbuild --release --target "${target}.${GLIBC}"
  cp "target/${target}/release/jottery" "${RELEASE_DIR}/jottery-linux-${name}"
}

build x86_64-unknown-linux-gnu x64
build aarch64-unknown-linux-gnu arm64
build armv7-unknown-linux-gnueabihf armv7

echo "Linux builds complete."
