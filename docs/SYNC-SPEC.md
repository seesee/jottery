# Jottery Sync Protocol Specification

**Version**: 1.0
**Last Updated**: January 2026

This document specifies the Jottery sync protocol, enabling third-party clients to synchronise notes with a Jottery server while maintaining end-to-end encryption.

## Overview

Jottery uses a client-side encrypted sync model where:

- All note content and tags are encrypted **before** leaving the client
- The server stores only encrypted payloads and cannot read note content
- Each user can have multiple devices (clients)
- Sync uses a push/pull model with server-side versioning
- Conflicts are resolved using Last-Write-Wins based on `modifiedAt` timestamps

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Web Client    │     │   TUI Client    │     │  Third-Party    │
│  (JavaScript)   │     │    (Rust)       │     │    Client       │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         │  HTTPS/JSON           │  HTTPS/JSON           │
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                                 ▼
                    ┌─────────────────────────┐
                    │    Jottery Server       │
                    │   (Stores encrypted     │
                    │    payloads only)       │
                    └─────────────────────────┘
```

## Authentication

### User Registration

Users must first create an account, which requires admin approval:

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

### Device Registration

Once approved, register each device to obtain an API key:

```http
POST /api/v1/auth/register-device
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "user-password",
  "deviceName": "My Laptop",
  "deviceType": "cli"
}
```

**Device Types**: `web`, `cli`

**Response** (201 Created):
```json
{
  "clientId": "uuid-v4",
  "apiKey": "64-character-hex-string",
  "userId": "uuid-v4",
  "deviceName": "My Laptop"
}
```

> ⚠️ **Important**: The `apiKey` is only returned once. Store it securely.

### API Authentication

All sync endpoints require the API key in the Authorization header:

```http
Authorization: Bearer <api-key>
```

## Data Models

### Note

```typescript
interface Note {
  id: string;                    // UUID v4
  createdAt: string;             // ISO 8601 (e.g., "2025-01-01T10:00:00Z")
  modifiedAt: string;            // ISO 8601 - used for conflict resolution
  content: string;               // Encrypted JSON string (see Encryption)
  tags: string[];                // Array of encrypted tag strings
  attachments: Attachment[];     // Attachment references
  pinned: boolean;               // Pin status (unencrypted)
  deleted: boolean;              // Soft delete flag
  deletedAt?: string;            // ISO 8601 (if deleted)
  version: number;               // Client version counter
  wordWrap?: boolean;            // Editor preference
  syntaxLanguage?: string;       // Syntax highlighting language
}
```

### Attachment Reference

```typescript
interface Attachment {
  id: string;                    // UUID v4
  filename: string;              // Encrypted filename
  mimeType: string;              // MIME type (unencrypted)
  size: number;                  // Size in bytes
  data: string;                  // Reference ID for blob storage
}
```

### Attachment Data

```typescript
interface AttachmentData {
  id: string;                    // Matches Attachment.id
  data: string;                  // Base64-encoded encrypted blob
}
```

## Encryption

### Key Derivation

Clients must derive an encryption key from the user's password:

1. **PBKDF2** (minimum 100,000 iterations) or **Argon2id** (recommended)
2. Use a unique salt per user (stored locally, not synced)
3. Derive a 256-bit key for AES-256-GCM

**Web Client Example (PBKDF2)**:
```javascript
const salt = crypto.getRandomValues(new Uint8Array(16));
const keyMaterial = await crypto.subtle.importKey(
  "raw",
  new TextEncoder().encode(password),
  "PBKDF2",
  false,
  ["deriveBits", "deriveKey"]
);
const key = await crypto.subtle.deriveKey(
  {
    name: "PBKDF2",
    salt: salt,
    iterations: 100000,
    hash: "SHA-256"
  },
  keyMaterial,
  { name: "AES-GCM", length: 256 },
  false,
  ["encrypt", "decrypt"]
);
```

### Encryption Format

All encrypted data uses AES-256-GCM with a unique IV per encryption:

```typescript
interface EncryptedData {
  iv: string;         // Base64-encoded 12-byte IV
  data: string;       // Base64-encoded ciphertext
  tag: string;        // Base64-encoded 16-byte auth tag (optional, may be appended to data)
}
```

**JSON Serialisation**: Encrypted fields are stored as JSON-stringified `EncryptedData` objects.

### What Gets Encrypted

| Field | Encrypted | Notes |
|-------|-----------|-------|
| `note.content` | ✅ Yes | Full note text |
| `note.tags[]` | ✅ Yes | Each tag encrypted separately |
| `attachment.filename` | ✅ Yes | Original filename |
| `attachment.data` | ✅ Yes | Binary content |
| `note.id` | ❌ No | Required for indexing |
| `note.createdAt` | ❌ No | Required for ordering |
| `note.modifiedAt` | ❌ No | Required for conflict resolution |
| `note.pinned` | ❌ No | Required for sorting |
| `note.deleted` | ❌ No | Required for filtering |
| `attachment.mimeType` | ❌ No | Required for content-type handling |
| `attachment.size` | ❌ No | Required for quota management |

### Encryption Example

```javascript
async function encryptText(plaintext, key) {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encoded = new TextEncoder().encode(plaintext);

  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    encoded
  );

  return JSON.stringify({
    iv: btoa(String.fromCharCode(...iv)),
    data: btoa(String.fromCharCode(...new Uint8Array(ciphertext)))
  });
}

