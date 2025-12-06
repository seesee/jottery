#!/bin/bash
set -e

# Define the output directory
OUTPUT_DIR="public/releases"

# Create the output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

# Build for Linux (x86_64)
echo "Building for Linux..."
rustup target add x86_64-unknown-linux-gnu
cargo build --manifest-path tui/Cargo.toml --target x86_64-unknown-linux-gnu --release
mv tui/target/x86_64-unknown-linux-gnu/release/jottery "$OUTPUT_DIR/jottery-linux-amd64"

# Build for macOS (x86_64)
echo "Building for macOS..."
rustup target add x86_64-apple-darwin
cargo build --manifest-path tui/Cargo.toml --target x86_64-apple-darwin --release
mv tui/target/x86_64-apple-darwin/release/jottery "$OUTPUT_DIR/jottery-macos-amd64"

# Build for Windows (x86_64)
echo "Building for Windows..."
rustup target add x86_64-pc-windows-gnu
cargo build --manifest-path tui/Cargo.toml --target x86_64-pc-windows-gnu --release
mv tui/target/x86_64-pc-windows-gnu/release/jottery.exe "$OUTPUT_DIR/jottery-windows-amd64.exe"

echo "TUI cross-compilation complete."
