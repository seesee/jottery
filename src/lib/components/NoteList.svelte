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
  const ESTIMATED_ITEM_HEIGHT = 80; // Estimated height per note item in pixels
  const OVERSCAN = 5; // Number of items to render outside viewport for smooth scrolling
  let viewportHeight = 0;
  let scrollTop = 0;
  let startIndex = 0;
  let endIndex = 0;
  let visibleNotes: DecryptedNote[] = [];
  let totalHeight = 0;
  let offsetY = 0;

  // Calculate visible range based on scroll position
  function updateVisibleRange() {
    if (!scrollContainer) return;

    viewportHeight = scrollContainer.clientHeight;
    scrollTop = scrollContainer.scrollTop;

    // Calculate which items should be visible
    const itemCount = $filteredNotes.length;
    const visibleCount = Math.ceil(viewportHeight / ESTIMATED_ITEM_HEIGHT);

    startIndex = Math.max(0, Math.floor(scrollTop / ESTIMATED_ITEM_HEIGHT) - OVERSCAN);
    endIndex = Math.min(itemCount, startIndex + visibleCount + (OVERSCAN * 2));

    visibleNotes = $filteredNotes.slice(startIndex, endIndex);
    totalHeight = itemCount * ESTIMATED_ITEM_HEIGHT;
    offsetY = startIndex * ESTIMATED_ITEM_HEIGHT;
  }

  // Handle scroll events
  function handleScroll() {
    updateVisibleRange();
  }

  // Update visible range when filtered notes change
  $: if ($filteredNotes) {
    updateVisibleRange();
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

  // Restore scroll position after DOM updates
  afterUpdate(() => {
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
      {#each visibleNotes as note (note.id)}
        <NoteListItem {note} {onNoteSelect} onDeleteRequest={requestDelete} />
      {/each}
    </div>
  {/if}
</div>
