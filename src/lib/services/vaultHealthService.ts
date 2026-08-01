/**
 * Vault health: tracks notes that failed to decrypt and attachments that are
 * referenced by notes but missing from local storage. Only plaintext metadata
 * is ever stored here — never note content.
 */
import { writable } from 'svelte/store';
import type { Note } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { syncRepository } from './syncRepository';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';
import { fetchAttachment, SyncApiError } from './syncClient';
import { base64ToArrayBuffer } from '../utils/base64';
import { getNoteTitle } from '../utils/noteTitle';

export interface UndecryptableNote {
  id: string;
  createdAt: string;
  modifiedAt: string;
  ciphertextLength: number;
  error: string;
}

export interface MissingAttachment {
  attachmentId: string;
  noteId: string;
  noteTitle: string | null;
}

export const undecryptableNotes = writable<UndecryptableNote[]>([]);

/**
 * Attachments the sync server reported it has no data for, and which this
 * device cannot supply either. Grouped per note, deduplicated by attachment
 * id. Populated from push-response attachmentWarnings.
 */
export interface ServerMissingAttachments {
  noteId: string;
  attachmentIds: string[];
}

export const serverMissingAttachments = writable<ServerMissingAttachments[]>([]);

export function reportServerMissingAttachments(warnings: ServerMissingAttachments[]): void {
  serverMissingAttachments.update(existing => {
    const known = new Set(existing.flatMap(w => w.attachmentIds));
    const merged = [...existing];
    for (const warning of warnings) {
      const fresh = warning.attachmentIds.filter(id => !known.has(id));
      if (fresh.length === 0) continue;
      fresh.forEach(id => known.add(id));
      const entry = merged.find(w => w.noteId === warning.noteId);
      if (entry) {
        entry.attachmentIds = [...entry.attachmentIds, ...fresh];
      } else {
        merged.push({ noteId: warning.noteId, attachmentIds: fresh });
      }
    }
    return merged;
  });
}

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
  serverMissingAttachments.set([]);
}

/**
 * Find attachments referenced by notes but absent from local blob storage.
 * The owning note's title is included where the note decrypts; a note that
 * cannot be decrypted yields a null title rather than failing the scan.
 */
export async function scanMissingAttachments(): Promise<MissingAttachment[]> {
  // Imported lazily: noteService imports this module for failure reporting,
  // so a static import here would create a cycle
  const { noteService } = await import('./noteService');

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
          titleCache.set(
            note.id,
            decrypted ? getNoteTitle({ content: decrypted.content, tags: decrypted.tags }) : null
          );
        } catch {
          titleCache.set(note.id, null);
        }
      }
      missing.push({
        attachmentId: ref.id,
        noteId: note.id,
        noteTitle: titleCache.get(note.id) ?? null,
      });
    }
  }
  return missing;
}

export type RepairResult = 'repaired' | 'not-on-server' | 'failed';

/**
 * Re-fetch a missing attachment blob from the sync server and store it
 * locally. Never throws — callers surface the result inline:
 * - 'repaired': blob fetched and stored
 * - 'not-on-server': the server has no data for this attachment either
 *   (e.g. a client pushed the reference but the upload never completed);
 *   the remedy is to open the note and remove the dead reference
 * - 'failed': sync unconfigured, app locked, or a transport error
 */
export async function repairAttachment(attachmentId: string): Promise<RepairResult> {
  try {
    const metadata = await syncRepository.getMetadata();
    if (!metadata?.syncEnabled || !metadata.apiKey || !metadata.syncEndpoint) return 'failed';

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) return 'failed';

    const apiKey = await cryptoService.decryptText(JSON.parse(metadata.apiKey), masterKey.key);
    const result = await fetchAttachment(metadata.syncEndpoint, apiKey, attachmentId);
    await attachmentRepository.storeBlob(result.id, base64ToArrayBuffer(result.data));
    return 'repaired';
  } catch (error) {
    console.error(`[VaultHealth] Failed to repair attachment ${attachmentId}:`, error);
    if (error instanceof SyncApiError && error.status === 404) return 'not-on-server';
    return 'failed';
  }
}

/**
 * Permanently delete an undecryptable note. Uses the standard permanent-delete
 * path, which also writes a tombstone to the deletions store so the removal
 * propagates to the sync server and all other devices on the next push.
 * The store entry is only pruned once deletion succeeds.
 */
export async function deleteUndecryptableNote(id: string): Promise<void> {
  // Imported lazily to avoid a static import cycle with noteService
  const { noteService } = await import('./noteService');
  await noteService.permanentlyDeleteNote(id);
  undecryptableNotes.update(entries => entries.filter(e => e.id !== id));
}
