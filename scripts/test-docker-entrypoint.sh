#!/usr/bin/env bash
# Tests scripts/docker-entrypoint.sh and scripts/docker-healthcheck.sh without
# Docker. The server, Caddy, setpriv, chown and id are replaced with stubs on PATH
# that log what they were asked to do, so this runs anywhere with bash 5.1 or later.
#
# Usage: scripts/test-docker-entrypoint.sh
set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ENTRYPOINT="$ROOT/scripts/docker-entrypoint.sh"
HEALTHCHECK="$ROOT/scripts/docker-healthcheck.sh"
TEST_UID=10001

if (( BASH_VERSINFO[0] < 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] < 1) )); then
  echo "bash 5.1 or later is needed, to match the image" >&2
  exit 2
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

passed=0
failed=0

pass() { passed=$((passed + 1)); echo "ok   $1"; }
fail() { failed=$((failed + 1)); echo "FAIL $1"; [[ -n "${2:-}" ]] && sed 's/^/     /' <<<"$2"; }

# Writes a stub for a long-running service. It logs its uid and any signal, and
# runs until told otherwise: STUB_<NAME>_EXIT_AFTER=<seconds>:<status> makes it exit.
write_service_stub() {
  local name=$1 var=$2
  cat > "$BIN/$name" <<STUB
#!/usr/bin/env bash
echo "$name started uid=\$(id -u) args=\$* body_limit=\${JOTTERY_MAX_BODY_SIZE:-unset}" >> "\$STUB_LOG"
trap 'echo "$name got TERM" >> "\$STUB_LOG"; exit 0' TERM
if [[ -n "\${$var:-}" ]]; then
  sleep "\${$var%%:*}"
  exit "\${$var##*:}"
fi
while true; do sleep 0.05; done
STUB
  chmod +x "$BIN/$name"
}

# A fresh sandbox: stub binaries, a data directory and a log.
setup() {
  CASE=$(mktemp -d "$WORK/case.XXXXXX")
  BIN="$CASE/bin"
  DATA="$CASE/data"
  LOG="$CASE/log"
  mkdir -p "$BIN" "$DATA"
  : > "$LOG"

  write_service_stub jottery-server STUB_SERVER_EXIT
  write_service_stub caddy STUB_CADDY_EXIT

  # id reports whatever the sandbox's current uid is meant to be
  cat > "$BIN/id" <<'STUB'
#!/usr/bin/env bash
echo "${STUB_UID:-0}"
STUB

  cat > "$BIN/chown" <<'STUB'
#!/usr/bin/env bash
echo "chown $*" >> "$STUB_LOG"
STUB

  # setpriv "drops" to the requested uid by changing what the id stub reports
  cat > "$BIN/setpriv" <<'STUB'
#!/usr/bin/env bash
echo "setpriv $*" >> "$STUB_LOG"
uid=
while [[ $# -gt 0 ]]; do
  case $1 in
    --reuid=*) uid=${1#--reuid=}; shift ;;
    --*) shift ;;
    *) break ;;
  esac
done
STUB_UID=$uid exec "$@"
STUB
  chmod +x "$BIN/id" "$BIN/chown" "$BIN/setpriv"
}

# Starts the entrypoint in the sandbox. Sets ENTRY_PID.
start_entrypoint() {
  ( export PATH="$BIN:$PATH" STUB_LOG="$LOG" JOTTERY_DATA_DIR="$DATA"
    exec bash "$ENTRYPOINT" "$@" ) > "$CASE/out" 2>&1 &
  ENTRY_PID=$!
}

wait_for_log() {
  local pattern=$1 attempt
  for attempt in $(seq 1 200); do
    grep -q -- "$pattern" "$LOG" && return 0
    sleep 0.02
  done
  return 1
}

# Waits for the entrypoint to exit, by attempts rather than a fixed sleep. Sets STATUS.
wait_for_exit() {
  local attempt
  for attempt in $(seq 1 300); do
    if ! kill -0 "$ENTRY_PID" 2>/dev/null; then
      wait "$ENTRY_PID"
      STATUS=$?
      return 0
    fi
    sleep 0.02
  done
  kill -KILL "$ENTRY_PID" 2>/dev/null
  STATUS=timeout
  return 1
}

test_server_death_stops_the_container() {
  setup
  export STUB_SERVER_EXIT=0.2:101 STUB_UID=$TEST_UID
  start_entrypoint
  if ! wait_for_exit; then
    fail "server death stops the container" "still running: $(cat "$LOG")"
  elif [[ $STATUS != 101 ]]; then
    fail "server death stops the container" "exit status was $STATUS, not 101"
  elif ! grep -q 'caddy got TERM' "$LOG"; then
    fail "server death stops the container" "Caddy wasn't stopped: $(cat "$LOG")"
  elif grep -q 'not a child' "$CASE/out"; then
    fail "server death stops the container" "shell errors: $(cat "$CASE/out")"
  else
    pass "server death stops the container with its status"
  fi
  unset STUB_SERVER_EXIT STUB_UID
}

