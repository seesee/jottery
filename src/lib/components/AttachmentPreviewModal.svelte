<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { attachmentService } from '../services/attachmentService';
  import PdfViewer from './PdfViewer.svelte';
  import type { Attachment } from '../types';
  import { modal, createBackdropHandler } from '../actions';

  export let show: boolean = false;
  export let attachment: Attachment | null = null;
  export let filename: string | null = null;
  export let onClose: () => void;
  export let onDownload: (attachment: Attachment) => Promise<void>;

  type PreviewType = 'image' | 'text' | 'pdf' | 'audio' | 'video' | 'unsupported' | null;

  let previewContent: string | null = null;
  let previewType: PreviewType = null;
  let isLoading = false;

  // Check if attachment can be previewed
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

  // Get preview type for attachment
  function getPreviewType(mimeType: string): PreviewType {
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

  // Load preview when attachment changes
  $: if (show && attachment) {
    loadPreview(attachment);
  }

  async function loadPreview(att: Attachment) {
    if (!canPreview(att.mimeType)) {
      previewType = 'unsupported';
      return;
    }

    isLoading = true;
    previewType = getPreviewType(att.mimeType);
    previewContent = null;

    try {
      // Get the decrypted data (returns Blob)
      const blob = await attachmentService.getAttachmentData(att);

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
      isLoading = false;
    }
  }

  function handleClose() {
    // Revoke blob URLs to free memory
    if (previewContent && (previewType === 'image' || previewType === 'audio' || previewType === 'video' || previewType === 'pdf')) {
      URL.revokeObjectURL(previewContent);
    }

    previewContent = null;
    previewType = null;
    onClose();
  }

  async function handleDownload() {
    if (attachment) {
      await onDownload(attachment);
    }
  }

  $: backdropHandler = createBackdropHandler(handleClose);

  $: displayFilename = filename || attachment?.filename || $_('attachments.preview.title');
</script>

{#if show && attachment}
  <div
    class="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
    on:click={backdropHandler}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: handleClose }}
  >
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-5xl w-full max-h-[90vh] flex flex-col"
      on:click|stopPropagation
      role="document"
    >
      <!-- Header -->
      <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <div class="flex-1 min-w-0">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100 truncate">
            {displayFilename}
          </h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {attachmentService.formatFileSize(attachment.size)}
          </p>
        </div>
        <div class="flex gap-2 ml-4">
          <button
            on:click={handleDownload}
            class="px-3 py-1.5 bg-blue-600 text-white rounded-md active:bg-blue-700 transition-colors text-sm"
          >
            {$_('attachments.download')}
          </button>
          <button
            on:click={handleClose}
            class="p-1.5 active:bg-gray-100 dark:active:bg-gray-700 rounded"
            aria-label={$_('attachments.close')}
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-auto {previewType === 'image' ? '' : 'p-4'}">
        {#if isLoading}
          <div class="flex items-center justify-center h-full">
            <div class="text-gray-500 dark:text-gray-400">{$_('attachments.preview.loading')}</div>
          </div>
        {:else if previewType === 'image' && previewContent}
          <div class="w-full h-full flex items-center justify-center p-4">
            <img
              src={previewContent}
              alt={displayFilename}
              class="max-w-full max-h-full object-contain"
              style="max-height: calc(90vh - 120px);"
            />
          </div>
        {:else if previewType === 'text' && previewContent}
          <pre class="text-sm bg-gray-50 dark:bg-gray-900 p-4 rounded overflow-auto max-h-full"><code class="text-gray-900 dark:text-gray-100">{previewContent}</code></pre>
        {:else if previewType === 'pdf' && previewContent}
          <PdfViewer pdfUrl={previewContent} />
        {:else if previewType === 'audio' && previewContent}
          <div class="flex items-center justify-center h-full">
            <audio controls class="w-full max-w-xl">
              <source src={previewContent} type={attachment.mimeType} />
              {$_('attachments.preview.audioNotSupported')}
            </audio>
          </div>
        {:else if previewType === 'video' && previewContent}
          <div class="flex items-center justify-center h-full">
            <video controls class="max-w-full max-h-full" aria-label="Video preview - captions not available for user-uploaded content">
              <source src={previewContent} type={attachment.mimeType} />
              <track kind="captions" />
              {$_('attachments.preview.videoNotSupported')}
            </video>
          </div>
        {:else}
          <div class="flex items-center justify-center h-full">
            <div class="text-center text-gray-500 dark:text-gray-400">
              <p class="mb-4">{$_('attachments.preview.notAvailable')}</p>
              <button
                on:click={handleDownload}
                class="px-4 py-2 bg-blue-600 text-white rounded-md active:bg-blue-700 transition-colors"
              >
                {$_('attachments.downloadToView')}
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
