#!/usr/bin/env bash
# Docker HEALTHCHECK for the Jottery image. Asks Caddy for /health, which it proxies
# to the server, so one request proves both are answering. Uses bash's /dev/tcp
# because the image has no curl.
set -uo pipefail

PORT=${JOTTERY_HEALTH_PORT:-8088}
TIMEOUT=${JOTTERY_HEALTH_TIMEOUT:-4}

exec 3<>"/dev/tcp/127.0.0.1/$PORT" || exit 1
printf 'GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n' >&3

read -r -t "$TIMEOUT" _ code _ <&3 || exit 1
[ "${code%$'\r'}" = 200 ]
