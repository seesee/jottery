<script lang="ts">
  import { filteredNotes, notes, searchQuery } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';
</script>

<div class="h-full overflow-y-auto bg-white dark:bg-gray-900">
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
      <NoteListItem {note} />
    {/each}
  {/if}
</div>
