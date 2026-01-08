<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Attachment } from '../types';
  import { attachmentService } from '../services';
  import { onMount } from 'svelte';
  import { toast } from '../utils/toast.svelte';
  import AttachmentPreviewModal from './AttachmentPreviewModal.svelte';

  export let attachments: Attachment[] = [];
  export let onDelete: (attachment: Attachment) => void;
  export let readonly: boolean = false;

  // Thumbnail cache
  let thumbnails: Map<string, string | null> = new Map();

  // Preview state
  let previewAttachment: Attachment | null = null;

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

  // Check if attachment can be previewed (for UI hints)
  function canPreview(mimeType: string): boolean {
    if (mimeType.startsWith('image/')) return true;
    if (mimeType.startsWith('text/')) return true;
    if (mimeType.includes('json')) return true;
    if (mimeType.includes('javascript')) return true;
    if (mimeType.includes('xml')) return true;
    if (mimeType.includes('pdf')) return true;
    if (mimeType.startsWith('audio/')) return true;
    if (mimeType.startsWith('video/')) return true;
    return false;
  }

  // Show preview
  function handlePreview(attachment: Attachment) {
    previewAttachment = attachment;
  }

  // Close preview
  function closePreview() {
    previewAttachment = null;
  }

  // Download attachment
  async function handleDownload(attachment: Attachment) {
    try {
      await attachmentService.downloadAttachment(attachment);
    } catch (error) {
      console.error('Failed to download attachment:', error);
      toast.error(`${$_('attachments.error.download')}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  // Delete attachment
  function handleDelete(attachment: Attachment) {
    if (readonly) return;
    onDelete(attachment);
  }

  // Copy markdown link to clipboard
  async function handleCopyMarkdown(attachment: Attachment) {
    const filename = filenames.get(attachment.id) || $_('attachments.attachment');

    // Use image syntax for images, regular link for others
    const markdown = attachment.mimeType.startsWith('image/')
      ? `![${filename}](attachment:${filename})`
      : `[Attachment: ${filename}](attachment:${filename})`;

    try {
      await navigator.clipboard.writeText(markdown);
      toast.success($_('attachments.markdownCopied'));
    } catch (error) {
      console.error('Failed to copy markdown:', error);
      toast.error($_('attachments.error.copyMarkdown'));
    }
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
      return $_('attachments.attachment');
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
        class="attachment-item flex items-center gap-3 p-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
      >
        <!-- Thumbnail or icon - clickable for preview -->
        <button
          on:click={() => handlePreview(attachment)}
          class="attachment-thumbnail flex-shrink-0 w-12 h-12 flex items-center justify-center bg-white dark:bg-gray-900 rounded border border-gray-200 dark:border-gray-600 overflow-hidden hover:border-blue-400 dark:hover:border-blue-500 transition-colors cursor-pointer"
          title={$_('attachments.clickToPreview')}
        >
          {#if thumbnails.get(attachment.id)}
            <img
              src={thumbnails.get(attachment.id)}
              alt={$_('attachments.thumbnail')}
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
            {filenames.get(attachment.id) || $_('attachments.loading')}
          </div>
          <div class="text-xs text-gray-500 dark:text-gray-400">
            {attachmentService.formatFileSize(attachment.size)}
            {#if canPreview(attachment.mimeType)}
              <span class="ml-2 text-blue-600 dark:text-blue-400">{$_('attachments.clickToPreviewHint')}</span>
            {/if}
          </div>
        </button>

        <!-- Actions -->
        <div class="attachment-actions flex gap-1">
          <button
            on:click={() => handleDownload(attachment)}
            class="attachment-button px-2 py-1 text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-200 rounded hover:bg-blue-200 dark:hover:bg-blue-800 transition-colors"
            title={$_('attachments.download')}
          >
            ⬇️
          </button>
          <button
            on:click={() => handleCopyMarkdown(attachment)}
            class="attachment-button px-2 py-1 text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-200 rounded hover:bg-green-200 dark:hover:bg-green-800 transition-colors"
            title={$_('attachments.copyMarkdown')}
          >
            📋
          </button>
          {#if !readonly}
            <button
              on:click={() => handleDelete(attachment)}
              class="attachment-button px-2 py-1 text-xs bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-200 rounded hover:bg-red-200 dark:hover:bg-red-800 transition-colors"
              title={$_('attachments.delete')}
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
    {$_('attachments.noAttachments')}
  </div>
{/if}

<!-- Preview Modal -->
<AttachmentPreviewModal
  show={previewAttachment !== null}
  attachment={previewAttachment}
  filename={previewAttachment ? filenames.get(previewAttachment.id) : null}
  onClose={closePreview}
  onDownload={handleDownload}
/>

<style>
  /* Mobile improvements for better touch targets and spacing */
  @media (max-width: 768px) {
    .attachment-item {
      padding: 12px;
      gap: 12px;
      min-height: 72px;
    }

    .attachment-thumbnail {
      min-width: 48px;
      min-height: 48px;
      width: 48px;
      height: 48px;
    }

    .attachment-actions {
      gap: 8px;
      flex-wrap: wrap;
    }

    .attachment-button {
      min-width: 44px;
      min-height: 44px;
      padding: 8px 12px;
      display: flex;
      align-items: center;
      justify-content: center;
    }
  }
</style>
