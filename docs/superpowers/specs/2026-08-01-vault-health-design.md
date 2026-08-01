# Vault Health — design

**Date:** 2026-08-01
**Status:** Approved (Chris, 2026-08-01)
**Tracking:** beads issue to be created on implementation branch

## Problem

Notes that cannot be decrypted (wrong key or corrupted ciphertext) are silently
skipped during loading. They remain in the local database and on the sync
server forever: invisible in the UI, impossible to delete, and inflating the
active-note count. Separately, attachments referenced by notes can be missing
locally with no way to recover them — the client reports attachment IDs it
*references* (rather than blobs it *has*) as "known" during pull, so the server
never re-sends them.

Real-world instance (Chris's vault, 2026-08-01): one undecryptable note
(`a47650b1-07a5-4742-8559-d66a95d5178e`, created 2026-07-12 by a test device
with a different vault password; ciphertext byte-identical on client and
server, so unrecoverable) and two missing attachments
(`196cd697-611e-4d68-a8f3-88e27f021c85`, `a426403c-cf65-4fb9-a3cd-dbb762701c7f`).

## Goals

- Surface undecryptable notes and missing attachments to the user.
- Allow permanent deletion of undecryptable notes, propagated to the sync
  server and all other devices.
- Allow repair of missing attachments by re-fetching them from the server.
- Fix the known-attachment-IDs bug so future gaps self-heal where possible.

## Non-goals

- Retry-from-server for undecryptable notes (explicitly deselected; the
  observed failure mode is wrong-key, where the server copy is identical).
- Routing undecryptable notes through the recycle bin (it decrypts notes for
  display, so they would be stranded there).
- Any automatic deletion. Every destructive action is user-initiated.

## Design

### 1. Detection — `vaultHealthService` (new, client)

`src/lib/services/vaultHealthService.ts` exposes:

- `undecryptableNotes` — Svelte store of
  `{ id, createdAt, modifiedAt, ciphertextLength, error }`, deduplicated by id.
- `recordDecryptFailure(note: Note, error: unknown)` — called by
  `noteService` wherever a decrypt failure is currently caught and skipped
  (first batch and background batches of `getAllNotesBatched`, and
  `getAllNotes`). Only plaintext metadata is stored.
- `reset()` — clears the store. Called on lock and at the start of each
  `loadNotes` run so stale entries never survive a relock or reload.
- `scanMissingAttachments(): Promise<MissingAttachment[]>` — compares every
  attachment reference across all notes with
  `attachmentRepository.listAllIds()`; returns
  `{ attachmentId, noteId, noteTitle? }` (title only when the owning note
  decrypts). Cost on a 3,274-note vault: ~100ms; run on demand, not at load.
- `repairAttachment(attachmentId): Promise<boolean>` — fetches the blob from
  the new server endpoint and stores it via `attachmentRepository.storeBlob`.
- `deleteUndecryptableNote(id): Promise<void>` — delegates to
  `noteService.permanentlyDeleteNote(id)` (existing path: local delete +
  tombstone in the `deletions` store → sync push → server `note_deletions` →
  other devices apply on pull), then removes the entry from the store.

### 2. UI — Settings → Advanced → "Vault Health"

New `VaultHealthPanel.svelte`, rendered in the Advanced tab of SettingsModal.

- **Undecryptable notes list**: creation/modification dates, ciphertext size,
  error message; per-row Delete button. Confirmation uses the existing
  `ConfirmModal` (project rule: no native dialogs) and states that deletion is
  permanent and propagates to the server and all devices.
- **Missing attachments list**: attachment ID (short form), owning note title
  where available; per-row Repair button with success/failure feedback shown
  inline (no toasts for individual repairs).
- **Rescan** button re-runs the missing-attachment scan (the undecryptable
  list refreshes on each unlock automatically).
- Healthy state: "No problems found".

**Load-time toast**: when `loadNotes` completes with one or more recorded
decrypt failures, show a single informational toast: "N note(s) could not be
decrypted — review in Settings → Advanced". Uses the existing toast service.

All strings via i18n keys (`vaultHealth.*`), British English in `en-GB`,
translated equivalents added to the other locale files.

### 3. Server — attachment fetch endpoint

`GET /api/v1/sync/attachments/:id` in `server/src/api/sync.rs`, registered
under the existing API-key middleware (both router setups if duplicated across
`main.rs`/`lib.rs`).

- Verifies ownership: attachment's `attachments_meta.note_user_id` must match
  the authenticated user; 404 otherwise (no existence leak).
- Returns `{ "id": "...", "data": "<base64>" }` from `attachments_data`.
- New `sqlx::query!` calls require regenerating the offline query cache
  (`cargo sqlx prepare`) so `SQLX_OFFLINE=true` CI builds keep working.

### 4. Root-cause fix — known attachment IDs

`syncService.getKnownAttachmentIds` changes from "IDs referenced by notes"
(O(n²) `Array.includes` accumulation) to "blob IDs actually stored locally"
(`attachmentRepository.listAllIds()`, Set semantics). Computed once per pull,
not per page. Effect: when a note whose attachment is missing locally is next
pulled (any edit on any device), the server re-sends the attachment
automatically; the Repair button covers attachments whose notes never change.

## Error handling

- Repair failures (network, 404, quota) render inline per row; the row stays
  listed so repair can be retried.
- Delete failures surface via the existing error toast; the entry is only
  removed from the store after `permanentlyDeleteNote` resolves.
- `scanMissingAttachments` treats a note whose title cannot be decrypted as
  title-less rather than failing the scan.

## Testing

- **Unit (vitest)**: vaultHealthService — failure recording/dedup/reset;
  missing-attachment scan against a mocked repository; delete flow calls
  `permanentlyDeleteNote` and prunes the store. syncService — known-IDs now
  sourced from stored blobs.
- **Server (cargo test)**: endpoint returns data for the owner, 404 for other
  users' attachments and unknown IDs, 401 unauthenticated (following existing
  test patterns in `server/src`).
- **Manual E2E**: seed a vault, inject a wrong-key note and a dangling
  attachment reference, verify: toast on unlock, both lists populate, delete
  removes the note locally and writes a tombstone, repair restores the blob.

## Deployment note

The repair endpoint ships with the server, so the feature is fully functional
only after the next release tag + container redeploy. The client degrades
gracefully against an old server (repair reports failure; everything else
works).
