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
| Note content | AES-256-GCM | Master key |
| Tags | AES-256-GCM | Master key |
| Attachment data | AES-256-GCM | Master key |
| Attachment filenames | AES-256-GCM | Master key |
| Saved search names | AES-256-GCM | Master key |
| Saved search queries | AES-256-GCM | Master key |

**The server never sees unencrypted note content, tags, or attachments.**

### Envelope Encryption (v1.1.0+)

Jottery uses an envelope encryption model that decouples the master key from the user's password:

```
Password + Device Salt ──PBKDF2──▶ Device Key ──AES-GCM──▶ Wrapped Master Key (local)
Password + User ID     ──PBKDF2──▶ Wrapping Key ─AES-GCM─▶ Wrapped Master Key (server)
                                    Master Key ──AES-GCM──▶ Encrypted Notes
```

- A **random 256-bit master key** encrypts all note data
- The master key is **wrapped** (encrypted) with a device-specific key derived from the password
- A second wrapped copy is stored on the server for multi-device onboarding
- **Password changes** only re-wrap the master key — no note re-encryption needed (O(1) operation)
- **New devices** download the server-wrapped master key and unwrap it with the user's password

#### Key Derivation

| Purpose | Algorithm | Iterations | Salt |
|---------|-----------|------------|------|
| Device key (local unlock) | PBKDF2-SHA256 | 600,000 | 32-byte random per device |
| Wrapping key (server) | PBKDF2-SHA256 | 1,000,000 | User ID (UTF-8) |
| Legacy key (pre-envelope) | PBKDF2-SHA256 | 600,000 | 32-byte random per vault |

The higher iteration count for the server wrapping key provides additional protection since the wrapped blob is stored remotely.

#### Legacy Vaults

Vaults created before v1.1.0 use a legacy model where the master key is derived directly from the password + salt. These vaults are automatically migrated to envelope encryption on next unlock when sync is enabled. The migration is transparent and non-destructive — if it fails, the legacy path continues to work.

### Password Hashing (Server)

User account passwords are hashed using Argon2id:

- **Memory**: 19,456 KiB
- **Iterations**: 2
- **Parallelism**: 1

These parameters follow OWASP recommendations for server-side password storage.

### API Key Security

- Device API keys are 32 bytes of cryptographically secure random data
- Keys are SHA-256 hashed before storage in the database
- Each device has a unique API key
- Keys can be individually revoked or deleted from the admin dashboard

### Session Management

- Admin sessions use UUID v4 tokens, SHA-256 hashed before storage
- User portal sessions use the same mechanism
- Default session expiry: 7 days (configurable via `SESSION_EXPIRY_DAYS`)
- Sessions are invalidated on logout

### SSE Token Security

Server-Sent Events (SSE) connections use short-lived tokens instead of API keys:

- Tokens are obtained by exchanging a valid API key via an authenticated endpoint
- Tokens expire after 5 minutes
- Tokens are SHA-256 hashed before storage (in-memory only)
- This prevents API keys from appearing in URLs, browser history, or server logs

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
  - The server SQLite database is not encrypted at rest
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
| `DEFAULT_ADMIN_PASSWORD` | `changeme` | **Critical** — Change immediately |
| `SESSION_EXPIRY_DAYS` | `7` | Lower values reduce session hijacking window |
| `CORS_ALLOWED_ORIGINS` | *(none)* | Restrict to trusted domains |
| `MAX_PAYLOAD_SIZE` | `5242880` | Limits upload size (DoS prevention) |

## Known Limitations

### Server-Side SQLite At-Rest Encryption

The server-side SQLite database is **not encrypted at rest**. While all sensitive note data is encrypted client-side before storage, metadata (user IDs, timestamps, note IDs) is stored in plaintext.

**Mitigation**: Use full-disk encryption on the server.

### TUI Local Database

The TUI client uses SQLCipher for its local database, which provides transparent AES-256 encryption at rest. The database password is derived from the user's vault password.

### Remember Password Feature (TUI)

The TUI "Remember Password" feature has two backends:

**macOS Keychain** (preferred): On macOS, the TUI uses the system keychain for secure password storage. This requires the binary to be properly code-signed. Ad-hoc signed binaries fall back to file-based storage.

