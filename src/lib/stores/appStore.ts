/**
 * Application state store using Svelte stores
 */

import { writable, derived } from 'svelte/store';
import type { DecryptedNote, UserSettings } from '../types';
import { DEFAULT_SETTINGS } from '../types';

// Lock state
export const isLocked = writable<boolean>(true);
export const isLocking = writable<boolean>(false);
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
export const archiveMode = writable<boolean>(false);

// Sync state - prevents EditorPane from triggering sync during batch refresh
export const isSyncRefreshing = writable<boolean>(false);

// Sync in progress indicator - for UI feedback
export const isSyncing = writable<boolean>(false);

// Sync progress - for showing percentage when >= 5 items to sync
export const syncProgress = writable<{ total: number; completed: number }>({ total: 0, completed: 0 });

// Content-only update flag - prevents unnecessary search recalculation when only note content changed
// Set this before updating notes store, check it in search reactive block, then reset
export const isContentOnlyUpdate = writable<boolean>(false);

// Note list scroll position - preserved across mobile view transitions
export const noteListScrollPosition = writable<number>(0);

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

// Search result count (matches/total)
export const searchResultCount = derived(
  [filteredNotes, notes, searchQuery],
  ([$filteredNotes, $notes, $searchQuery]) => ({
    matches: $filteredNotes.length,
    total: $notes.length,
    isSearching: $searchQuery.trim().length > 0
  })
);

// Note: Search filtering is handled in App.svelte using searchService.searchNotes()
// The filteredNotes store is updated directly from there

// Multi-select state
export const selectedNoteIds = writable<Set<string>>(new Set());
export const isMultiSelectMode = writable<boolean>(false);
export const lastSelectedIndex = writable<number | null>(null);

// Derived store for selected count
export const selectedCount = derived(
  selectedNoteIds,
  ($selectedNoteIds) => $selectedNoteIds.size
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

export function enterDraftMode() {
  isDraftMode.set(true);
  // Don't clear selectedNoteId - draft mode will override display
}

export function exitDraftMode() {
  isDraftMode.set(false);
}

// Multi-select actions

/**
 * Toggle a note's selection state
 */
export function toggleNoteSelection(noteId: string, noteIndex: number) {
  selectedNoteIds.update((set) => {
    const newSet = new Set(set);
    if (newSet.has(noteId)) {
      newSet.delete(noteId);
    } else {
      newSet.add(noteId);
    }
    // Enable multi-select mode if any notes are selected
    isMultiSelectMode.set(newSet.size > 0);
    return newSet;
  });
  lastSelectedIndex.set(noteIndex);
}

/**
 * Select a range of notes (for shift+click)
 */
export function selectRange(fromIndex: number, toIndex: number, notesList: DecryptedNote[]) {
  const start = Math.min(fromIndex, toIndex);
  const end = Math.max(fromIndex, toIndex);

  selectedNoteIds.update((set) => {
    const newSet = new Set(set);
    for (let i = start; i <= end; i++) {
      if (notesList[i]) {
        newSet.add(notesList[i].id);
      }
    }
    isMultiSelectMode.set(newSet.size > 0);
    return newSet;
  });
  lastSelectedIndex.set(toIndex);
}

/**
 * Select all currently filtered notes
 */
export function selectAllFiltered(notesList: DecryptedNote[]) {
  const ids = new Set(notesList.map((n) => n.id));
  selectedNoteIds.set(ids);
  isMultiSelectMode.set(ids.size > 0);
}

/**
 * Clear all selections and exit multi-select mode
 */
export function clearMultiSelection() {
  selectedNoteIds.set(new Set());
  isMultiSelectMode.set(false);
  lastSelectedIndex.set(null);
}

/**
 * Toggle archive mode on/off
 * When enabled, shows archived notes instead of active notes
 */
export function toggleArchiveMode() {
  archiveMode.update((mode) => !mode);
  // Clear multi-select when toggling modes (but preserve search query)
  clearMultiSelection();
}
