<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { get } from 'svelte/store';
  import { noteService } from '../services';
  import type { DecryptedNote } from '../types';
  import { formatDate } from '../utils/dateFormat';
  import { toast } from '../utils/toast.svelte';
  import { settings } from '../stores/appStore';
  import { addNoteToStoreAndSearch } from '../stores/storeHelpers';
  import { getColorHex, getTagColor, resolveTheme } from '../services/colorService';
  import { getNoteTitle } from '../utils/noteTitle';

  // Helper to get formatted date synchronously (for use in templates)
  function getFormattedDate(date: string, options: Intl.DateTimeFormatOptions) {
    return get(formatDate(date, options));
  }

  export let show: boolean = false;
  export let onClose: () => void;

  let archivedNotes: DecryptedNote[] = [];
  let loading = false;

  // Color support: resolve theme (reactive dependency on settings for reactivity)
  $: currentTheme = resolveTheme($settings.theme);

  // Helper function to get tag color
  function getTagBackgroundColor(tag: string): string | undefined {
    const tagColorName = getTagColor(tag);
    return tagColorName ? getColorHex(tagColorName, currentTheme) : undefined;
  }

  async function loadArchivedNotes() {
    loading = true;
    try {
      archivedNotes = await noteService.getArchivedNotes();
    } catch (error) {
      console.error('Failed to load archived notes:', error);
    } finally {
      loading = false;
    }
  }

  async function handleUnarchive(noteId: string) {
    try {
      await noteService.unarchiveNote(noteId);

      // Fetch the unarchived note and add it to the notes store and search index
      const unarchivedNote = await noteService.getNote(noteId);
      if (unarchivedNote) {
        addNoteToStoreAndSearch(unarchivedNote);
      }

      // Reload archived notes list
      await loadArchivedNotes();
      toast.success($_('archive.unarchived'));
    } catch (error) {
      console.error('Failed to unarchive note:', error);
      toast.error($_('archive.error.unarchive') + ': ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  function getTitle(note: DecryptedNote): string {
    const title = getNoteTitle(note);
    return title === 'Untitled' ? $_('archive.untitled') : title;
  }

  // Load archived notes when modal opens
  $: if (show) {
    loadArchivedNotes();
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 w-full h-full tablet:h-auto tablet:max-w-4xl tablet:rounded-lg shadow-xl tablet:max-h-[90vh] flex flex-col">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between flex-shrink-0">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('archive.title')}</h2>
        <button
          on:click={onClose}
          class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
          aria-label={$_('archive.closeLabel')}
        >
          ✕
        </button>
      </div>

      <!-- Content (scrollable) -->
      <div class="p-4 flex-1 overflow-y-auto">
        {#if loading}
          <div class="text-center py-8">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
            <p class="mt-2 text-gray-600 dark:text-gray-400">{$_('archive.loading')}</p>
          </div>
        {:else if archivedNotes.length === 0}
          <div class="text-center py-8">
            <p class="text-lg text-gray-600 dark:text-gray-400">{$_('archive.empty')}</p>
            <p class="text-sm text-gray-500 dark:text-gray-500 mt-1">{$_('archive.emptyDescription')}</p>
          </div>
        {:else}
          <div class="space-y-2">
            {#each archivedNotes as note (note.id)}
              <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-3 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors">
                <div class="flex items-start justify-between gap-4">
                  <div class="flex-1 min-w-0">
                    <h3 class="font-medium text-gray-900 dark:text-gray-100 truncate">
                      {getTitle(note)}
                    </h3>
                    <p class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                      {$_('archive.archived')} {note.archivedAt ? getFormattedDate(note.archivedAt, { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' }) : $_('archive.unknown')}
                    </p>
                    {#if note.tags.length > 0}
                      <div class="flex gap-1 mt-2">
                        {#each note.tags.slice(0, 3) as tag}
                          {@const tagBgColor = getTagBackgroundColor(tag)}
                          <span
                            class="text-xs px-2 py-0.5 rounded {tagBgColor ? '' : 'bg-gray-200 dark:bg-gray-600'}"
                            style:background-color={tagBgColor}
                          >
                            #{tag}
                          </span>
                        {/each}
                        {#if note.tags.length > 3}
                          <span class="text-xs text-gray-500">+{note.tags.length - 3}</span>
                        {/if}
                      </div>
                    {/if}
                  </div>

                  <div class="flex gap-3">
                    <button
                      on:click={() => handleUnarchive(note.id)}
                      class="px-4 py-2.5 min-h-11 text-sm bg-blue-600 active:bg-blue-700 text-white rounded-md transition-colors whitespace-nowrap"
                      title={$_('archive.unarchiveTitle')}
                    >
                      {$_('archive.unarchive')}
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Footer with info -->
      {#if archivedNotes.length > 0}
        <div class="border-t border-gray-200 dark:border-gray-700 p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {$_(archivedNotes.length === 1 ? 'archive.footer.single' : 'archive.footer.multiple', { values: { count: archivedNotes.length } })}
          </p>
        </div>
      {/if}
    </div>
  </div>
{/if}
