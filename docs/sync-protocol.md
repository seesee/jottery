# Sync protocol

This is the reference for how the sync API behaves today. It covers push, pull, attachments and deletions, and the three additions that clients adopt by detecting them: the pull cursor, authoritative attachment lists, and rejected deletions.

`SYNC-SPEC.md` is the older overview for third-party authors. Where the two disagree, this document is correct.

## Conventions

- All paths are under `/api/v1`. Bodies are JSON, field names are camelCase.
- Device endpoints authenticate with `Authorization: Bearer <apiKey>`.
- Times are RFC 3339 strings. Send UTC.
- Note content, tags, attachment filenames and blobs are encrypted by the client. The server can't read them.
- **The server is untrusted.** Clients must validate everything it sends when it arrives: ids are UUIDs, timestamps parse, sizes and mime types are sane. Decryption failure is an error, never plaintext.

### Compatibility rules

- Every change to this protocol is additive. A new client must work with an old server, and an old client with a new server.
- Clients must ignore fields they don't recognise.
- Servers ignore request fields they don't recognise. This is what makes it safe to send the new request fields to an old server.

### Errors

An error response has the shape `{ "error": "message" }`. Some carry a stable `code` as well.

| Status | Meaning on a device endpoint |
|---|---|
| 400 | The request is malformed |
| 401 | The API key is unknown or the device was revoked. The device needs registering again |
| 403 | The key is valid but the account is deactivated or not approved. Stop syncing and tell the user. Don't discard the credentials: reactivating the account restores the key |
| 404 | The attachment doesn't exist for this user |
| 413 | The body is over the limit, or over the account's upload limit. A push over the limit writes nothing |
| 408 | The request took too long. Try again: sending the same push twice is safe |
| 429 | Rate limited. See [Rate limits](#rate-limits) |
| 500 | Server error. A push that fails writes nothing, so it is safe to retry |
| 503 | The server is busy. Try again after `Retry-After` |
| 507 | The account is at its storage quota, on a server that enforces one. `code` is `storage_quota_exceeded`. Nothing was written. Pushes that only delete still work, so tell the user to make room |

## Detecting what a server supports

The three additions shipped together, so one signal implies all of them, but each has its own check. There is no version endpoint: detect from responses.

| Feature | How to tell the server has it |
|---|---|
| Pull cursor | The pull response contains `nextCursor`. A server only sends it in reply to a request that contained `cursor` |
| `rejectedDeletions` | The push response contains `rejectedDeletions`, which is always an array, even when empty |
| `attachmentsAuthoritative` | Needs no detection. Send it whenever it is true: an old server ignores it, and the attachment simply stays on the server as it always has |

A client can't assume the server stays the same. It may be rolled back, or the user may point the client at another server. Check every response rather than remembering the answer.

## Push

```
POST /api/v1/sync/push
```

A push is **one transaction**. It either applies completely or changes nothing. Conflict detection happens inside the transaction, so of two concurrent pushes of the same note from the same parent, exactly one is accepted. A pull can't see part of a push.

### Request

| Field | Type | Required | Meaning |
|---|---|---|---|
| `notes` | [Note](#note)[] | yes | Notes to create or update |
| `attachments` | `{ id, data }`[] | yes | Blobs, `data` being standard base64. May be empty |
| `versions` | [Version](#version)[] | yes | Version history entries. May be empty |
| `savedSearches` | [SavedSearch](#saved-search)[] | no | Saved searches to create or update |
| `deletions` | `{ id, deletedAt }`[] | no | Notes permanently deleted (see [Deletions](#deletions)) |
| `attachmentsAuthoritative` | boolean | no | Each note's `attachments` is its complete list (see [Removing attachments](#removing-attachments)). Defaults to false |

Keep pushes to about 100 notes. The server holds the database write lock for the whole push, so every other device's write waits for it.

The size of `attachments[].data` plus note content must fit the account's upload limit, or the push fails with 413. A note over `MAX_NOTE_CONTENT_SIZE` also fails the whole push with 413.

### Response

| Field | Type | Meaning |
|---|---|---|
| `accepted` | `{ id, serverVersion, syncedAt }`[] | Notes stored |
| `rejected` | [Rejection](#rejection)[] | Notes refused because of a conflict, with the server's copy |
| `errors` | string[] | Items skipped without failing the push, such as a blob with invalid base64. For logging: don't parse these |
| `attachmentWarnings` | `{ noteId, attachmentIds }`[] | Only present when non-empty. Accepted notes referencing attachments the server has no blob for. Re-push any blobs you hold |
| `rejectedDeletions` | string[] | Always present. Note ids whose deletion was refused (see [Deletions](#deletions)) |
| `skippedDeleted` | string[] | Always present on a current server. Pushed notes skipped because they were permanently deleted after this edit. Stop sending them and drop the local copy, unless it has changed since. An older server doesn't send it, so treat it as empty |

### How a note is accepted

A note's times are checked first. If `createdAt` or `modifiedAt`, or `deletedAt`, `archivedAt` or `lockedAt` when present, can't be read as RFC 3339, or is more than 24 hours ahead of the server's clock, the note is skipped. It's in neither `accepted` nor `rejected`, the reason is in `errors`, and nothing else in the push is affected. A saved search's `modifiedAt` is checked the same way. The note stays on the device, and can be sent again once the device's clock is right.

Times are stored as they were sent, and compared as instants.

For each note, in order:

1. **A tombstone exists.** If the edit is later than the tombstone's `deletedAt`, the tombstone is cleared and the note is stored, resurrecting it. Otherwise the note is **skipped**: its id is in `skippedDeleted`, it appears in neither `accepted` nor `rejected`, and the client learns it is gone from `deletions` on a pull.
2. **The note is new to the server.** It is accepted.
3. **Both sides have hashes** (the push sends `contentHash` and `parentHash`, and the server has `contentHash`). It is accepted if the server's hash equals the pushed `parentHash` (a fast-forward), equals the pushed `contentHash` (no change), or appears in the pushed `hashChain`. Otherwise it is rejected as a conflict.
4. **Otherwise** it is accepted if `modifiedAt` is not earlier than the server's.

A note that isn't accepted changes nothing: its versions and attachment references are skipped too.

### Rejection

A `rejected` entry describes the server's current note, so a client can merge without another request: `id`, `reason`, `serverModifiedAt`, `serverContent`, `serverTags`, `serverVersion`, `serverAttachments`, `serverPinned`, `serverArchived`, `serverArchivedAt`, `serverLocked`, `serverLockedAt`, `serverSyntaxLanguage`, `serverWordWrap`, `serverShowPreview`, `serverColor`, `serverContentHash`, `serverParentHash`, `serverHashChain`.

`serverDeleted` and `serverDeletedAt` say whether the server's copy is in the bin. An older server doesn't send them, so treat them as `false` and `null`.

When a common ancestor is found it also has `ancestorHash`, `ancestorContent` and `ancestorTags`, for a three-way merge.

## Pull

```
POST /api/v1/sync/pull
```

There are two modes. **Cursor mode** is selected by a `cursor` field in the request, even an empty one. Without it the request is in **timestamp mode**, which older clients use. New clients should use cursor mode and fall back to timestamp mode only for a server that doesn't support it.

### Request

| Field | Type | Required | Meaning |
|---|---|---|---|
| `cursor` | string | no | A previous `nextCursor`, or `""` to start. Selects cursor mode. `null` is the same as leaving it out |
| `lastSyncAt` | string | no | Timestamp mode: return changes since this time. Cursor mode: used only when `cursor` is empty or unusable |
| `limit` | integer | no | Page size, default 100. See each mode for what it counts |
| `offset` | integer | no | Timestamp mode only. Ignored in cursor mode |
| `knownAttachmentIds` | string[] | no | Attachments this client holds, whose blobs needn't be sent |
| `knownNoteIds` | string[] | no | Ignored by the server |

Older servers require `knownNoteIds` and `knownAttachmentIds`, so always send them, as empty arrays if need be.

### Response

| Field | Type | Meaning |
|---|---|---|
| `notes` | [Note](#note)[] | Notes that changed. `attachments` is the note's current list |
| `deletions` | `{ id, deletedAt }`[] | Notes permanently deleted |
| `attachments` | `{ id, data }`[] | Blobs for the attachments of `notes` in this response, except those in `knownAttachmentIds`. Versions' attachments are never included: fetch those with `GET /sync/attachments/:id` |
| `versions` | [Version](#version)[] | Version history entries |
| `savedSearches` | [SavedSearch](#saved-search)[] | Saved searches, including soft-deleted ones |
| `syncedAt` | string | The server's clock when the pull began |
| `totalCount` | integer | For progress display only. See each mode |
| `hasMore` | boolean | There is more to fetch. Pull again |
| `nextCursor` | string | Cursor mode only. See below |
| `inboxItems`, `inboxCount` | | Only present when the inbox isn't empty. Sent in full on every page |

### Cursor mode

The server gives every change to a note, tombstone, version or saved search a position in a per-user sequence, assigned as the change commits. A cursor is a position in that sequence, so it can't skip a change: unlike a timestamp, the order of positions is the order of commits.

The rules:

- **`cursor` is opaque.** Store it and send it back. Don't parse it, compare it or construct one, other than the empty string. It is a short ASCII string.
- **One page is one range of the sequence.** Every kind of change in that range is in the response, and nothing is sent twice unless it changed again.
- **`limit` counts changes of every kind**, not just notes. It is clamped to 1–500. A page can also end sooner, once its blobs reach about 16 MB, so don't expect it to be full.
- **`hasMore: true` means pull again at once with `nextCursor`.** Use `hasMore` alone to decide. A page can hold no notes at all and still have more behind it, because it might be all versions.
- **Save `nextCursor` only after the page is applied.** If a client stops between applying and saving, it gets the same page again, which is harmless as long as applying is idempotent. Saving first can lose a page.
- **Every cursor-mode response has `nextCursor`, even an empty one.** It then equals the cursor sent.
- **`syncedAt` is still sent.** Store it as `lastSyncAt`, so a client can fall back if the server loses cursor support.
- **`totalCount`** is the number of this user's notes changed after the position the request started from, so it falls from page to page.
- **A cursor belongs to one account on one server.** Clear it when either changes, as with `lastSyncAt`.

**A cursor the server can't use restarts the pull.** If `cursor` is unparsable, or ahead of the sequence because the server was restored from a backup, the server treats it as empty. That can only send more than needed, so clients need no special handling: everything they receive is valid and applying it is idempotent. Restoring a backup can still leave a client's cursor within the restored sequence, and that can't be detected. An operator who restores a backup should have clients do a full pull.

**Starting a cursor.** Send `cursor: ""`:

| Request | What comes back |
|---|---|
| `cursor: ""`, no `lastSyncAt` | Everything, from the start: a full sync |
| `cursor: ""` with `lastSyncAt` | Everything since that time, with the two-minute overlap described under timestamp mode. Rows from around then may come too |

### Timestamp mode

This is how released clients pull, and it keeps working. It has known weaknesses that cursor mode doesn't.

- `lastSyncAt` selects rows the **server** stamped after that time. It is reached back two minutes first, to cover a row stamped before the client's timestamp but committed after it, and modest clock skew. Rows in that window are sent again.
- Omitting it, or sending `null`, selects everything.
- Notes are paged with `limit` and `offset`, ordered by server time. `limit` counts notes only, and `totalCount` is the number of matching notes across all pages. `limit` is clamped to 1–500.
- A page can hold fewer than `limit` notes, because it ends once its blobs reach about 16 MB. The first note with blobs is always sent, however large, so a page with `hasMore: true` always has a note in it. Advance `offset` by the number of notes received, not by `limit`.
- `deletions`, `versions` and `savedSearches` aren't paged: all of them are sent on every page.
- `deletions` leaves out tombstones older than 30 days, so a client offline for longer misses them. Cursor mode has no such limit.

Its weaknesses, which is why clients should move off it:

- If a note changes while a client is partway through its pages, that note moves to the end, the rest shift forward, and one note can be skipped.
- A push that takes longer than the overlap, or a clock further out than that, can still lose a change.

### Applying a pulled note

**Any pull can deliver a note the client already has.** A push's own notes come back on the next pull, a page can be repeated after a crash, and the timestamp overlap and a full pull both re-send notes. So applying a note must be idempotent, and it must never overwrite a local edit that hasn't been pushed yet.

The note's hashes say what to do. When the client has a local edit pending:

| Pulled note | Meaning | Action |
|---|---|---|
| `contentHash` equals the local `contentHash` | The same note | Nothing |
| `contentHash` equals the local `parentHash`, or appears in the local `hashChain` | Older than the local edit: the client has already seen it | Keep the local edit, and push it |
| `hashChain` contains the local `contentHash` | Newer, and includes the local edit | Take the pulled note |
| Anything else | The two have diverged, even if `hashChain` contains the local `parentHash` | A conflict: merge, or ask the user |

`hashChain` starts with the note's own `contentHash`, so a chain containing the local `parentHash` shows only that the two share an ancestor. It doesn't make the pulled note newer.

With no local edit pending, take the pulled note unless the first row applies.

### Moving a client to the cursor

Keep maintaining `lastSyncAt` from `syncedAt`, and add a stored `cursor`, initially empty.

```
request = { cursor: stored cursor or "",
            lastSyncAt: stored lastSyncAt,
            limit: 100, offset: 0,
            knownNoteIds: [], knownAttachmentIds: [...] }

loop:
    response = pull(request)
    apply response
    save response.syncedAt as lastSyncAt

    if response has nextCursor:
        save it as the stored cursor
        request.cursor = response.nextCursor
    else:
        # An older server. It ignored cursor and paged by offset.
        clear the stored cursor
        request.offset += number of notes in response

    if not response.hasMore: stop
```

The loop works against both kinds of server, because an old server ignores `cursor` and a new one ignores `offset`.

**Consider a one-off full pull on first adoption.** Timestamp mode can have silently skipped changes in the past, and nothing can detect those afterwards. Sending `cursor: ""` with no `lastSyncAt` once, the first time a client finds the server supports cursors, repairs that. It re-sends every note but no blobs the client lists in `knownAttachmentIds`.

## Attachments

An attachment belongs to a **user**, not to a note. A note references attachments, several notes can reference the same one, and the server keeps one blob for it. Attachment ids are unique per user, so two accounts can hold the same id with different contents.

To store an attachment, push a note whose `attachments` lists its reference, and push the blob in the top-level `attachments` array, in the same push if you can. A blob for an attachment with no reference from this user is ignored, without an error.

A blob sent after its note, in a later push, is fine. The server re-announces the notes that reference it, so devices that already pulled them get the blob on their next pull. Repair pushes rely on this: `{ notes: [], attachments: [...], versions: [] }`.

```
GET /api/v1/sync/attachments/:id
```

Returns `{ id, data }`, or 404 if this user has no such blob. Use it for attachments a pull doesn't send, such as those of a version.

### Removing attachments

By default, a push **adds** references and never removes any, so a client that sends a shorter list changes nothing. This is deliberate: some released clients send empty lists by mistake.

A push with `attachmentsAuthoritative: true` means every note in it carries its **complete** list. For each accepted note, the server removes references not in the list. Rejected and skipped notes are left alone.

- It applies to the whole request, so in a batched sync send it on every batch.
- Only send it when each note's list is complete and known to be correct. If a client can't vouch for a list, leave the flag off for that push.
- Other devices see the shorter list on their next pull, as a normal note change, and should drop the attachment.
- A device without the flag that still holds the old list puts the reference back when it next pushes that note. This stops once all of a user's clients send the flag.

**When the blob is deleted.** The server deletes a blob once no note references it and no stored version snapshot does, because restoring that version needs it. It waits seven days after the last reference goes, so a device that is catching up can link it again, then deletes it in hourly housekeeping. Blobs of permanently deleted notes follow the same rule.

## Deletions

There are two kinds of deletion.

**Soft delete** is an ordinary note update with `deleted: true` and `deletedAt`, pushed in `notes`. It syncs like any other edit. The server purges soft-deleted notes 30 days after it received them, and records a tombstone as it does, with the note's `deletedAt`.

**Permanent delete**, as when the bin is emptied, is pushed in `deletions` with the time it happened. The server deletes the note and records a tombstone, which other devices receive in `deletions`.

### An edit beats an older deletion

One rule applies whichever reaches the server first: **the note survives if its `modifiedAt` is later than the deletion's `deletedAt`.** Times are compared as instants, so their formatting doesn't matter. A time that can't be parsed keeps the note, because keeping one can be undone and deleting one can't, and so does a `deletedAt` more than 24 hours ahead of the server's clock.

If a deletion loses, the server:

- keeps the note, its versions and its attachments,
- records no tombstone,
- lists the id in `rejectedDeletions`, and
- re-announces the note, so the device that deleted it receives it on its next pull.

A client that gets an id in `rejectedDeletions` should **stop sending that deletion**, removing it from its pending list. It needn't do anything else: the note comes back through the normal pull. A client that keeps re-sending a refused deletion causes the note to be re-announced to every device on each push.

A deletion for a note the server doesn't have is ignored. No tombstone is recorded, and it isn't reported.

Don't use `DELETE /api/v1/sync/notes/:id`. It bypasses the rule above: it deletes whatever edit the server holds. No client uses it.

## Other endpoints

```
GET /api/v1/sync/status
```

Returns `clientId`, `deviceName`, `serverLastModified`, `noteCount` and `lastSyncedAt`, which is always null.

```
DELETE /api/v1/sync/device
```

Revokes the calling device. Returns 204.

### Live updates

```
GET /api/v1/sync/events/token
```

Exchanges the API key for `{ token, expires_in }`, valid for five minutes, so the key stays out of URLs. Note the snake_case field. It follows the same 401 and 403 rules as other device endpoints.

```
GET /api/v1/sync/events?token=<token>
```

A Server-Sent Events stream. An event named `sync` with data `pull` means another of the user's devices changed something, so pull. The device that made the change isn't told. Heartbeats arrive every 30 seconds.

A token can be used to reconnect for as long as it's valid, but only while the device and its account are. Otherwise connecting gets 401, and a stream that's already open ends within a minute. Ask for a new token after a 401: that request says why, with the usual 401 or 403.

### Wrapped master key

```
GET /api/v1/auth/wrapped-key
PUT /api/v1/auth/wrapped-key
```

Stores the opaque wrapped key as `{ blob, kdfVersion, kdfIterations }`. `blob` is limited to 8 KB, `kdfVersion` must be at least 1 and `kdfIterations` at least 100,000. GET returns 404 when there is none.

GET also returns `updatedAt`, an opaque string that changes on every write. Its presence means the server supports conditional writes. Check for it on each GET: sending `If-Match` to an older server can fail, because a browser's preflight for a client on another origin isn't allowed that header.

There's one key per account, and overwriting it with the wrong one locks out every new device. So a new client says what it wants written, in one of three ways:

| Purpose | Send | Fails when |
|---|---|---|
| First upload, after GET returned 404 | `"replace": false` in the body | The account already has a key |
| Repair of a blob this device read but couldn't open | Header `If-Match: <updatedAt>`, with the value from the GET | The key has changed since that GET, or has gone |
| Password change | `"replace": true` in the body | Never |

A write that fails gets 409 and changes nothing. Don't retry it: GET again, and decide again from what's there. A 409 during a repair usually means another device has just changed the password.

Send these on every PUT, including to an older server, which ignores `replace`. `If-Match` wins if a request has both. The `/user/wrapped-key` route for sessions follows the same rules.

A PUT with neither overwrites whatever is stored. That's only there because released clients send it for password changes, and refusing them would leave a blob that only the old password opens. New clients must not rely on it.

## Rate limits

Device sync isn't rate limited. These apply to the anonymous routes that clients use to sign in and register:

- Sign-in, `POST /user/status`, device registration and device cloning are limited per client address.
- Account registration and `GET /api/v1/user/status` have their own per-address limits.
- Wrong passwords are counted per email address across `POST /auth/login`, `POST /user/login`, `POST /user/status` and `POST /auth/register-device`. After five, each attempt waits, doubling from one second up to 15 minutes. A correct password is refused during a wait.

A refused request gets 429 with a `Retry-After` header in seconds, and the body:

```json
{ "error": "Too many attempts. Try again in 30 seconds.", "code": "rate_limited", "retryAfter": 30 }
```

Show the message and don't retry until the wait is over.

## Account states on sign-in

These affect clients that sign in, not device sync.

- `POST /user/login` and `POST /auth/login` return 403 for an account that isn't approved (`Account pending admin approval`) or is deactivated (`Account deactivated`), after checking the password. Show the `error` text. Poll `GET /user/status` for approval rather than retrying sign-in.
- `POST /user/status` with `{ email, password }` returns the same `{ exists, isApproved, isActive }` as the GET, but only for the right password. Anything else gets the 401 that sign-in gives, and counts as a wrong password. Prefer it where the client has the password: the GET tells anyone whether an address has an account, and stays only for released clients. Fall back to the GET on 404 or 405, which mean an older server.
- Both login responses include `mustChangePassword`. When it is true the account has to change its password first: every other account or admin route, and `POST /auth/register-device`, returns 403 with `code: "password_change_required"`. The routes that stay open are `POST /admin/change-password`, `POST /user/change-password`, `POST /user/logout` and `POST /admin/logout`. Existing device keys keep syncing.

## Dashboard sessions

This applies to the admin dashboard and user portal, not to device clients. Anything that doesn't ask for a cookie is unaffected.

`POST /auth/login` and `POST /user/login` accept an optional `cookieSession: true`. On a server with `ENABLE_HSTS=true`, the session then comes back as a cookie, and the response has `cookieSession: true` and no `sessionId`:

```
Set-Cookie: session_token=<token>; HttpOnly; Secure; SameSite=Strict; Max-Age=<session expiry>; Path=/api/v1/admin
```

The path is `/api/v1/admin` from `/auth/login` and `/api/v1/user` from `/user/login`. Any other server ignores the field and returns `sessionId` as usual, because a `Secure` cookie wouldn't come back over plain HTTP.

A `POST`, `PUT`, `PATCH` or `DELETE` that authenticates with the cookie gets 403 unless it has the header `X-Requested-With: JotteryDashboard`. A request with `Authorization: Bearer <token>` doesn't need it.

- `DELETE /user/account` and `DELETE /user/notes` take `{ "currentPassword": "..." }` as a JSON body. A wrong one gets 401 with an `error`, and counts as a wrong password. Without one they only work within five minutes of signing in, which is what a client that asks for the password and signs in first relies on. After that they get 403 with `code: "password_required"`.
- `GET /admin/me` returns `{ id, email, isAdmin }` for the signed-in admin. It's the check that a cookie session still works, because that cookie isn't sent to `/user/account`. During a forced password change it returns 403 with `code: "password_change_required"`.
- `POST /admin/logout` and `POST /user/logout` end the session and clear that route's cookie. Both work with a token too. `/admin/logout` is new, so a dashboard holding a token should keep using `/user/logout`, which every server has.

## Data shapes

### Note

| Field | Type | Required on push | Notes |
|---|---|---|---|
| `id` | string | yes | UUID, chosen by the client. Unique per user |
| `createdAt`, `modifiedAt` | string | yes | |
| `content` | string | yes | Encrypted |
| `tags` | string[] | yes | Encrypted |
| `attachments` | [AttachmentRef](#attachment-reference)[] | yes | |
| `pinned`, `archived`, `deleted` | boolean | yes | |
| `locked` | boolean | no | Defaults to false |
| `archivedAt`, `lockedAt`, `deletedAt` | string or null | no | |
| `version` | integer | yes | |
| `wordWrap`, `showPreview` | boolean or null | no | |
| `syntaxLanguage`, `color` | string or null | no | |
| `contentHash`, `parentHash` | string or null | no | For conflict detection. Computed by the client |
| `hashChain` | string[] or null | no | Ancestor hashes |

### Attachment reference

`id`, `filename` (encrypted), `mimeType`, `size` (bytes), and `data`, which is the same as `id`. All required.

### Version

`versionKey` (`<noteId>:<version>`), `noteId`, `version`, `createdAt`, `syncedAt`, `content`, `tags`, `attachments` and `reason` are required. `syntaxLanguage`, `wordWrap`, `showPreview`, `color`, `contentHash` and `parentHash` are optional. A version is only stored if its note was accepted in the same push.

### Saved search

`id`, `name`, `query`, `order`, `createdAt`, `modifiedAt`, `deleted` and `version` are required, and `deletedAt` is optional. Conflicts are last-write-wins on `modifiedAt`. Ids are unique per user.

## What each client needs to do

To adopt these additions, a client needs to:

1. Pull with the loop under [Moving a client to the cursor](#moving-a-client-to-the-cursor), applying pages idempotently and saving `nextCursor` only after a page is applied.
2. Clear the stored cursor when the account or server changes.
3. Send `attachmentsAuthoritative: true` on pushes, once its attachment lists are known to be complete.
4. Read `rejectedDeletions`, treating its absence as an empty list, and drop those ids from its pending deletions.
5. Treat 403 from a device endpoint as "account disabled", not "device removed".
6. Honour `Retry-After` on 429.
7. Say what it wants on every `PUT /auth/wrapped-key`, as described under [Wrapped master key](#wrapped-master-key), and treat 409 as "read it again".
8. Read `skippedDeleted`, treating its absence as an empty list, and stop sending those notes.
