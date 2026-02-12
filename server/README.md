# Jottery Sync Server

A lightweight, self-hosted sync server for Jottery that stores encrypted note data. The server never decrypts data - it simply manages encrypted payloads and handles conflict resolution through versioning.

## Features

- **End-to-end encryption**: Server stores only encrypted payloads
- **SQLite backend**: Simple, portable database
- **RESTful API**: Standard HTTP/JSON interface
- **Conflict resolution**: Last-Write-Wins with server versioning
- **Multi-client support**: Each device gets a unique API key
- **Attachment support**: Store encrypted file attachments
- **CORS enabled**: Works with web clients from any origin

## Requirements

- **Rust**: 1.70 or later (for building)
- **SQLite**: 3.35+ (bundled with sqlx)

## Quick Start

### 1. Configuration

Copy the example environment file and edit as needed:

```bash
cd server
cp .env.example .env
```

Edit `.env` to configure your server:

```env
# Database location (SQLite file)
DATABASE_URL=sqlite:jottery.db

# Server port
PORT=3030

# Maximum request payload size (bytes)
# Default: 10MB (10485760 bytes)
MAX_PAYLOAD_SIZE=10485760

# Logging level (trace, debug, info, warn, error)
RUST_LOG=info
```

### 2. Build

Build the server in release mode for optimal performance:

```bash
cargo build --release
```

The compiled binary will be at `./target/release/jottery-server`.

### 3. Run

Run the server directly:

```bash
./target/release/jottery-server
```

Or with cargo:

```bash
cargo run --release
```

The server will:
1. Initialize the SQLite database
2. Run database migrations automatically
3. Start listening on the configured port (default: 3030)

### 4. Verify

Check the server is running:

```bash
curl http://localhost:3030/health
# Should return: OK
```

## Admin Dashboard

The server includes a web-based admin dashboard for managing users, viewing statistics, and monitoring server activity.

### Accessing the Dashboard

Navigate to: `http://localhost:3030/admin`

### Default Credentials

**⚠️ IMPORTANT: Change the default password immediately after first login!**

- **Email**: `admin@localhost`
- **Password**: `changeme`

### Admin Features

The admin dashboard provides comprehensive user and system management:

**User Management:**
- View all registered users with statistics
- Approve pending user registrations
- Deactivate/reactivate user accounts
- Delete user accounts (with cascade to all user data)
- View per-user device list and statistics

**System Monitoring:**
- Total users, notes, and storage usage
- Active vs inactive users
- Recent sync activity
- Audit log of all sync operations

**Account Management:**
- Change admin password (requires current password)
- Session management (configurable expiry, default 7 days)
- Logout functionality

### Security Recommendations

1. **Change Default Admin Credentials:** Set `DEFAULT_ADMIN_EMAIL` and `DEFAULT_ADMIN_PASSWORD` in `.env` before deployment, then change password after first login
2. **Configure CORS:** Set `CORS_ALLOWED_ORIGINS` to your frontend domain(s) in production (comma-separated list)
3. **HTTPS Only:** Always use HTTPS in production (configure with reverse proxy like Nginx/Caddy)
4. **Strong Password Hashing:** Adjust `ARGON2_M_COST`, `ARGON2_T_COST`, `ARGON2_P_COST` for your security requirements
5. **Session Security:** Configure `SESSION_EXPIRY_DAYS` based on your security needs (shorter = more secure)
6. **Rate Limiting:** Configure rate limiting at the reverse proxy level (see `.env.example` for examples)
7. **Review Users:** Regularly review pending registrations and active users
8. **Monitor Audit Log:** Check for suspicious sync activity
9. **Backup Database:** Regular backups include all user data and credentials (passwords are hashed)

## API Endpoints

### User Registration & Authentication

#### Register User Account

Create a new user account (pending admin approval).

```http
POST /api/v1/auth/register-user
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "secure-password-min-12-chars"
}
```

**Response** (201 Created):
```json
{
  "userId": "uuid-v4",
  "email": "user@example.com",
  "status": "pending_approval",
  "message": "Account created. Awaiting admin approval."
}
```

**Errors:**
- `400` - Invalid email format or password too short (min 12 characters)
- `409` - Email already registered

#### Admin Login

Login to admin dashboard (creates session).

```http
POST /api/v1/admin/login
Content-Type: application/json

{
  "email": "admin@localhost",
  "password": "changeme"
}
```

**Response** (200 OK):
```json
{
  "sessionId": "uuid-session-token",
  "expiresAt": "2025-01-06T10:30:00Z",
  "user": {
    "id": "uuid-v4",
    "email": "admin@localhost",
    "isAdmin": true
  }
}
```

