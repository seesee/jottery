#!/usr/bin/env bash
# Entry point for the Jottery image. Runs the server and Caddy as an unprivileged
# user, and stops the container if either exits, so a restart policy and docker ps
# see the failure instead of Caddy serving 502s from a container that looks healthy.
#
# Started as root (the default), it hands the data directory to the jottery user
# and restarts itself as that user. Volumes from earlier images are owned by root,
# so this has to happen here: an image that simply set USER couldn't open them.
#
# Given a command, it runs that instead of the services, as the same user.
set -uo pipefail

APP_UID=10001
DATA_DIR=${JOTTERY_DATA_DIR:-/app/data}

if [ "$(id -u)" = 0 ]; then
  mkdir -p "$DATA_DIR"
  # Only touches what needs changing, and -h stops a symlink in the volume from
  # redirecting this to a file elsewhere. Failure isn't fatal here: without the
  # CHOWN capability a volume that's already right needs nothing, and the check
  # below reports one that isn't.
  find "$DATA_DIR" ! -uid "$APP_UID" -exec chown -h "$APP_UID:$APP_UID" {} + \
    || echo "jottery: couldn't change the owner of everything in $DATA_DIR" >&2
  exec setpriv --reuid="$APP_UID" --regid="$APP_UID" --clear-groups "$0" "$@"
fi

if [ $# -gt 0 ]; then
  exec "$@"
fi

if ! touch "$DATA_DIR/jottery.db"; then
  echo "jottery: $DATA_DIR is not writable by uid $(id -u)." >&2
  echo "jottery: give that user the directory, or start the container as root once." >&2
  exit 1
fi

# The server ignores a MAX_PAYLOAD_SIZE it can't read and uses its default, but Caddy
# won't start on one. Only hand on what both accept, so a typo in a setting that used
# to do nothing can't take the site down.
if [[ "${MAX_PAYLOAD_SIZE:-}" =~ ^[0-9]{1,18}$ ]]; then
  export JOTTERY_MAX_BODY_SIZE=$MAX_PAYLOAD_SIZE
else
  unset JOTTERY_MAX_BODY_SIZE
fi

jottery-server &
server_pid=$!
caddy run --config /etc/caddy/Caddyfile --adapter caddyfile &
caddy_pid=$!

stopping=false
stop_services() {
  stopping=true
  kill -TERM "$server_pid" "$caddy_pid" 2>/dev/null
}
trap stop_services TERM INT

# Returns when the first of the two exits, or when a trapped signal interrupts it
wait -n "$server_pid" "$caddy_pid"
status=$?

if [ "$stopping" = true ]; then
  wait "$server_pid" "$caddy_pid"
  exit 0
fi

echo "jottery: a service exited with status $status, stopping the container" >&2
stop_services
wait "$server_pid" "$caddy_pid"

# Neither service should ever stop by itself, so a clean exit is still a failure
[ "$status" = 0 ] && status=1
exit "$status"