async function decryptText(encryptedJson, key) {
  const { iv, data } = JSON.parse(encryptedJson);

  const ivBytes = Uint8Array.from(atob(iv), c => c.charCodeAt(0));
  const dataBytes = Uint8Array.from(atob(data), c => c.charCodeAt(0));

  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: ivBytes },
    key,
    dataBytes
  );

  return new TextDecoder().decode(plaintext);
}
```

## Sync Protocol

### Sync Flow

1. **Check Status**: Get server state to determine if sync is needed
2. **Pull Changes**: Fetch notes modified since last sync
3. **Merge Locally**: Apply remote changes, handling conflicts
4. **Push Changes**: Send local modifications to server
5. **Update Timestamps**: Store `syncedAt` for next sync

### 1. Get Sync Status

```http
GET /api/v1/sync/status
Authorization: Bearer <api-key>
```

**Response**:
```json
{
  "clientId": "uuid-v4",
  "serverLastModified": "2025-03-12T10:30:00Z",
  "noteCount": 42,
  "lastSyncedAt": "2025-03-12T10:00:00Z"
}
```

### 2. Pull Changes

```http
POST /api/v1/sync/pull
Authorization: Bearer <api-key>
Content-Type: application/json

{
  "lastSyncAt": "2025-03-12T10:00:00Z",
  "knownNoteIds": ["note-uuid-1", "note-uuid-2"],
  "knownAttachmentIds": ["attachment-uuid-1"]
}
```

**Response**:
```json
{
  "notes": [
    {
      "id": "note-uuid",
      "createdAt": "2025-03-12T10:00:00Z",
      "modifiedAt": "2025-03-12T10:30:00Z",
      "content": "{\"iv\":\"...\",\"data\":\"...\"}",
      "tags": ["{\"iv\":\"...\",\"data\":\"...\"}"],
      "attachments": [],
      "pinned": false,
      "deleted": false,
      "version": 2,
      "wordWrap": true,
      "syntaxLanguage": "markdown"
    }
  ],
  "deletions": [
    {
      "id": "deleted-note-uuid",
      "deletedAt": "2025-03-12T10:25:00Z"
    }
  ],
  "attachments": [
    {
      "id": "attachment-uuid",
      "data": "base64-encoded-encrypted-blob"
    }
  ],
  "versions": [
    {
      "versionKey": "note-uuid:1",
      "noteId": "note-uuid",
      "version": 1,
      "createdAt": "2025-03-12T09:00:00Z",
      "syncedAt": "2025-03-12T10:30:00Z",
      "content": "{\"iv\":\"...\",\"data\":\"...\"}",
      "tags": [],
      "attachments": [],
      "syntaxLanguage": "markdown",
      "wordWrap": true,
      "reason": "sync"
    }
  ],
  "syncedAt": "2025-03-12T10:35:00Z"
}
```

### 3. Push Changes

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
      "content": "{\"iv\":\"...\",\"data\":\"...\"}",
      "tags": ["{\"iv\":\"...\",\"data\":\"...\"}"],
      "attachments": [],
      "pinned": false,
      "deleted": false,
      "version": 1
    }
  ],
  "attachments": [
    {
      "id": "attachment-uuid",
      "data": "base64-encoded-encrypted-blob"
    }
  ],
  "versions": []
}
```

