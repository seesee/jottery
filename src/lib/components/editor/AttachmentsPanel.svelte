<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Attachment } from '../../types';
  import AttachmentList from '../AttachmentList.svelte';
  import FileUpload from '../FileUpload.svelte';

  // State props
  export let attachments: Attachment[];
  export let isExpanded: boolean;
  export let isDraggingFile: boolean;
  export let isUploading: boolean;
  export let backgroundColor: string | undefined = undefined;

  // Callbacks
  export let onToggleExpanded: () => void;
  export let onDelete: (attachment: Attachment) => void;
  export let onFileUpload: (files: FileList) => void;
</script>

<!-- Attachments Section -->
<div
  class="border-t border-gray-200 dark:border-gray-700 {backgroundColor ? '' : 'bg-gray-50 dark:bg-gray-800/50'}"
  style:background-color={backgroundColor}
>
  <!-- Header - Always visible -->
  <button
    on:click={onToggleExpanded}
    class="w-full px-3 py-2 flex items-center justify-between text-sm font-medium text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 transition-colors"
  >
    <span>
      📎 {$_('editor.attachmentsHeader', { values: { count: attachments.length } })}
    </span>
    <span class="text-xs transform transition-transform {isExpanded ? 'rotate-180' : ''}">
      ▼
    </span>
  </button>

  <!-- Expanded content or drop zone -->
  {#if isExpanded || isDraggingFile}
    <div class="px-3 pb-3 max-h-64 overflow-y-auto space-y-3">
      <!-- File Upload - Show when expanded or dragging -->
      <FileUpload
        onUpload={onFileUpload}
        disabled={isUploading}
      />

      <!-- Attachment List -->
      {#if attachments.length > 0}
        <AttachmentList
          {attachments}
          onDelete={onDelete}
          readonly={false}
        />
      {/if}

      {#if isUploading}
        <div class="text-sm text-blue-600 dark:text-blue-400">
          {$_('editor.uploading')}
        </div>
      {/if}
    </div>
  {:else if attachments.length > 0}
    <!-- Collapsed view - show attachment count -->
    <div class="px-3 pb-2 text-xs text-gray-500 dark:text-gray-400">
      {$_(isExpanded ? 'attachments.clickToHide' : 'attachments.clickToView')}
    </div>
  {/if}
</div>
