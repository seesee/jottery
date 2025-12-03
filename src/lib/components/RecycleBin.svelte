<script lang="ts">
  import { onMount } from 'svelte';
  import { noteService } from '../services';
  import type { DecryptedNote } from '../types';

  export let show: boolean = false;
  export let onClose: () => void;

  let deletedNotes: DecryptedNote[] = [];
  let loading = false;

  async function loadDeletedNotes() {
    loading = true;
    try {
      deletedNotes = await noteService.getDeletedNotes();
    } catch (error) {
      console.error('Failed to load deleted notes:', error);
    } finally {
      loading = false;
    }
  }

  async function handleRestore(noteId: string) {
    try {
      await noteService.restoreNote(noteId);
      // Reload deleted notes
      await loadDeletedNotes();
    } catch (error) {
      console.error('Failed to restore note:', error);
      alert('Failed to restore note: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  async function handlePermanentDelete(noteId: string) {
    if (confirm('Permanently delete this note? This cannot be undone!')) {
      try {
        await noteService.permanentlyDeleteNote(noteId);
        // Reload deleted notes
        await loadDeletedNotes();
      } catch (error) {
        console.error('Failed to delete note:', error);
        alert('Failed to delete note: ' + (error instanceof Error ? error.message : String(error)));
      }
    }
  }

  async function handleEmptyRecycleBin() {
    if (confirm(`Permanently delete all ${deletedNotes.length} notes in recycle bin? This cannot be undone!`)) {
      try {
        for (const note of deletedNotes) {
          await noteService.permanentlyDeleteNote(note.id);
        }
        deletedNotes = [];
      } catch (error) {
        console.error('Failed to empty recycle bin:', error);
        alert('Failed to empty recycle bin: ' + (error instanceof Error ? error.message : String(error)));
      }
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  function formatDate(dateString: string): string {
    return new Date(dateString).toLocaleString('en-GB', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  function getTitle(note: DecryptedNote): string {
    return note.content.split('\n')[0] || 'Untitled';
  }

  // Load deleted notes when modal opens
  $: if (show) {
    loadDeletedNotes();
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-4xl w-full mx-4 max-h-[90vh] overflow-y-auto">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">Recycle Bin</h2>
        <div class="flex items-center gap-2">
          {#if deletedNotes.length > 0}
            <button
              on:click={handleEmptyRecycleBin}
              class="px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-md transition-colors"
            >
              Empty Bin
            </button>
          {/if}
          <button
            on:click={onClose}
            class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- Content -->
      <div class="p-4">
        {#if loading}
          <div class="text-center py-8">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
            <p class="mt-2 text-gray-600 dark:text-gray-400">Loading...</p>
          </div>
        {:else if deletedNotes.length === 0}
          <div class="text-center py-8">
            <p class="text-lg text-gray-600 dark:text-gray-400">Recycle bin is empty</p>
            <p class="text-sm text-gray-500 dark:text-gray-500 mt-1">Deleted notes will appear here</p>
          </div>
        {:else}
          <div class="space-y-2">
            {#each deletedNotes as note (note.id)}
              <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-3 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors">
                <div class="flex items-start justify-between gap-4">
                  <div class="flex-1 min-w-0">
                    <h3 class="font-medium text-gray-900 dark:text-gray-100 truncate">
                      {getTitle(note)}
                    </h3>
                    <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                      Deleted: {note.deletedAt ? formatDate(note.deletedAt) : 'Unknown'}
                    </p>
                    {#if note.tags.length > 0}
                      <div class="flex gap-1 mt-2">
                        {#each note.tags.slice(0, 3) as tag}
                          <span class="text-xs bg-gray-200 dark:bg-gray-600 px-2 py-0.5 rounded">
                            #{tag}
                          </span>
                        {/each}
                        {#if note.tags.length > 3}
                          <span class="text-xs text-gray-500">+{note.tags.length - 3}</span>
                        {/if}
                      </div>
                    {/if}
                  </div>

                  <div class="flex gap-2">
                    <button
                      on:click={() => handleRestore(note.id)}
                      class="px-3 py-1.5 text-sm bg-blue-600 hover:bg-blue-700 text-white rounded-md transition-colors"
                      title="Restore note"
                    >
                      Restore
                    </button>
                    <button
                      on:click={() => handlePermanentDelete(note.id)}
                      class="px-3 py-1.5 text-sm bg-red-600 hover:bg-red-700 text-white rounded-md transition-colors"
                      title="Permanently delete"
                    >
                      Delete Forever
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Footer with info -->
      {#if deletedNotes.length > 0}
        <div class="border-t border-gray-200 dark:border-gray-700 p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {deletedNotes.length} {deletedNotes.length === 1 ? 'note' : 'notes'} in recycle bin.
            Notes are automatically purged after 30 days.
          </p>
        </div>
      {/if}
    </div>
  </div>
{/if}
