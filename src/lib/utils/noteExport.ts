/**
 * Note Export Utilities
 *
 * Handles exporting notes to various formats including:
 * - Copy to clipboard
 * - Export as file (with appropriate extension)
 * - Print to PDF
 *
 * Extracted from EditorPane for better testability and reusability.
 */

import type { Attachment } from '../types';
import { attachmentService, cryptoService, keyManager } from '../services';

/**
 * Map syntax language to file extension
 */
const EXTENSION_MAP: Record<string, string> = {
  'plain': 'txt',
  'javascript': 'js',
  'typescript': 'ts',
  'python': 'py',
  'markdown': 'md',
  'json': 'json',
  'html': 'html',
  'css': 'css',
  'sql': 'sql',
  'bash': 'sh',
  'shell': 'sh',
  'perl': 'pl',
  'ruby': 'rb',
  'rust': 'rs',
  'go': 'go',
  'java': 'java',
  'c': 'c',
  'cpp': 'cpp',
  'csharp': 'cs',
  'php': 'php',
  'xml': 'xml',
  'yaml': 'yaml',
  'toml': 'toml',
};

/**
 * Copy content to clipboard with fallback for older browsers
 */
export async function copyToClipboard(content: string): Promise<boolean> {
  if (!content) return false;

  try {
    await navigator.clipboard.writeText(content);
    return true;
  } catch (error) {
    console.error('Failed to copy note:', error);

    // Fallback for older browsers
    try {
      const textArea = document.createElement('textarea');
      textArea.value = content;
      textArea.style.position = 'fixed';
      textArea.style.left = '-999999px';
      document.body.appendChild(textArea);
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
      return true;
    } catch (fallbackError) {
      console.error('Failed to copy note (fallback):', fallbackError);
      return false;
    }
  }
}

/**
 * Generate a safe filename from content
 */
export function generateFilename(content: string, createdAt?: string): string {
  const firstLine = content.split('\n')[0].trim();
  const sanitizedFirstLine = firstLine
    .substring(0, 50) // Max 50 chars
    .replace(/[^a-z0-9_\-\.]/gi, '_') // Replace invalid chars
    .replace(/_{2,}/g, '_') // Replace multiple underscores
    .replace(/^_|_$/g, ''); // Trim underscores

  return sanitizedFirstLine ||
    new Date(createdAt || new Date().toISOString()).toISOString().split('T')[0];
}

/**
 * Get file extension for a syntax language
 */
export function getFileExtension(language: string): string {
  return EXTENSION_MAP[language] || 'txt';
}

export interface ExportOptions {
  content: string;
  language: string;
  createdAt?: string;
}

/**
 * Export note content as a file download
 */
