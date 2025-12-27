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

// Draft mode for new notes (deferred creation)
export const isDraftMode = writable<boolean>(false);

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

// Filtered notes using search service (will be populated when search is performed)
export const filteredNotes = writable<DecryptedNote[]>([]);

// Update filtered notes when search query or notes change
export function updateFilteredNotes(allNotes: DecryptedNote[], query: string) {
  if (!query.trim()) {
    filteredNotes.set(allNotes);
  } else {
    // Will use searchService in App.svelte
    filteredNotes.set(allNotes);
  }
}

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

export function enterDraftMode() {
  isDraftMode.set(true);
  // Don't clear selectedNoteId - draft mode will override display
}

export function exitDraftMode() {
  isDraftMode.set(false);
}
