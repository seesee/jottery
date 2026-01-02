<script lang="ts">
  import { filteredNotes, notes, searchQuery, selectedNoteId, settings } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';
  import { beforeUpdate, afterUpdate, onMount } from 'svelte';
  import { noteRepository } from '../services/noteRepository';
  import { noteService } from '../services/noteService';
  import { keyManager } from '../services/keyManager';
  import type { DecryptedNote, KeyboardShortcut } from '../types';

  export let onNoteSelect: (() => void) | undefined = undefined;

  let scrollContainer: HTMLDivElement;
  let savedScrollTop = 0;

  // Virtual scrolling state
  const ESTIMATED_ITEM_HEIGHT = 80; // Initial estimated height per note item in pixels
  const OVERSCAN = 15; // Number of items to render outside viewport for smooth scrolling
  let viewportHeight = 0;
  let scrollTop = 0;
  let startIndex = 0;
  let endIndex = 0;
  let visibleNotes: DecryptedNote[] = [];
  let totalHeight = 0;
  let offsetY = 0;

  // Height cache: stores measured heights for each note ID
  let heightCache = new Map<string, number>();
  let itemElements: (HTMLElement | null)[] = [];

  // Get height for a note (measured or estimated)
  function getItemHeight(index: number): number {
    if (index < 0 || index >= $filteredNotes.length) return ESTIMATED_ITEM_HEIGHT;
    const noteId = $filteredNotes[index].id;
    return heightCache.get(noteId) || ESTIMATED_ITEM_HEIGHT;
  }

  // Calculate total height based on measured/estimated heights
  function calculateTotalHeight(): number {
    let total = 0;
    for (let i = 0; i < $filteredNotes.length; i++) {
      total += getItemHeight(i);
    }
    return total;
  }

  // Calculate offset for items before startIndex
  function calculateOffset(upToIndex: number): number {
    let offset = 0;
    for (let i = 0; i < upToIndex && i < $filteredNotes.length; i++) {
      offset += getItemHeight(i);
    }
    return offset;
  }

  // Find which item index contains a given scroll position
  function findIndexAtPosition(position: number): number {
    let currentPos = 0;
    for (let i = 0; i < $filteredNotes.length; i++) {
      const itemHeight = getItemHeight(i);
      if (currentPos + itemHeight > position) {
        return i;
      }
      currentPos += itemHeight;
    }
    return Math.max(0, $filteredNotes.length - 1);
  }

  // Calculate visible range based on scroll position
  function updateVisibleRange() {
    if (!scrollContainer) return;

    viewportHeight = scrollContainer.clientHeight;
    scrollTop = scrollContainer.scrollTop;

    // Find start index based on scroll position (subtract OVERSCAN for buffer above)
    startIndex = Math.max(0, findIndexAtPosition(scrollTop) - OVERSCAN);

    // Find end index based on scroll position + viewport height (add OVERSCAN for buffer below)
    const endPosition = scrollTop + viewportHeight;
    endIndex = Math.min($filteredNotes.length, findIndexAtPosition(endPosition) + 1 + OVERSCAN);

    visibleNotes = $filteredNotes.slice(startIndex, endIndex);
    totalHeight = calculateTotalHeight();
    offsetY = calculateOffset(startIndex);
  }

  // Measure heights of rendered items
  function measureHeights() {
    let heightsChanged = false;

    // Measure each visible item
    for (let i = 0; i < visibleNotes.length; i++) {
      const element = itemElements[i];
      const note = visibleNotes[i];

      if (element && note) {
        const height = element.offsetHeight;
        const cachedHeight = heightCache.get(note.id);

        if (height > 0 && height !== cachedHeight) {
          heightCache.set(note.id, height);
          heightsChanged = true;
        }
      }
    }

    // If heights changed, recalculate visible range
    if (heightsChanged) {
      updateVisibleRange();
    }
  }

  // Handle scroll events
  function handleScroll() {
    updateVisibleRange();
  }

  // Update visible range when filtered notes change
  $: if ($filteredNotes) {
    updateVisibleRange();
  }

  let previousSelectedNoteId: string | null = null;

  // Scroll selected note into view when selection changes (not during manual scrolling)
  $: if ($selectedNoteId && scrollContainer && $selectedNoteId !== previousSelectedNoteId) {
    scrollToNote($selectedNoteId);
    previousSelectedNoteId = $selectedNoteId;
  }

  function scrollToNote(noteId: string) {
    const noteIndex = $filteredNotes.findIndex(n => n.id === noteId);
    if (noteIndex === -1) return;

    // Calculate the position of the note
    const notePosition = calculateOffset(noteIndex);
    const noteHeight = getItemHeight(noteIndex);

    const scrollTop = scrollContainer.scrollTop;
    const viewportHeight = scrollContainer.clientHeight;

    // Check if note is outside the viewport
    const isAboveViewport = notePosition < scrollTop;
    const isBelowViewport = notePosition + noteHeight > scrollTop + viewportHeight;

    if (isAboveViewport) {
      // Scroll to show note at top of viewport
      scrollContainer.scrollTop = notePosition;
    } else if (isBelowViewport) {
      // Scroll to show note at bottom of viewport
      scrollContainer.scrollTop = notePosition + noteHeight - viewportHeight;
    }
    // If note is already visible, don't scroll
  }

  // Initialize on mount
  onMount(() => {
    updateVisibleRange();
  });

  // Save scroll position before DOM updates
  beforeUpdate(() => {
    if (scrollContainer) {
      savedScrollTop = scrollContainer.scrollTop;
    }
  });

  // Restore scroll position after DOM updates and measure heights
  afterUpdate(() => {
    // Measure heights of rendered items
    measureHeights();

    // Restore scroll position
    if (scrollContainer && savedScrollTop >= 0) {
      scrollContainer.scrollTop = savedScrollTop;
    }
  });

  async function reloadNotes() {
    // Reload notes after deletion
    try {
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
    } catch (error) {
      console.error('Failed to reload notes:', error);
    }
  }

  async function requestDelete(note: DecryptedNote) {
    // Pinned notes should not reach here (UI prevents it), but double-check
    if (note.pinned) {
      console.warn('Attempted to delete pinned note');
      return;
    }

    // Soft delete - notes go to recycle bin
    try {
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      await noteRepository.softDelete(note.id);
      await reloadNotes();
    } catch (error) {
      console.error('Failed to delete note:', error);
    }
  }

  function matchesShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut): boolean {
    const ctrlOrCmd = event.ctrlKey || event.metaKey;
    return (
      event.key === shortcut.key &&
      (shortcut.ctrl ? ctrlOrCmd : !ctrlOrCmd) &&
      (shortcut.alt ? event.altKey : !event.altKey) &&
      (shortcut.shift ? event.shiftKey : !event.shiftKey)
    );
  }

  function handleKeydown(event: KeyboardEvent) {
    // Handle delete shortcut when a note is selected
    if ($selectedNoteId && $settings.keyboardShortcuts) {
      const deleteShortcut = $settings.keyboardShortcuts.deleteNote;
      if (matchesShortcut(event, deleteShortcut)) {
        const selectedNote = $filteredNotes.find(n => n.id === $selectedNoteId);
        if (selectedNote) {
          event.preventDefault();
          requestDelete(selectedNote);
        }
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={scrollContainer}
  class="h-full overflow-y-auto bg-white dark:bg-gray-900"
  on:scroll={handleScroll}
  on:keydown={handleKeydown}
  role="list"
  tabindex="-1"
>
  {#if $filteredNotes.length === 0}
    <div class="flex items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4 text-center">
      <div>
        {#if $notes.length === 0}
          <p class="text-lg mb-2">No notes yet</p>
          <p class="text-sm">Create your first note to get started</p>
        {:else if $searchQuery.trim()}
          <p class="text-lg mb-2">No results found</p>
          <p class="text-sm">Try a different search query</p>
        {:else}
          <p class="text-lg mb-2">No notes</p>
          <p class="text-sm">Something went wrong</p>
        {/if}
      </div>
    </div>
  {:else}
    <!-- Virtual scrolling container -->
    <div style="height: {totalHeight}px; position: relative;">
      <!-- Offset for items before viewport -->
      <div style="height: {offsetY}px;"></div>

      <!-- Render only visible items -->
      {#each visibleNotes as note, i (note.id)}
        <div bind:this={itemElements[i]}>
          <NoteListItem {note} {onNoteSelect} onDeleteRequest={requestDelete} />
        </div>
      {/each}
    </div>
  {/if}
</div>
