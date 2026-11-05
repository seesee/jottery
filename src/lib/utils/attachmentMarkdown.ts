import type { Attachment } from '../types';

/**
 * Markdown that shows an attachment. It's referred to by id, as a paste is: a link
 * destination can't hold the spaces a filename can.
 */
export function attachmentMarkdown(attachment: Attachment, filename: string, attachmentLabel: string): string {
  const label = filename.replace(/[[\]\\]/g, '\\$&');

  return attachment.mimeType.startsWith('image/')
    ? `![${label}](attachment:${attachment.id})`
    : `[${attachmentLabel}: ${label}](attachment:${attachment.id})`;
}
