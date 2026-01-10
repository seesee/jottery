<script lang="ts">
  import { get } from 'svelte/store';
  import { _ } from 'svelte-i18n';
  import { conflictService, cryptoService, keyManager, decryptJSON } from '../services';
  import type { ConflictInfo } from '../services/conflictService';
  import { formatDate } from '../utils/dateFormat';
  import { toast } from '../utils/toast.svelte';

  // Helper to get formatted date synchronously
  function getFormattedDate(date: string, options: Intl.DateTimeFormatOptions) {
    return get(formatDate(date, options));
  }

  export let show: boolean = false;
  export let noteId: string | undefined = undefined;
  export let onClose: () => void;
  export let onResolved: () => void;

  let loading = false;
  let resolving = false;
  let conflictInfo: ConflictInfo | null = null;
  let localContent: string = '';
  let localTags: string[] = [];

  async function loadConflict() {
    if (!noteId) return;

    loading = true;
    try {
      conflictInfo = await conflictService.getConflictInfo(noteId);
      if (conflictInfo) {
        // Decrypt local content
        const masterKey = keyManager.getMasterKey();
        if (!masterKey) throw new Error('Application is locked');

        try {
          const encryptedContent = JSON.parse(conflictInfo.localNote.content);
          localContent = await cryptoService.decryptText(encryptedContent, masterKey.key);
        } catch {
          localContent = conflictInfo.localNote.content;
        }

        // Decrypt local tags
        if (conflictInfo.localNote.tags.length > 0 && conflictInfo.localNote.tags[0]) {
          try {
            const encryptedTags = JSON.parse(conflictInfo.localNote.tags[0]);
            localTags = await decryptJSON(encryptedTags, masterKey.key);
          } catch {
            localTags = conflictInfo.localNote.tags;
          }
        }
      }
    } catch (error) {
      console.error('Failed to load conflict:', error);
      toast.error('Failed to load conflict data');
    } finally {
      loading = false;
    }
  }

  async function handleKeepMine() {
    if (!noteId) return;
    resolving = true;
    try {
      await conflictService.resolveKeepMine(noteId);
      toast.success($_('conflict.resolved'));
      onResolved();
      onClose();
    } catch (error) {
      console.error('Failed to resolve conflict:', error);
      toast.error('Failed to resolve conflict');
    } finally {
      resolving = false;
    }
  }

  async function handleKeepServer() {
    if (!noteId) return;
    resolving = true;
    try {
      await conflictService.resolveKeepServer(noteId);
      toast.success($_('conflict.resolved'));
      onResolved();
      onClose();
    } catch (error) {
      console.error('Failed to resolve conflict:', error);
      toast.error('Failed to resolve conflict');
    } finally {
      resolving = false;
    }
  }

  async function handleKeepBoth() {
    if (!noteId) return;
    resolving = true;
    try {
      await conflictService.resolveKeepBoth(noteId);
      toast.success($_('conflict.resolvedBoth'));
      onResolved();
      onClose();
    } catch (error) {
      console.error('Failed to resolve conflict:', error);
      toast.error('Failed to resolve conflict');
    } finally {
      resolving = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
    }
  }

  $: if (show && noteId) {
    loadConflict();
  }

  // Get first line as title
  function getTitle(content: string): string {
    const firstLine = content.split('\n')[0];
    return firstLine.length > 50 ? firstLine.substring(0, 50) + '...' : firstLine;
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if show}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    on:click={onClose}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-6xl max-h-[90vh] flex flex-col mx-4"
      on:click|stopPropagation
      on:wheel|stopPropagation
    >
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
        <div>
          <h2 class="text-xl font-semibold text-gray-900 dark:text-gray-100">
            {$_('conflict.title')}
          </h2>
          {#if conflictInfo}
            <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {getTitle(localContent)}
            </p>
          {/if}
        </div>
        <button
          on:click={onClose}
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700"
          aria-label={$_('common.close')}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-hidden p-6">
        {#if loading}
          <div class="flex items-center justify-center h-full">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
          </div>
        {:else if conflictInfo}
          <!-- Timestamp comparison -->
          <div class="mb-4 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg">
            <div class="flex items-center gap-2 text-amber-700 dark:text-amber-400">
              <svg class="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <span class="text-sm font-medium">{$_('conflict.description')}</span>
            </div>
            <div class="mt-2 grid grid-cols-2 gap-4 text-sm">
              <div>
                <span class="text-gray-600 dark:text-gray-400">{$_('conflict.yourVersion')}:</span>
                <span class="font-medium text-gray-900 dark:text-gray-100 ml-1">
                  {getFormattedDate(conflictInfo.localNote.modifiedAt, { dateStyle: 'medium', timeStyle: 'short' })}
                </span>
              </div>
              <div>
                <span class="text-gray-600 dark:text-gray-400">{$_('conflict.serverVersion')}:</span>
                <span class="font-medium text-gray-900 dark:text-gray-100 ml-1">
                  {getFormattedDate(conflictInfo.serverModifiedAt, { dateStyle: 'medium', timeStyle: 'short' })}
                </span>
              </div>
            </div>
          </div>

          <!-- Side-by-side comparison -->
          <div class="grid grid-cols-2 gap-4 h-[calc(100%-8rem)]">
            <!-- Local version -->
            <div class="flex flex-col border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
              <div class="bg-blue-50 dark:bg-blue-900/20 px-4 py-2 border-b border-gray-200 dark:border-gray-700">
                <h3 class="font-medium text-blue-700 dark:text-blue-400">{$_('conflict.yourVersion')}</h3>
                {#if localTags.length > 0}
                  <div class="flex flex-wrap gap-1 mt-1">
                    {#each localTags as tag}
                      <span class="px-2 py-0.5 text-xs bg-blue-100 dark:bg-blue-800 text-blue-700 dark:text-blue-300 rounded">#{tag}</span>
                    {/each}
                  </div>
                {/if}
              </div>
              <div class="flex-1 overflow-auto p-4 font-mono text-sm whitespace-pre-wrap text-gray-700 dark:text-gray-300">
                {localContent}
              </div>
            </div>

            <!-- Server version -->
            <div class="flex flex-col border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
              <div class="bg-purple-50 dark:bg-purple-900/20 px-4 py-2 border-b border-gray-200 dark:border-gray-700">
                <h3 class="font-medium text-purple-700 dark:text-purple-400">{$_('conflict.serverVersion')}</h3>
                {#if conflictInfo.serverTags.length > 0}
                  <div class="flex flex-wrap gap-1 mt-1">
                    {#each conflictInfo.serverTags as tag}
                      <span class="px-2 py-0.5 text-xs bg-purple-100 dark:bg-purple-800 text-purple-700 dark:text-purple-300 rounded">#{tag}</span>
                    {/each}
                  </div>
                {/if}
              </div>
              <div class="flex-1 overflow-auto p-4 font-mono text-sm whitespace-pre-wrap text-gray-700 dark:text-gray-300">
                {conflictInfo.serverContent}
              </div>
            </div>
          </div>
        {:else}
          <div class="flex items-center justify-center h-full text-gray-500">
            {$_('conflict.noConflict')}
          </div>
        {/if}
      </div>

      <!-- Footer with actions -->
      <div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <div class="flex flex-wrap items-center justify-between gap-4">
          <p class="text-sm text-gray-600 dark:text-gray-400">
            {$_('conflict.chooseAction')}
          </p>
          <div class="flex flex-wrap gap-2">
            <button
              on:click={handleKeepMine}
              disabled={resolving || !conflictInfo}
              class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-sm font-medium"
            >
              {$_('conflict.keepMine')}
            </button>
            <button
              on:click={handleKeepServer}
              disabled={resolving || !conflictInfo}
              class="px-4 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-sm font-medium"
            >
              {$_('conflict.keepServer')}
            </button>
            <button
              on:click={handleKeepBoth}
              disabled={resolving || !conflictInfo}
              class="px-4 py-2 bg-gray-600 text-white rounded-md hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-sm font-medium"
            >
              {$_('conflict.keepBoth')}
            </button>
            <button
              on:click={onClose}
              disabled={resolving}
              class="px-4 py-2 text-gray-700 dark:text-gray-300 bg-gray-200 dark:bg-gray-700 rounded-md hover:bg-gray-300 dark:hover:bg-gray-600 disabled:opacity-50 transition-colors text-sm font-medium"
            >
              {$_('common.cancel')}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}
