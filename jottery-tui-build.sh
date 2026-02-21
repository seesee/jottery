#!/usr/bin/env bash
set -e

cd tui

RELEASE_DIR="../releases"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

echo "=== Linux x64 (SQLCipher) ==="
cargo build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/jottery "$RELEASE_DIR/jottery-linux-x64"

echo "=== Linux ARM64 ==="
cargo build --release --target aarch64-unknown-linux-gnu
cp target/aarch64-unknown-linux-gnu/release/jottery "$RELEASE_DIR/jottery-linux-arm64"

echo "=== Linux ARMv7 ==="
cargo build --release --target armv7-unknown-linux-gnueabihf
cp target/armv7-unknown-linux-gnueabihf/release/jottery "$RELEASE_DIR/jottery-linux-armv7"

echo "Linux builds complete."
