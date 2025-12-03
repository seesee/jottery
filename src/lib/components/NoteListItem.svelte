<script lang="ts">
  import type { DecryptedNote } from '../types';
  import { selectNote, selectedNoteId } from '../stores/appStore';

  export let note: DecryptedNote;

  $: isSelected = $selectedNoteId === note.id;
  $: title = note.content.split('\n')[0] || 'Untitled';
  $: preview = note.content.split('\n').slice(1).join(' ').slice(0, 100);
  $: formattedDate = new Date(note.modifiedAt).toLocaleDateString('en-GB', {
    month: 'short',
    day: 'numeric',
    year: 'numeric'
  });

  function handleClick() {
    selectNote(note.id);
  }
</script>

<button
  on:click={handleClick}
  class="w-full text-left p-3 border-b border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors {isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-4 border-l-blue-500' : ''}"
>
  <div class="flex items-start justify-between mb-1">
    <div class="flex items-center gap-2 flex-1 min-w-0">
      {#if note.pinned}
        <span class="text-yellow-500 flex-shrink-0">⭐</span>
      {/if}
      <h3 class="font-medium text-gray-900 dark:text-gray-100 truncate">
        {title}
      </h3>
    </div>
  </div>

  {#if preview}
    <p class="text-sm text-gray-600 dark:text-gray-400 line-clamp-2 mb-1">
      {preview}
    </p>
  {/if}

  <div class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-500">
    <span>{formattedDate}</span>
    {#if note.tags.length > 0}
      <div class="flex gap-1">
        {#each note.tags.slice(0, 2) as tag}
          <span class="bg-gray-200 dark:bg-gray-700 px-2 py-0.5 rounded">
            #{tag}
          </span>
        {/each}
        {#if note.tags.length > 2}
          <span class="text-gray-400">+{note.tags.length - 2}</span>
        {/if}
      </div>
    {/if}
  </div>
</button>

<style>
  .line-clamp-2 {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
