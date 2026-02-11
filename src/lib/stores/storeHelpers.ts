/**
 * Store helper utilities for common note store operations
 * Reduces code duplication across components
 */

import type { DecryptedNote } from '../types';
import { get } from 'svelte/store';
import { notes } from './appStore';
import { searchService } from '../services/searchService';

/**
 * Update a single note in the notes store
 * Finds the note by ID and replaces it with the updated version
 * Uses get/set pattern to ensure proper Svelte reactivity for derived stores
 */
export function updateNoteInStore(updatedNote: DecryptedNote): void {
  const allNotes = get(notes);
  const index = allNotes.findIndex(n => n.id === updatedNote.id);
  if (index !== -1) {
    // Create completely new array with new note object to trigger reactivity
    const newNotes = allNotes.map((n, i) => i === index ? updatedNote : n);
    notes.set(newNotes);
  }
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