**Response**:
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
      "serverModifiedAt": "2025-03-12T10:35:00Z",
      "serverContent": "{encrypted content}",
      "serverTags": ["{encrypted tag 1}", "{encrypted tag 2}"],
      "serverVersion": 3,
      "serverAttachments": [
        {
          "id": "attachment-uuid",
          "filename": "{encrypted filename}",
          "mimeType": "image/png",
          "size": 12345,
          "data": "attachment-uuid"
        }
      ],
      "serverPinned": false,
      "serverSyntaxLanguage": "markdown",
      "serverWordWrap": true
    }
  ],
  "errors": []
}
```

### 4. Delete Note (Permanent)

```http
DELETE /api/v1/sync/notes/:id
Authorization: Bearer <api-key>
```

**Response**: 204 No Content

## Conflict Resolution

Jottery uses **Last-Write-Wins** (LWW) conflict resolution:

1. Compare `modifiedAt` timestamps
2. The note with the later timestamp wins
3. Server rejects pushes if `serverModifiedAt > clientModifiedAt`
4. Rejected notes must be re-pulled and merged locally

### Handling Rejected Pushes

When a push is rejected, the server includes the complete server note data in the rejection response. This enables clients to show a diff/comparison view for manual conflict resolution without requiring a separate pull.

**Resolution options:**

| Option | Description |
|--------|-------------|
| Keep Mine | Discard server version, push local with updated `modifiedAt` |
| Keep Server | Replace local with server version, clear conflict |
| Merge | Open editor with both versions, save merged result |
| Keep Both | Create duplicate note from server version with new ID |

**Client implementation:**

1. Store server version data from rejection response
2. Mark note with `lastSyncStatus: 'conflict'`
3. Show conflict indicator in UI
4. Present diff/comparison modal with resolution options
5. Apply chosen resolution and clear conflict status
6. Push resolved version on next sync

## Version History

The server maintains version history for notes. Each version includes:

- Full snapshot of encrypted content
- Timestamp of creation
- Reason: `sync` (automatic) or `manual-sync` (user-triggered)

Clients should create a version before restoring an older version.

## Health Check

```http
GET /health
```

**Response**: `OK` (200)

## Error Codes

| Status | Meaning |
|--------|---------|
| 200 | Success |
| 201 | Created |
| 204 | No Content (success, no body) |
| 400 | Bad Request (validation error) |
| 401 | Unauthorized (invalid/missing API key) |
| 403 | Forbidden (user not approved or deactivated) |
| 404 | Not Found |
| 409 | Conflict (e.g., email already registered) |
| 500 | Internal Server Error |

## Sync Credentials Sharing

Clients may share sync credentials between devices using a base64-encoded JSON payload:

```typescript
interface SyncCredentials {
  endpoint: string;    // Server URL (e.g., "https://sync.example.com")
  apiKey: string;      // Device API key
  clientId: string;    // Device UUID
  salt?: string;       // Optional encryption salt (for web clients)
}

// Encode
const encoded = btoa(JSON.stringify(credentials));

// Decode
const credentials = JSON.parse(atob(encoded));
```

> **Note**: Each device should register separately for better security and audit trails. Credential sharing is provided for convenience during initial setup.

## Implementation Checklist

For a compliant Jottery client:

- [ ] User registration and device registration
- [ ] Secure key derivation (PBKDF2 100k+ iterations or Argon2id)
- [ ] AES-256-GCM encryption/decryption
- [ ] Encrypt content and tags before push
- [ ] Decrypt content and tags after pull
- [ ] Track `lastSyncAt` for incremental sync
- [ ] Handle rejected pushes with re-pull
- [ ] Support soft delete (deleted flag)
- [ ] Support attachments (optional)
- [ ] Support version history (optional)
- [ ] Implement auto-sync with configurable interval (recommended: 1 minute)
- [ ] Handle network errors gracefully
- [ ] Store encryption salt securely (never sync the salt)

## Security Considerations

1. **Never transmit plaintext**: All sensitive data must be encrypted client-side
2. **Unique IVs**: Generate a new IV for each encryption operation
3. **Secure storage**: Store the encryption key only in memory during session
4. **Auto-lock**: Clear key after inactivity timeout (recommended: 15 minutes)
5. **HTTPS only**: Always use TLS in production
6. **Salt isolation**: Each client should have its own salt, never synced
7. **Password requirements**: Minimum 12 characters recommended

## Example: Full Sync Cycle

```javascript
async function syncNotes() {
  // 1. Check status
  const status = await fetch('/api/v1/sync/status', {
    headers: { 'Authorization': `Bearer ${apiKey}` }
  }).then(r => r.json());

  // 2. Pull changes since last sync
  const pullResponse = await fetch('/api/v1/sync/pull', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      lastSyncAt: localStorage.getItem('lastSyncAt'),
      knownNoteIds: localNotes.map(n => n.id),
      knownAttachmentIds: localAttachments.map(a => a.id)
    })
  }).then(r => r.json());

  // 3. Decrypt and merge pulled notes
  for (const note of pullResponse.notes) {
    note.content = await decryptText(note.content, encryptionKey);
    note.tags = await Promise.all(note.tags.map(t => decryptText(t, encryptionKey)));
    mergeNote(note);
  }

  // 4. Push local changes
  const localChanges = getUnsynced();
  const pushPayload = {
    notes: await Promise.all(localChanges.map(async note => ({
      ...note,
      content: await encryptText(note.content, encryptionKey),
      tags: await Promise.all(note.tags.map(t => encryptText(t, encryptionKey)))
    }))),
    attachments: [],
    versions: []
  };

  const pushResponse = await fetch('/api/v1/sync/push', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(pushPayload)
  }).then(r => r.json());

  // 5. Handle results
  for (const accepted of pushResponse.accepted) {
    markSynced(accepted.id, accepted.syncedAt);
  }

  for (const rejected of pushResponse.rejected) {
    // Re-pull and merge conflicting note
    handleConflict(rejected);
  }

  // 6. Update sync timestamp
  localStorage.setItem('lastSyncAt', pullResponse.syncedAt);
}
```

## Changelog

### 1.0 (January 2026)
- Initial specification
- Multi-user support with admin approval
- Device-based API key authentication
- AES-256-GCM encryption
- Push/pull sync with LWW conflict resolution
- Version history support
- Attachment support
