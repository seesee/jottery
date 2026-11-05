# Backup format

Every client writes this format, so that any client can restore any other client's
backup. The reference is `src/lib/services/backupService.ts`. If this document and
that file disagree, the file is right and this document needs fixing.

A backup holds everything needed to restore a vault on a device with nothing else on
it. Its contents are encrypted with the vault's master key, and the password that
unlocks the vault is what unlocks the backup.

## The file

A backup is one UTF-8 JSON object, in a file ending `.jottery-backup`. The web client
refuses any name that doesn't end in `.jottery-backup` or `.json`.

```json
{
  "version": "3.0",
  "type": "jottery-encrypted-backup",
  "createdAt": "2026-11-05T02:36:39.488Z",
  "encryption": { },
  "batches": [
    { "type": "notes", "index": 0, "count": 2, "data": "eyJhbGciOiJkaXIi..." }
  ],
  "summary": { "notes": 2, "attachments": 1, "versions": 1, "savedSearches": 1 }
}
```

| Field | Meaning |
| --- | --- |
| `version` | `"3.0"`. Write this. Web also reads `2.x` (see [Version 2.1](#version-21)). |
| `type` | Always `"jottery-encrypted-backup"`. |
| `createdAt` | ISO 8601, with a timezone. |
| `encryption` | How to get the key. See [Getting the key](#getting-the-key). |
| `batches` | The encrypted contents. See [Batches](#batches). |
| `summary` | How many items the backup holds. It is checked before a restore. |

**Keep the keys in this order.** A file can be too large to load, so a reader may take
the header from the first 10 KB and the summary from the last 1 KB. The web client
does this for its backup statistics, and for restoring any file over 100 MB. So
`version`, `type`, `createdAt` and `encryption` come first, `summary` comes last, and
the header has to fit in 10 KB. A batch has its own `type`, so a file that puts
`batches` first has its header misread.

## Getting the key

The key is 32 bytes. It is the master key of the vault, used directly for every batch.
The `encryption` object is the vault's own local metadata, in one of two forms.

### Envelope vaults

```json
{
  "envelopeVersion": 1,
  "deviceSalt": "base64",
  "localWrappedMaster": "{\"ciphertext\":\"base64\",\"iv\":\"base64\"}",
  "wrappingKdfVersion": 1,
  "createdAt": "2026-11-05T02:36:39.488Z",
  "algorithm": "AES-256-GCM"
}
```

1. Derive a 256-bit key with PBKDF2-HMAC-SHA256 from the password and the
   base64-decoded `deviceSalt`, using **600,000** iterations. The header doesn't record
   the count, so it is fixed for this format.
2. `localWrappedMaster` is a JSON object held as a string. Decrypt its `ciphertext` with
   AES-256-GCM using that key and its `iv`, with no additional data. The authentication
   tag is the last 16 bytes of `ciphertext`.
3. The result is the master key. If decryption fails, the password is wrong.

`wrappingKdfVersion` describes the copy of the key held on the sync server, and isn't
needed to read a backup.

### Legacy vaults

```json
{ "salt": "base64", "iterations": 100000, "algorithm": "AES-256-GCM", "createdAt": "..." }
```

The key comes straight from the password: PBKDF2-HMAC-SHA256 with the base64-decoded
`salt` and the given `iterations`. Nothing in the header can show a wrong password, so
the first batch has to be decrypted to find out.

## Batches

```json
{ "type": "notes", "index": 0, "count": 2, "data": "<JWE>" }
```

| Field | Meaning |
| --- | --- |
| `type` | `notes`, `attachments`, `versions`, `saved_searches`, `settings` or `sync_metadata`. |
| `index` | Position among batches of the same type, from 0. |
| `count` | How many items `data` holds. It is checked. |
| `data` | The items, encrypted. |

`data` is made like this:

1. Serialise the items as a JSON array, in UTF-8.
2. Compress with zlib (RFC 1950, as `pako.deflate` does), **not** raw DEFLATE.
3. Encrypt as a JWE in compact serialisation (RFC 7516), with the protected header
   `{"alg":"dir","enc":"A256GCM"}`, a 96-bit IV and a 128-bit tag. The key is the master
   key, so the encrypted key part is empty and the token has the form
   `header..iv.ciphertext.tag`.

As RFC 7516 requires, **the additional authenticated data is the ASCII bytes of the
first part of the token**, the base64url protected header. Sealing with no additional
data produces a token that looks right and won't decrypt.

Web writes at most 250 notes, 50 attachments or 250 versions per batch, and starts a
new batch of attachments once one holds about 16 MB. A reader
shouldn't depend on those sizes, or on the order of the batches. A type with nothing to
hold can be left out. `settings` and `sync_metadata` are optional, and iOS writes
neither.

## Items

Items are the records as a client stores them, so **encrypted fields stay encrypted
inside the batch**. Unless it says otherwise below, an encrypted field is a JSON object
held as a string, `{"ciphertext":"base64","iv":"base64"}`, sealed with AES-256-GCM under
the master key, with no additional data.

Lists are JSON arrays, not strings holding JSON.

### `notes`

```json
{
  "id": "10a2c41e-06df-4535-bf61-ff6d27e4189f",
  "createdAt": "...",
  "modifiedAt": "...",
  "content": "<encrypted text>",
  "tags": ["<encrypted JSON array of tags>"],
  "attachments": [
    { "id": "uuid", "filename": "<encrypted text>", "mimeType": "image/png", "size": 1024, "data": "uuid" }
  ],
  "pinned": false,
  "archived": false,
  "locked": false,
  "deleted": false,
  "version": 1,
  "contentHash": "base64",
  "parentHash": null,
  "hashChain": ["base64"]
}
```

Optional: `archivedAt`, `lockedAt`, `deletedAt`, `syncedAt`, `wordWrap`, `syntaxLanguage`,
`showPreview`, `color`, `needsSync`.

`tags` normally has one entry: the whole list, encrypted as a JSON array. An empty list
means no tags. Older notes can instead have one entry per tag, each an encrypted
JSON-encoded string, so a reader has to accept both. Soft-deleted notes are included.

### `attachments`

```json
{ "id": "uuid", "blob": "base64", "thumbnail": "base64" }
```

`blob` is the stored blob, base64-encoded. Decoded, it is UTF-8 JSON in the usual
`{"ciphertext","iv"}` form, and the ciphertext decrypts to the file. `thumbnail` is
optional. An attachment that no note or version refers to is skipped on restore, but
still counts towards `summary.attachments`.

### `versions`

```json
{
  "versionKey": "<noteId>:<version>",
  "noteId": "uuid",
  "version": 1,
  "createdAt": "...",
  "syncedAt": "...",
  "content": "<encrypted text>",
  "tags": ["<as for a note>"],
  "attachments": [],
  "reason": "sync"
}
```

`reason` is `sync` or `manual-sync`. Optional: `syntaxLanguage`, `wordWrap`, `showPreview`,
`color`, `contentHash`, `parentHash`.

### `saved_searches`

```json
{
  "id": "uuid",
  "name": "<encrypted text>",
  "query": "<encrypted text>",
  "order": 1,
  "createdAt": "...",
  "modifiedAt": "...",
  "deleted": false,
  "version": 1
}
```

Optional: `syncedAt`, `deletedAt`, `needsSync`. A backup from an older web client can
hold `name` and `query` as plaintext. Web encrypts those once, after restoring.

### `settings`

One item, in a batch of its own. These are the web client's `UserSettings`, and a
reader takes the fields it knows. `rememberPassword`, `persistSession` and
`persistSessionTimeout` stay behind on a restore: they are security choices about one
device, made with a warning and the password. A client that writes this batch should use those
names, such as `language`, `theme`, `sortOrder` and `autoLockTimeout`.

`theme` is `light`, `dark` or `auto`. The TUI's colour schemes don't fit that, so it adds
an optional **`tuiTheme`** field holding the scheme's name, and sets `theme` to `light` or
`dark` to match. Only the TUI acts on `tuiTheme`. Web doesn't use it, but keeps it with
the rest of the settings, so it survives being restored and backed up again on web.

### `sync_metadata`

One item, in a batch of its own, and optional. It holds the sync connection, including
`apiKey`, which is encrypted text. A restored vault has to register as a device of its
own, so web marks the key with a `RESTORE:` prefix when restoring, and gets a new one at
the next unlock. A client that can't do this should leave the batch out, and the user
connects again.

## The summary

```json
{ "notes": 2, "attachments": 1, "versions": 1, "savedSearches": 1 }
```

Each number is the total of `count` over the batches of that type. A client must only
count what it wrote: a mismatch means the file is treated as damaged.

## Restoring

A restore replaces the vault. IndexedDB only frees space when a database is deleted, so
web deletes first, which is why everything is checked before that happens. Clients
should work the same way:

1. Check the structure: the type, the version, and that every batch has a five-part
   token and a whole-number `count`.
2. Get the key, and decrypt the first batch. If either fails, the password is wrong.
3. Refuse a backup with no batches. There is nothing to restore, and nothing that proves
   the password, so any password would pass and the vault would be wiped.
4. Decrypt **every** batch. Each must hold `count` items, and the totals must agree with
   `summary`. Any failure stops here, with the vault untouched.
5. Replace the vault.
6. Never skip an item and report success. If storage runs out, web goes back to an empty
   vault and says so: the backup file is still good, but part of a vault could be
   mistaken for all of it.

## Version 2.1

Web still reads this older form, and no client should write it. In place of `batches`
and `summary` it has `data`, an array of JWE tokens, one per record. Each token is sealed
as above but isn't compressed, and decrypts to `{"type": "...", "data": {...}}`, where
`type` is `note`, `attachment`, `version`, `saved_search`, `settings` or `sync_metadata`.

## Older iOS backups

iOS backups made before this format was adopted differ from it. They seal the JWE with
no additional data, use raw DEFLATE, hold tags and attachments as strings, put `batches`
first, and can have a `.jottery` name. The iOS app still opens them. Web doesn't: restore
one on iOS, then back up again.

## Testing

`src/test/fixtures/` holds a real file from each other client: iOS, the TUI, and Android
from a legacy and an envelope vault. Each password sits beside its file, in a
`.password` file. `backupService.crossclient.test.ts` restores them all, through both
readers, and unlocks the result. The Android tests read fixtures written by web, in
`android-native/app/src/test/resources/backup/`.
