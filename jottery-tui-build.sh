#!/usr/bin/env bash
set -e

cd tui

RELEASE_DIR="../releases"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

echo "--- Building Linux x64 (with SQLCipher) ---"
cargo build --release --target x86_64-unknown-linux-gnu --features sqlcipher
cp target/x86_64-unknown-linux-gnu/release/jottery "$RELEASE_DIR/jottery-linux-x64"

echo "--- Building Linux ARM64 (SQLite only) ---"
cargo build --release --target aarch64-unknown-linux-gnu --no-default-features --features sqlite
cp target/aarch64-unknown-linux-gnu/release/jottery "$RELEASE_DIR/jottery-linux-arm64"

echo "--- Building Linux ARMv7 (SQLite only) ---"
cargo build --release --target armv7-unknown-linux-gnueabihf --no-default-features --features sqlite
cp target/armv7-unknown-linux-gnueabihf/release/jottery "$RELEASE_DIR/jottery-linux-armv7"

echo "Linux builds complete."