**Session Usage:**
Include session token in subsequent admin requests:
```http
Authorization: Bearer <sessionId>
```
Or via cookie (set automatically by browser):
```http
Cookie: session_token=<sessionId>
```

#### Register Device

Register a device for an existing, approved user.

```http
POST /api/v1/auth/register-device
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "user-password",
  "deviceName": "My Laptop",
  "deviceType": "web"
}
```

**Response** (201 Created):
```json
{
  "clientId": "uuid-v4",
  "apiKey": "64-character-hex-string",
  "userId": "uuid-v4",
  "deviceName": "My Laptop"
}
```

**Errors:**
- `401` - Invalid email or password
- `403` - User not approved or account deactivated

⚠️ **Important**: Save the `apiKey` - it's only returned once and cannot be recovered!

### Sync Operations

All sync endpoints require authentication via Bearer token:

```http
Authorization: Bearer <your-api-key>
```

#### Get Sync Status

Check server status and sync information.

```http
GET /api/v1/sync/status
Authorization: Bearer <api-key>
```

**Response** (200 OK):
```json
{
  "clientId": "uuid-v4",
  "serverLastModified": "2025-03-12T10:30:00Z",
  "noteCount": 42,
  "lastSyncedAt": null
}
```

#### Push Changes

Push local changes to the server.

```http
POST /api/v1/sync/push
Authorization: Bearer <api-key>
Content-Type: application/json

{
  "notes": [
    {
      "id": "note-uuid",
      "createdAt": "2025-03-12T10:00:00Z",
      "modifiedAt": "2025-03-12T10:30:00Z",
      "content": "encrypted-content",
      "tags": ["encrypted", "tags"],
      "attachments": [],
      "pinned": false,
      "deleted": false,
      "version": 1
    }
  ],
  "attachments": [
    {
      "id": "attachment-uuid",
      "data": "base64-encoded-encrypted-data"
    }
  ]
}
```

**Response** (200 OK):
```json
{
  "accepted": [
    {
      "id": "note-uuid",
      "serverVersion": 1,
      "syncedAt": "2025-03-12T10:30:00Z"
    }
  ],
  "rejected": [
    {
      "id": "another-note-uuid",
      "reason": "Server version is newer",
      "serverModifiedAt": "2025-03-12T10:35:00Z"
    }
  ],
  "errors": []
}
```

#### Pull Changes

Pull server changes since last sync.

```http
POST /api/v1/sync/pull
Authorization: Bearer <api-key>
Content-Type: application/json

{
  "lastSyncAt": "2025-03-12T10:00:00Z",
  "knownNoteIds": ["note-uuid-1", "note-uuid-2"]
}
```

**Response** (200 OK):
```json
{
  "notes": [
    {
      "id": "note-uuid",
      "createdAt": "2025-03-12T10:00:00Z",
      "modifiedAt": "2025-03-12T10:30:00Z",
      "content": "encrypted-content",
      "tags": ["encrypted"],
      "attachments": [],
      "pinned": false,
      "deleted": false,
      "version": 2,
      "wordWrap": true,
      "syntaxLanguage": "plain"
    }
  ],
  "deletions": [],
  "attachments": [
    {
      "id": "attachment-uuid",
      "data": "base64-encoded-encrypted-data"
    }
  ],
  "syncedAt": "2025-03-12T10:35:00Z"
}
```

#### Delete Note

Permanently delete a note from the server.

```http
DELETE /api/v1/sync/notes/:id
Authorization: Bearer <api-key>
```

**Response** (204 No Content)

### Admin API Endpoints

All admin endpoints require session authentication (see Admin Login above).

#### List Users

```http
GET /api/v1/admin/users
Authorization: Bearer <session-token>
```

**Response** (200 OK):
```json
{
  "users": [
    {
      "id": "uuid",
      "email": "user@example.com",
      "approved": false,
      "isAdmin": false,
      "isActive": true,
      "createdAt": "2025-01-01T10:00:00Z",
      "deviceCount": 2,
      "noteCount": 15
    }
  ]
}
```

#### Get User Details

```http
GET /api/v1/admin/users/:userId
Authorization: Bearer <session-token>
```

