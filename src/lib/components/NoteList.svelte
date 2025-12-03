<script lang="ts">
  import { filteredNotes } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';
</script>

<div class="h-full overflow-y-auto bg-white dark:bg-gray-900">
  {#if $filteredNotes.length === 0}
    <div class="flex items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4 text-center">
      <div>
        <p class="text-lg mb-2">No notes yet</p>
        <p class="text-sm">Create your first note to get started</p>
      </div>
    </div>
  {:else}
    {#each $filteredNotes as note (note.id)}
      <NoteListItem {note} />
    {/each}
  {/if}
</div>
