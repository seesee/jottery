#!/usr/bin/env bash
#
# Start a demo sync server for TUI demo recordings
#
# This script:
#   1. Creates a fresh server database with a demo user
#   2. Starts the sync server on port 3030
#   3. Waits for the server to be ready
#
# Usage:
#   ./start-demo-server.sh         # Start server (blocks)
#   ./start-demo-server.sh --bg    # Start in background, print PID
#   ./start-demo-server.sh --stop  # Stop background server
#
# Demo credentials:
#   Email: demo@localhost
#   Password: demo-password-2026

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SERVER_DIR="$PROJECT_ROOT/server"
DEMO_DIR="$SCRIPT_DIR/demo-server"
DEMO_DB="$DEMO_DIR/jottery-demo.db"
PID_FILE="$DEMO_DIR/server.pid"

# Demo credentials (must match what's used in VHS tapes)
DEMO_EMAIL="demo@example.com"
DEMO_PASSWORD="demo-password-2026"
DEMO_USER_ID="00000000-0000-0000-0000-000000000002"

# Server port
PORT=3030

# Colours
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if server binary exists
check_server() {
    if [[ ! -x "$SERVER_DIR/target/release/jottery-server" ]]; then
        log_error "Server binary not found at: $SERVER_DIR/target/release/jottery-server"
        log_info "Build it with: cd $SERVER_DIR && cargo build --release"
        exit 1
    fi
}

# Create demo database with pre-approved user
create_demo_db() {
    log_info "Creating demo server database..."

    mkdir -p "$DEMO_DIR"
    rm -f "$DEMO_DB" "$DEMO_DB-wal" "$DEMO_DB-shm"

    # Create empty database file for SQLx
    touch "$DEMO_DB"

    log_info "Database will be initialised by server migrations"
}

# Wait for server to be ready
wait_for_server() {
    local max_attempts=30
    local attempt=0

    log_info "Waiting for server to be ready..."

    while [[ $attempt -lt $max_attempts ]]; do
        if curl -s "http://localhost:$PORT/health" > /dev/null 2>&1; then
            log_success "Server is ready on port $PORT"
            return 0
        fi
        attempt=$((attempt + 1))
        sleep 0.5
    done

    log_error "Server failed to start within 15 seconds"
    return 1
}

# Create demo user via API (after server starts)
create_demo_user() {
    log_info "Creating demo user via API..."

    # First, check if demo user exists by trying to register
    # The register-user endpoint will fail if user already exists

    # Register user (will be pending approval)
    local register_result
    register_result=$(curl -s -X POST "http://localhost:$PORT/api/v1/auth/register-user" \
        -H "Content-Type: application/json" \
        -d "{\"email\": \"$DEMO_EMAIL\", \"password\": \"$DEMO_PASSWORD\"}" 2>&1)

    log_info "Register result: $register_result"

    # Login as admin to approve the user
    log_info "Logging in as admin..."
    local admin_login
    admin_login=$(curl -s -X POST "http://localhost:$PORT/api/v1/auth/login" \
        -H "Content-Type: application/json" \
        -d '{"email": "admin@localhost", "password": "changeme"}')

    local session_token
    session_token=$(echo "$admin_login" | grep -o '"sessionId":"[^"]*"' | cut -d'"' -f4)

    if [[ -z "$session_token" ]]; then
        log_error "Failed to get admin session token"
        log_info "Admin login response: $admin_login"
        return 1
    fi

    log_info "Got admin session token: ${session_token:0:8}..."

    # Get list of users to find demo user ID
    local users_list
    users_list=$(curl -s "http://localhost:$PORT/api/v1/admin/users" \
        -H "Authorization: Bearer $session_token")

    # Extract demo user ID - find user object with demo email
    # The JSON structure is {"users":[{...},...]}, we need to find the one with demo@example.com
    local demo_user_id
    # Use python for reliable JSON parsing if available, fall back to grep
    if command -v python3 &> /dev/null; then
        demo_user_id=$(echo "$users_list" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for user in data.get('users', []):
    if user.get('email') == '$DEMO_EMAIL':
        print(user.get('id', ''))
        break
" 2>/dev/null)
    else
        # Fallback: extract using grep (less reliable but works for simple cases)
        demo_user_id=$(echo "$users_list" | grep -o '"email":"demo@example.com"' -B 5 | grep -o '"id":"[^"]*"' | tail -1 | cut -d'"' -f4)
    fi

    if [[ -z "$demo_user_id" ]]; then
        log_error "Could not find demo user in users list"
        log_info "Users: $users_list"
        return 1
    fi

    log_info "Found demo user ID: $demo_user_id"

    # Approve the demo user
    log_info "Approving demo user..."
    local approve_result
    approve_result=$(curl -s -X POST "http://localhost:$PORT/api/v1/admin/users/$demo_user_id/approve" \
        -H "Authorization: Bearer $session_token")

    log_info "Approve result: $approve_result"

    log_success "Demo user created and approved: $DEMO_EMAIL"
}

# Stop background server
stop_server() {
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            log_info "Stopping server (PID: $pid)..."
            kill "$pid"
            rm -f "$PID_FILE"
            log_success "Server stopped"
        else
            log_info "Server not running (stale PID file)"
            rm -f "$PID_FILE"
        fi
    else
        log_info "No PID file found"
    fi
}

# Start server
start_server() {
    local background=false

    if [[ "$1" == "--bg" ]]; then
        background=true
    fi

    check_server
    create_demo_db

    # Set environment for server
    # Use sqlite: URL with ?mode=rwc to allow create
    export DATABASE_URL="sqlite:$DEMO_DB?mode=rwc"
    export PORT="$PORT"
    export CORS_ALLOWED_ORIGINS="*"
    export RUST_LOG="warn"
    export PASSWORD_COMPLEXITY="none"  # Allow simple demo password

    cd "$SERVER_DIR"

    if $background; then
        log_info "Starting server in background..."
        "$SERVER_DIR/target/release/jottery-server" > "$DEMO_DIR/server.log" 2>&1 &
        echo $! > "$PID_FILE"
        log_info "Server PID: $(cat "$PID_FILE")"

        if wait_for_server; then
            create_demo_user
            log_success "Demo server running on http://localhost:$PORT"
            log_info "Stop with: $0 --stop"
        else
            stop_server
            exit 1
        fi
    else
        log_info "Starting server (foreground)..."
        log_info "Press Ctrl+C to stop"

        # Start server in background temporarily to create user
        "$SERVER_DIR/target/release/jottery-server" > "$DEMO_DIR/server.log" 2>&1 &
        local temp_pid=$!

        if wait_for_server; then
            create_demo_user
            kill $temp_pid 2>/dev/null || true

            # Now run in foreground
            log_info "Server ready, running in foreground..."
            exec "$SERVER_DIR/target/release/jottery-server"
        else
            kill $temp_pid 2>/dev/null || true
            exit 1
        fi
    fi
}

# Main
case "${1:-}" in
    --stop)
        stop_server
        ;;
    --bg)
        start_server --bg
        ;;
    *)
        start_server
        ;;
esac
