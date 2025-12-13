<script lang="ts">
  import type { DecryptedNote } from '../types';
  import { selectNote, selectedNoteId, searchQuery } from '../stores/appStore';

  export let note: DecryptedNote;
  export let onNoteSelect: (() => void) | undefined = undefined;

  $: isSelected = $selectedNoteId === note.id;
  $: title = note.content.split('\n')[0] || 'Untitled';
  // Responsive preview length: shorter on mobile, longer on desktop
  $: isMobile = typeof window !== 'undefined' && window.innerWidth < 768;
  $: previewLength = isMobile ? 60 : 100;
  $: preview = note.content.split('\n').slice(1).join(' ').slice(0, previewLength);
  $: formattedDate = new Date(note.modifiedAt).toLocaleDateString('en-GB', {
    month: 'short',
    day: 'numeric',
    year: 'numeric'
  });

  function handleClick() {
    selectNote(note.id);
    // Call mobile navigation callback if provided
    if (onNoteSelect) {
      onNoteSelect();
    }
  }

  function handleTagClick(event: MouseEvent, tag: string) {
    // Stop propagation to prevent note selection
    event.stopPropagation();
    event.preventDefault();

    // Set search query to filter by this tag
    searchQuery.set(`#${tag}`);
  }
</script>

<button
  on:click={handleClick}
  class="w-full text-left p-4 min-h-[60px] border-b border-gray-200 dark:border-gray-700 active:bg-gray-100 dark:active:bg-gray-700 transition-colors {isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-4 border-l-blue-500' : ''}"
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
    <span class="flex-shrink-0">{formattedDate}</span>
    {#if note.tags.length > 0}
      <div class="flex gap-1 flex-wrap justify-end ml-2">
        {#each note.tags.slice(0, 2) as tag}
          <span
            on:click={(e) => handleTagClick(e, tag)}
            on:keydown={(e) => e.key === 'Enter' && handleTagClick(e, tag)}
            role="button"
            tabindex="0"
            class="bg-gray-200 dark:bg-gray-700 hover:bg-blue-200 dark:hover:bg-blue-800 active:bg-blue-300 dark:active:bg-blue-700 px-2 py-1 rounded text-xs whitespace-nowrap transition-colors cursor-pointer"
            title="Filter by #{tag}"
          >
            #{tag}
          </span>
        {/each}
        {#if note.tags.length > 2}
          <span class="text-gray-400 flex-shrink-0">+{note.tags.length - 2}</span>
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
