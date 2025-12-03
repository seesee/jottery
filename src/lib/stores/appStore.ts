/**
 * Application state store using Svelte stores
 */

import { writable, derived } from 'svelte/store';
import type { DecryptedNote, UserSettings } from '../types';
import { DEFAULT_SETTINGS } from '../types';

// Lock state
export const isLocked = writable<boolean>(true);
export const isInitialized = writable<boolean>(false);

// Notes
export const notes = writable<DecryptedNote[]>([]);
export const selectedNoteId = writable<string | null>(null);
export const searchQuery = writable<string>('');

// Settings
export const settings = writable<UserSettings>(DEFAULT_SETTINGS);

// UI state
export const showSettings = writable<boolean>(false);
export const showRecycleBin = writable<boolean>(false);

// Derived stores
export const selectedNote = derived(
  [notes, selectedNoteId],
  ([$notes, $selectedNoteId]) => {
    if (!$selectedNoteId) return null;
    return $notes.find(note => note.id === $selectedNoteId) || null;
  }
);

export const filteredNotes = derived(
  [notes, searchQuery],
  ([$notes, $searchQuery]) => {
    if (!$searchQuery) return $notes;

    const query = $searchQuery.toLowerCase();
    return $notes.filter(note => {
      const contentMatch = note.content.toLowerCase().includes(query);
      const tagsMatch = note.tags.some(tag => tag.toLowerCase().includes(query));
      return contentMatch || tagsMatch;
    });
  }
);

// Actions
export function selectNote(noteId: string | null) {
  selectedNoteId.set(noteId);
}

export function clearSelection() {
  selectedNoteId.set(null);
}

export function setSearchQuery(query: string) {
  searchQuery.set(query);
}