**File-based** (fallback): Stores an obfuscated copy of the password:

- Uses AES-256-GCM with a **hardcoded key** (visible in source code)
- Provides convenience, not security
- File stored at `~/.config/jottery/.jottery_remember` with mode `0600`
- Anyone with access to the file and source code can recover the password

**Recommendation**: Only use file-based password storage on trusted, single-user systems. Prefer the macOS Keychain backend where available. For shared or untrusted systems, enter your password each time.

### Browser Session Storage

The web client can optionally store an encrypted session token in localStorage for convenience. This is **not** synced to the server.

### Password Recovery

There is **no password recovery mechanism**. If you forget your password:

- Your encrypted notes cannot be recovered
- You must delete your account and start fresh
- This is by design — we cannot decrypt your data

With envelope encryption, the server stores a wrapped copy of the master key, but it can only be unwrapped with the correct password. There is no backdoor.

**Recommendation**: Use a password manager.

### Wrapped Key Security

The server-stored wrapped master key is encrypted with a key derived from `password + userId` using PBKDF2 with 1,000,000 iterations. An attacker who compromises the server database would need to brute-force the user's password to unwrap the master key. The high iteration count makes this computationally expensive.

The wrapped key blob contains no information about the password — it is indistinguishable from random data without the correct wrapping key.

## Security Best Practices

### For Users

1. **Use a strong, unique password** (12+ characters)
2. **Use a password manager** to store your Jottery password
3. **Enable HTTPS** when connecting to sync servers
4. **Review connected devices** periodically in the admin or user portal
5. **Delete devices** you no longer use
6. **Create backups** regularly and store them securely

### For Administrators

1. **Keep software updated** — Apply updates promptly
2. **Monitor access logs** for suspicious activity
3. **Review user accounts** — Deactivate unused accounts
4. **Test backups** — Verify you can restore from backups
5. **Use separate admin account** — Don't use the admin account for daily notes

## Threat Model

### What Jottery Protects Against

- **Server compromise** — Attacker gains encrypted blobs, not plaintext. The wrapped master key requires the user's password to unwrap.
- **Network eavesdropping** — TLS + client-side encryption. Even without TLS, note content is encrypted.
- **Unauthorised access** — Authentication required for all operations.
- **Accidental data exposure** — All sensitive data is encrypted before leaving the client.
- **Password change overhead** — Envelope encryption means password changes don't require re-encrypting all notes.

### What Jottery Does NOT Protect Against

- **Compromised client device** — Attacker has access to decrypted notes in memory
- **Keylogger/malware** — Password can be captured at input
- **Weak passwords** — Brute-force attacks on the wrapped master key
- **Rubber-hose cryptanalysis** — User coercion
- **Compromised server + weak password** — If the server is compromised AND the user has a weak password, the wrapped master key could be brute-forced

## Cryptographic Details

### Algorithms

| Purpose | Algorithm | Key Size |
|---------|-----------|----------|
| Note encryption | AES-256-GCM | 256 bits |
| Master key wrapping | AES-256-GCM | 256 bits |
| Device key derivation | PBKDF2-SHA256 | 256 bits |
| Server wrapping key derivation | PBKDF2-SHA256 | 256 bits |
| Password hashing (server) | Argon2id | — |
| API key hashing | SHA-256 | 256 bits |
| Session tokens | UUID v4 + SHA-256 | 128 bits (UUID) |
| SSE tokens | UUID v4 + SHA-256 | 128 bits (UUID) |

### Libraries

| Client | Cryptographic Library |
|--------|----------------------|
| Web client | Web Crypto API (browser native) |
| TUI client | `aes-gcm`, `pbkdf2`, `sha2` (Rust crates) |
| Server | `argon2`, `sha2` (Rust crates) |

### Nonce/IV Generation

All AES-256-GCM operations use a 12-byte (96-bit) randomly generated nonce via a cryptographically secure random number generator. Nonces are stored alongside the ciphertext in the encrypted data format:

```json
{"ciphertext": "<base64>", "iv": "<base64>"}
```

The GCM authentication tag (16 bytes) is appended to the ciphertext.

## Updates

This security policy was last updated: 24th March 2026

For the latest version, see the repository at: https://github.com/seesee/jottery