export function exportAsFile(options: ExportOptions): void {
  const { content, language, createdAt } = options;

  if (!content) return;

  const extension = getFileExtension(language);
  const filename = generateFilename(content, createdAt);

  // Create blob and download
  const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${filename}.${extension}`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export interface PrintOptions {
  content: string;
  language: string;
  createdAt?: string;
  previewHtml?: string;
  attachments?: Attachment[];
}

/**
 * Convert blob to data URL
 */
function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onloadend = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

/**
 * Escape HTML special characters
 */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/**
 * Generate print-ready HTML document
 */
export async function generatePrintHtml(options: PrintOptions): Promise<string | null> {
  const { content, language, createdAt, previewHtml, attachments = [] } = options;

  if (!content) return null;

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) return null;

  // Generate title from first line or date
  const firstLine = content.split('\n')[0].trim();
  const title = firstLine.substring(0, 100) ||
    new Date(createdAt || new Date().toISOString()).toISOString().split('T')[0];

  // Build the HTML content based on language type
  let htmlContent: string;

  if (language === 'markdown' && previewHtml) {
    // Use rendered markdown preview
    htmlContent = previewHtml;
  } else if (language === 'html' || language === 'xml') {
    // Use raw HTML/XML content directly
    htmlContent = content;
  } else {
    // For code/plain text, wrap in a pre/code block with styling
    const escapedContent = escapeHtml(content);
    htmlContent = `<pre style="white-space: pre-wrap; word-wrap: break-word; font-family: 'Menlo', 'Monaco', 'Courier New', monospace; font-size: 12px; line-height: 1.5; padding: 1em; background: #f5f5f5; border-radius: 4px;"><code>${escapedContent}</code></pre>`;
  }

  // For markdown, we need to resolve attachment images to data URLs
  if (language === 'markdown' && attachments.length > 0) {
    htmlContent = await resolveAttachmentsForPrint(htmlContent, attachments, masterKey.key);
  }

  // Create the full HTML document
  return `
    <!DOCTYPE html>
    <html>
      <head>
        <title>${escapeHtml(title)}</title>
        <style>
          @media print {
            body { margin: 0; padding: 20px; }
          }
          body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
          }
          pre {
            white-space: pre-wrap;
            word-wrap: break-word;
            overflow-wrap: break-word;
          }
          code {
            font-family: 'Menlo', 'Monaco', 'Courier New', monospace;
          }
          img {
            max-width: 100%;
            height: auto;
          }
          h1, h2, h3, h4, h5, h6 {
            margin-top: 1.5em;
            margin-bottom: 0.5em;
          }
          p {
            margin: 1em 0;
          }
          a {
            color: #0066cc;
          }
          blockquote {
            border-left: 4px solid #ddd;
            padding-left: 1em;
            margin-left: 0;
            color: #666;
          }
          table {
            border-collapse: collapse;
            width: 100%;
          }
          th, td {
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
          }
          th {
            background-color: #f5f5f5;
          }
        </style>
      </head>
      <body>
        ${htmlContent}
      </body>
    </html>
  `;
}

/**
 * Resolve attachment images in HTML to data URLs for printing
 */
async function resolveAttachmentsForPrint(
  htmlContent: string,
  attachments: Attachment[],
  cryptoKey: CryptoKey
): Promise<string> {
  // Parse the HTML to find and replace attachment images
  const parser = new DOMParser();
  const doc = parser.parseFromString(htmlContent, 'text/html');

  // Process images with attachment IDs
  const imagesById = doc.querySelectorAll('img[data-attachment-id]');
  for (const img of imagesById) {
    const attachmentId = img.getAttribute('data-attachment-id');
    if (!attachmentId) continue;

    try {
      const attachment = attachments.find(a => a.data === attachmentId);
      if (attachment) {
        const blob = await attachmentService.getAttachmentData(attachment);
        const dataUrl = await blobToDataUrl(blob);
        img.setAttribute('src', dataUrl);
      }
    } catch (error) {
      console.error(`Failed to load attachment ${attachmentId} for print:`, error);
    }
  }

  // Process images with filenames
  const imagesByFilename = doc.querySelectorAll('img[data-attachment-filename]');
  for (const img of imagesByFilename) {
    const filename = img.getAttribute('data-attachment-filename');
    if (!filename) continue;

    try {
      // Find attachment by decrypted filename
      const foundAttachment = await findAttachmentByFilename(attachments, filename, cryptoKey);

      if (foundAttachment) {
        const blob = await attachmentService.getAttachmentData(foundAttachment);
        const dataUrl = await blobToDataUrl(blob);
        img.setAttribute('src', dataUrl);
      }
    } catch (error) {
      console.error(`Failed to load attachment by filename ${filename} for print:`, error);
    }
  }

  // Get the updated HTML
  return doc.body.innerHTML;
}

/**
 * Find attachment by decrypted filename
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
      // Skip attachments with decryption errors
    }
  }
  return null;
}

/**
 * Open print dialog with note content
 */
export async function printNote(options: PrintOptions): Promise<boolean> {
  const html = await generatePrintHtml(options);
  if (!html) return false;

  const printWindow = window.open('', '_blank');
  if (!printWindow) return false;

  printWindow.document.write(html);
  printWindow.document.close();

  // Wait for content to load, then print
  printWindow.onload = () => {
    printWindow.print();
  };

  return true;
}
