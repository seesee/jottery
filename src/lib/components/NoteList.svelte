<script lang="ts">
  import { filteredNotes, notes, searchQuery, selectedNoteId, settings } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';
  import { beforeUpdate, afterUpdate } from 'svelte';
  import { noteRepository } from '../services/noteRepository';
  import { keyManager } from '../services/keyManager';
  import type { DecryptedNote, KeyboardShortcut } from '../types';

  export let onNoteSelect: (() => void) | undefined = undefined;

  let scrollContainer: HTMLDivElement;
  let savedScrollTop = 0;

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
    const masterKey = keyManager.getMasterKey();
    if (masterKey) {
      await noteRepository.loadNotes(masterKey.key);
    }
  }

  async function requestDelete(note: DecryptedNote) {
    // Pinned notes should not reach here (UI prevents it), but double-check
    if (note.pinned) {
      console.warn('Attempted to delete pinned note');
      return;
    }

    // Delete directly - notes go to recycle bin
    try {
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      await noteRepository.delete(note.id);
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
    {#each $filteredNotes as note (note.id)}
      <NoteListItem {note} {onNoteSelect} onDeleteRequest={requestDelete} />
    {/each}
  {/if}
</div>
