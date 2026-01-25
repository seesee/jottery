<script lang="ts">
  import { _ } from 'svelte-i18n';
  import CodeEditor from '../CodeEditor.svelte';
  import TagInput from '../TagInput.svelte';

  // State props
  export let showPreview: boolean;
  export let canPreview: boolean;
  export let content: string;
  export let tags: string[];
  export let language: string;
  export let wordWrap: boolean;
  export let isDark: boolean;
  export let availableTags: string[];

  // Editor ref (for bind:this)
  export let codeEditor: any = null;

  // Callbacks
  export let onContentChange: (newContent: string) => void;
  export let onTagsChange: (newTags: string[]) => void;
  export let onTagClick: ((tag: string) => void) | undefined = undefined;
  export let onImagePaste: ((file: File) => Promise<string | null>) | undefined = undefined;
  export let onNoteLinkClick: ((noteId: string) => void) | undefined = undefined;

  // Preview HTML
  export let previewHtml: string;

  // Use iframe for HTML/XML preview (allows scripts to run)
  export let useIframePreview: boolean = false;

  // Raw content for iframe preview
  export let rawContent: string = '';

  // Handler wrappers
  function handleContentChange(newValue: string) {
    content = newValue;
    onContentChange(newValue);
  }

  function handleTagsChange(newTags: string[]) {
    tags = newTags;
    onTagsChange(newTags);
  }

  // Reference to the preview scroll container
  let previewContainer: HTMLDivElement;

  // Normalize an anchor ID (same logic as heading ID generation)
  function normalizeAnchorId(id: string): string {
    return id.toLowerCase()
      .replace(/[^\w\s-]/g, '')   // Remove special characters
      .replace(/\s+/g, '-')        // Replace spaces with hyphens
      .replace(/-+/g, '-')         // Collapse multiple hyphens
      .replace(/^-|-$/g, '');      // Trim leading/trailing hyphens
  }

  // Handle anchor link clicks for internal navigation in preview
  function handlePreviewClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const link = target.closest('a');
    if (link) {
      const href = link.getAttribute('href');

      // Handle note links
      if (href?.startsWith('note:') && onNoteLinkClick) {
        e.preventDefault();
        e.stopPropagation();
        const noteId = link.getAttribute('data-note-id');
        if (noteId && !noteId.startsWith('not-found:')) {
          onNoteLinkClick(noteId);
        }
        return;
      }

      // Handle anchor links
      if (href?.startsWith('#')) {
        e.preventDefault();
        e.stopPropagation();
        // Normalize the ID to match our heading ID generation
        const id = normalizeAnchorId(href.slice(1));
        const element = document.getElementById(id);
        if (element && previewContainer) {
          // Calculate position relative to the scroll container
          const containerRect = previewContainer.getBoundingClientRect();
          const elementRect = element.getBoundingClientRect();
          const scrollTop = previewContainer.scrollTop + (elementRect.top - containerRect.top) - 20;
          previewContainer.scrollTo({ top: scrollTop, behavior: 'smooth' });
        }
      }
    }
  }
</script>

<!-- Tags Input -->
<div class="border-b border-gray-200 dark:border-gray-700 p-2">
  <TagInput
    bind:tags
    onChange={handleTagsChange}
    {availableTags}
    placeholder={$_('editor.addTags')}
    {onTagClick}
  />
</div>

<!-- Content Editor with CodeMirror OR Preview (swap, not side-by-side) -->
<div class="flex-1 overflow-hidden bg-white dark:bg-gray-900">
  <!-- Editor - Only shown when NOT in preview mode -->
  {#if !showPreview}
    <div class="h-full overflow-hidden">
      <CodeEditor
        bind:this={codeEditor}
        value={content}
        onChange={handleContentChange}
        {language}
        {wordWrap}
        {isDark}
        {onImagePaste}
      />
    </div>
  {/if}

  <!-- Preview Panel - Only shown when IN preview mode -->
  {#if showPreview && canPreview}
    {#if useIframePreview}
      <!-- Iframe-based preview for HTML/XML (allows scripts to run safely) -->
      <div class="h-full w-full bg-white">
        <iframe
          srcdoc={rawContent}
          title="HTML Preview"
          class="w-full h-full border-0"
          sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-popups"
        ></iframe>
      </div>
    {:else}
      <!-- Standard markdown preview -->
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div bind:this={previewContainer} class="h-full overflow-auto p-8 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100" on:click={handlePreviewClick}>
        <div class="prose dark:prose-invert max-w-none">
          {@html previewHtml}
        </div>
      </div>
    {/if}
  {/if}
</div>
