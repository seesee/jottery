<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Editor } from '@tiptap/core';
  import { StarterKit } from '@tiptap/starter-kit';
  import { Link } from '@tiptap/extension-link';
  import { Image } from '@tiptap/extension-image';
  import { Table } from '@tiptap/extension-table';
  import { TableRow } from '@tiptap/extension-table-row';
  import { TableCell } from '@tiptap/extension-table-cell';
  import { TableHeader } from '@tiptap/extension-table-header';
  import { TaskList } from '@tiptap/extension-task-list';
  import { TaskItem } from '@tiptap/extension-task-item';
  import { Placeholder } from '@tiptap/extension-placeholder';
  import { Typography } from '@tiptap/extension-typography';
  import { CodeBlockLowlight } from '@tiptap/extension-code-block-lowlight';
  import { common, createLowlight } from 'lowlight';
  import { marked } from 'marked';
  import TurndownService from 'turndown';
  import { gfm } from 'turndown-plugin-gfm';
  import { settings } from '../stores/appStore';
  import { getFontSize } from '../utils/fontSize';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let readonly: boolean = false;
  export let isDark: boolean = false;
  export let placeholder: string = '';

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
             (node as HTMLAnchorElement).getAttribute('href')?.startsWith('link:');
    },
    replacement: (content, node) => {
      const href = (node as HTMLAnchorElement).getAttribute('href') || '';
      // Restore empty link if placeholder was used (trim to handle any whitespace)
      const trimmedContent = content?.trim() || '';
      const text = (trimmedContent === EMPTY_LINK_PLACEHOLDER) ? '' : trimmedContent;
      return `[${text}](${href})`;
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

  // Configure marked to allow note links (link: protocol)
  // In marked v17+, renderer functions receive a token object
  marked.use({
    renderer: {
      link({ href, title, text }: { href: string; title: string | null; text: string }) {
        // Preserve note links with link: protocol
        const titleAttr = title ? ` title="${title}"` : '';
        // Add placeholder for empty note links so Tiptap doesn't strip them
        const displayText = (!text && href.startsWith('link:')) ? EMPTY_LINK_PLACEHOLDER : text;
        return `<a href="${href}"${titleAttr}>${displayText}</a>`;
      }
    }
  });

  // Convert markdown to HTML for Tiptap
  function markdownToHtml(md: string): string {
    if (!md) return '';
    try {
      return marked.parse(md, { breaks: true, gfm: true }) as string;
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
        Image.configure({
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
          placeholder: placeholder || 'Start writing...',
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

  // Cleanup
  onDestroy(() => {
    editor?.destroy();
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
