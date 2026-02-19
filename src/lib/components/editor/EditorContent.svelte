<script lang="ts">
  import { _ } from 'svelte-i18n';
  import CodeEditor from '../CodeEditor.svelte';
  import WysiwygEditor from '../WysiwygEditor.svelte';
  import OutlinerEditor from '../OutlinerEditor.svelte';
  import WysiwygToolbar from './WysiwygToolbar.svelte';
  import TagInput from '../TagInput.svelte';
  import type { CodeEditorRef, WysiwygEditorRef } from '../../types';

  // State props
  export let showPreview: boolean;
  export let canPreview: boolean;
  export let content: string;
  export let tags: string[];
  export let language: string;
  export let wordWrap: boolean;
  export let isDark: boolean;
  export let availableTags: string[];
  export let readOnly: boolean = false;
  export let readOnlyBanner: string | undefined = undefined;
  export let hasConflict: boolean = false;
  export let onResolveConflict: (() => void) | undefined = undefined;

  // Editor ref (for bind:this)
  export let codeEditor: CodeEditorRef | null = null;

  // Editor mode ('raw' or 'wysiwyg')
  export let editorMode: 'raw' | 'wysiwyg' = 'raw';

  // WYSIWYG editor ref (WysiwygEditor component)
  let wysiwygEditor: WysiwygEditorRef | null = null;

  // Callbacks
  export let onContentChange: (newContent: string) => void;
  export let onTagsChange: (newTags: string[]) => void;
  export let onTagClick: ((tag: string) => void) | undefined = undefined;
  export let onImagePaste: ((file: File) => Promise<string | null>) | undefined = undefined;
  export let onNoteLinkClick: ((noteId: string) => void) | undefined = undefined;

  // Attachments for resolving attachment: URLs in WYSIWYG mode
  export let attachments: Array<{ id: string; filename: string; mimeType: string; size: number; data?: string }> = [];

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

      // Handle note links (both note: and link: protocols)
      if ((href?.startsWith('note:') || href?.startsWith('link:')) && onNoteLinkClick) {
        e.preventDefault();
        e.stopPropagation();
        // Try data-note-id first, then extract from href
        let noteId = link.getAttribute('data-note-id');
        if (!noteId && href) {
          // Extract ID from href (note:uuid or link:uuid)
          noteId = href.replace(/^(note:|link:)/, '');
        }
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

<!-- Read-only banner -->
{#if readOnly && readOnlyBanner}
  <div class="bg-amber-100 dark:bg-amber-900/30 border-b border-amber-200 dark:border-amber-700 px-4 py-2 text-sm text-amber-800 dark:text-amber-200 flex items-center gap-2">
    <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
    </svg>
    <span>{readOnlyBanner}</span>
  </div>
{/if}

<!-- Conflict banner -->
{#if hasConflict}
  <div class="bg-red-100 dark:bg-red-900/30 border-b border-red-200 dark:border-red-700 px-4 py-2 text-sm text-red-800 dark:text-red-200 flex items-center justify-between gap-2">
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <span>{$_('conflict.bannerText')}</span>
    </div>
    {#if onResolveConflict}
      <button
        on:click={onResolveConflict}
        class="px-3 py-1 text-xs font-medium bg-red-600 hover:bg-red-700 text-white rounded transition-colors"
      >
        {$_('conflict.resolveButton')}
      </button>
    {/if}
  </div>
{/if}

<!-- Tags Input -->
<div class="border-b border-gray-200 dark:border-gray-700 p-2">
  <TagInput
    bind:tags
    onChange={handleTagsChange}
    {availableTags}
    placeholder={$_('editor.addTags')}
    {onTagClick}
    disabled={readOnly}
  />
</div>

<!-- Content Editor with CodeMirror/WYSIWYG OR Preview (swap, not side-by-side) -->
<div class="flex-1 overflow-hidden bg-white dark:bg-gray-900 flex flex-col" data-testid="editor-content">
  <!-- Editor - Only shown when NOT in preview mode -->
  {#if !showPreview}
    <!-- WYSIWYG mode (markdown only) -->
    {#if editorMode === 'wysiwyg' && language === 'markdown'}
      <WysiwygToolbar editor={wysiwygEditor} disabled={readOnly} />
      <div class="flex-1 overflow-hidden">
        <WysiwygEditor
          bind:this={wysiwygEditor}
          value={content}
          onChange={handleContentChange}
          {isDark}
          {attachments}
          {onImagePaste}
          readonly={readOnly}
        />
      </div>
    {:else if language === 'outliner'}
      <!-- Outliner mode -->
      <div class="h-full overflow-hidden">
        <OutlinerEditor
          value={content}
          onChange={handleContentChange}
          {isDark}
          readonly={readOnly}
        />
      </div>
    {:else}
      <!-- Raw/CodeMirror mode -->
      <div class="h-full overflow-hidden">
        <CodeEditor
          bind:this={codeEditor}
          value={content}
          onChange={handleContentChange}
          {language}
          {wordWrap}
          {isDark}
          {onImagePaste}
          readonly={readOnly}
        />
      </div>
    {/if}
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
        <div class="prose dark:prose-invert max-w-none" data-preview-container>
          {@html previewHtml}
        </div>
      </div>
    {/if}
  {/if}
</div>
