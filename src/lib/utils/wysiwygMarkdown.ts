/**
 * WYSIWYG Markdown Conversion Utilities
 *
 * Handles markdown↔HTML conversion for the WYSIWYG editor,
 * with special handling for attachment images and note links.
 */

import { Marked } from 'marked';
import TurndownService from 'turndown';
import { gfm } from 'turndown-plugin-gfm';

// Placeholder for empty note links (Tiptap strips empty anchors)
const EMPTY_LINK_PLACEHOLDER = '🔗';

/**
 * Create a configured TurndownService for converting HTML to Markdown.
 * Includes custom rules for preserving attachment URLs and note links.
 */
export function createTurndownService(): TurndownService {
  const turndownService = new TurndownService({
    headingStyle: 'atx',
    codeBlockStyle: 'fenced',
    bulletListMarker: '-',
    emDelimiter: '*',
    strongDelimiter: '**',
  });

  // Use GFM plugin for tables, strikethrough, etc.
  turndownService.use(gfm);

  // Custom rule to preserve note links (link:uuid format)
  turndownService.addRule('noteLink', {
    filter: (node) => {
      return node.nodeName === 'A' &&
             ((node as HTMLAnchorElement).getAttribute('href')?.startsWith('link:') ?? false);
    },
    replacement: (content, node) => {
      const href = (node as HTMLAnchorElement).getAttribute('href') || '';
      // Restore empty link if placeholder was used (trim to handle any whitespace)
      const trimmedContent = content?.trim() || '';
      const text = (trimmedContent === EMPTY_LINK_PLACEHOLDER) ? '' : trimmedContent;
      return `[${text}](${href})`;
    }
  });

  // Custom rule to preserve attachment images (attachment:uuid format)
  turndownService.addRule('attachmentImage', {
    filter: (node) => {
      return node.nodeName === 'IMG' &&
             ((node as HTMLImageElement).getAttribute('data-attachment-url')?.startsWith('attachment:') ?? false);
    },
    replacement: (_content, node) => {
      const attachmentUrl = (node as HTMLImageElement).getAttribute('data-attachment-url') || '';
      const alt = (node as HTMLImageElement).getAttribute('alt') || '';
      const title = (node as HTMLImageElement).getAttribute('title');
      if (title) {
        return `![${alt}](${attachmentUrl} "${title}")`;
      }
      return `![${alt}](${attachmentUrl})`;
    }
  });

  // Custom rule for task lists
  turndownService.addRule('taskListItem', {
    filter: (node) => {
      return node.nodeName === 'LI' &&
             node.parentNode?.nodeName === 'UL' &&
             (node as HTMLElement).hasAttribute('data-checked');
    },
    replacement: (content, node) => {
      const checked = (node as HTMLElement).getAttribute('data-checked') === 'true';
      const checkbox = checked ? '[x]' : '[ ]';
      return `${checkbox} ${content.trim()}\n`;
    }
  });

  // Custom rule for list items to prevent extra blank lines between items
  turndownService.addRule('listItem', {
    filter: 'li',
    replacement: (content, node) => {
      // Let taskListItem rule handle task list items
      if ((node as HTMLElement).hasAttribute('data-checked')) {
        return '';
      }
      const prefix = node.parentNode?.nodeName === 'OL'
        ? `${Array.from(node.parentNode!.children).indexOf(node as Element) + 1}. `
        : '- ';
      // Trim leading/trailing whitespace and ensure nested content is indented
      const cleaned = content.replace(/^\s+/, '').replace(/\n+$/, '').replace(/\n/g, '\n  ');
      return prefix + cleaned + '\n';
    }
  });

  // Custom rule for code blocks with language
  turndownService.addRule('codeBlock', {
    filter: (node) => {
      return node.nodeName === 'PRE' && node.firstChild?.nodeName === 'CODE';
    },
    replacement: (_content, node) => {
      const codeNode = node.firstChild as HTMLElement;
      const language = codeNode?.className?.match(/language-(\w+)/)?.[1] || '';
      const code = codeNode?.textContent || '';
      return `\n\`\`\`${language}\n${code}\n\`\`\`\n`;
    }
  });

  return turndownService;
}

/**
 * Convert markdown to HTML for the WYSIWYG editor.
 * Preserves attachment URLs in data attributes for roundtrip conversion.
 */
export function markdownToHtml(md: string): string {
  if (!md) return '';
  try {
    const marked = new Marked({
      breaks: true,
      gfm: true,
      renderer: {
        link({ href, title, text }: { href: string; title?: string | null; text: string }) {
          // Preserve note links with link: protocol
          const titleAttr = title ? ` title="${title}"` : '';
          // Add placeholder for empty note links so Tiptap doesn't strip them
          const displayText = (!text && href.startsWith('link:')) ? EMPTY_LINK_PLACEHOLDER : text;
          return `<a href="${href}"${titleAttr}>${displayText}</a>`;
        },
        image({ href, title, text }: { href: string; title?: string | null; text: string }) {
          // Handle attachment: URLs - render with placeholder, resolve async
          if (href && href.startsWith('attachment:')) {
            const attachmentRef = href.substring('attachment:'.length);
            const titleAttr = title ? ` title="${title}"` : '';
            // Store original attachment URL in data attribute for turndown to restore
            return `<img src="" alt="${text || attachmentRef}"${titleAttr} data-attachment-url="${href}" class="attachment-placeholder" />`;
          }
          // Default image rendering
          const titleAttr = title ? ` title="${title}"` : '';
          return `<img src="${href}" alt="${text || ''}"${titleAttr} />`;
        }
      }
    });
    return marked.parse(md) as string;
  } catch (e) {
    console.error('Failed to parse markdown:', e);
    return md;
  }
}

/**
 * Convert HTML from the WYSIWYG editor to markdown.
 * Restores attachment URLs from data attributes.
 */
export function htmlToMarkdown(html: string, turndownService?: TurndownService): string {
  if (!html || html === '<p></p>') return '';
  try {
    const service = turndownService || createTurndownService();
    return service.turndown(html);
  } catch (e) {
    console.error('Failed to convert to markdown:', e);
    return '';
  }
}
