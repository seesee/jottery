# Vault Health Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface undecryptable notes and missing attachments in a Settings "Vault Health" panel, with permanent delete (server-propagated) and attachment repair.

**Architecture:** A new `vaultHealthService` (Svelte store + scan/repair/delete functions) fed by `noteService` decrypt-failure sites; a `VaultHealthPanel` component in the Settings Advanced tab; one new authenticated server route to fetch a single attachment blob; and a corrected `getKnownAttachmentIds` in `syncService` so future gaps self-heal.

**Tech Stack:** Svelte 4 + TypeScript + vitest (client), Axum + SQLx/SQLite (server, runtime queries to avoid sqlx offline-cache regeneration).

**Spec:** `docs/superpowers/specs/2026-08-01-vault-health-design.md`
**Bead:** jottery-vir1

## Global Constraints

- British English in all user-facing text and comments ("recognised", "colour").
- No native browser dialogs — use `ConfirmModal` / `toast` (`src/lib/utils/toast.svelte.ts`: `toast.info/success/error(message)`).
- All strings via i18n keys; `en-GB` is canonical, every file in `src/locales/` gets the same keys translated.
- Commit after each task; message format `<type>: <description>` with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` footer.
- Server: filter every query by the authenticated user; return 404 (not 403) for other users' resources.

---

### Task 1: vaultHealthService — failure store

**Files:**
- Create: `src/lib/services/vaultHealthService.ts`
- Test: `src/lib/services/vaultHealthService.test.ts`

**Interfaces:**
- Produces: `undecryptableNotes: Writable<UndecryptableNote[]>`, `recordDecryptFailure(note: Note, error: unknown): void`, `resetVaultHealth(): void`, type `UndecryptableNote { id, createdAt, modifiedAt, ciphertextLength, error }`.

- [ ] **Step 1: Write the failing tests**

```typescript
// src/lib/services/vaultHealthService.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { undecryptableNotes, recordDecryptFailure, resetVaultHealth } from './vaultHealthService';
import type { Note } from '../types';

const makeNote = (id: string): Note => ({
  id,
  createdAt: '2026-07-12T21:39:56.350Z',
  modifiedAt: '2026-07-12T21:39:56.350Z',
  content: '{"ciphertext":"abc","iv":"def"}',
  tags: [],
  attachments: [],
  pinned: false,
  deleted: false,
  version: 1,
} as unknown as Note);

