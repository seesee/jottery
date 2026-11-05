#!/bin/bash
# npm run build:tui. The real build is jottery-tui-build.sh, which writes to releases/.
# Binaries must never go under public/, because Vite copies that into the web client.
set -e

cd "$(dirname "$0")/.."
exec ./jottery-tui-build.sh
