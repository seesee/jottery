<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Editor } from '@tiptap/core';
  import {
    createWysiwygExtensions,
    installMarkdownEscaping,
    getMarkdownFromEditor,
  } from '../utils/wysiwygMarkdown';
  import { settings } from '../stores/appStore';
  import { getFontSize } from '../utils/fontSize';
  import { attachmentService } from '../services';

  export let value: string = '';
  export let onChange: (value: string) => void = () => {};
  export let readonly: boolean = false;
  export let isDark: boolean = false;
  export let attachments: Array<{ id: string; filename: string; mimeType: string; size: number; data?: string }> = [];
  export let onImagePaste: ((file: File) => Promise<string | null>) | undefined = undefined;

  let editorElement: HTMLDivElement;
  let editor: Editor | null = null;
  let isUpdatingFromProp = false;

  // Compute font size from settings
  $: fontSize = getFontSize($settings.fontSize);

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

  // Resolve attachment images in the editor after content is set.
  // The markdown parser leaves src empty and keeps the reference in
  // data-attachment-url; this loads the decrypted blob and points src at it.
  // The DOM is updated directly so the document (and therefore the saved
  // markdown) is not touched.
  async function resolveAttachmentImages(): Promise<void> {
    if (!editorElement) return;

    const images = editorElement.querySelectorAll('img[data-attachment-url]');
    for (const img of images) {
      // Skip if already loaded
      if (img.getAttribute('data-loaded') === 'true') continue;

      const attachmentUrl = img.getAttribute('data-attachment-url');
      if (!attachmentUrl?.startsWith('attachment:')) continue;

      img.classList.add('attachment-placeholder');
      const attachmentRef = attachmentUrl.substring('attachment:'.length);
      const attachment = findAttachment(attachmentRef);

      if (attachment && attachment.mimeType.startsWith('image/') && attachment.data) {
        try {
          // Load and decrypt the blob using attachmentService
          // Cast to Attachment type since we've verified data exists
          const blob = await attachmentService.getAttachmentData(attachment as import('../types').Attachment);
          const blobUrl = URL.createObjectURL(blob);
          blobUrls.push(blobUrl);
          img.setAttribute('src', blobUrl);
          img.setAttribute('data-loaded', 'true');
          img.classList.remove('attachment-placeholder');
        } catch (error) {
          console.error('Failed to load attachment:', error);
          img.setAttribute('data-loaded', 'true');
        }
      } else {
        img.setAttribute('data-loaded', 'true');
      }
    }
  }

  function setMarkdown(markdown: string): void {
    if (!editor) return;
    isUpdatingFromProp = true;
    editor.commands.setContent(markdown, { contentType: 'markdown' });
    isUpdatingFromProp = false;
    resolveAttachmentImages();
  }

  // Initialize editor
  onMount(() => {
    editor = new Editor({
      element: editorElement,
      extensions: createWysiwygExtensions({
        placeholder: 'Start writing...',
        linkClass: 'text-blue-600 dark:text-blue-400 underline',
        imageClass: 'max-w-full h-auto rounded',
      }),
      content: value,
      contentType: 'markdown',
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
        onChange(getMarkdownFromEditor(editor));
      },
    });
    installMarkdownEscaping(editor);

    // Resolve attachment images after editor is created
    resolveAttachmentImages();
  });

  // Update content when value prop changes
  $: if (editor && value !== undefined) {
    const currentMarkdown = getMarkdownFromEditor(editor);
    // Only update if the markdown is actually different (avoid infinite loops)
    if (currentMarkdown !== value && !isUpdatingFromProp) {
      setMarkdown(value);
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
      // Clear old blob URLs before re-rendering
      blobUrls.forEach(url => URL.revokeObjectURL(url));
      blobUrls = [];
      setMarkdown(value);
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