**Response** (200 OK):
```json
{
  "id": "uuid",
  "email": "user@example.com",
  "approved": true,
  "approvedAt": "2025-01-01T11:00:00Z",
  "isAdmin": false,
  "isActive": true,
  "createdAt": "2025-01-01T10:00:00Z",
  "lastLoginAt": "2025-01-05T14:30:00Z",
  "devices": [
    {
      "id": "device-uuid",
      "deviceName": "My Laptop",
      "deviceType": "web",
      "lastSeenAt": "2025-01-05T14:30:00Z"
    }
  ],
  "statistics": {
    "noteCount": 15,
    "attachmentCount": 3,
    "storageUsedBytes": 52428800
  }
}
```

#### Approve User

```http
POST /api/v1/admin/users/:userId/approve
Authorization: Bearer <session-token>
```

**Response:** 204 No Content

#### Deactivate/Activate User

```http
POST /api/v1/admin/users/:userId/deactivate
POST /api/v1/admin/users/:userId/activate
Authorization: Bearer <session-token>
```

**Response:** 204 No Content

#### Delete User

Permanently delete user and all associated data (notes, devices, attachments).

```http
DELETE /api/v1/admin/users/:userId
Authorization: Bearer <session-token>
```

**Response:** 204 No Content

#### Get Server Statistics

```http
GET /api/v1/admin/stats
Authorization: Bearer <session-token>
```

**Response** (200 OK):
```json
{
  "totalUsers": 10,
  "pendingUsers": 2,
  "activeUsers": 8,
  "totalDevices": 18,
  "totalNotes": 245,
  "totalAttachments": 42,
  "totalStorageBytes": 524288000,
  "recentSyncActivity": [
    {
      "userId": "uuid",
      "email": "user@example.com",
      "lastSyncAt": "2025-01-05T14:30:00Z",
      "noteCount": 15
    }
  ]
}
```

#### Change Admin Password

```http
POST /api/v1/admin/change-password
Authorization: Bearer <session-token>
Content-Type: application/json

{
  "currentPassword": "old-password",
  "newPassword": "new-password-min-12-chars"
}
```

**Response:** 204 No Content

**Errors:**
- `401` - Current password incorrect
- `400` - New password too short

See admin dashboard at `/admin` for full user management UI with visual interface for all these operations.

## Database Schema

The server uses SQLite with the following tables:

- **`users`**: User accounts with email, Argon2id password hash, approval status, admin role
- **`sessions`**: Admin dashboard sessions with token hash and expiry (7-day lifespan)
- **`clients`**: Registered devices with hashed API keys (linked to users via user_id)
- **`notes`**: Encrypted note data with versioning (linked to users via user_id)
- **`attachments_meta`**: Attachment metadata
- **`attachments_data`**: Binary attachment storage (BLOB)
- **`note_versions`**: Note version history (linked to users)

**Multi-User Schema:**
- Each user can have multiple devices (clients)
- Each note belongs to a user (via user_id foreign key)
- Device API keys are hashed (SHA-256) before storage
- Admin passwords use Argon2id hashing (memory=19456, time=2, parallelism=1)
- Sessions expire after 7 days and are automatically cleaned up

- **`sync_operations`**: Audit trail of sync operations

The schema is created automatically via SQLx migrations on first run. Migration 003 adds the multi-user system and creates the default admin user.

## Production Deployment

### Systemd Service

Create `/etc/systemd/system/jottery-server.service`:

```ini
[Unit]
Description=Jottery Sync Server
After=network.target

[Service]
Type=simple
User=jottery
Group=jottery
WorkingDirectory=/opt/jottery
EnvironmentFile=/opt/jottery/.env
ExecStart=/opt/jottery/jottery-server
Restart=on-failure
RestartSec=5s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/jottery

[Install]
WantedBy=multi-user.target
```

Install and start:

```bash
# Copy binary and config
sudo mkdir -p /opt/jottery
sudo cp target/release/jottery-server /opt/jottery/
sudo cp .env /opt/jottery/

# Create user
sudo useradd -r -s /bin/false jottery
sudo chown -R jottery:jottery /opt/jottery

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable jottery-server
sudo systemctl start jottery-server

# Check status
sudo systemctl status jottery-server
```

### Nginx Reverse Proxy

To expose the server with HTTPS, configure Nginx as a reverse proxy.

Create `/etc/nginx/sites-available/jottery`:

```nginx
server {
    listen 80;
    server_name sync.example.com;

    # Redirect HTTP to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name sync.example.com;

    # SSL certificates (use certbot/Let's Encrypt)
    ssl_certificate /etc/letsencrypt/live/sync.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/sync.example.com/privkey.pem;

    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Proxy to Jottery server
    location / {
        proxy_pass http://127.0.0.1:3030;
        proxy_http_version 1.1;

        # Headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Timeouts (for large attachments)
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;

        # Buffer settings
        client_max_body_size 10M;
    }
}
```