test_caddy_death_stops_the_container() {
  setup
  export STUB_CADDY_EXIT=0.2:1 STUB_UID=$TEST_UID
  start_entrypoint
  if wait_for_exit && [[ $STATUS == 1 ]] && grep -q 'jottery-server got TERM' "$LOG"; then
    pass "Caddy death stops the container and the server"
  else
    fail "Caddy death stops the container and the server" "status=$STATUS $(cat "$LOG")"
  fi
  unset STUB_CADDY_EXIT STUB_UID
}

test_clean_server_exit_still_counts_as_failure() {
  setup
  export STUB_SERVER_EXIT=0.1:0 STUB_UID=$TEST_UID
  start_entrypoint
  if wait_for_exit && [[ $STATUS != 0 ]]; then
    pass "a server exiting with status 0 is still a failure"
  else
    fail "a server exiting with status 0 is still a failure" "status=$STATUS"
  fi
  unset STUB_SERVER_EXIT STUB_UID
}

test_sigterm_stops_both_cleanly() {
  setup
  export STUB_UID=$TEST_UID
  start_entrypoint
  wait_for_log 'caddy started' && wait_for_log 'jottery-server started'
  kill -TERM "$ENTRY_PID"
  if wait_for_exit && [[ $STATUS == 0 ]] && grep -q 'caddy got TERM' "$LOG" && grep -q 'jottery-server got TERM' "$LOG"; then
    pass "SIGTERM reaches both processes and exits 0"
  else
    fail "SIGTERM reaches both processes and exits 0" "status=$STATUS $(cat "$LOG")"
  fi
  unset STUB_UID
}

test_root_start_hands_over_data_and_drops() {
  setup
  touch "$DATA/jottery.db" "$DATA/jottery.db-wal"
  export STUB_UID=0
  start_entrypoint
  wait_for_log 'caddy started' && wait_for_log 'jottery-server started'
  kill -TERM "$ENTRY_PID"
  wait_for_exit
  local log
  log=$(cat "$LOG")
  if [[ $STATUS != 0 ]]; then
    fail "root start hands over an existing volume" "exit status $STATUS: $log $(cat "$CASE/out")"
  elif ! grep -q "chown .*$TEST_UID:$TEST_UID" <<<"$log"; then
    fail "root start hands over an existing volume" "$log"
  elif ! grep -q 'jottery.db-wal' <<<"$log"; then
    fail "root start hands over an existing volume" "files weren't chowned: $log"
  elif ! grep -q "setpriv .*--reuid=$TEST_UID .*--regid=$TEST_UID" <<<"$log"; then
    fail "root start hands over an existing volume" "no drop: $log"
  elif grep -q 'started uid=0' <<<"$log"; then
    fail "services never run as root" "$log"
  else
    pass "root start hands over an existing volume, then drops to $TEST_UID"
  fi
  unset STUB_UID
}

test_non_root_start_leaves_ownership_alone() {
  setup
  export STUB_UID=1234
  start_entrypoint
  wait_for_log 'caddy started'
  kill -TERM "$ENTRY_PID"
  wait_for_exit
  if grep -qE '^(chown|setpriv)' "$LOG"; then
    fail "a user set by the operator is respected" "$(cat "$LOG")"
  elif ! grep -q 'caddy started uid=1234' "$LOG"; then
    fail "a user set by the operator is respected" "$(cat "$LOG")"
  else
    pass "a user set by the operator is respected"
  fi
  unset STUB_UID
}

test_unwritable_data_dir_fails_with_a_reason() {
  setup
  chmod 555 "$DATA"
  export STUB_UID=$TEST_UID
  start_entrypoint
  wait_for_exit
  chmod 755 "$DATA"
  if [[ $STATUS == 0 || $STATUS == timeout ]]; then
    fail "an unwritable data directory is refused" "status=$STATUS"
  elif ! grep -qi 'not writable' "$CASE/out"; then
    fail "an unwritable data directory is refused" "no explanation: $(cat "$CASE/out")"
  elif grep -q 'started' "$LOG"; then
    fail "an unwritable data directory is refused" "services started: $(cat "$LOG")"
  else
    pass "an unwritable data directory is refused with a reason"
  fi
  unset STUB_UID
}

test_database_file_is_created() {
  setup
  export STUB_UID=$TEST_UID
  start_entrypoint
  wait_for_log 'jottery-server started'
  kill -TERM "$ENTRY_PID"
  wait_for_exit
  if [[ -f "$DATA/jottery.db" ]]; then
    pass "the database file exists before the server starts"
  else
    fail "the database file exists before the server starts"
  fi
  unset STUB_UID
}

