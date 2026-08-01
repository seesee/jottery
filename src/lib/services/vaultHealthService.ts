/**
 * Vault health: tracks notes that failed to decrypt and attachments that are
 * referenced by notes but missing from local storage. Only plaintext metadata
 * is ever stored here — never note content.
 */
import { writable } from 'svelte/store';
import type { Note } from '../types';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
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
