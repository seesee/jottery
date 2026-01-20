/**
 * Attachment Preview Resolver
 *
 * Handles resolving attachment references in preview HTML to blob URLs.
 * Extracted from EditorPane for better testability and reusability.
 */

import type { Attachment } from '../types';
import { attachmentService, cryptoService, keyManager } from '../services';

export interface AttachmentResolverOptions {
  /** Container element to search for attachment references */
  container: Element;
  /** List of attachments available for resolution */
  attachments: Attachment[];
  /** Set of blob URLs to track for cleanup */
  blobUrls: Set<string>;
  /** Callback when an attachment download link is clicked */
  onPreviewAttachment?: (attachment: Attachment) => void;
  /** i18n function for error messages */
  getErrorMessage?: () => string;
}

/**
 * Resolve all attachment references in a preview container.
 * Processes images by ID, images by filename, and download links.
 */
export async function resolveAttachmentPreviews(options: AttachmentResolverOptions): Promise<void> {
  const { container, attachments, blobUrls, onPreviewAttachment, getErrorMessage } = options;

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) return;

  await Promise.all([
    resolveImagesById(container, attachments, blobUrls),
    resolveImagesByFilename(container, attachments, blobUrls, masterKey.key),
    resolveDownloadLinksById(container, attachments, onPreviewAttachment),
    resolveDownloadLinksByFilename(container, attachments, onPreviewAttachment, masterKey.key, getErrorMessage),
  ]);
}

/**
 * Resolve images that have data-attachment-id attributes
 */
async function resolveImagesById(
  container: Element,
  attachments: Attachment[],
  blobUrls: Set<string>
): Promise<void> {
  const images = container.querySelectorAll('img[data-attachment-id]');

  for (const img of images) {
    const attachmentId = img.getAttribute('data-attachment-id');
    if (!attachmentId) continue;

    // Skip if already loaded
    if (img.getAttribute('data-loaded') === 'true') continue;

    try {
      // Find the attachment object to get decrypted data
      const attachment = attachments.find(a => a.data === attachmentId);
      if (attachment) {
        // Load and decrypt the blob using attachmentService
        const blob = await attachmentService.getAttachmentData(attachment);
        const blobUrl = URL.createObjectURL(blob);
        img.setAttribute('src', blobUrl);
        img.setAttribute('data-loaded', 'true');

        // Track blob URL for cleanup
        blobUrls.add(blobUrl);
      } else {
        console.error(`Failed to find attachment with data: ${attachmentId}`);
        img.setAttribute('alt', '[Attachment not found]');
        img.setAttribute('data-loaded', 'true');
      }
    } catch (error) {
      console.error(`Failed to load attachment ${attachmentId}:`, error);
      img.setAttribute('alt', '[Failed to load image]');
      img.setAttribute('data-loaded', 'true');
    }
  }
}

/**
 * Resolve images that have data-attachment-filename attributes (need decryption)
 */
async function resolveImagesByFilename(
  container: Element,
  attachments: Attachment[],
  blobUrls: Set<string>,
  cryptoKey: CryptoKey
): Promise<void> {
  const images = container.querySelectorAll('img[data-attachment-filename]');

  for (const img of images) {
    const filename = img.getAttribute('data-attachment-filename');
    if (!filename) continue;

    // Skip if already loaded
    if (img.getAttribute('data-loaded') === 'true') continue;

    try {
      // Find attachment by decrypted filename
      const foundAttachment = await findAttachmentByFilename(attachments, filename, cryptoKey);

      if (foundAttachment) {
        // Load and decrypt the blob using attachmentService
        const blob = await attachmentService.getAttachmentData(foundAttachment);
        const blobUrl = URL.createObjectURL(blob);
        img.setAttribute('src', blobUrl);
        img.setAttribute('data-loaded', 'true');

        // Track blob URL for cleanup
        blobUrls.add(blobUrl);
      } else {
        console.error(`[Preview] No attachment found with filename: ${filename}`);
        img.setAttribute('alt', `[Attachment not found: ${filename}]`);
        img.setAttribute('data-loaded', 'true');
      }
    } catch (error) {
      console.error(`Failed to load attachment by filename ${filename}:`, error);
      img.setAttribute('alt', '[Failed to load image]');
      img.setAttribute('data-loaded', 'true');
    }
  }
}

/**
 * Resolve download links that have data-attachment-id attributes
 */
