<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Readable } from 'svelte/store';

  // State props
  export let show: boolean;
  export let noteStats: {
    characters: number;
    charactersNoSpaces: number;
    words: number;
    lines: number;
    tags: number;
    attachments: number;
  };
  export let language: string;
  export let noteId: string;
  export let noteVersion: number;
  export let createdAtFormatted: Readable<string> | null;
  export let modifiedAtFormatted: Readable<string> | null;

  // Callbacks
  export let onClose: () => void;
</script>

<!-- Info Modal -->
{#if show}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    on:click={onClose}
    on:keydown={(e) => e.key === 'Enter' && onClose()}
    role="button"
    tabindex="-1"
    aria-label="Close modal"
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="0"
    >
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-xl font-semibold text-gray-900 dark:text-gray-100">{$_('editor.noteInformation')}</h2>
        <button
          on:click={onClose}
          class="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
          aria-label={$_('common.close')}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div class="space-y-3 text-sm">
        <div class="grid grid-cols-2 gap-2">
          <div class="text-gray-600 dark:text-gray-400">{$_('editor.characters')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.characters.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.charactersNoSpaces')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.charactersNoSpaces.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.words')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.words.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.lines')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.lines.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.tags')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.tags}</div>

          <div class="text-gray-600 dark:text-gray-400">{$_('editor.attachmentsCount')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.attachments}</div>
        </div>

        <div class="border-t border-gray-200 dark:border-gray-700 pt-3 mt-3">
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('editor.syntax')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100 capitalize">{language.replace('-', ' ')}</div>
        </div>

        <div class="border-t border-gray-200 dark:border-gray-700 pt-3 mt-3">
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('note.created')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{createdAtFormatted ? $createdAtFormatted : ''}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('note.modified')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{modifiedAtFormatted ? $modifiedAtFormatted : ''}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('versionHistory.version')}</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">v{noteVersion}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">{$_('editor.noteId')}</div>
          <div class="font-mono text-xs text-gray-900 dark:text-gray-100 break-all">{noteId}</div>
        </div>
      </div>

      <div class="mt-6 flex justify-end">
        <button
          on:click={onClose}
          class="px-4 py-2 min-h-11 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
        >
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
{/if}
