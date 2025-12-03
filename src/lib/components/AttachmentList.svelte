<script lang="ts">
  import type { Attachment } from '../types';
  import { attachmentService } from '../services';
  import { onMount } from 'svelte';

  export let attachments: Attachment[] = [];
  export let onDelete: (attachment: Attachment) => void;
  export let readonly: boolean = false;

  // Thumbnail cache
  let thumbnails: Map<string, string | null> = new Map();

  // Load thumbnails for image attachments
  async function loadThumbnails() {
    for (const attachment of attachments) {
      if (attachmentService.supportsThumbnail(attachment.mimeType)) {
        try {
          const thumbnailUrl = await attachmentService.getThumbnailData(attachment);
          thumbnails.set(attachment.id, thumbnailUrl);
          thumbnails = thumbnails; // Trigger reactivity
        } catch (error) {
          console.error('Failed to load thumbnail:', error);
        }
      }
    }
  }

  // Download attachment
  async function handleDownload(attachment: Attachment) {
    try {
      await attachmentService.downloadAttachment(attachment);
    } catch (error) {
      console.error('Failed to download attachment:', error);
      alert(`Failed to download attachment: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  // Delete attachment
  function handleDelete(attachment: Attachment) {
    if (readonly) return;
    onDelete(attachment);
  }

  // Get file icon based on MIME type
  function getFileIcon(mimeType: string): string {
    if (mimeType.startsWith('image/')) return '🖼️';
    if (mimeType.startsWith('video/')) return '🎥';
    if (mimeType.startsWith('audio/')) return '🎵';
    if (mimeType.includes('pdf')) return '📄';
    if (mimeType.includes('zip') || mimeType.includes('tar') || mimeType.includes('gz')) return '📦';
    if (mimeType.includes('text/')) return '📝';
    return '📎';
  }

  // Get decrypted filename
  async function getFilename(attachment: Attachment): Promise<string> {
    try {
      return await attachmentService.getDecryptedFilename(attachment);
    } catch (error) {
      return 'attachment';
    }
  }

  // Load filenames
  let filenames: Map<string, string> = new Map();
  async function loadFilenames() {
    for (const attachment of attachments) {
      const name = await getFilename(attachment);
      filenames.set(attachment.id, name);
      filenames = filenames; // Trigger reactivity
    }
  }

  onMount(() => {
    loadThumbnails();
    loadFilenames();
  });

  // Reload when attachments change
  $: if (attachments) {
    loadThumbnails();
    loadFilenames();
  }
</script>

{#if attachments.length > 0}
  <div class="space-y-2">
    {#each attachments as attachment (attachment.id)}
      <div
        class="flex items-center gap-3 p-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
      >
        <!-- Thumbnail or icon -->
        <div class="flex-shrink-0 w-12 h-12 flex items-center justify-center bg-white dark:bg-gray-900 rounded border border-gray-200 dark:border-gray-600 overflow-hidden">
          {#if thumbnails.get(attachment.id)}
            <img
              src={thumbnails.get(attachment.id)}
              alt="Thumbnail"
              class="w-full h-full object-cover"
            />
          {:else}
            <span class="text-2xl">{getFileIcon(attachment.mimeType)}</span>
          {/if}
        </div>

        <!-- File info -->
        <div class="flex-1 min-w-0">
          <div class="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
            {filenames.get(attachment.id) || 'Loading...'}
          </div>
          <div class="text-xs text-gray-500 dark:text-gray-400">
            {attachmentService.formatFileSize(attachment.size)}
          </div>
        </div>

        <!-- Actions -->
        <div class="flex gap-1">
          <button
            on:click={() => handleDownload(attachment)}
            class="px-2 py-1 text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-200 rounded hover:bg-blue-200 dark:hover:bg-blue-800 transition-colors"
            title="Download"
          >
            ⬇️ Download
          </button>
          {#if !readonly}
            <button
              on:click={() => handleDelete(attachment)}
              class="px-2 py-1 text-xs bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-200 rounded hover:bg-red-200 dark:hover:bg-red-800 transition-colors"
              title="Delete"
            >
              🗑️
            </button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{:else}
  <div class="text-sm text-gray-500 dark:text-gray-400 text-center py-2">
    No attachments
  </div>
{/if}
