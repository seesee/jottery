#!/usr/bin/env bash
set -e

cd tui

RELEASE_DIR="../releases"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

rustup target add \
    x86_64-unknown-linux-gnu \
    aarch64-unknown-linux-gnu \
    armv7-unknown-linux-gnueabihf

echo "Building Linux x86_64..."
cargo build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/jottery "$RELEASE_DIR/jottery-linux-x64"

echo "Building Linux aarch64..."
cargo build --release --target aarch64-unknown-linux-gnu
cp target/aarch64-unknown-linux-gnu/release/jottery "$RELEASE_DIR/jottery-linux-arm64"

echo "Building Linux armv7..."
cargo build --release --target armv7-unknown-linux-gnueabihf
cp target/armv7-unknown-linux-gnueabihf/release/jottery "$RELEASE_DIR/jottery-linux-armv7"

echo "Linux builds complete."
