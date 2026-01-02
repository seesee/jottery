<script lang="ts">
  import type { Attachment } from '../types';
  import { attachmentService } from '../services';
  import { onMount } from 'svelte';
  import { toast } from '../utils/toast.svelte';

  export let attachments: Attachment[] = [];
  export let onDelete: (attachment: Attachment) => void;
  export let readonly: boolean = false;

  // Thumbnail cache
  let thumbnails: Map<string, string | null> = new Map();

  // Preview state
  let previewAttachment: Attachment | null = null;
  let previewContent: string | null = null;
  let previewType: 'image' | 'text' | 'pdf' | 'audio' | 'video' | 'unsupported' | null = null;
  let isLoadingPreview = false;

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

  // Check if attachment can be previewed
  function canPreview(mimeType: string): boolean {
    // Images
    if (mimeType.startsWith('image/')) return true;

    // Text-based files
    if (mimeType.startsWith('text/')) return true;
    if (mimeType.includes('json')) return true;
    if (mimeType.includes('javascript')) return true;
    if (mimeType.includes('xml')) return true;

    // PDF
    if (mimeType.includes('pdf')) return true;

    // Audio
    if (mimeType.startsWith('audio/')) return true;

    // Video
    if (mimeType.startsWith('video/')) return true;

    return false;
  }

  // Get preview type for attachment
  function getPreviewType(mimeType: string): typeof previewType {
    if (mimeType.startsWith('image/')) return 'image';
    if (mimeType.startsWith('audio/')) return 'audio';
    if (mimeType.startsWith('video/')) return 'video';
    if (mimeType.includes('pdf')) return 'pdf';
    if (mimeType.startsWith('text/') ||
        mimeType.includes('json') ||
        mimeType.includes('javascript') ||
        mimeType.includes('xml')) return 'text';
    return 'unsupported';
  }

  // Show preview
  async function handlePreview(attachment: Attachment) {
    if (!canPreview(attachment.mimeType)) {
      // If can't preview, just download
      handleDownload(attachment);
      return;
    }

    isLoadingPreview = true;
    previewAttachment = attachment;
    previewType = getPreviewType(attachment.mimeType);
    previewContent = null;

    try {
      // Get the decrypted data (returns Blob)
      const blob = await attachmentService.getAttachmentData(attachment);

      if (previewType === 'image' || previewType === 'audio' || previewType === 'video' || previewType === 'pdf') {
        // Create blob URL for media files
        previewContent = URL.createObjectURL(blob);
      } else if (previewType === 'text') {
        // Convert blob to text for text files
        const arrayBuffer = await blob.arrayBuffer();
        const text = new TextDecoder().decode(arrayBuffer);
        previewContent = text;
      }
    } catch (error) {
      console.error('Failed to load preview:', error);
      previewType = 'unsupported';
    } finally {
      isLoadingPreview = false;
    }
  }

  // Close preview
  function closePreview() {
    // Revoke blob URLs to free memory
    if (previewContent && (previewType === 'image' || previewType === 'audio' || previewType === 'video' || previewType === 'pdf')) {
      URL.revokeObjectURL(previewContent);
    }

    previewAttachment = null;
    previewContent = null;
    previewType = null;
  }

  // Download attachment
  async function handleDownload(attachment: Attachment) {
    try {
      await attachmentService.downloadAttachment(attachment);
    } catch (error) {
      console.error('Failed to download attachment:', error);
      toast.error(`Failed to download attachment: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  // Download from preview
  async function handleDownloadFromPreview() {
    if (previewAttachment) {
      await handleDownload(previewAttachment);
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
        <!-- Thumbnail or icon - clickable for preview -->
        <button
          on:click={() => handlePreview(attachment)}
          class="flex-shrink-0 w-12 h-12 flex items-center justify-center bg-white dark:bg-gray-900 rounded border border-gray-200 dark:border-gray-600 overflow-hidden hover:border-blue-400 dark:hover:border-blue-500 transition-colors cursor-pointer"
          title="Click to preview"
        >
          {#if thumbnails.get(attachment.id)}
            <img
              src={thumbnails.get(attachment.id)}
              alt="Thumbnail"
              class="w-full h-full object-cover"
            />
          {:else}
            <span class="text-2xl">{getFileIcon(attachment.mimeType)}</span>
          {/if}
        </button>

        <!-- File info - clickable for preview -->
        <button
          on:click={() => handlePreview(attachment)}
          class="flex-1 min-w-0 text-left hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
        >
          <div class="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
            {filenames.get(attachment.id) || 'Loading...'}
          </div>
          <div class="text-xs text-gray-500 dark:text-gray-400">
            {attachmentService.formatFileSize(attachment.size)}
            {#if canPreview(attachment.mimeType)}
              <span class="ml-2 text-blue-600 dark:text-blue-400">• Click to preview</span>
            {/if}
          </div>
        </button>

        <!-- Actions -->
        <div class="flex gap-1">
          <button
            on:click={() => handleDownload(attachment)}
            class="px-2 py-1 text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-200 rounded hover:bg-blue-200 dark:hover:bg-blue-800 transition-colors"
            title="Download"
          >
            ⬇️
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

<!-- Preview Modal -->
{#if previewAttachment}
  <div
    class="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
    on:click={closePreview}
    on:keydown={(e) => e.key === 'Enter' && closePreview()}
    role="button"
    tabindex="-1"
    aria-label="Close preview"
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-5xl w-full max-h-[90vh] flex flex-col"
      on:click|stopPropagation
      on:keydown|stopPropagation
      role="dialog"
      aria-modal="true"
      tabindex="0"
    >
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <div class="flex-1 min-w-0">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 truncate">
            {filenames.get(previewAttachment.id) || 'Attachment Preview'}
          </h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {attachmentService.formatFileSize(previewAttachment.size)}
          </p>
        </div>
        <div class="flex gap-2 ml-4">
          <button
            on:click={handleDownloadFromPreview}
            class="px-3 py-1.5 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors text-sm"
          >
            ⬇️ Download
          </button>
          <button
            on:click={closePreview}
            class="p-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
            aria-label="Close"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-auto {previewType === 'image' ? '' : 'p-4'}">
        {#if isLoadingPreview}
          <div class="flex items-center justify-center h-full">
            <div class="text-gray-500 dark:text-gray-400">Loading preview...</div>
          </div>
        {:else if previewType === 'image' && previewContent}
          <div class="w-full h-full flex items-center justify-center p-4">
            <img
              src={previewContent}
              alt={filenames.get(previewAttachment.id)}
              class="max-w-full max-h-full object-contain"
              style="max-height: calc(90vh - 120px);"
            />
          </div>
        {:else if previewType === 'text' && previewContent}
          <pre class="text-sm bg-gray-50 dark:bg-gray-900 p-4 rounded overflow-auto max-h-full"><code class="text-gray-900 dark:text-gray-100">{previewContent}</code></pre>
        {:else if previewType === 'pdf' && previewContent}
          <iframe
            src={previewContent}
            class="w-full h-full min-h-[60vh] border-0"
            title="PDF Preview"
          ></iframe>
        {:else if previewType === 'audio' && previewContent}
          <div class="flex items-center justify-center h-full">
            <audio controls class="w-full max-w-xl">
              <source src={previewContent} type={previewAttachment.mimeType} />
              Your browser does not support audio playback.
            </audio>
          </div>
        {:else if previewType === 'video' && previewContent}
          <div class="flex items-center justify-center h-full">
            <video controls class="max-w-full max-h-full" aria-label="Video preview - captions not available for user-uploaded content">
              <source src={previewContent} type={previewAttachment.mimeType} />
              <track kind="captions" />
              Your browser does not support video playback.
            </video>
          </div>
        {:else}
          <div class="flex items-center justify-center h-full">
            <div class="text-center text-gray-500 dark:text-gray-400">
              <p class="mb-4">Preview not available for this file type.</p>
              <button
                on:click={handleDownloadFromPreview}
                class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
              >
                Download to view
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