describe('vaultHealthService failure store', () => {
  beforeEach(() => resetVaultHealth());

  it('records a decrypt failure with plaintext metadata only', () => {
    recordDecryptFailure(makeNote('n1'), new Error('Decryption failed'));
    const entries = get(undecryptableNotes);
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({
      id: 'n1',
      createdAt: '2026-07-12T21:39:56.350Z',
      ciphertextLength: 31,
      error: 'Decryption failed',
    });
  });

  it('deduplicates by note id', () => {
    recordDecryptFailure(makeNote('n1'), new Error('a'));
    recordDecryptFailure(makeNote('n1'), new Error('b'));
    expect(get(undecryptableNotes)).toHaveLength(1);
  });

  it('stringifies non-Error failures', () => {
    recordDecryptFailure(makeNote('n2'), 'plain string');
    expect(get(undecryptableNotes)[0].error).toBe('plain string');
  });

  it('reset clears all entries', () => {
    recordDecryptFailure(makeNote('n1'), new Error('a'));
    resetVaultHealth();
    expect(get(undecryptableNotes)).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run to verify failure** — `npx vitest run src/lib/services/vaultHealthService.test.ts` → FAIL (module not found).

- [ ] **Step 3: Minimal implementation**

```typescript
// src/lib/services/vaultHealthService.ts
/**
 * Vault health: tracks notes that failed to decrypt and attachments that are
 * referenced by notes but missing from local storage. Only plaintext metadata
 * is ever stored here — never note content.
 */
import { writable, get } from 'svelte/store';
import type { Note } from '../types';

export interface UndecryptableNote {
  id: string;
  createdAt: string;
  modifiedAt: string;
  ciphertextLength: number;
  error: string;
}

export const undecryptableNotes = writable<UndecryptableNote[]>([]);

export function recordDecryptFailure(note: Note, error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  undecryptableNotes.update(entries => {
    if (entries.some(e => e.id === note.id)) return entries;
    return [...entries, {
      id: note.id,
      createdAt: note.createdAt,
      modifiedAt: note.modifiedAt,
      ciphertextLength: note.content?.length ?? 0,
      error: message,
    }];
  });
}

export function resetVaultHealth(): void {
  undecryptableNotes.set([]);
}
```

- [ ] **Step 4: Run to verify pass** — same command → PASS.
- [ ] **Step 5: Commit** — `feat(vault-health): add decrypt-failure store`

---

### Task 2: report decrypt failures from noteService

**Files:**
- Modify: `src/lib/services/noteService.ts` (three `Promise.allSettled` skip sites: `getAllNotes` ~line 112, `getAllNotesBatched` first batch ~line 157, `decryptRemainingBatches` ~line 199)
- Test: `src/lib/services/vaultHealthService.test.ts` (extend) or verify via existing `noteService.test.ts` patterns

**Interfaces:**
- Consumes: `recordDecryptFailure` from Task 1.
- Produces: nothing new — side effect only.

- [ ] **Step 1: Write the failing test** — in `vaultHealthService.test.ts`, add an integration-style test that calls `noteService.getAllNotesBatched` with a mocked repository returning one good and one corrupt note (follow the mocking pattern used in `noteService.test.ts` — `vi.mock` of `./noteRepository` and `./keyManager`), asserting the corrupt note's id lands in `undecryptableNotes`. If the existing mock scaffolding makes this heavier than ~40 lines, instead assert at unit level: export nothing new, and test via `noteService.test.ts`'s existing corrupt-note test (there is one covering skipping) by adding the store assertion to it.

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implementation** — in each of the three skip sites, the `for (const result of results)` loop currently only logs. Change the loops to iterate with an index so the failed `Note` is in scope, e.g. for the first-batch site:

```typescript
const firstDecrypted: DecryptedNote[] = [];
firstResults.forEach((result, i) => {
  if (result.status === 'fulfilled') {
    firstDecrypted.push(result.value);
  } else {
    console.error('[NoteService] Skipping undecryptable note:', result.reason);
    recordDecryptFailure(firstBatch[i], result.reason);
  }
});
```

Apply the same shape to `getAllNotes` (indexing `notes[i]`) and `decryptRemainingBatches` (indexing `batch[i]`). Import `recordDecryptFailure` from `./vaultHealthService`.

- [ ] **Step 4: Run** — `npx vitest run src/lib/services/` → PASS, no regressions.
- [ ] **Step 5: Commit** — `feat(vault-health): report decrypt failures from noteService`

---

### Task 3: missing-attachment scan

**Files:**
- Modify: `src/lib/services/vaultHealthService.ts`
- Test: `src/lib/services/vaultHealthService.test.ts`

**Interfaces:**
- Consumes: `noteRepository.getAllNonDeleted()`, `attachmentRepository.listAllIds()`, `noteService.getNote(id)`, `getNoteTitle` from `../utils/noteTitle`.
- Produces: `scanMissingAttachments(): Promise<MissingAttachment[]>`, type `MissingAttachment { attachmentId, noteId, noteTitle: string | null }`.

- [ ] **Step 1: Write the failing test**

```typescript
// append to vaultHealthService.test.ts — mock the three collaborators
vi.mock('./noteRepository', () => ({
  noteRepository: { getAllNonDeleted: vi.fn() },
}));
vi.mock('./attachmentRepository', () => ({
  attachmentRepository: { listAllIds: vi.fn() },
}));
vi.mock('./noteService', () => ({
  noteService: { getNote: vi.fn() },
}));

describe('scanMissingAttachments', () => {
  it('reports refs with no stored blob, with title when note decrypts', async () => {
    (noteRepository.getAllNonDeleted as Mock).mockResolvedValue([
      { id: 'note1', attachments: [{ id: 'att1' }, { id: 'att2' }] },
      { id: 'note2', attachments: [{ id: 'att3' }] },
    ]);
    (attachmentRepository.listAllIds as Mock).mockResolvedValue(['att1', 'att3']);
    (noteService.getNote as Mock).mockResolvedValue({ content: 'Shopping list\nmilk' });

    const missing = await scanMissingAttachments();
    expect(missing).toEqual([
      { attachmentId: 'att2', noteId: 'note1', noteTitle: 'Shopping list' },
    ]);
  });

  it('uses null title when the owning note cannot be decrypted', async () => {
    (noteRepository.getAllNonDeleted as Mock).mockResolvedValue([
      { id: 'note1', attachments: [{ id: 'attX' }] },
    ]);
    (attachmentRepository.listAllIds as Mock).mockResolvedValue([]);
    (noteService.getNote as Mock).mockRejectedValue(new Error('nope'));

    const missing = await scanMissingAttachments();
    expect(missing[0].noteTitle).toBeNull();
  });
});
```

(Adjust title assertion to whatever `getNoteTitle` returns for that content — check `src/lib/utils/noteTitle.ts` first and use its real behaviour in the expectation.)

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implementation**

```typescript
export interface MissingAttachment {
  attachmentId: string;
  noteId: string;
  noteTitle: string | null;
}

export async function scanMissingAttachments(): Promise<MissingAttachment[]> {
  const [allNotes, blobIds] = await Promise.all([
    noteRepository.getAllNonDeleted(),
    attachmentRepository.listAllIds(),
  ]);
  const stored = new Set(blobIds);
  const missing: MissingAttachment[] = [];
  const titleCache = new Map<string, string | null>();

  for (const note of allNotes) {
    for (const ref of note.attachments ?? []) {
      if (stored.has(ref.id)) continue;
      if (!titleCache.has(note.id)) {
        try {
          const decrypted = await noteService.getNote(note.id);
          titleCache.set(note.id, decrypted ? getNoteTitle(decrypted.content) : null);
        } catch {
          titleCache.set(note.id, null); // note undecryptable — scan continues
        }
      }
      missing.push({ attachmentId: ref.id, noteId: note.id, noteTitle: titleCache.get(note.id) ?? null });
    }
  }
  return missing;
}
```

(Beware import cycles: `noteService` will import `vaultHealthService` after Task 2. Import `noteService` lazily inside the function — `const { noteService } = await import('./noteService');` — if vitest reports a circular-import failure.)

- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `feat(vault-health): scan for attachments missing locally`

---

### Task 4: server endpoint — fetch single attachment

**Files:**
- Modify: `server/src/api/sync.rs` (new handler), `server/src/main.rs:117-122` (route)
- Test: `server/tests/attachment_and_version_tests.rs` (append tests, reuse `create_test_app` helper)

**Interfaces:**
- Produces: `GET /api/v1/sync/attachments/:id` → 200 `{"id": "...", "data": "<base64>"}` | 404.

- [ ] **Step 1: Write the failing tests** — append to `attachment_and_version_tests.rs`, following its existing request-building pattern (`Request::builder()` + `app.oneshot()`), covering: (a) owner fetches attachment → 200 with correct base64 data; (b) another user's API key → 404; (c) unknown id → 404. Register the new route inside the test router the same way existing sync routes are registered there.

- [ ] **Step 2: Run** — `cd server && cargo test --test attachment_and_version_tests` → FAIL (handler missing).

- [ ] **Step 3: Implementation** — in `server/src/api/sync.rs` (use the runtime query API, not `sqlx::query!`, so the SQLX_OFFLINE cache needs no regeneration):

```rust
/// Fetch a single attachment blob the client is missing locally.
/// Ownership is enforced via attachments_meta.note_user_id; unknown or
/// foreign attachments return 404 so existence is not leaked.
pub async fn get_attachment(
    State(state): State<Arc<AppState>>,
    AuthClient(client_info): AuthClient,
    Path(attachment_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let row: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT d.data FROM attachments_data d
         JOIN attachments_meta m ON m.id = d.id
         WHERE d.id = ? AND m.note_user_id = ?",
    )
    .bind(&attachment_id)
    .bind(&client_info.user_id)
    .fetch_optional(&state.pool)
    .await?;

    match row {
        Some((data,)) => {
            tracing::info!(
                "Serving attachment {} to user {} (client {})",
                attachment_id, client_info.user_id, client_info.client_id
            );
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            Ok(Json(serde_json::json!({ "id": attachment_id, "data": encoded })))
        }
        None => Err(AppError::NotFound("Attachment not found".to_string())),
    }
}
```

Check `AppError` variants first (`server/src/error.rs` or similar) — use the existing not-found variant name. Register in `main.rs`:

```rust
.route("/api/v1/sync/attachments/:id", get(api::sync::get_attachment))
```

- [ ] **Step 4: Run** — `cargo test --test attachment_and_version_tests` → PASS; `cargo check` clean.
- [ ] **Step 5: Commit** — `feat(server): endpoint to fetch a single attachment blob`

---

### Task 5: client repair + syncClient.fetchAttachment

**Files:**
- Modify: `src/lib/services/syncClient.ts`, `src/lib/services/vaultHealthService.ts`
- Test: `src/lib/services/vaultHealthService.test.ts`

**Interfaces:**
- Consumes: Task 4's endpoint; `syncRepository.getMetadata()`, `keyManager.getMasterKey()`, `cryptoService.decryptText`, `attachmentRepository.storeBlob`, `base64ToArrayBuffer`.
- Produces: `fetchAttachment(endpoint, apiKey, id): Promise<{id: string, data: string}>` in syncClient; `repairAttachment(attachmentId): Promise<boolean>` in vaultHealthService.

- [ ] **Step 1: Write the failing test** — mock `./syncClient`'s `fetchAttachment`, `./syncRepository`, `./keyManager`, `./crypto`, `./attachmentRepository`; assert `repairAttachment('att2')` decrypts the API key, calls `fetchAttachment` with endpoint/key/id, stores the decoded blob under `att2`, and returns `true`; assert it returns `false` (not throws) when `fetchAttachment` rejects.

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implementation** — `fetchAttachment` in syncClient mirrors `getServerStatus` exactly (GET, Bearer header, `fetchWithTimeout`, throw `SyncApiError` on !ok) with URL `${endpoint}/api/${API_VERSION}/sync/attachments/${id}`. In vaultHealthService:

```typescript
export async function repairAttachment(attachmentId: string): Promise<boolean> {
  try {
    const metadata = await syncRepository.getMetadata();
    if (!metadata?.syncEnabled || !metadata.apiKey || !metadata.syncEndpoint) return false;
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) return false;
    const apiKey = await cryptoService.decryptText(JSON.parse(metadata.apiKey), masterKey.key);
    const result = await fetchAttachment(metadata.syncEndpoint, apiKey, attachmentId);
    await attachmentRepository.storeBlob(result.id, base64ToArrayBuffer(result.data));
    return true;
  } catch (error) {
    console.error(`[VaultHealth] Failed to repair attachment ${attachmentId}:`, error);
    return false;
  }
}

export async function deleteUndecryptableNote(id: string): Promise<void> {
  const { noteService } = await import('./noteService');
  await noteService.permanentlyDeleteNote(id); // local delete + deletions tombstone → sync push
  undecryptableNotes.update(entries => entries.filter(e => e.id !== id));
}
```

Add a delete test too: mock `./noteService`, assert `permanentlyDeleteNote` called and entry pruned; assert the entry is NOT pruned if `permanentlyDeleteNote` rejects.

- [ ] **Step 4: Run to verify pass.**
- [ ] **Step 5: Commit** — `feat(vault-health): repair missing attachments and delete undecryptable notes`

---

### Task 6: fix getKnownAttachmentIds (blob-based, Set)

**Files:**
- Modify: `src/lib/services/syncService.ts:687-698` (`getKnownAttachmentIds`) and its call site in `pull()` (~line 944)
- Test: `src/lib/services/syncService.test.ts`

**Interfaces:**
- Produces: `getKnownAttachmentIds(): Promise<string[]>` (no longer takes `allNotes`; sourced from `attachmentRepository.listAllIds()`).

- [ ] **Step 1: Write the failing test** — in `syncService.test.ts` (follow its existing mock setup), assert that the pull request body's `knownAttachmentIds` equals what `attachmentRepository.listAllIds()` returned, NOT the ids referenced by notes. If the existing test scaffolding doesn't reach `pull()` easily, test the helper directly by exporting it — but prefer the observable behaviour.

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implementation** — replace the method:

```typescript
/** Attachment blobs actually present locally — the server re-sends any
 *  attachment of a pulled note that is not in this list. Sourcing this from
 *  stored blobs (not note references) lets locally-missing attachments
 *  self-heal when their note is next pulled. */
private async getKnownAttachmentIds(): Promise<string[]> {
  return attachmentRepository.listAllIds();
}
```

In `pull()`, compute once before the pagination loop (`const knownAttachmentIds = await this.getKnownAttachmentIds();`) and use it in each page's request. Remove the now-unused `allNotes` parameter plumbing if `allNotes` has no other use in the request (it is still used for `knownNoteIds` — keep that).

- [ ] **Step 4: Run** — `npx vitest run src/lib/services/syncService.test.ts` → PASS.
- [ ] **Step 5: Commit** — `fix(sync): report stored attachment blobs as known IDs so gaps self-heal`

---

### Task 7: load-time integration — reset + toast + i18n keys

**Files:**
- Modify: `src/App.svelte` (`loadNotes` and the lock reactive at ~line 385), `src/locales/*.json` (all)
- Test: manual (reactive wiring); i18n key presence via `npx vitest run` remaining green + svelte-check

**Interfaces:**
- Consumes: `resetVaultHealth`, `undecryptableNotes` (Task 1), `toast`.

- [ ] **Step 1: Wire App.svelte** — import `{ resetVaultHealth, undecryptableNotes }` and `get` from svelte/store. In `loadNotes`, call `resetVaultHealth()` right after the `loadingNotes = true;` line. In `finishLoading`, after clearing `loadingProgress`, add:

```typescript
const failed = get(undecryptableNotes).length;
if (failed > 0) {
  toast.info($_('vaultHealth.loadToast', { values: { count: failed } }));
}
```

In the lock reactive (`$: if ($isLocked && initialized) { ... }`) add `resetVaultHealth();`.

- [ ] **Step 2: Add i18n keys** — new top-level `vaultHealth` section in `src/locales/en-GB.json`:

```json
"vaultHealth": {
  "title": "Vault Health",
  "description": "Detects notes that cannot be decrypted and attachments that are missing from this device.",
  "healthy": "No problems found.",
  "loadToast": "{count} note(s) could not be decrypted — review in Settings → Advanced",
  "undecryptableTitle": "Undecryptable notes",
  "undecryptableHint": "These notes cannot be read with your current password. This usually means they were created with a different password. Deleting is permanent and removes them from the sync server and all devices.",
  "createdAt": "Created",
  "modifiedAt": "Modified",
  "size": "Size",
  "errorLabel": "Error",
  "delete": "Delete permanently",
  "deleteConfirmTitle": "Delete undecryptable note?",
  "deleteConfirmMessage": "This note cannot be decrypted and its content cannot be recovered. Deleting removes it from this device, the sync server, and all your other devices. This cannot be undone.",
  "deleteFailed": "Failed to delete note",
  "missingTitle": "Missing attachments",
  "missingHint": "These attachments are referenced by notes but not stored on this device.",
  "noteLabel": "Note",
  "unknownNote": "(note cannot be decrypted)",
  "repair": "Repair from server",
  "repairSuccess": "Attachment restored",
  "repairFailed": "Repair failed — check sync connection and server version",
  "rescan": "Rescan",
  "scanning": "Scanning…"
}
```

Add the same section, translated, to every other file in `src/locales/` (enumerate with `ls src/locales/`; translate each string into that file's language, matching the file's existing tone and formality).

- [ ] **Step 3: Verify** — `npx svelte-check --tsconfig ./tsconfig.json` → 0 errors; `node -e` JSON-parse every locale file to catch syntax slips:
`for f in src/locales/*.json; do node -e "JSON.parse(require('fs').readFileSync('$f'))" || echo "BAD $f"; done`
- [ ] **Step 4: Commit** — `feat(vault-health): load-time toast, store reset, i18n strings`

---

### Task 8: VaultHealthPanel component + Advanced tab

**Files:**
- Create: `src/lib/components/settings/VaultHealthPanel.svelte`
- Modify: `src/lib/components/settings/AdvancedTab.svelte` (render panel), `src/lib/components/settings/index.ts` if it barrels exports
- Test: manual + svelte-check (component is thin over the tested service)

**Interfaces:**
- Consumes: everything from `vaultHealthService`; `ConfirmModal` (`show`, `title`, `message`, `confirmText`, `onConfirm`, `onCancel` props); `toast`.

- [ ] **Step 1: Component** — follow AdvancedTab's section markup (`<h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">` headers, `space-y-3` bodies):

```svelte
<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { undecryptableNotes, scanMissingAttachments, repairAttachment, deleteUndecryptableNote, type MissingAttachment } from '../../services/vaultHealthService';
  import ConfirmModal from '../ConfirmModal.svelte';
  import { toast } from '../../utils/toast.svelte';

  let missing: MissingAttachment[] = [];
  let scanning = false;
  let repairing: Record<string, boolean> = {};
  let repairResult: Record<string, 'ok' | 'fail'> = {};
  let confirmDeleteId: string | null = null;

  async function rescan() {
    scanning = true;
    try { missing = await scanMissingAttachments(); }
    finally { scanning = false; }
  }

  async function repair(id: string) {
    repairing = { ...repairing, [id]: true };
    const ok = await repairAttachment(id);
    repairing = { ...repairing, [id]: false };
    repairResult = { ...repairResult, [id]: ok ? 'ok' : 'fail' };
    if (ok) missing = missing.filter(m => m.attachmentId !== id);
  }

  async function confirmDelete() {
    if (!confirmDeleteId) return;
    try {
      await deleteUndecryptableNote(confirmDeleteId);
    } catch (error) {
      console.error('[VaultHealth] Delete failed:', error);
      toast.error($_('vaultHealth.deleteFailed'));
    }
    confirmDeleteId = null;
  }

  rescan();
</script>
```

Markup: `vaultHealth.title` header + `description`; if both lists are empty and not scanning, show `healthy`; undecryptable list rows show formatted `createdAt`/`modifiedAt` (use the app's existing date formatting util if one exists — check `src/lib/utils` for a `formatDate`; otherwise `new Date(x).toLocaleString()`), `ciphertextLength` in bytes, `error`, and a red Delete button setting `confirmDeleteId`; missing list rows show `noteTitle ?? $_('vaultHealth.unknownNote')` + short attachment id (`attachmentId.slice(0, 8)`) + Repair button (disabled while `repairing[id]`, inline `repairSuccess`/`repairFailed` text after); `rescan` button. One `ConfirmModal` bound to `confirmDeleteId !== null` with the `deleteConfirm*` strings.

- [ ] **Step 2: Mount in AdvancedTab** — import and render `<VaultHealthPanel />` as a new bordered section (copy an existing section wrapper) after the existing sections.

- [ ] **Step 3: Verify** — `npx svelte-check` → 0 errors; `npx vitest run` green.
- [ ] **Step 4: Commit** — `feat(vault-health): settings panel for undecryptable notes and missing attachments`

---

### Task 9: end-to-end verification

**Files:** none (verification only)

- [ ] **Step 1: Full client suite** — `npx vitest run` and `npx svelte-check` → green.
- [ ] **Step 2: Server suite** — `cd server && cargo test` → green.
- [ ] **Step 3: Manual E2E (scripted via Playwright against `npm run dev`)** —
  1. Create a vault at `/test`, seed ~300 notes.
  2. Inject a wrong-key note directly into IndexedDB (copy a real note record, replace `content` with ciphertext produced under a different key — encrypt any string with a throwaway `AES-GCM` key via `crypto.subtle` in the page, matching the `{ciphertext, iv}` JSON shape).
  3. Add a dangling attachment ref to one note (attachments array entry whose `id` has no blob).
  4. Reload + unlock. Expect: toast "1 note(s) could not be decrypted…", Settings → Advanced shows the note (with dates/size/error) and the missing attachment.
  5. Delete the note via the panel; verify it leaves the store, a tombstone row exists in the `deletions` IndexedDB store, and note count drops.
  6. Repair against the dev server is expected to fail gracefully (no real sync configured) — assert the inline failure message, not a crash.
- [ ] **Step 4: E2E regression** — `npx playwright test e2e/smoke.spec.ts e2e/settings.spec.ts e2e/notes.spec.ts --project=chromium` → green.
- [ ] **Step 5: Close out** — `bd close jottery-vir1`, final commit if fixes were needed, push branch, open PR referencing the spec.
