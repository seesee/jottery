/**
 * Bulk operations service for multi-note actions
 */

import type { DecryptedNote, ExportNote, ExportData } from '../types';
import { noteRepository } from './noteRepository';
import { noteService } from './noteService';
import { notes, clearMultiSelection, selectNote } from '../stores/appStore';
import { searchService } from './searchService';
import { get } from 'svelte/store';

export interface BulkProgress {
  current: number;
  total: number;
  status: 'running' | 'complete' | 'error';
  message?: string;
}

export type ProgressCallback = (progress: BulkProgress) => void;

/**
 * Add tags to multiple notes
 */
export async function addTagsToNotes(
  noteIds: string[],
  tagsToAdd: string[],
  onProgress?: ProgressCallback
): Promise<void> {
  const allNotes = get(notes);
  const total = noteIds.length;

  for (let i = 0; i < noteIds.length; i++) {
    const noteId = noteIds[i];
    const note = allNotes.find((n) => n.id === noteId);

    if (note) {
      // Add tags that aren't already present
      const newTags = [...new Set([...note.tags, ...tagsToAdd])];

      if (newTags.length !== note.tags.length) {
        // Update note with new tags (properly encrypts via noteService)
        await noteService.updateNote(noteId, { tags: newTags });

        // Update store with decrypted version
        const updated: DecryptedNote = {
          ...note,
          tags: newTags,
          modifiedAt: new Date().toISOString(),
        };
        notes.update((n) =>
          n.map((existingNote) => (existingNote.id === noteId ? updated : existingNote))
        );

        // Update search index
        searchService.updateNote(updated);
      }
    }

    onProgress?.({
      current: i + 1,
      total,
      status: 'running',
    });
  }

  onProgress?.({
    current: total,
    total,
    status: 'complete',
  });
}

/**
 * Remove tags from multiple notes
 */
export async function removeTagsFromNotes(
  noteIds: string[],
  tagsToRemove: string[],
  onProgress?: ProgressCallback
): Promise<void> {
  const allNotes = get(notes);
  const total = noteIds.length;
  const tagsLower = tagsToRemove.map((t) => t.toLowerCase());

  for (let i = 0; i < noteIds.length; i++) {
    const noteId = noteIds[i];
    const note = allNotes.find((n) => n.id === noteId);

    if (note) {
      // Remove matching tags (case-insensitive)
      const newTags = note.tags.filter(
        (tag) => !tagsLower.includes(tag.toLowerCase())
      );

      if (newTags.length !== note.tags.length) {
        // Update note with new tags (properly encrypts via noteService)
        await noteService.updateNote(noteId, { tags: newTags });

        // Update store with decrypted version
        const updated: DecryptedNote = {
          ...note,
          tags: newTags,
          modifiedAt: new Date().toISOString(),
        };
        notes.update((n) =>
          n.map((existingNote) => (existingNote.id === noteId ? updated : existingNote))
        );

        // Update search index
        searchService.updateNote(updated);
      }
    }

    onProgress?.({
      current: i + 1,
      total,
      status: 'running',
    });
  }

  onProgress?.({
    current: total,
    total,
    status: 'complete',
  });
}

/**
 * Delete multiple notes (soft delete - move to recycle bin)
 */
export async function deleteNotes(
  noteIds: string[],
  onProgress?: ProgressCallback
): Promise<void> {
  const total = noteIds.length;

  for (let i = 0; i < noteIds.length; i++) {
    const noteId = noteIds[i];

    await noteRepository.softDelete(noteId);

    // Update store
    notes.update((n) => n.filter((note) => note.id !== noteId));

    onProgress?.({
      current: i + 1,
      total,
      status: 'running',
    });
  }

  // Clear multi-selection after delete
  clearMultiSelection();

  onProgress?.({
    current: total,
    total,
    status: 'complete',
  });
}

/**
 * Export multiple notes as JSON
 */
export async function exportNotes(
  noteIds: string[],
  onProgress?: ProgressCallback
): Promise<string> {
  const allNotes = get(notes);
  const total = noteIds.length;

  const exportNotes: ExportNote[] = [];

  for (let i = 0; i < noteIds.length; i++) {
    const noteId = noteIds[i];
    const note = allNotes.find((n) => n.id === noteId);

    if (note) {
      exportNotes.push({
        id: note.id,
        createdAt: note.createdAt,
        modifiedAt: note.modifiedAt,
        content: note.content,
        tags: note.tags,
        attachments: note.attachments.map((a) => ({
          filename: a.filename,
          mimeType: a.mimeType,
          data: a.data,
        })),
        pinned: note.pinned,
        wordWrap: note.wordWrap,
        syntaxLanguage: note.syntaxLanguage,
        showPreview: note.showPreview,
        color: note.color,
      });
    }

    onProgress?.({
      current: i + 1,
      total,
      status: 'running',
    });
  }

  const exportData: ExportData = {
    version: '1.0',
    exportDate: new Date().toISOString(),
    notes: exportNotes,
  };

  onProgress?.({
    current: total,
    total,
    status: 'complete',
  });

  return JSON.stringify(exportData, null, 2);
}

