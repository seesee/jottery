#!/usr/bin/env bash
# Fails if the repository tracks a file that should never be published: a
# database, a backup, key material, an environment file, or anything large.
#
# The repo is public and GitHub's secret scanning can't see inside a SQLite
# file, so this is what stops another real database going out in a release
# commit. Run by .github/workflows/repo-checks.yml.
#
# Usage: scripts/check-tracked-files.sh [repo-dir]
set -euo pipefail

cd "${1:-.}"

# Largest file allowed in the tree. The biggest legitimate files are the
# localised screenshot GIFs at a little over 2 MB.
MAX_BYTES=$((5 * 1024 * 1024))

# Data files that are safe to publish. Add to this deliberately, with a reason.
# jottery-demo.db: seed data for the demo generator, with one example.com user
# and the default admin.
ALLOWED_DATA_FILES=(
  demo-generation/demo-server/jottery-demo.db
)

# Lower-cased path patterns. Extended regular expressions.
FORBIDDEN='(^|/)\.env(\.|$)|\.(db|sqlite|sqlite3)($|[.-])|\.(jwe|enex|p12|pfx|pem|key|jks|keystore|mobileprovision)$'
NOT_FORBIDDEN='\.env\.example$'

is_allowed() {
  local candidate
  for candidate in "${ALLOWED_DATA_FILES[@]}"; do
    [[ "$1" == "$candidate" ]] && return 0
  done
  return 1
}

failures=0

while IFS= read -r -d '' path; do
  # Tracked but deleted in the working copy, or a submodule.
  [[ -f "$path" ]] || continue

  lower=$(printf '%s' "$path" | tr '[:upper:]' '[:lower:]')

  if [[ "$lower" =~ $FORBIDDEN ]] && ! [[ "$lower" =~ $NOT_FORBIDDEN ]] && ! is_allowed "$path"; then
    echo "::error file=$path::Data or secret file is tracked: $path"
    failures=$((failures + 1))
  fi

  size=$(wc -c < "$path" | tr -d ' ')
  if (( size > MAX_BYTES )); then
    echo "::error file=$path::File is $size bytes, over the $MAX_BYTES byte limit: $path"
    failures=$((failures + 1))
  fi
done < <(git ls-files -z)

if (( failures > 0 )); then
  echo "$failures tracked file(s) must be removed. If one is genuinely safe, add it to ALLOWED_DATA_FILES in scripts/check-tracked-files.sh." >&2
  exit 1
fi

echo "Tracked files OK."
