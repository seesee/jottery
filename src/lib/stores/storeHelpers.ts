/**
 * Store helper utilities for common note store operations
 * Reduces code duplication across components
 */

import type { DecryptedNote } from '../types';
import { notes } from './appStore';
import { searchService } from '../services/searchService';

/**
 * Update a single note in the notes store
 * Finds the note by ID and replaces it with the updated version
 * Returns a new array reference to ensure proper Svelte reactivity
 */
export function updateNoteInStore(updatedNote: DecryptedNote): void {
  notes.update(allNotes => {
    const index = allNotes.findIndex(n => n.id === updatedNote.id);
    if (index !== -1) {
      // Create new array to ensure proper reactivity in derived stores
      const newNotes = [...allNotes];
      newNotes[index] = updatedNote;
      return newNotes;
    }
    return allNotes;
  });
}

/**
 * Update a single note in both the notes store and search index
 * This is the most common pattern used after saving a note
 */
export function updateNoteInStoreAndSearch(updatedNote: DecryptedNote): void {
  updateNoteInStore(updatedNote);
  searchService.updateNote(updatedNote);
}

/**
 * Remove a note from the notes store by ID
 */
export function removeNoteFromStore(noteId: string): void {
  notes.update(allNotes => allNotes.filter(n => n.id !== noteId));
}

/**
 * Remove a note from both the notes store and search index
 */
export function removeNoteFromStoreAndSearch(noteId: string): void {
  removeNoteFromStore(noteId);
  searchService.removeNote(noteId);
}

/**
 * Add a note to the notes store at a specific position
 * @param note The note to add
 * @param afterPinned If true, inserts after pinned notes (default for new unpinned notes)
 */
export function addNoteToStore(note: DecryptedNote, afterPinned = true): void {
  notes.update(allNotes => {
    if (afterPinned && !note.pinned) {
      // Insert after pinned notes
      const pinnedCount = allNotes.filter(n => n.pinned).length;
      const newNotes = [...allNotes];
      newNotes.splice(pinnedCount, 0, note);
      return newNotes;
    }
    // Add to beginning if pinned or afterPinned is false
    return [note, ...allNotes];
  });
}

/**
 * Add a note to both the notes store and search index
 */
export function addNoteToStoreAndSearch(note: DecryptedNote, afterPinned = true): void {
  addNoteToStore(note, afterPinned);
  searchService.updateNote(note);
}