async function resolveDownloadLinksById(
  container: Element,
  attachments: Attachment[],
  onPreviewAttachment?: (attachment: Attachment) => void
): Promise<void> {
  const downloads = container.querySelectorAll('.attachment-download[data-attachment-id]');

  for (const div of downloads) {
    const attachmentId = div.getAttribute('data-attachment-id');
    if (!attachmentId) continue;

    // Skip if already processed
    if (div.classList.contains('clickable')) continue;

    const htmlDiv = div as HTMLElement;
    htmlDiv.classList.add('clickable');
    htmlDiv.style.cursor = 'pointer';

    div.addEventListener('click', async () => {
      try {
        const attachment = attachments.find(a => a.id === attachmentId);
        if (attachment && onPreviewAttachment) {
          onPreviewAttachment(attachment);
        }
      } catch (error) {
        console.error(`Failed to preview attachment ${attachmentId}:`, error);
      }
    });
  }
}

/**
 * Resolve download links that have data-attachment-filename attributes (need decryption)
 */
async function resolveDownloadLinksByFilename(
  container: Element,
  attachments: Attachment[],
  onPreviewAttachment: ((attachment: Attachment) => void) | undefined,
  cryptoKey: CryptoKey,
  getErrorMessage?: () => string
): Promise<void> {
  const downloads = container.querySelectorAll('.attachment-download[data-attachment-filename]');

  for (const div of downloads) {
    const filename = div.getAttribute('data-attachment-filename');
    if (!filename) continue;

    // Skip if already loaded
    if (div.getAttribute('data-loaded') === 'true') continue;

    try {
      // Find attachment by decrypted filename
      const foundAttachment = await findAttachmentByFilename(attachments, filename, cryptoKey);

      if (foundAttachment) {
        // Update the div with the attachment ID and mark as loaded
        div.setAttribute('data-attachment-id', foundAttachment.id);
        div.setAttribute('data-attachment-data', foundAttachment.data);
        div.setAttribute('data-loaded', 'true');
        div.removeAttribute('data-attachment-filename');

        // Skip if already has click handler
        if (div.classList.contains('clickable')) continue;

        div.classList.add('clickable');

        div.addEventListener('click', async () => {
          try {
            if (onPreviewAttachment) {
              onPreviewAttachment(foundAttachment);
            }
          } catch (error) {
            console.error(`Failed to preview attachment ${foundAttachment.id}:`, error);
          }
        });
      } else {
        // Attachment not found - show error message
        console.error(`No attachment found with filename: ${filename}`);
        div.setAttribute('data-loaded', 'true');
        div.removeAttribute('data-attachment-filename');

        // Update content to show error
        div.innerHTML = `
          <span class="text-2xl mr-2">⚠️</span>
          <div class="flex-1">
            <span class="font-medium text-red-700 dark:text-red-400">Attachment not found</span>
            <span class="ml-2 text-sm text-gray-600 dark:text-gray-400">${escapeHtml(filename)}</span>
          </div>
        `;

        // Update styling to show error state
        div.classList.remove('bg-gray-50', 'dark:bg-gray-800', 'hover:bg-gray-100', 'dark:hover:bg-gray-700', 'cursor-pointer', 'border-gray-300', 'dark:border-gray-600');
        div.classList.add('bg-red-50', 'dark:bg-red-900/20', 'border-red-300', 'dark:border-red-700', 'cursor-not-allowed');
      }
    } catch (error) {
      console.error(`Failed to resolve attachment by filename ${filename}:`, error);
      div.setAttribute('data-loaded', 'true');
      div.removeAttribute('data-attachment-filename');

      // Show error state for resolution failure
      const errorMessage = getErrorMessage?.() ?? 'Error loading attachment';
      div.innerHTML = `
        <span class="text-2xl mr-2">⚠️</span>
        <div class="flex-1">
          <span class="font-medium text-red-700 dark:text-red-400">${escapeHtml(errorMessage)}</span>
          <span class="ml-2 text-sm text-gray-600 dark:text-gray-400">${escapeHtml(filename)}</span>
        </div>
      `;

      div.classList.remove('bg-gray-50', 'dark:bg-gray-800', 'hover:bg-gray-100', 'dark:hover:bg-gray-700', 'cursor-pointer', 'border-gray-300', 'dark:border-gray-600');
      div.classList.add('bg-red-50', 'dark:bg-red-900/20', 'border-red-300', 'dark:border-red-700', 'cursor-not-allowed');
    }
  }
}

/**
 * Find an attachment by decrypted filename
 */
async function findAttachmentByFilename(
  attachments: Attachment[],
  filename: string,
  cryptoKey: CryptoKey
): Promise<Attachment | null> {
  for (const attachment of attachments) {
    try {
      const encryptedFilename = JSON.parse(attachment.filename);
      const decryptedFilename = await cryptoService.decryptText(encryptedFilename, cryptoKey);

      if (decryptedFilename === filename) {
        return attachment;
      }
    } catch (err) {
      console.error('Failed to decrypt filename for attachment:', attachment.id, err);
    }
  }

  return null;
}

/**
 * Escape HTML to prevent XSS when inserting into innerHTML
 */
function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}