/**
 * Download exported data as a file
 */
export function downloadExport(jsonData: string, filename: string = 'jottery-export.json'): void {
  const blob = new Blob([jsonData], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export interface CombineNotesOptions {
  mergeTags?: boolean; // If true, merge tags from all notes; if false, only keep tags from first note
}

/**
 * Set color for multiple notes
 */
export async function setColorForNotes(
  noteIds: string[],
  color: string | undefined,
  onProgress?: ProgressCallback
): Promise<void> {
  const allNotes = get(notes);
  const total = noteIds.length;

  for (let i = 0; i < noteIds.length; i++) {
    const noteId = noteIds[i];
    const note = allNotes.find((n) => n.id === noteId);

    if (note) {
      // Update note with new color (properly persists via noteService)
      await noteService.updateNote(noteId, { color });

      // Update store with decrypted version
      const updated: DecryptedNote = {
        ...note,
        color,
        modifiedAt: new Date().toISOString(),
      };
      notes.update((n) =>
        n.map((existingNote) => (existingNote.id === noteId ? updated : existingNote))
      );

      // Update search index
      searchService.updateNote(updated);
    }

    onProgress?.({
      current: i + 1,
      total,
      status: 'running',
    });
  }

  onProgress?.({
    current: total,
    total,
    status: 'complete',
  });
}

/**
 * Combine multiple notes into a single note
 * - Content is joined with horizontal rule separator
 * - Notes are ordered by creation date (oldest first)
 * - Tags: merged from all notes if mergeTags is true, otherwise only from first note
 * - Attachments are merged from all notes
 * - Original notes are soft-deleted
 */
export async function combineNotes(
  noteIds: string[],
  onProgress?: ProgressCallback,
  options?: CombineNotesOptions
): Promise<DecryptedNote> {
  const allNotes = get(notes);
  const total = noteIds.length;
  const mergeTags = options?.mergeTags ?? true; // Default to merging tags

  // Get selected notes sorted by creation date (oldest first)
  const selectedNotes = allNotes
    .filter((n) => noteIds.includes(n.id))
    .sort((a, b) => a.createdAt.localeCompare(b.createdAt));

  // Combine content with horizontal rule separator
  const combinedContent = selectedNotes.map((n) => n.content).join('\n\n---\n\n');

  // Tags: merge all unique tags or only keep from first note
  const combinedTags = mergeTags
    ? [...new Set(selectedNotes.flatMap((n) => n.tags))]
    : [...selectedNotes[0]?.tags ?? []];

  // Combine all attachments
  const combinedAttachments = selectedNotes.flatMap((n) => n.attachments);

  // Create new combined note
  const newNote = await noteService.createNote(combinedContent, combinedTags, {
    attachments: combinedAttachments,
    syntaxLanguage: 'markdown',
  });

  // Get decrypted version of the new note
  const decryptedNewNote = await noteService.getNote(newNote.id);
  if (!decryptedNewNote) {
    throw new Error('Failed to retrieve newly created combined note');
  }

  // Soft-delete original notes with progress
  for (let i = 0; i < noteIds.length; i++) {
    const noteId = noteIds[i];
    await noteService.deleteNote(noteId);
    searchService.removeNote(noteId);

    onProgress?.({
      current: i + 1,
      total,
      status: 'running',
    });
  }

  // Update store: remove deleted notes, add combined note
  notes.update((n) => {
    const filtered = n.filter((note) => !noteIds.includes(note.id));
    // Insert after pinned notes
    const pinnedCount = filtered.filter((note) => note.pinned).length;
    filtered.splice(pinnedCount, 0, decryptedNewNote);
    return filtered;
  });

  // Add new note to search index
  searchService.updateNote(decryptedNewNote);

  // Clear multi-selection and select the new note
  clearMultiSelection();
  selectNote(decryptedNewNote.id);

  onProgress?.({
    current: total,
    total,
    status: 'complete',
  });

  return decryptedNewNote;
}

export const bulkOperationsService = {
  addTagsToNotes,
  removeTagsFromNotes,
  setColorForNotes,
  deleteNotes,
  exportNotes,
  downloadExport,
  combineNotes,
};
