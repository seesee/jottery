# Versioning Guide

Jottery uses semantic versioning (MAJOR.MINOR.PATCH) across all components.

## Version Display

### Web Client
- Version shown in Settings modal (bottom left corner)
- Console log on database initialization: `[Jottery Client vX.Y.Z]`
- Read from `package.json` at build time

### TUI Client  
- Version shown in window title: `Jottery vX.Y.Z - Notes`
- Command line flag: `jottery --version`
- Read from `tui/Cargo.toml` at compile time

## Release Process

### 1. Update Version Numbers

Use the provided script to update all version numbers consistently:

```bash
./scripts/set-version.sh 0.5.0
```

This updates:
- `package.json` (web client)
- `tui/Cargo.toml` (TUI client)

### 2. Review Changes

```bash
git diff
```

Verify both files have been updated correctly.

### 3. Commit Version Bump

```bash
git add package.json tui/Cargo.toml
git commit -m "Bump version to v0.5.0"
```

### 4. Create Git Tag

```bash
git tag v0.5.0
```

### 5. Push to Repository

```bash
git push
git push --tags
```

Pushing tags will trigger the GitHub Actions build (if configured).

## Version Sources

- **Web**: `package.json` version → `__APP_VERSION__` global (injected by Vite)
- **TUI**: `Cargo.toml` version → `env!("CARGO_PKG_VERSION")` macro

## Manual Updates

If you need to update versions manually:

### Web (package.json)
```json
{
  "version": "0.5.0"
}
```

### TUI (tui/Cargo.toml)
```toml
[package]
version = "0.5.0"
```

After manual updates, rebuild to see changes:
- Web: `npm run build`
- TUI: `cargo build --release`

## Current Versions

Run these commands to check current versions:

```bash
# Web client version
grep '"version"' package.json

# TUI client version  
grep '^version' tui/Cargo.toml

# Latest git tag
git describe --tags --abbrev=0
```
