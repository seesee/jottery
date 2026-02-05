<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { get } from 'svelte/store';
  import { inboxItems, inboxCount } from '../stores/appStore';
  import { inboxService } from '../services/inboxService';
  import type { InboxItem } from '../types';
  import { formatDate } from '../utils/dateFormat';
  import ConfirmModal from './ConfirmModal.svelte';
  import { toast } from '../utils/toast.svelte';

  function getFormattedDate(date: string, options: Intl.DateTimeFormatOptions) {
    return get(formatDate(date, options));
  }

  export let show: boolean = false;
  export let onClose: () => void;

  let showDeleteConfirm = false;
  let showDeleteAllConfirm = false;
  let itemToDelete: string | null = null;
  let accepting = false;
  let deleting = false;

  function getTitle(item: InboxItem): string {
    const firstLine = item.content.split('\n')[0]?.trim();
    return firstLine || $_('inbox.untitled');
  }

  async function handleAccept(item: InboxItem) {
    accepting = true;
    try {
      await inboxService.acceptItem(item);
    } catch (error) {
      console.error('Failed to accept inbox item:', error);
      toast.error($_('inbox.error.accept'));
    } finally {
      accepting = false;
    }
  }

  function handleDelete(itemId: string) {
    itemToDelete = itemId;
    showDeleteConfirm = true;
  }

  async function confirmDelete() {
    if (!itemToDelete) return;
    showDeleteConfirm = false;
    deleting = true;

    try {
      await inboxService.deleteItem(itemToDelete);
      itemToDelete = null;
    } catch (error) {
      console.error('Failed to delete inbox item:', error);
      toast.error($_('inbox.error.delete'));
    } finally {
      deleting = false;
    }
  }

  async function handleAcceptAll() {
    accepting = true;
    try {
      await inboxService.acceptAll();
    } catch (error) {
      console.error('Failed to accept all inbox items:', error);
      toast.error($_('inbox.error.acceptAll'));
    } finally {
      accepting = false;
    }
  }

  function handleDeleteAll() {
    showDeleteAllConfirm = true;
  }

  async function confirmDeleteAll() {
    showDeleteAllConfirm = false;
    deleting = true;
    try {
      await inboxService.deleteAll();
    } catch (error) {
      console.error('Failed to delete all inbox items:', error);
      toast.error($_('inbox.error.deleteAll'));
    } finally {
      deleting = false;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
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
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">
          {$_('inbox.title')} ({$inboxCount})
        </h2>
        <div class="flex items-center gap-3">
          {#if $inboxCount > 0}
            <button
              on:click={handleAcceptAll}
              disabled={accepting}
              class="px-4 py-2.5 min-h-11 text-sm bg-blue-600 active:bg-blue-700 disabled:opacity-50 text-white rounded-md transition-colors"
            >
              {accepting ? $_('inbox.accepting') : $_('inbox.acceptAll')}
            </button>
            <button
              on:click={handleDeleteAll}
              disabled={deleting}
              class="px-4 py-2.5 min-h-11 text-sm text-red-600 active:bg-red-50 dark:active:bg-red-900/20 disabled:opacity-50 rounded-md transition-colors"
            >
              {deleting ? $_('inbox.deleting') : $_('inbox.deleteAll')}
            </button>
          {/if}
          <button
            on:click={onClose}
            class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
            aria-label={$_('inbox.closeLabel')}
          >
            ✕
          </button>
        </div>
      </div>

      <!-- Content (scrollable) -->
      <div class="p-4 flex-1 overflow-y-auto">
        {#if $inboxCount === 0}
          <div class="text-center py-8">
            <p class="text-lg text-gray-600 dark:text-gray-400">{$_('inbox.empty')}</p>
            <p class="text-sm text-gray-500 dark:text-gray-500 mt-1">{$_('inbox.emptyDescription')}</p>
          </div>
        {:else}
          <div class="space-y-2">
            {#each $inboxItems as item (item.id)}
              <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-3 hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors">
                <div class="flex items-start justify-between gap-4">
                  <div class="flex-1 min-w-0">
                    <h3 class="font-medium text-gray-900 dark:text-gray-100 truncate">
                      {getTitle(item)}
                    </h3>
                    <!-- Content preview as plain text only (security: never render as HTML/Markdown) -->
                    <p class="text-sm text-gray-600 dark:text-gray-400 mt-1 line-clamp-2 whitespace-pre-line">
                      {item.content.length > 200 ? item.content.slice(0, 200) + '...' : item.content}
                    </p>
                    <div class="flex items-center gap-2 mt-2 text-xs text-gray-500 dark:text-gray-400">
                      <span>{getFormattedDate(item.createdAt, { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' })}</span>
                      {#if item.source}
                        <span class="text-gray-400 dark:text-gray-500">·</span>
                        <span>{$_('inbox.source', { values: { source: item.source } })}</span>
                      {/if}
                    </div>
                    {#if item.tags.length > 0}
                      <div class="flex gap-1 mt-2">
                        {#each item.tags as tag}
                          <span class="text-xs px-2 py-0.5 rounded bg-gray-200 dark:bg-gray-600">
                            #{tag}
                          </span>
                        {/each}
                      </div>
                    {/if}
                  </div>

                  <div class="flex gap-3 flex-shrink-0">
                    <button
                      on:click={() => handleAccept(item)}
                      disabled={accepting}
                      class="px-4 py-2.5 min-h-11 text-sm bg-blue-600 active:bg-blue-700 disabled:opacity-50 text-white rounded-md transition-colors whitespace-nowrap"
                    >
                      {$_('inbox.accept')}
                    </button>
                    <button
                      on:click={() => handleDelete(item.id)}
                      disabled={deleting}
                      class="px-4 py-2.5 min-h-11 text-sm bg-red-600 active:bg-red-700 disabled:opacity-50 text-white rounded-md transition-colors whitespace-nowrap"
                    >
                      {$_('inbox.delete')}
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Delete Confirmation Modal -->
  <ConfirmModal
    show={showDeleteConfirm}
    title={$_('inbox.confirm.delete.title')}
    message={$_('inbox.confirm.delete.message')}
    confirmText={$_('inbox.confirm.delete.confirmButton')}
    cancelText={$_('common.cancel')}
    confirmClass="bg-red-600 hover:bg-red-700"
    onConfirm={confirmDelete}
    onCancel={() => showDeleteConfirm = false}
  />

  <!-- Delete All Confirmation Modal -->
  <ConfirmModal
    show={showDeleteAllConfirm}
    title={$_('inbox.confirm.deleteAll.title')}
    message={$_('inbox.confirm.deleteAll.message', { values: { count: $inboxCount } })}
    confirmText={$_('inbox.confirm.deleteAll.confirmButton')}
    cancelText={$_('common.cancel')}
    confirmClass="bg-red-600 hover:bg-red-700"
    onConfirm={confirmDeleteAll}
    onCancel={() => showDeleteAllConfirm = false}
  />
{/if}
