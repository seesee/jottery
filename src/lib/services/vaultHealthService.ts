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
