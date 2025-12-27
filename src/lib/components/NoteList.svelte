<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { filteredNotes, notes, searchQuery, selectNote, selectedNoteId } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';

  export let onNoteSelect: (() => void) | undefined = undefined;

  let focusedIndex = -1;

  // Update focused index when selection changes externally
  $: if ($selectedNoteId) {
    const index = $filteredNotes.findIndex(n => n.id === $selectedNoteId);
    if (index !== -1) {
      focusedIndex = index;
    }
  }

  function handleKeyDown(event: KeyboardEvent) {
    if ($filteredNotes.length === 0) return;

    switch (event.key) {
      case 'ArrowDown':
      case 'j':
        event.preventDefault();
        focusedIndex = Math.min(focusedIndex + 1, $filteredNotes.length - 1);
        break;

      case 'ArrowUp':
      case 'k':
        event.preventDefault();
        focusedIndex = Math.max(focusedIndex - 1, 0);
        break;

      case 'Enter':
        event.preventDefault();
        if (focusedIndex >= 0 && focusedIndex < $filteredNotes.length) {
          selectNote($filteredNotes[focusedIndex].id);
          if (onNoteSelect) {
            onNoteSelect();
          }
        }
        break;
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeyDown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });
</script>

<div class="h-full overflow-y-auto bg-white dark:bg-gray-900" role="list">
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
    {#each $filteredNotes as note, index (note.id)}
      <NoteListItem {note} {onNoteSelect} focused={index === focusedIndex} />
    {/each}
  {/if}
</div>
