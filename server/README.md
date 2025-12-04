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

## API Endpoints

### Authentication

#### Register Client

Register a new device and receive an API key.

```http
POST /api/v1/auth/register
Content-Type: application/json

{
  "deviceName": "My Laptop",
  "deviceType": "web"
}
```

**Response** (201 Created):
```json
{
  "apiKey": "64-character-hex-string",
  "clientId": "uuid-v4",
  "createdAt": "2025-03-12T10:30:00Z"
}
```

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

## Database Schema

The server uses SQLite with the following tables:

- **`clients`**: Registered devices with hashed API keys
- **`notes`**: Encrypted note data with versioning
- **`attachments_meta`**: Attachment metadata
- **`attachments_data`**: Binary attachment storage (BLOB)
- **`sync_operations`**: Audit trail of sync operations

The schema is created automatically via SQLx migrations on first run.

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
5. **Firewall**: Only expose port 443 (HTTPS) publicly, keep 3000 internal
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

Development `.env` example:

```env
DATABASE_URL=sqlite:dev.db
PORT=3000
RUST_LOG=debug
```

## License

MIT License - see LICENSE file in project root

## Support

For issues and questions:
- GitHub: https://github.com/yourusername/jottery
- Spec: See `jottery-spec.md` in project root
