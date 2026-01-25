/**
 * Markdown preview renderer utility
 * Extracts the preview rendering logic from EditorPane for better maintainability
 */

import { marked } from 'marked';
import type { Attachment } from '../types';

export interface MarkdownPreviewOptions {
  attachments: Attachment[];
  openLinksInNewTab: boolean;
  hljs?: any; // highlight.js instance (optional, for code highlighting)
  loadingText?: string;
  notes?: Array<{ id: string; content: string }>; // For resolving note links by title
}

/**
 * Extract title from note content (first non-empty line, stripped of markdown formatting)
 */
function extractNoteTitle(content: string): string {
  const lines = content.split('\n');
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed) {
      // Remove markdown heading markers and formatting
      return trimmed
        .replace(/^#+\s*/, '')      // Remove heading markers
        .replace(/\*\*|__/g, '')    // Remove bold
        .replace(/\*|_/g, '')       // Remove italic
        .replace(/`/g, '')          // Remove code
        .trim();
    }
  }
  return 'Untitled';
}

/**
 * Find note by title or UUID
 */
function findNoteByTitleOrId(
  notes: Array<{ id: string; content: string }>,
  ref: string
): { id: string; title: string } | undefined {
  // Check if ref looks like a UUID
  const isUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(ref);

  if (isUuid) {
    const note = notes.find(n => n.id === ref);
    if (note) {
      return { id: note.id, title: extractNoteTitle(note.content) };
    }
    return undefined;
  }

  // Search by title (case-insensitive)
  const searchTitle = ref.toLowerCase();
  for (const note of notes) {
    const noteTitle = extractNoteTitle(note.content);
    if (noteTitle.toLowerCase() === searchTitle) {
      return { id: note.id, title: noteTitle };
    }
  }

  return undefined;
}

/**
 * Parse wiki-style links [[Note Title]] or [[uuid|Display Text]]
 * and convert to markdown links with note: protocol
 */
function parseWikiLinks(
  content: string,
  notes: Array<{ id: string; content: string }>
): string {
  // Match [[...]] patterns, but not inside code blocks or inline code
  // This regex matches [[content]] where content can include | for display text
  const wikiLinkRegex = /\[\[([^\]]+)\]\]/g;

  return content.replace(wikiLinkRegex, (match, innerContent) => {
    // Check if this link has a display text separator
    const pipeIndex = innerContent.indexOf('|');
    let noteRef: string;
    let displayText: string;

    if (pipeIndex !== -1) {
      // Format: [[uuid|Display Text]] or [[Title|Display Text]]
      noteRef = innerContent.substring(0, pipeIndex).trim();
      displayText = innerContent.substring(pipeIndex + 1).trim();
    } else {
      // Format: [[Note Title]] or [[uuid]]
      noteRef = innerContent.trim();
      displayText = noteRef;
    }

    // Try to find the note
    const foundNote = findNoteByTitleOrId(notes, noteRef);

    if (foundNote) {
      // Found the note - create a link with the note's UUID
      // Use the note's actual title if no custom display text was provided
      const linkText = pipeIndex !== -1 ? displayText : foundNote.title;
      return `[${linkText}](note:${foundNote.id})`;
    } else {
      // Note not found - render as a broken link
      return `[${displayText}](note:not-found:${encodeURIComponent(noteRef)})`;
    }
  });
}

/**
 * Get icon for attachment based on mime type
 */
function getAttachmentIcon(mimeType: string): string {
  if (mimeType.includes('pdf')) return '📄';
  if (mimeType.startsWith('image/')) return '🖼️';
  if (mimeType.startsWith('video/')) return '🎥';
  if (mimeType.startsWith('audio/')) return '🎵';
  return '📎';
}

/**
 * Find attachment by reference (ID, data field, or filename)
 */
function findAttachment(attachments: Attachment[], ref: string): Attachment | undefined {
  return attachments.find(a => {
    // Direct ID match (UUID)
    if (a.id === ref) return true;
    // Match UUID without hyphens
    const idWithoutHyphens = a.id.replace(/-/g, '');
    if (ref.startsWith(idWithoutHyphens)) return true;
    // Match by data field
    if (a.data === ref) return true;
    // Match data without hyphens
    const dataWithoutHyphens = a.data.replace(/-/g, '');
    if (ref.startsWith(dataWithoutHyphens)) return true;
    return false;
  });
}

/**
 * Create HTML for attachment download link
 */
function createAttachmentDownloadHtml(
  attachment: Attachment,
  displayText: string
): string {
  const icon = getAttachmentIcon(attachment.mimeType);
  return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-id="${attachment.id}" data-attachment-data="${attachment.data}">
    <span class="text-2xl mr-2">${icon}</span>
    <span class="font-medium text-gray-900 dark:text-gray-100">${displayText}</span>
    <span class="ml-2 text-gray-600 dark:text-gray-400 text-sm">(${(attachment.size / 1024).toFixed(1)} KB)</span>
  </div>`;
}

/**
 * Create placeholder HTML for unresolved attachment (will be resolved by filename in afterUpdate)
 */
function createAttachmentPlaceholderHtml(ref: string, displayText: string): string {
  return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-filename="${ref}" data-loaded="false">
    <span class="text-2xl mr-2">📎</span>
    <span class="font-medium text-gray-900 dark:text-gray-100">${displayText}</span>
  </div>`;
}

/**
 * Create loading placeholder SVG for images
 */
function createLoadingPlaceholder(loadingText: string = 'Loading...'): string {
  const encodedText = encodeURIComponent(loadingText);
  return `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Ctext x='50%25' y='50%25' text-anchor='middle' fill='%23999'%3E${encodedText}%3C/text%3E%3C/svg%3E`;
}

/**
 * Render markdown content to HTML with custom attachment and note link handling
 */
export function renderMarkdown(content: string, options: MarkdownPreviewOptions): string {
  const { attachments, openLinksInNewTab, hljs, loadingText = 'Loading...', notes = [] } = options;

  try {
    // Pre-process wiki-style links [[Note Title]] before markdown parsing
    const processedContent = parseWikiLinks(content, notes);
    // Configure marked for better rendering
    marked.setOptions({
      breaks: true, // Convert \n to <br>
      gfm: true, // GitHub Flavored Markdown
    });

    // Set up custom renderer
    const renderer = new marked.Renderer();
    const originalCode = renderer.code.bind(renderer);
    const originalImage = renderer.image.bind(renderer);
    const originalLink = renderer.link.bind(renderer);

    // Custom heading renderer to add IDs for anchor links
    renderer.heading = function(token) {
      const text = token.text;
      const depth = token.depth;
      const id = text.toLowerCase()
        .replace(/[^\w\s-]/g, '')  // Remove special characters
        .replace(/\s+/g, '-')       // Replace spaces with hyphens
        .replace(/-+/g, '-')        // Collapse multiple hyphens
        .replace(/^-|-$/g, '');     // Trim leading/trailing hyphens
      return `<h${depth} id="${id}">${text}</h${depth}>`;
    };

    // Custom link renderer to handle attachment: and note: URLs
    renderer.link = function(token) {
      const href = token.href;
      const text = token.text || '';

      // Check if this is an attachment URL
      if (href && href.startsWith('attachment:')) {
        const attachmentRef = href.substring('attachment:'.length);
        const attachment = findAttachment(attachments, attachmentRef);

        if (attachment) {
          return createAttachmentDownloadHtml(attachment, text || attachmentRef);
        } else {
          return createAttachmentPlaceholderHtml(attachmentRef, text || attachmentRef);
        }
      }

      // Check if this is a note link
      if (href && href.startsWith('note:')) {
        const noteRef = href.substring('note:'.length);

        // Check if it's a broken link (note not found during pre-processing)
        if (noteRef.startsWith('not-found:')) {
          const originalRef = decodeURIComponent(noteRef.substring('not-found:'.length));
          return `<span class="note-link-broken inline-flex items-center px-1.5 py-0.5 rounded text-sm bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 cursor-not-allowed" title="Note not found: ${originalRef}">
            <svg class="w-3.5 h-3.5 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
            </svg>
            ${text}
          </span>`;
        }

        // Valid note link - render as clickable
        return `<a href="${href}" data-note-id="${noteRef}" class="note-link inline-flex items-center px-1.5 py-0.5 rounded text-sm bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 hover:bg-blue-200 dark:hover:bg-blue-800/40 cursor-pointer transition-colors no-underline">
          <svg class="w-3.5 h-3.5 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          ${text}
        </a>`;
      }

      // For non-attachment links, check if we should open in new tab
      const isExternal = href && (href.startsWith('http://') || href.startsWith('https://'));

      if (isExternal && openLinksInNewTab) {
        // Escape HTML entities in href to prevent XSS
        const safeHref = href.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
        const title = token.title ? ` title="${token.title.replace(/"/g, '&quot;')}"` : '';
        return `<a href="${safeHref}"${title} target="_blank" rel="noopener noreferrer">${text}</a>`;
      }

      // For internal links or when setting is disabled, use default renderer
      return originalLink.call(this, token);
    };

    // Custom image renderer to handle attachment: URLs
    renderer.image = function(token) {
      const href = token.href;
      const title = token.title || '';
      const text = token.text || '';

      // Check if this is an attachment URL
      if (href && href.startsWith('attachment:')) {
        const attachmentRef = href.substring('attachment:'.length);
        const attachment = findAttachment(attachments, attachmentRef);

        // If not found by UUID, mark for filename lookup in afterUpdate
        if (!attachment) {
          const placeholder = createLoadingPlaceholder(loadingText);
          return `<img src="${placeholder}" data-attachment-filename="${attachmentRef}" data-loaded="false" alt="${text}" title="${title}" class="attachment-image" style="max-width: 100%; height: auto;" />`;
        }

        if (attachment) {
          // Check if it's an image
          if (attachment.mimeType.startsWith('image/')) {
            const placeholder = createLoadingPlaceholder(loadingText);
            return `<img src="${placeholder}" data-attachment-id="${attachment.data}" data-loaded="false" alt="${text}" title="${title}" class="attachment-image" style="max-width: 100%; height: auto;" />`;
          } else {
            // For non-image attachments, create a download link
            return createAttachmentDownloadHtml(attachment, text || attachmentRef);
          }
        }
      }

      // For non-attachment images, use default renderer
      return originalImage.call(this, token);
    };

    // Custom code block renderer with syntax highlighting
    renderer.code = function(token) {
      const code = token.text;
      const language = token.lang;

      // Only use syntax highlighting if hljs is loaded
      if (hljs) {
        // Highlight the code if language is specified
        if (language && hljs.getLanguage(language)) {
          try {
            const highlighted = hljs.highlight(code, { language }).value;
            return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
          } catch (err) {
            console.error('Syntax highlighting error:', err);
          }
        }

        // Auto-detect if no language specified
        if (code) {
          try {
            const highlighted = hljs.highlightAuto(code).value;
            return `<pre><code class="hljs">${highlighted}</code></pre>`;
          } catch (err) {
            console.error('Auto-highlight error:', err);
          }
        }
      }

      // Fallback to plain code block if hljs not loaded or highlighting failed
      return originalCode.call(this, token);
    };

    return marked.parse(processedContent, { renderer }) as string;
  } catch (error) {
    console.error('Markdown parsing error:', error);
    return `<p>Error rendering markdown: ${error}</p>`;
  }
}

/**
 * Render HTML preview (just returns the content as-is)
 */
export function renderHtml(content: string): string {
  return content;
}

/**
 * Get preview HTML for content based on language
 */
export function getPreviewHtml(
  content: string,
  language: string,
  options: MarkdownPreviewOptions
): string {
  if (language === 'markdown') {
    return renderMarkdown(content, options);
  } else if (language === 'html') {
    return renderHtml(content);
  }
  return '';
}
