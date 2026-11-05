/**
 * Markdown preview renderer utility
 * Extracts the preview rendering logic from EditorPane for better maintainability
 */

import { marked } from 'marked';
import type { Attachment, HighlightJsInstance } from '../types';
import { getNoteTitle, getTitleTagValue } from './noteTitle';
import { escapeAttribute, escapeHtml, sanitiseHtml } from './htmlSanitiser';

export interface MarkdownPreviewOptions {
  attachments: Attachment[];
  openLinksInNewTab: boolean;
  hljs?: HighlightJsInstance | null; // highlight.js instance (optional, for code highlighting)
  loadingText?: string;
  notes?: Array<{ id: string; content: string; tags?: string[] }>; // For resolving note links by title
}

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type LinkableNote = NonNullable<MarkdownPreviewOptions['notes']>[number];

interface NoteLinks {
  byId: Map<string, { id: string; title: string }>;
  byTitle: Map<string, { id: string; title: string }>;
}

/** The title a wiki link names a note by: as in the note list, without markdown */
function linkTitle(note: LinkableNote): string {
  const tags = note.tags ?? [];
  const customTitle = getTitleTagValue(tags)?.trim();
  if (customTitle) return customTitle;

  return getNoteTitle({ content: note.content, tags: [] })
    .replace(/^#+\s*/, '')      // Remove heading markers
    .replace(/\*\*|__/g, '')    // Remove bold
    .replace(/\*|_/g, '')       // Remove italic
    .replace(/`/g, '')          // Remove code
    .trim();
}

// A change to any note makes a new list, so a list only needs reading once, however
// many links a note has and however often the preview renders
const noteLinksByList = new WeakMap<object, NoteLinks>();

function noteLinksFor(notes: LinkableNote[]): NoteLinks {
  const known = noteLinksByList.get(notes);
  if (known) return known;

  const links: NoteLinks = { byId: new Map(), byTitle: new Map() };
  for (const note of notes) {
    const link = { id: note.id, title: linkTitle(note) };
    links.byId.set(note.id, link);

    const title = link.title.toLowerCase();
    if (!links.byTitle.has(title)) {
      links.byTitle.set(title, link);
    }
  }

  noteLinksByList.set(notes, links);
  return links;
}

/**
 * Find note by title or UUID
 */
function findNoteByTitleOrId(links: NoteLinks, ref: string): { id: string; title: string } | undefined {
  return UUID_PATTERN.test(ref) ? links.byId.get(ref) : links.byTitle.get(ref.toLowerCase());
}

/**
 * Parse wiki-style links [[Note Title]] or [[uuid|Display Text]]
 * and convert to markdown links with note: protocol
 */
function parseWikiLinks(content: string, links: NoteLinks): string {
  // Match [[...]] patterns, but not inside code blocks or inline code
  // This regex matches [[content]] where content can include | for display text
  // Limited so a run of unclosed brackets can't make each one scan the whole note
  const wikiLinkRegex = /\[\[([^\]]{1,500})\]\]/g;

  return content.replace(wikiLinkRegex, (_match, innerContent) => {
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
    const foundNote = findNoteByTitleOrId(links, noteRef);

    if (foundNote && UUID_PATTERN.test(foundNote.id)) {
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
 * Create HTML for attachment download link.
 * displayText is HTML: rendered link text, or an escaped reference.
 */
function createAttachmentDownloadHtml(
  attachment: Attachment,
  displayText: string
): string {
  const icon = getAttachmentIcon(String(attachment.mimeType));
  const sizeKb = (Number(attachment.size) / 1024 || 0).toFixed(1);
  return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-id="${escapeAttribute(attachment.id)}" data-attachment-data="${escapeAttribute(attachment.data)}">
    <span class="text-2xl mr-2">${icon}</span>
    <span class="font-medium text-gray-900 dark:text-gray-100">${displayText}</span>
    <span class="ml-2 text-gray-600 dark:text-gray-400 text-sm">(${sizeKb} KB)</span>
  </div>`;
}

/**
 * Create placeholder HTML for unresolved attachment (will be resolved by filename in afterUpdate).
 * displayText is HTML: rendered link text, or an escaped reference.
 */
function createAttachmentPlaceholderHtml(ref: string, displayText: string): string {
  return `<div class="attachment-download p-3 my-2 border border-gray-300 dark:border-gray-600 rounded bg-gray-50 dark:bg-gray-800 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors" data-attachment-filename="${escapeAttribute(ref)}" data-loaded="false">
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
    const noteLinks = noteLinksFor(notes);
    const processedContent = parseWikiLinks(content, noteLinks);
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
      const text = this.parser.parseInline(token.tokens);
      const depth = token.depth;
      const id = token.text.toLowerCase()
        .replace(/[^\w\s-]/g, '')  // Remove special characters
        .replace(/\s+/g, '-')       // Replace spaces with hyphens
        .replace(/-+/g, '-')        // Collapse multiple hyphens
        .replace(/^-|-$/g, '');     // Trim leading/trailing hyphens
      return `<h${depth} id="${id}">${text}</h${depth}>`;
    };

    // Custom link renderer to handle attachment: and note: URLs
    renderer.link = function(token) {
      const href = token.href;
      const text = token.tokens ? this.parser.parseInline(token.tokens) : escapeHtml(token.text || '');

      // Check if this is an attachment URL
      if (href && href.startsWith('attachment:')) {
        const attachmentRef = href.substring('attachment:'.length);
        const attachment = findAttachment(attachments, attachmentRef);

        if (attachment) {
          return createAttachmentDownloadHtml(attachment, text || escapeHtml(attachmentRef));
        } else {
          return createAttachmentPlaceholderHtml(attachmentRef, text || escapeHtml(attachmentRef));
        }
      }

      // Check if this is a note link (note: protocol from wiki-style conversion)
      if (href && href.startsWith('note:')) {
        const noteRef = href.substring('note:'.length);

        // Check if it's a broken link (note not found during pre-processing)
        if (noteRef.startsWith('not-found:')) {
          const originalRef = decodeURIComponent(noteRef.substring('not-found:'.length));
          return `<span class="note-link-broken inline-flex items-center px-1.5 py-0.5 rounded text-sm bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 cursor-not-allowed" title="Note not found: ${escapeAttribute(originalRef)}">
            <span class="note-link-icon note-link-icon-broken"></span>
            ${text}
          </span>`;
        }

        // Valid note link - render as clickable
        return `<a href="${escapeAttribute(href)}" data-note-id="${escapeAttribute(noteRef)}" class="note-link inline-flex items-center px-1.5 py-0.5 rounded text-sm bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 hover:bg-blue-200 dark:hover:bg-blue-800/40 cursor-pointer transition-colors no-underline">
          <span class="note-link-icon"></span>
          ${text}
        </a>`;
      }

      // Check if this is a link: protocol note link (UUID-based)
      // Format: [](link:UUID) or [custom text](link:UUID)
      if (href && href.startsWith('link:')) {
        const noteId = href.substring('link:'.length);

        // Look up the note to get its title (for empty link text)
        const foundNote = noteLinks.byId.get(noteId);

        if (!foundNote) {
          // Note not found - render as broken link
          return `<span class="note-link-broken inline-flex items-center px-1.5 py-0.5 rounded text-sm bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 cursor-not-allowed" title="Note not found: ${escapeAttribute(noteId)}">
            <span class="note-link-icon note-link-icon-broken"></span>
            ${text || 'Note not found'}
          </span>`;
        }

        // Auto-populate title if link text is empty
        const displayText = text || escapeHtml(foundNote.title);

        // Valid note link - render as clickable
        return `<a href="note:${escapeAttribute(noteId)}" data-note-id="${escapeAttribute(noteId)}" class="note-link inline-flex items-center px-1.5 py-0.5 rounded text-sm bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 hover:bg-blue-200 dark:hover:bg-blue-800/40 cursor-pointer transition-colors no-underline">
          <span class="note-link-icon"></span>
          ${displayText}
        </a>`;
      }

      // For non-attachment links, check if we should open in new tab
      const isExternal = href && (href.startsWith('http://') || href.startsWith('https://'));

      if (isExternal && openLinksInNewTab) {
        const title = token.title ? ` title="${escapeAttribute(token.title)}"` : '';
        return `<a href="${escapeAttribute(href)}"${title} target="_blank" rel="noopener noreferrer">${text}</a>`;
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
          return `<img src="${placeholder}" data-attachment-filename="${escapeAttribute(attachmentRef)}" data-loaded="false" alt="${escapeAttribute(text)}" title="${escapeAttribute(title)}" class="attachment-image" />`;
        }

        if (attachment) {
          // Check if it's an image
          if (String(attachment.mimeType).startsWith('image/')) {
            const placeholder = createLoadingPlaceholder(loadingText);
            return `<img src="${placeholder}" data-attachment-id="${escapeAttribute(attachment.data)}" data-loaded="false" alt="${escapeAttribute(text)}" title="${escapeAttribute(title)}" class="attachment-image" />`;
          } else {
            // For non-image attachments, create a download link
            return createAttachmentDownloadHtml(attachment, escapeHtml(text || attachmentRef));
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
            return `<pre><code class="hljs language-${escapeAttribute(language)}">${highlighted}</code></pre>`;
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

    // Last, over the final string: attachment and note values are interpolated
    // by the renderers above, after marked has finished with the source
    return sanitiseHtml(marked.parse(processedContent, { renderer }) as string);
  } catch (error) {
    console.error('Markdown parsing error:', error);
    return `<p>Error rendering markdown: ${escapeHtml(error)}</p>`;
  }
}

/**
 * Render an HTML note for use in the app's own document.
 * The live preview of these notes uses a sandboxed iframe instead.
 */
export function renderHtml(content: string): string {
  return sanitiseHtml(content);
}

/**
 * Wrap an HTML or XML note for the preview iframe.
 *
 * The iframe's sandbox stops script, and this policy stops the note reaching the
 * network with anything else. Neither depends on the app's own policy, which a
 * srcdoc frame inherits but which allows same-origin requests.
 */
export function buildSandboxedPreview(content: string): string {
  const policy = "default-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:";
  return `<!doctype html><meta http-equiv="Content-Security-Policy" content="${policy}">${content}`;
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
