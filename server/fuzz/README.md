# Fuzz Testing for Jottery Server

This directory contains fuzz tests for security-critical input handling.

## Prerequisites

1. Install cargo-fuzz:
   ```bash
   cargo install cargo-fuzz
   ```

2. Rust nightly toolchain:
   ```bash
   rustup install nightly
   ```

## Running Fuzz Tests

### Input Validation Fuzzing
Tests `validate_device_name`, `validate_inbox_content`, `validate_note_content`, and `validate_tags`:

```bash
cd server
cargo +nightly fuzz run fuzz_validation
```

### Email Validation Fuzzing
Tests email address validation (RFC 5321/5322 compliance):

```bash
cd server
cargo +nightly fuzz run fuzz_email
```

## Running for a Specific Duration

```bash
# Run for 60 seconds
cargo +nightly fuzz run fuzz_validation -- -max_total_time=60

# Run for 1000 iterations
cargo +nightly fuzz run fuzz_validation -- -runs=1000
```

## Corpus

Interesting test cases are saved in `fuzz/corpus/<target>/`. These are automatically used as seeds for future runs.

## Crashes

If a crash is found, it will be saved in `fuzz/artifacts/<target>/`. To reproduce:

```bash
cargo +nightly fuzz run fuzz_validation fuzz/artifacts/fuzz_validation/crash-xxxxx
```

## What We're Testing

1. **No panics**: Validation functions should never panic on arbitrary input
2. **Memory safety**: No buffer overflows or undefined behaviour
3. **Edge cases**: Very long strings, unicode, null bytes, special characters
4. **Attack vectors**: SQL injection patterns, XSS payloads, path traversal

## Coverage

To generate coverage report:

```bash
cargo +nightly fuzz coverage fuzz_validation
```
