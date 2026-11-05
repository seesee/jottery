#!/usr/bin/env bash
# Fails unless a macOS TUI binary is universal, needs nothing beyond macOS itself,
# runs, and encrypts its database.
#
# A binary that links a Homebrew library builds and runs fine on the machine that
# made it, then stops at launch on any Mac without that library, before any of
# our code runs. Plain SQLite is the opposite trap: it ignores the encryption key
# without complaint, so a wrong feature flag would leave every vault unencrypted.
# Run by the macos-tui job in .github/workflows/build-multiarch.yml.
#
# Usage: scripts/check-macos-binary.sh <binary>
set -euo pipefail

binary=$1
failures=0

archs=$(lipo -archs "$binary")

for required in arm64 x86_64; do
  if [[ " $archs " != *" $required "* ]]; then
    echo "::error::$binary has no $required code (it has: $archs)"
    failures=$((failures + 1))
  fi
done

for arch in $archs; do
  while read -r library; do
    case "$library" in
      /usr/lib/* | /System/Library/*) ;;
      *)
        echo "::error::The $arch code in $binary links $library, which isn't on every Mac"
        failures=$((failures + 1))
        ;;
    esac
  done < <(otool -arch "$arch" -L "$binary" | tail -n +2 | awk '{print $1}')
done

"$binary" --version

workdir=$(mktemp -d)
printf '{"version":"1.0","exportDate":"2026-01-01T00:00:00Z","notes":[]}' > "$workdir/notes.json"
"$binary" -d "$workdir/vault.db" import -i "$workdir/notes.json" -p "$(uuidgen)" > /dev/null
if [[ "$(head -c 15 "$workdir/vault.db")" == "SQLite format 3" ]]; then
  echo "::error::$binary wrote an unencrypted database, so it isn't using SQLCipher"
  failures=$((failures + 1))
fi

if ((failures > 0)); then
  exit 1
fi

echo "$binary is universal ($archs), links only system libraries and encrypts its database."
