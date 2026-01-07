/**
 * Bulk operations service for multi-note actions
 */

import type { DecryptedNote, ExportNote, ExportData } from '../types';
import { noteRepository } from './noteRepository';
import { notes, clearMultiSelection } from '../stores/appStore';
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
        // Update note with new tags
        const updated: DecryptedNote = {
          ...note,
          tags: newTags,
          modifiedAt: new Date().toISOString(),
        };

        await noteRepository.update(updated);

        // Update store
        notes.update((n) =>
          n.map((existingNote) => (existingNote.id === noteId ? updated : existingNote))
        );
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
        // Update note with new tags
        const updated: DecryptedNote = {
          ...note,
          tags: newTags,
          modifiedAt: new Date().toISOString(),
        };

        await noteRepository.update(updated);

        // Update store
        notes.update((n) =>
          n.map((existingNote) => (existingNote.id === noteId ? updated : existingNote))
        );
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

export const bulkOperationsService = {
  addTagsToNotes,
  removeTagsFromNotes,
  deleteNotes,
  exportNotes,
  downloadExport,
};
