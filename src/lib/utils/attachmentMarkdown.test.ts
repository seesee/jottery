import { describe, it, expect } from 'vitest';
import { attachmentMarkdown } from './attachmentMarkdown';
import { renderMarkdown } from './markdownPreview';
import type { Attachment } from '../types';

const ID = '3f2b8c1e-7d4a-4e9b-8a6f-1c2d3e4f5a6b';
const attachment = (mimeType: string): Attachment => ({ id: ID, filename: 'x', mimeType, size: 1, data: ID });

describe('attachmentMarkdown', () => {
  it('refers to an image by id, as pasting does', () => {
    expect(attachmentMarkdown(attachment('image/png'), 'photo.png', 'Attachment')).toBe(`![photo.png](attachment:${ID})`);
  });

  it('refers to anything else with a link', () => {
    expect(attachmentMarkdown(attachment('application/pdf'), 'a.pdf', 'Attachment')).toBe(`[Attachment: a.pdf](attachment:${ID})`);
  });

  it('still renders when the name has a space, which a link destination cannot hold', () => {
    const image = attachment('image/png');
    const html = renderMarkdown(attachmentMarkdown(image, 'holiday photo.png', 'Attachment'), { attachments: [image], openLinksInNewTab: false });

    expect(html).toContain(`data-attachment-id="${ID}"`);
    expect(html).not.toContain('![');
  });

  it('escapes brackets, which would end the label early', () => {
    const image = attachment('image/png');
    const html = renderMarkdown(attachmentMarkdown(image, 'scan [final].png', 'Attachment'), { attachments: [image], openLinksInNewTab: false });

    expect(html).toContain(`data-attachment-id="${ID}"`);
  });
});
