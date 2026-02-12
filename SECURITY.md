# Security Policy

This document outlines the security architecture of Jottery and provides guidance for secure deployment.

## Reporting Vulnerabilities

If you discover a security vulnerability, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email security concerns to the repository maintainer
3. Include a detailed description of the vulnerability
4. Allow reasonable time for a fix before public disclosure

We aim to acknowledge reports within 48 hours and provide a fix timeline within 7 days.

## Security Architecture

### End-to-End Encryption

All sensitive data is encrypted on the client before transmission to the server:

| Data Type | Encryption | Key Source |
|-----------|------------|------------|
| Note content | AES-256-GCM | Master key (derived from password) |
| Tags | AES-256-GCM | Master key |
| Attachment data | AES-256-GCM | Master key |
| Attachment filenames | AES-256-GCM | Master key |

**The server never sees unencrypted note content, tags, or attachments.**

### Key Derivation

Master keys are derived from user passwords using PBKDF2-SHA256:

- **Iterations**: 600,000 (OWASP 2023 recommendation)
- **Salt**: 32 bytes, randomly generated per user
- **Output**: 256-bit key

The salt and iteration count are stored with the encrypted data to allow key re-derivation on unlock.

### Password Hashing (Server)

User account passwords are hashed using Argon2id:

- **Memory**: 19,456 KiB
- **Iterations**: 2
- **Parallelism**: 1

### API Key Security

- Device API keys are 32 bytes of cryptographically secure random data
- Keys are SHA-256 hashed before storage in the database
- Each device has a unique API key
- Keys can be individually revoked

### Session Management

- Admin sessions use UUID v4 tokens, SHA-256 hashed before storage
- Default session expiry: 7 days (configurable via `SESSION_EXPIRY_DAYS`)
- Sessions are invalidated on logout

## Deployment Security Checklist

Before deploying Jottery to production, complete this checklist:

### Required

- [ ] **Change default admin password**
  - Default: `admin@localhost` / `changeme`
  - Set via environment variables or change immediately after first login
  - Use a strong password (12+ characters recommended)

- [ ] **Enable HTTPS/TLS**
  - Deploy behind a reverse proxy (Caddy, Nginx, Traefik)
  - Obtain valid TLS certificates (Let's Encrypt recommended)
  - Never expose the application over plain HTTP in production

- [ ] **Set CORS origins** (if using external clients)
  ```bash
  -e CORS_ALLOWED_ORIGINS=https://your-domain.com
  ```

### Recommended

- [ ] **Configure session expiry**
  ```bash
  -e SESSION_EXPIRY_DAYS=1  # Shorter for sensitive environments
  ```

- [ ] **Set up database backups**
  - The SQLite database is stored in `/app/data/jottery.db`
  - Back up regularly (daily recommended)
  - Store backups encrypted and off-site

- [ ] **Enable disk encryption**
  - SQLite databases are not encrypted at rest
  - Use full-disk encryption (LUKS, FileVault, BitLocker)

- [ ] **Configure logging and monitoring**
  - Monitor for unusual access patterns
  - Set up alerts for failed login attempts
  - Review logs periodically

- [ ] **Restrict network access**
  - Use firewall rules to limit access
  - Consider VPN for internal deployments

### Environment Variables

| Variable | Default | Security Impact |
|----------|---------|-----------------|
| `DEFAULT_ADMIN_PASSWORD` | `changeme` | **Critical** - Change immediately |
| `SESSION_EXPIRY_DAYS` | `7` | Lower values reduce session hijacking window |
| `CORS_ALLOWED_ORIGINS` | *(none)* | Restrict to trusted domains |
| `MAX_PAYLOAD_SIZE` | `5242880` | Limits upload size (DoS prevention) |

## Known Limitations

### SQLite At-Rest Encryption

The server-side SQLite database is **not encrypted at rest**. While all sensitive note data is encrypted client-side before storage, metadata (user IDs, timestamps, note IDs) is stored in plaintext.

**Mitigation**: Use full-disk encryption on the server.

### Remember Password Feature (TUI)

The TUI "Remember Password" feature stores an obfuscated (not encrypted) copy of your password:

- Uses AES-256-GCM with a **hardcoded key** (visible in source code)
- Provides convenience, not security
- File stored at `~/.config/jottery/.jottery_remember` with mode `0600`
- Anyone with access to the file and source code can recover the password

**Recommendation**: Only use this feature on trusted, single-user systems. For shared or untrusted systems, enter your password each time.

### macOS Keychain (TUI)

On macOS, the TUI can use the system keychain for password storage instead of file-based obfuscation. This requires the binary to be properly code-signed. Ad-hoc signed binaries fall back to file-based storage.

### Browser Session Storage

The web client can optionally store an encrypted session token in localStorage for convenience. This is **not** synced to the server.

### Password Recovery

There is **no password recovery mechanism**. If you forget your password:

- Your encrypted notes cannot be recovered
- You must delete your account and start fresh
- This is by design - we cannot decrypt your data

**Recommendation**: Use a password manager.

## Security Best Practices

### For Users

1. **Use a strong, unique password** (12+ characters)
2. **Use a password manager** to store your Jottery password
3. **Enable HTTPS** when connecting to sync servers
4. **Review connected devices** periodically in the admin portal
5. **Revoke devices** you no longer use
6. **Export backups** regularly and store them securely

### For Administrators

1. **Keep software updated** - Apply updates promptly
2. **Monitor access logs** for suspicious activity
3. **Review user accounts** - Deactivate unused accounts
4. **Test backups** - Verify you can restore from backups
5. **Use separate admin account** - Don't use admin for daily notes

## Threat Model

### What Jottery Protects Against

- Server compromise - Attacker gains encrypted blobs, not plaintext
- Network eavesdropping - TLS + client-side encryption
- Unauthorized access - Authentication required
- Accidental data exposure - All sensitive data encrypted

### What Jottery Does NOT Protect Against

- Compromised client device - Attacker has access to decrypted notes
- Keylogger/malware - Password can be captured
- Weak passwords - Brute-force attacks on exported data
- Rubber-hose cryptanalysis - User coercion

## Cryptographic Details

### Algorithms

| Purpose | Algorithm | Key Size |
|---------|-----------|----------|
| Note encryption | AES-256-GCM | 256 bits |
| Key derivation | PBKDF2-SHA256 | 256 bits |
| Password hashing | Argon2id | - |
| API key hashing | SHA-256 | 256 bits |
| Session tokens | UUID v4 + SHA-256 | 128 bits (UUID) |

### Libraries

- **Web Client**: Web Crypto API (browser native)
- **TUI Client**: `aes-gcm`, `pbkdf2`, `sha2` (Rust crates)
- **Server**: `argon2`, `sha2` (Rust crates)

## Updates

This security policy was last updated: February 2026

For the latest version, see the repository at: https://github.com/seesee/jottery
