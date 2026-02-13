<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Editor } from '@tiptap/core';
  import { StarterKit } from '@tiptap/starter-kit';
  import { Link } from '@tiptap/extension-link';
  import { Image } from '@tiptap/extension-image';
  import { Table } from '@tiptap/extension-table';

  // Custom Image extension that preserves attachment URL attribute
  const AttachmentImage = Image.extend({
    addAttributes() {
      return {
        ...this.parent?.(),
        'data-attachment-url': {
          default: null,
          parseHTML: element => element.getAttribute('data-attachment-url'),
          renderHTML: attributes => {
            if (!attributes['data-attachment-url']) {
              return {};
            }
            return { 'data-attachment-url': attributes['data-attachment-url'] };
          },
        },
      };
    },
  });
  import { TableRow } from '@tiptap/extension-table-row';
  import { TableCell } from '@tiptap/extension-table-cell';
  import { TableHeader } from '@tiptap/extension-table-header';
  import { TaskList } from '@tiptap/extension-task-list';
  import { TaskItem } from '@tiptap/extension-task-item';
  import { Placeholder } from '@tiptap/extension-placeholder';
  import { Typography } from '@tiptap/extension-typography';
  import { CodeBlockLowlight } from '@tiptap/extension-code-block-lowlight';
  import { common, createLowlight } from 'lowlight';
  import { Marked } from 'marked';
  import TurndownService from 'turndown';
  import { gfm } from 'turndown-plugin-gfm';
  import { settings } from '../stores/appStore';
  import { getFontSize } from '../utils/fontSize';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let readonly: boolean = false;
  export let isDark: boolean = false;
  export let attachments: Array<{ id: string; filename: string; mimeType: string; size: number; data?: string }> = [];
  export let onImagePaste: ((file: File) => Promise<string | null>) | undefined = undefined;

  let editorElement: HTMLDivElement;
  let editor: Editor | null = null;
  let isUpdatingFromProp = false;

  // Create lowlight instance for syntax highlighting
  const lowlight = createLowlight(common);

  // Create turndown service for HTML to Markdown conversion
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

  // Custom rule for tables to ensure markdown format (override any issues with gfm plugin)
  turndownService.addRule('table', {
    filter: 'table',
    replacement: (_content, node) => {
      const table = node as HTMLTableElement;
      const rows: string[][] = [];
      const headerRow: string[] = [];
      let hasHeader = false;

      // Process thead
      const thead = table.querySelector('thead');
      if (thead) {
        const headerCells = thead.querySelectorAll('th, td');
        headerCells.forEach(cell => {
          headerRow.push(cell.textContent?.trim() || '');
        });
        if (headerRow.length > 0) {
          hasHeader = true;
          rows.push(headerRow);
        }
      }

      // Process tbody
      const tbody = table.querySelector('tbody') || table;
      const bodyRows = tbody.querySelectorAll('tr');
      bodyRows.forEach((tr, index) => {
        // Skip if this is in thead
        if (tr.parentNode === thead) return;

        const cells = tr.querySelectorAll('th, td');
        const rowData: string[] = [];
        cells.forEach(cell => {
          rowData.push(cell.textContent?.trim() || '');
        });

        // If no header and this is first row with th elements, treat as header
        if (!hasHeader && index === 0 && tr.querySelector('th')) {
          hasHeader = true;
          rows.unshift(rowData);
        } else if (rowData.length > 0) {
          rows.push(rowData);
        }
      });

      if (rows.length === 0) return '';

      // Determine column count
      const colCount = Math.max(...rows.map(r => r.length));

      // Build markdown table
      let result = '\n';

      // Header row (or first row if no explicit header)
      const header = rows[0] || [];
      result += '| ' + header.map(cell => cell || ' ').concat(Array(colCount - header.length).fill(' ')).join(' | ') + ' |\n';

      // Separator row
      result += '| ' + Array(colCount).fill('---').join(' | ') + ' |\n';

      // Data rows
      for (let i = 1; i < rows.length; i++) {
        const row = rows[i];
        result += '| ' + row.map(cell => cell || ' ').concat(Array(colCount - row.length).fill(' ')).join(' | ') + ' |\n';
      }

      return result + '\n';
    }
  });

  // Compute font size from settings
  $: fontSize = getFontSize($settings.fontSize);

  // Placeholder for empty note links (Tiptap strips empty anchors)
  const EMPTY_LINK_PLACEHOLDER = '🔗';

  // Helper to find attachment by reference (supports id, data field, or stripped id)
  function findAttachment(ref: string): typeof attachments[0] | undefined {
    if (!ref || !attachments.length) return undefined;
    const normalizedRef = ref.replace(/-/g, '').toLowerCase();
    return attachments.find(a => {
      const normalizedId = a.id.replace(/-/g, '').toLowerCase();
      const normalizedData = a.data?.replace(/-/g, '').toLowerCase() || '';
      return a.id === ref || normalizedId === normalizedRef || a.data === ref || normalizedData === normalizedRef;
    });
  }

  // Track blob URLs for cleanup
  let blobUrls: string[] = [];

  // Create blob URL from base64 data
  function createBlobUrl(attachment: typeof attachments[0]): string | null {
    if (!attachment.data) return null;
    try {
      const byteCharacters = atob(attachment.data);
      const byteNumbers = new Array(byteCharacters.length);
      for (let i = 0; i < byteCharacters.length; i++) {
        byteNumbers[i] = byteCharacters.charCodeAt(i);
      }
      const byteArray = new Uint8Array(byteNumbers);
      const blob = new Blob([byteArray], { type: attachment.mimeType });
      const url = URL.createObjectURL(blob);
      blobUrls.push(url);
      return url;
    } catch (e) {
      console.error('Failed to create blob URL:', e);
      return null;
    }
  }

  // Convert markdown to HTML for Tiptap
  // Uses a local Marked instance with custom renderers for attachments and note links
  function markdownToHtml(md: string): string {
    if (!md) return '';
    try {
      // Create a new Marked instance with our custom renderers
      // This ensures we use the current attachments array
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
            // Handle attachment: URLs
            if (href && href.startsWith('attachment:')) {
              const attachmentRef = href.substring('attachment:'.length);
              const attachment = findAttachment(attachmentRef);
              const titleAttr = title ? ` title="${title}"` : '';
              // Store original attachment URL in data attribute for turndown to restore
              const dataAttr = ` data-attachment-url="${href}"`;
              if (attachment && attachment.mimeType.startsWith('image/')) {
                const blobUrl = createBlobUrl(attachment);
                if (blobUrl) {
                  return `<img src="${blobUrl}" alt="${text || attachment.filename}"${titleAttr}${dataAttr} />`;
                }
              }
              // Attachment not found or not loadable - still render as img with placeholder src
              // This preserves the attachment URL through roundtrip
              return `<img src="" alt="${text || attachmentRef}"${titleAttr}${dataAttr} class="attachment-placeholder" />`;
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

  // Convert HTML from Tiptap to markdown
  function htmlToMarkdown(html: string): string {
    if (!html || html === '<p></p>') return '';
    try {
      return turndownService.turndown(html);
    } catch (e) {
      console.error('Failed to convert to markdown:', e);
      return '';
    }
  }

  // Initialize editor
  onMount(() => {
    editor = new Editor({
      element: editorElement,
      extensions: [
        StarterKit.configure({
          codeBlock: false, // We use CodeBlockLowlight instead
        }),
        CodeBlockLowlight.configure({
          lowlight,
        }),
        Link.configure({
          openOnClick: false,
          HTMLAttributes: {
            class: 'text-blue-600 dark:text-blue-400 underline',
          },
          // Allow note links with link: protocol
          protocols: ['http', 'https', 'mailto', 'tel', 'link'],
          validate: (href) => /^(https?:\/\/|mailto:|tel:|link:)/.test(href),
        }),
        AttachmentImage.configure({
          HTMLAttributes: {
            class: 'max-w-full h-auto rounded',
          },
        }),
        Table.configure({
          resizable: true,
        }),
        TableRow,
        TableCell,
        TableHeader,
        TaskList,
        TaskItem.configure({
          nested: true,
        }),
        Placeholder.configure({
          placeholder: 'Start writing...',
        }),
        Typography,
      ],
      content: markdownToHtml(value),
      editable: !readonly,
      editorProps: {
        attributes: {
          class: 'prose dark:prose-invert max-w-none focus:outline-none min-h-full',
          style: `font-size: ${fontSize}px`,
        },
        handlePaste: (_view, event) => {
          if (!onImagePaste) return false;

          const clipboardData = event.clipboardData;
          if (!clipboardData) return false;

          // Check for image files in clipboard
          const items = Array.from(clipboardData.items);
          const imageItem = items.find(item => item.type.startsWith('image/'));

          if (!imageItem) return false;

          const file = imageItem.getAsFile();
          if (!file) return false;

          // Prevent default paste behaviour
          event.preventDefault();

          // Handle the image paste asynchronously
          (async () => {
            try {
              // Create a temporary blob URL for immediate display
              const tempBlobUrl = URL.createObjectURL(file);
              blobUrls.push(tempBlobUrl);

              const markdownRef = await onImagePaste(file);
              if (markdownRef && editor) {
                // Parse the markdown reference to extract attachment URL
                // Format: ![alt](attachment:uuid) or ![](attachment:uuid)
                const match = markdownRef.match(/^!\[(.*?)\]\((attachment:[^)]+)\)$/);
                if (match) {
                  const alt = match[1] || 'image';
                  const attachmentUrl = match[2];
                  // Insert as image node with blob URL for display and attachment URL for markdown
                  editor.chain().focus().setImage({
                    src: tempBlobUrl,
                    alt,
                    'data-attachment-url': attachmentUrl,
                  } as any).run();
                }
              }
            } catch (error) {
              console.error('Failed to handle image paste:', error);
            }
          })();

          return true; // Event handled
        },
      },
      onUpdate: ({ editor }) => {
        if (isUpdatingFromProp) return;
        const html = editor.getHTML();
        const markdown = htmlToMarkdown(html);
        onChange(markdown);
      },
    });
  });

  // Update content when value prop changes
  $: if (editor && value !== undefined) {
    const currentMarkdown = htmlToMarkdown(editor.getHTML());
    // Only update if the markdown is actually different (avoid infinite loops)
    if (currentMarkdown !== value && !isUpdatingFromProp) {
      isUpdatingFromProp = true;
      editor.commands.setContent(markdownToHtml(value));
      isUpdatingFromProp = false;
    }
  }

  // Update readonly state
  $: if (editor) {
    editor.setEditable(!readonly);
  }

  // Re-render content when attachments change (so images can be resolved)
  // Track previous attachments length to detect changes
  let prevAttachmentsLength = 0;
  $: if (editor && attachments.length !== prevAttachmentsLength) {
    prevAttachmentsLength = attachments.length;
    // Only re-render if we have content that might contain attachment references
    if (value && value.includes('attachment:')) {
      isUpdatingFromProp = true;
      // Clear old blob URLs before re-rendering
      blobUrls.forEach(url => URL.revokeObjectURL(url));
      blobUrls = [];
      editor.commands.setContent(markdownToHtml(value));
      isUpdatingFromProp = false;
    }
  }

  // Cleanup
  onDestroy(() => {
    editor?.destroy();
    // Revoke blob URLs to free memory
    blobUrls.forEach(url => URL.revokeObjectURL(url));
    blobUrls = [];
  });

  // Export methods for toolbar
  export function toggleBold() {
    editor?.chain().focus().toggleBold().run();
  }

  export function toggleItalic() {
    editor?.chain().focus().toggleItalic().run();
  }

  export function toggleStrike() {
    editor?.chain().focus().toggleStrike().run();
  }

  export function toggleCode() {
    editor?.chain().focus().toggleCode().run();
  }

  export function toggleHeading(level: 1 | 2 | 3 | 4 | 5 | 6) {
    editor?.chain().focus().toggleHeading({ level }).run();
  }

  export function toggleBulletList() {
    editor?.chain().focus().toggleBulletList().run();
  }

  export function toggleOrderedList() {
    editor?.chain().focus().toggleOrderedList().run();
  }

  export function toggleTaskList() {
    editor?.chain().focus().toggleTaskList().run();
  }

  export function toggleBlockquote() {
    editor?.chain().focus().toggleBlockquote().run();
  }

  export function toggleCodeBlock() {
    editor?.chain().focus().toggleCodeBlock().run();
  }

  export function setLink(url: string) {
    if (url) {
      editor?.chain().focus().setLink({ href: url }).run();
    } else {
      editor?.chain().focus().unsetLink().run();
    }
  }

  export function insertImage(src: string, alt: string = '') {
    editor?.chain().focus().setImage({ src, alt }).run();
  }

  export function insertTable(rows: number = 3, cols: number = 3) {
    editor?.chain().focus().insertTable({ rows, cols, withHeaderRow: true }).run();
  }

  export function insertHorizontalRule() {
    editor?.chain().focus().setHorizontalRule().run();
  }

  export function undo() {
    editor?.chain().focus().undo().run();
  }

  export function redo() {
    editor?.chain().focus().redo().run();
  }

  export function focus() {
    editor?.chain().focus().run();
  }

  // Check if format is active (for toolbar state)
  export function isActive(name: string, attributes?: Record<string, any>): boolean {
    return editor?.isActive(name, attributes) ?? false;
  }

  // Get the editor instance for advanced use
  export function getEditor(): Editor | null {
    return editor;
  }
</script>

<div
  class="wysiwyg-editor h-full overflow-auto"
  class:dark={isDark}
>
  <div
    bind:this={editorElement}
    class="h-full p-4"
    style="font-size: {fontSize}px"
  ></div>
</div>

<style>
  .wysiwyg-editor :global(.ProseMirror) {
    min-height: 100%;
    outline: none;
  }

  .wysiwyg-editor :global(.ProseMirror p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    color: #9ca3af;
    pointer-events: none;
    height: 0;
  }

  .wysiwyg-editor.dark :global(.ProseMirror p.is-editor-empty:first-child::before) {
    color: #6b7280;
  }

  /* Task list styling */
  .wysiwyg-editor :global(ul[data-type="taskList"]) {
    list-style: none;
    padding-left: 0;
  }

  .wysiwyg-editor :global(ul[data-type="taskList"] li) {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .wysiwyg-editor :global(ul[data-type="taskList"] li > label) {
    flex-shrink: 0;
    margin-top: 0.25rem;
  }

  .wysiwyg-editor :global(ul[data-type="taskList"] li > div) {
    flex: 1;
  }

  /* Table styling */
  .wysiwyg-editor :global(table) {
    border-collapse: collapse;
    width: 100%;
    margin: 1rem 0;
  }

  .wysiwyg-editor :global(th),
  .wysiwyg-editor :global(td) {
    border: 1px solid #d1d5db;
    padding: 0.5rem;
    text-align: left;
  }

  .wysiwyg-editor.dark :global(th),
  .wysiwyg-editor.dark :global(td) {
    border-color: #4b5563;
  }

  .wysiwyg-editor :global(th) {
    background-color: #f3f4f6;
    font-weight: 600;
  }

  .wysiwyg-editor.dark :global(th) {
    background-color: #374151;
  }

  /* Code block styling */
  .wysiwyg-editor :global(pre) {
    background-color: #1f2937;
    color: #e5e7eb;
    padding: 1rem;
    border-radius: 0.375rem;
    overflow-x: auto;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.875em;
  }

  .wysiwyg-editor :global(pre code) {
    background: none;
    padding: 0;
    color: inherit;
  }

  /* Inline code */
  .wysiwyg-editor :global(code) {
    background-color: #f3f4f6;
    padding: 0.125rem 0.25rem;
    border-radius: 0.25rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.875em;
  }

  .wysiwyg-editor.dark :global(code) {
    background-color: #374151;
  }

  /* Blockquote */
  .wysiwyg-editor :global(blockquote) {
    border-left: 4px solid #d1d5db;
    padding-left: 1rem;
    margin-left: 0;
    color: #6b7280;
  }

  .wysiwyg-editor.dark :global(blockquote) {
    border-left-color: #4b5563;
    color: #9ca3af;
  }

  /* Links */
  .wysiwyg-editor :global(a) {
    color: #2563eb;
    text-decoration: underline;
  }

  .wysiwyg-editor.dark :global(a) {
    color: #60a5fa;
  }

  /* Images */
  .wysiwyg-editor :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: 0.375rem;
  }

  /* Placeholder for images with missing attachment data */
  .wysiwyg-editor :global(img.attachment-placeholder) {
    display: inline-block;
    min-width: 100px;
    min-height: 60px;
    background-color: #f3f4f6;
    border: 2px dashed #d1d5db;
  }

  .wysiwyg-editor.dark :global(img.attachment-placeholder) {
    background-color: #374151;
    border-color: #4b5563;
  }

  /* Horizontal rule */
  .wysiwyg-editor :global(hr) {
    border: none;
    border-top: 2px solid #e5e7eb;
    margin: 1.5rem 0;
  }

  .wysiwyg-editor.dark :global(hr) {
    border-top-color: #374151;
  }
</style>
