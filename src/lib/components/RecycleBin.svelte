<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { noteService } from '../services';
  import type { DecryptedNote } from '../types';
  import { formatDate } from '../utils/dateFormat';
  import ConfirmModal from './ConfirmModal.svelte';
  import { toast } from '../utils/toast.svelte';

  export let show: boolean = false;
  export let onClose: () => void;

  let deletedNotes: DecryptedNote[] = [];
  let loading = false;
  let showPermanentDeleteConfirm = false;
  let showEmptyBinConfirm = false;
  let noteToDelete: string | null = null;

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
      toast.error($_('recycleBin.error.restore') + ': ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handlePermanentDelete(noteId: string) {
    noteToDelete = noteId;
    showPermanentDeleteConfirm = true;
  }

  async function confirmPermanentDelete() {
    if (!noteToDelete) return;
    showPermanentDeleteConfirm = false;

    try {
      await noteService.permanentlyDeleteNote(noteToDelete);
      noteToDelete = null;
      // Reload deleted notes
      await loadDeletedNotes();
    } catch (error) {
      console.error('Failed to delete note:', error);
      toast.error($_('recycleBin.error.delete') + ': ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleEmptyRecycleBin() {
    showEmptyBinConfirm = true;
  }

  async function confirmEmptyRecycleBin() {
    showEmptyBinConfirm = false;

    try {
      for (const note of deletedNotes) {
        await noteService.permanentlyDeleteNote(note.id);
      }
      deletedNotes = [];
    } catch (error) {
      console.error('Failed to empty recycle bin:', error);
      toast.error($_('recycleBin.error.emptyBin') + ': ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  function getTitle(note: DecryptedNote): string {
    return note.content.split('\n')[0] || $_('recycleBin.untitled');
  }

  // Load deleted notes when modal opens
  $: if (show) {
    loadDeletedNotes();
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
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('recycleBin.title')}</h2>
        <div class="flex items-center gap-3">
          {#if deletedNotes.length > 0}
            <button
              on:click={handleEmptyRecycleBin}
              class="px-4 py-2.5 min-h-11 text-sm text-red-600 active:bg-red-50 dark:active:bg-red-900/20 rounded-md transition-colors"
            >
              {$_('recycleBin.emptyBin')}
            </button>
          {/if}
          <button
            on:click={onClose}
            class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
            aria-label={$_('recycleBin.closeLabel')}
          >
            ✕
          </button>
        </div>
      </div>

      <!-- Content (scrollable) -->
      <div class="p-4 flex-1 overflow-y-auto">
        {#if loading}
          <div class="text-center py-8">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
            <p class="mt-2 text-gray-600 dark:text-gray-400">{$_('recycleBin.loading')}</p>
          </div>
        {:else if deletedNotes.length === 0}
          <div class="text-center py-8">
            <p class="text-lg text-gray-600 dark:text-gray-400">{$_('recycleBin.empty')}</p>
            <p class="text-sm text-gray-500 dark:text-gray-500 mt-1">{$_('recycleBin.emptyDescription')}</p>
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
                      {@const deletedDateStore = note.deletedAt ? formatDate(note.deletedAt, { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' }) : null}
                      {$_('recycleBin.deleted')} {deletedDateStore ? $deletedDateStore : $_('recycleBin.unknown')}
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

                  <div class="flex gap-3">
                    <button
                      on:click={() => handleRestore(note.id)}
                      class="px-4 py-2.5 min-h-11 text-sm bg-blue-600 active:bg-blue-700 text-white rounded-md transition-colors whitespace-nowrap"
                      title={$_('recycleBin.restoreTitle')}
                    >
                      {$_('recycleBin.restore')}
                    </button>
                    <button
                      on:click={() => handlePermanentDelete(note.id)}
                      class="px-4 py-2.5 min-h-11 text-sm bg-red-600 active:bg-red-700 text-white rounded-md transition-colors whitespace-nowrap"
                      title={$_('recycleBin.deletePermanentlyTitle')}
                    >
                      {$_('recycleBin.deleteForever')}
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
            {$_(deletedNotes.length === 1 ? 'recycleBin.footer.single' : 'recycleBin.footer.multiple', { values: { count: deletedNotes.length } })}
          </p>
        </div>
      {/if}
    </div>
  </div>

  <!-- Permanent Delete Confirmation Modal -->
  <ConfirmModal
    show={showPermanentDeleteConfirm}
    title={$_('recycleBin.confirm.deleteNote.title')}
    message={$_('recycleBin.confirm.deleteNote.message')}
    confirmText={$_('recycleBin.confirm.deleteNote.confirmButton')}
    cancelText={$_('common.cancel')}
    confirmClass="bg-red-600 hover:bg-red-700"
    onConfirm={confirmPermanentDelete}
    onCancel={() => showPermanentDeleteConfirm = false}
  />

  <!-- Empty Recycle Bin Confirmation Modal -->
  <ConfirmModal
    show={showEmptyBinConfirm}
    title={$_('recycleBin.confirm.emptyBin.title')}
    message={$_('recycleBin.confirm.emptyBin.message', {
      values: {
        count: deletedNotes.length,
        notes: deletedNotes.length === 1 ? $_('recycleBin.confirm.emptyBin.note') : $_('recycleBin.confirm.emptyBin.notes')
      }
    })}
    confirmText={$_('recycleBin.confirm.emptyBin.confirmButton')}
    cancelText={$_('common.cancel')}
    confirmClass="bg-red-600 hover:bg-red-700"
    onConfirm={confirmEmptyRecycleBin}
    onCancel={() => showEmptyBinConfirm = false}
  />
{/if}
