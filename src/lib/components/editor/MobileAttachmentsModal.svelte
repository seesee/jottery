<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Attachment } from '../../types';
  import AttachmentList from '../AttachmentList.svelte';
  import FileUpload from '../FileUpload.svelte';

  // State props
  export let show: boolean;
  export let forceMobileLayout: boolean;
  export let attachments: Attachment[];
  export let isUploading: boolean;

  // Callbacks
  export let onClose: () => void;
  export let onDelete: (attachment: Attachment) => void;
  export let onFileUpload: (files: FileList) => void;
</script>

<!-- Attachments Modal (Mobile) -->
{#if show && forceMobileLayout}
  <div
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
    on:click={onClose}
    on:keydown={(e) => e.key === 'Enter' && onClose()}
    role="button"
    tabindex="-1"
    aria-label="Close attachments"
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl max-h-[80vh] flex flex-col"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="0"
    >
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {$_('editor.attachments')} ({attachments.length})
        </h2>
        <button
          on:click={onClose}
          class="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded transition-colors"
          aria-label={$_('common.close')}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-4">
        {#if attachments.length > 0}
          <AttachmentList
            {attachments}
            readonly={false}
            onDelete={onDelete}
          />
        {:else}
          <div class="text-center text-gray-500 dark:text-gray-400 py-8">
            {$_('attachments.noAttachments')}
          </div>
        {/if}
      </div>

      <!-- Footer with Add Button -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4">
        <FileUpload
          onUpload={onFileUpload}
          disabled={isUploading}
        />
      </div>
    </div>
  </div>
{/if}