Enable and reload:

```bash
sudo ln -s /etc/nginx/sites-available/jottery /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

Get SSL certificate with certbot:

```bash
sudo certbot --nginx -d sync.example.com
```

## Backup and Maintenance

### Backup Database

The SQLite database is a single file. Back it up regularly:

```bash
# Stop the server
sudo systemctl stop jottery-server

# Backup database
sudo cp /opt/jottery/jottery.db /opt/jottery/backups/jottery-$(date +%Y%m%d).db

# Restart server
sudo systemctl start jottery-server
```

Or use SQLite's backup command while server is running:

```bash
sqlite3 /opt/jottery/jottery.db ".backup /opt/jottery/backups/jottery-$(date +%Y%m%d).db"
```

### Automated Backups

Add to crontab for daily backups:

```bash
# Daily backup at 2 AM
0 2 * * * sqlite3 /opt/jottery/jottery.db ".backup /opt/jottery/backups/jottery-$(date +\%Y\%m\%d).db"

# Delete backups older than 30 days
0 3 * * * find /opt/jottery/backups -name "jottery-*.db" -mtime +30 -delete
```

### Monitoring

Check server logs:

```bash
# View recent logs
sudo journalctl -u jottery-server -n 100

# Follow logs in real-time
sudo journalctl -u jottery-server -f

# Filter by log level
sudo journalctl -u jottery-server -p err
```

Monitor disk usage:

```bash
# Check database size
ls -lh /opt/jottery/jottery.db

# Analyze database
sqlite3 /opt/jottery/jottery.db "PRAGMA integrity_check; VACUUM;"
```

## Security Considerations

1. **API Keys**: Generated with cryptographically secure random bytes, hashed with SHA-256 before storage
2. **HTTPS Required**: Always use HTTPS in production (configure via Nginx)
3. **CORS**: Configured to allow all origins by default - restrict in production if needed
4. **Rate Limiting**: Not implemented - consider adding Nginx rate limiting
5. **Firewall**: Only expose port 443 (HTTPS) publicly, keep 3030 internal
6. **Updates**: Keep Rust dependencies updated with `cargo update`

## Troubleshooting

### Database Locked

If you see "database is locked" errors:

```bash
# Check for stale lock
fuser /opt/jottery/jottery.db

# If needed, stop server and remove locks
sudo systemctl stop jottery-server
rm -f /opt/jottery/jottery.db-shm /opt/jottery/jottery.db-wal
sudo systemctl start jottery-server
```

### Port Already in Use

If port 3030 is already in use:

```bash
# Find process using port 3030
sudo lsof -i :3030

# Change port in .env
echo "PORT=3031" >> .env
```

### High Memory Usage

SQLite connection pool can be tuned in `src/db.rs`:

```rust
// Reduce max connections
.max_connections(5)  // Default is 10
```

### Migration Failures

If migrations fail:

```bash
# Check migration status
sqlx migrate info --database-url sqlite:jottery.db

# Reset database (⚠️ destroys all data)
rm jottery.db
./target/release/jottery-server  # Will recreate with migrations
```

## Development

### Running in Development

```bash
# Run with auto-reload
cargo watch -x run

# Run tests
cargo test

# Check for issues
cargo clippy
```

### Database Migrations

Create new migration:

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features sqlite

# Create migration
sqlx migrate add <migration_name>

# Edit the generated file in migrations/
# Then run the server to apply it automatically
```

### Environment Variables

See `.env.example` for a complete list of configuration options with documentation.

Key environment variables:

```env
# Basic configuration
DATABASE_URL=sqlite:jottery.db
PORT=3030
MAX_PAYLOAD_SIZE=5242880

# CORS security
CORS_ALLOWED_ORIGINS=https://example.com

# Session security
SESSION_EXPIRY_DAYS=7

# Default admin account (CHANGE IN PRODUCTION!)
DEFAULT_ADMIN_EMAIL=admin@localhost
DEFAULT_ADMIN_PASSWORD=changeme

# Password hashing (Argon2id)
ARGON2_M_COST=19456  # Memory in KiB
ARGON2_T_COST=2      # Iterations
ARGON2_P_COST=1      # Threads

# User settings
DEFAULT_STORAGE_QUOTA_MB=1000

# Logging
RUST_LOG=info
```

For production deployments, review and customize all security settings in `.env.example`.

## License

MIT License - see LICENSE file in project root

## Support

For issues and questions:
- GitHub: https://github.com/seesee/jottery
- Sync Protocol: See [docs/SYNC-SPEC.md](../docs/SYNC-SPEC.md)
