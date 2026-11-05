# Versioning Guide

Jottery uses semantic versioning (MAJOR.MINOR.PATCH) across all components.

## Release Types

- **Release Candidates**: `vX.Y.Z-rc.N` (e.g., `v1.0.0-rc.1`) - Feature-complete releases for testing
- **Stable Releases**: `vX.Y.Z` (e.g., `v1.0.0`) - Production-ready releases

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
- `package.json` and `admin/package.json` (web client and admin dashboard)
- `tui/Cargo.toml` and `tui/Cargo.lock` (TUI client)
- `server/Cargo.toml`, `server/Cargo.lock` and `server/.sqlx` (sync server)
- `ios-native/Jottery/Jottery.xcodeproj/project.pbxproj` (iOS client)

### 2. Review Changes

```bash
git diff
```

Verify the files have been updated correctly.

### 3. Commit on a Branch

Name the files, and don't use `git add -A`: the repository is public, and anything else in the working copy would be published with the release. `main` only receives merged pull requests.

```bash
git checkout -b release/v0.5.0
git add package.json admin/package.json tui/Cargo.toml tui/Cargo.lock \
  server/Cargo.toml server/Cargo.lock server/.sqlx \
  ios-native/Jottery/Jottery.xcodeproj/project.pbxproj
git commit -m "jottery release v0.5.0"
git push -u origin release/v0.5.0
gh pr create --fill
```

### 4. Create Git Tag

Once the pull request is merged:

```bash
git checkout main && git pull
git tag v0.5.0
```

### 5. Push the Tag

```bash
git push origin v0.5.0
```

Pushing the tag starts `.github/workflows/build-multiarch.yml`. Every test suite must pass before the Docker image or GitHub release is published, so if one fails, nothing goes out. Fix it, or use "Re-run failed jobs" if it was a flaky run.

A tag with a hyphen, such as `v1.0.0-rc.1`, publishes a prerelease. It gets its own image tag, and `:latest` and the `/releases/latest` download links stay where they are.

To rehearse a release, run the workflow by hand from the Actions tab. It runs the tests, builds the binaries and builds the Docker image for amd64, but publishes nothing.

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
