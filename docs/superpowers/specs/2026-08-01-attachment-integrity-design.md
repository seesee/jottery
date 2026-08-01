# Estate-wide attachment integrity — design

**Date:** 2026-08-01
**Status:** Approved (Chris, 2026-08-01)
**Tracking:** umbrella bead + per-surface children (created on implementation branches)

## Problem

Every client can push a note whose attachment references have no binary data
behind them, and the server stores those references without complaint:

| Client  | Push-time behaviour when a blob cannot be read            |
|---------|-----------------------------------------------------------|
| Web     | silently omits it (`if (blob)`)                           |
| iOS     | silently omits it (`try? attachmentRepo.getBlob`)         |
| Android | silently omits it (`?: continue`)                         |
| TUI     | aborts the entire push (`.context("not found")`)          |

The result is a dangling `attachments_meta` row with no `attachments_data`,
which no client can ever repair by pulling. Confirmed instance: two ~2.7MB
JPEG references pushed by the iOS client on 2026-02-15 (likely rejected blob
upload under the then-5MB payload limit); a server-wide audit on 2026-08-01
found no other cases. The TUI's abort is the opposite failure: one missing
blob permanently bricks its sync.

## Goals

- The server tells clients when a pushed note references attachment data it
  does not hold ("accept + warn" — never reject, never mutate user data).
- Every client reacts: re-push the blob if it holds it (estate-wide
  self-heal — any device with the blob repairs the server), otherwise log
  loudly; on web, also surface in Vault Health.
- No client silently omits a blob at push time any more; the TUI stops
  aborting its whole push over one missing blob.

## Non-goals

- Rejecting or stripping notes server-side (breaks or mutates old clients).
- Native vault-health UIs on iOS/Android/TUI (web remains the remediation
  surface; natives auto-heal + log only).
- A server-side backfill sweep for pre-existing dangling refs: the guard
  fires when an affected note is next pushed, the only two known cases are
  already surfaced by web Vault Health, and the audit found nothing else.

## Design

### 1. Sync protocol (server)

`POST /api/v1/sync/push` response gains an optional field:

```json
"attachmentWarnings": [
  { "noteId": "…", "attachmentIds": ["…", "…"] }
]
```

After the handler stores notes and attachment blobs, one query finds refs of
just-accepted notes with no `attachments_data` row (scoped to the
authenticated user). Field omitted when empty. A `tracing::warn!` logs each
warning server-side. All four clients' JSON decoders ignore unknown fields,
so deployed clients are unaffected.

An **attachments-only push** (empty `notes`, non-empty `attachments`) is the
repair vehicle. The handler already upserts `attachments_data` independently
of notes; the design relies on this and adds a regression test so it stays
true. Cross-user safety: the data upsert must not attach blobs to another
user's metadata — the handler verifies the attachment id belongs to the
authenticated user (via `attachments_meta.note_user_id`) before storing, and
a test covers it.

### 2. Client reaction (shared semantics, four implementations)

On every push response:

1. For each warned attachment id, check the local blob store.
2. If held: collect into a single follow-up attachments-only push (fired
   immediately, once, not retried in a loop — the next regular sync retries
   naturally).
3. If not held: log a warning naming note id and attachment id. Web
   additionally records it in Vault Health's missing list with the existing
   "not on the server" message.

Push-time behaviour change in the same commit per client:

- **Web** (`collectAttachmentsForBatch`), **iOS** (`SyncService.push`),
  **Android** (`SyncService` push): when a referenced blob cannot be read,
  log an explicit warning (id + note) instead of skipping silently. The push
  still proceeds — the server guard now provides the integrity signal.
- **TUI** (`ui/operations/sync.rs`): replace the whole-push abort with
  skip-plus-warning (stderr/log line), matching the other clients.

### 3. Per-surface delivery

Umbrella bead with four children, each its own branch/PR so a native build
issue cannot block the server guard:

1. **server + web** — protocol change, guard, web reaction (reference
   implementation), Vault Health integration.
2. **TUI** — abort→skip softening + warning reaction.
3. **iOS** — silent-skip logging + warning reaction.
4. **Android** — silent-skip logging + warning reaction.

Native clients tolerate servers without the field (optional decode with
default empty), so PR order is unconstrained; the server PR simply activates
the behaviour.

## Error handling

- Follow-up repair push failures are logged and not retried within the same
  sync run; `needsSync`-driven regular syncs provide natural retry.
- Warnings for attachments the client cannot heal must not fail the sync:
  the push is still marked successful.
- The server guard query failing must not fail the push (log and omit the
  field).

## Testing

- **Server (cargo)**: warning emitted when payload omits a referenced blob;
  no warning when data already exists server-side; attachments-only push
  stores data and clears the warning on the next push; cross-user: pushing
  data for another user's attachment id does not attach it.
- **Web (vitest + scripted E2E)**: reaction unit tests (held → repair push
  fired once with exactly the held blobs; not held → vault health entry, no
  repair push); push-time skip now logs; E2E against a local server build if
  practical, else MSW-simulated warnings.
- **TUI (cargo)**: push with a missing blob completes and warns instead of
  aborting.
- **iOS (xcodebuild test, JotteryTests)**: response decoding with and
  without the field; reaction logic (held/not-held paths).
- **Android (gradle test)**: same coverage as iOS.