# Caddy refuses to start on a size it can't parse, where the server uses its default
check_body_limit() {
  local name=$1 value=$2 expected=$3
  setup
  export STUB_UID=$TEST_UID MAX_PAYLOAD_SIZE=$value
  start_entrypoint
  if wait_for_log 'caddy started' && grep -q "caddy started.*body_limit=$expected\$" "$LOG"; then
    pass "$name"
  else
    fail "$name" "$(cat "$LOG")"
  fi
  kill -TERM "$ENTRY_PID"
  wait_for_exit
  unset STUB_UID MAX_PAYLOAD_SIZE
}

test_body_limit_follows_a_size_both_services_accept() {
  check_body_limit "Caddy's body limit follows MAX_PAYLOAD_SIZE" 5242880 5242880
  check_body_limit "a mistyped MAX_PAYLOAD_SIZE can't stop Caddy starting" 5MB unset
  check_body_limit "an empty MAX_PAYLOAD_SIZE leaves the default" "" unset
  check_body_limit "a size too large to parse leaves the default" 99999999999999999999 unset
}

test_a_command_runs_unprivileged_instead_of_the_services() {
  setup
  cat > "$BIN/jottery-admin" <<'STUB'
#!/usr/bin/env bash
echo "jottery-admin uid=$(id -u) args=$*" >> "$STUB_LOG"
exit 7
STUB
  chmod +x "$BIN/jottery-admin"
  export STUB_UID=0
  start_entrypoint jottery-admin users list
  wait_for_exit
  if [[ $STATUS == 7 ]] && grep -q "jottery-admin uid=$TEST_UID args=users list" "$LOG" && ! grep -q 'caddy started' "$LOG"; then
    pass "a command runs as the jottery user, in place of the services"
  else
    fail "a command runs as the jottery user, in place of the services" "status=$STATUS $(cat "$LOG")"
  fi
  unset STUB_UID
}

# Serves one canned response on a free port. Sets RESPONDER_PID and PORT.
start_responder() {
  local reply=$1
  python3 -I - "$reply" "$CASE/port" <<'PY' &
import socket, sys
reply, port_file = sys.argv[1], sys.argv[2]
server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(('127.0.0.1', 0))
server.listen(5)
open(port_file + '.tmp', 'w').write(str(server.getsockname()[1]))
__import__('os').rename(port_file + '.tmp', port_file)
while True:
    connection, _ = server.accept()
    connection.recv(4096)
    if reply != 'HANG':
        connection.sendall(reply.encode().decode('unicode_escape').encode())
        connection.close()
PY
  RESPONDER_PID=$!
  local attempt
  for attempt in $(seq 1 200); do
    [[ -f "$CASE/port" ]] && break
    sleep 0.02
  done
  PORT=$(cat "$CASE/port")
}

check_healthcheck() {
  local name=$1 reply=$2 expected=$3 status
  setup
  start_responder "$reply"
  JOTTERY_HEALTH_PORT=$PORT JOTTERY_HEALTH_TIMEOUT=1 bash "$HEALTHCHECK" > /dev/null 2>&1
  status=$?
  kill "$RESPONDER_PID" 2>/dev/null
  wait "$RESPONDER_PID" 2>/dev/null
  if [[ $status == "$expected" ]]; then
    pass "$name"
  else
    fail "$name" "exit status was $status, expected $expected"
  fi
}

test_healthcheck_with_nothing_listening() {
  setup
  # Bind and release a port, so that nothing is listening on it
  local port
  port=$(python3 -I -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
  if JOTTERY_HEALTH_PORT=$port JOTTERY_HEALTH_TIMEOUT=1 bash "$HEALTHCHECK" > /dev/null 2>&1; then
    fail "the healthcheck fails when nothing is listening"
  else
    pass "the healthcheck fails when nothing is listening"
  fi
}

test_server_death_stops_the_container
test_caddy_death_stops_the_container
test_clean_server_exit_still_counts_as_failure
test_sigterm_stops_both_cleanly
test_root_start_hands_over_data_and_drops
test_non_root_start_leaves_ownership_alone
test_unwritable_data_dir_fails_with_a_reason
test_database_file_is_created
test_body_limit_follows_a_size_both_services_accept
test_a_command_runs_unprivileged_instead_of_the_services

check_healthcheck "the healthcheck passes on 200" 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK' 0
check_healthcheck "the healthcheck passes without a reason phrase" 'HTTP/1.1 200\r\nContent-Length: 0\r\n\r\n' 0
check_healthcheck "the healthcheck fails on 502, a dead backend" 'HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n' 1
check_healthcheck "the healthcheck fails on a stalled response" 'HANG' 1
test_healthcheck_with_nothing_listening

echo
echo "$passed passed, $failed failed"
[[ $failed == 0 ]]
