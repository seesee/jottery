<script lang="ts">
  import { selectedNote, clearSelection, notes, settings } from '../stores/appStore';
  import { noteService, tagService, searchService, attachmentService, syncService, syncRepository } from '../services';
  import { formatDateTime } from '../utils/dateFormat';
  import type { Attachment } from '../types';
  import CodeEditor from './CodeEditor.svelte';
  import TagInput from './TagInput.svelte';
  import AttachmentList from './AttachmentList.svelte';
  import FileUpload from './FileUpload.svelte';
  import { marked } from 'marked';

  export let onBackToList: (() => void) | undefined = undefined;

  let content = '';
  let tags: string[] = [];
  let attachments: Attachment[] = [];
  let isEditing = false;

  // Wrapper to handle closing note and returning to list on mobile
  function handleClose() {
    clearSelection();
    if (onBackToList) {
      onBackToList();
    }
  }
  let saveTimeout: number | null = null;
  let language: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash' | 'perl' = 'plain';
  let wordWrap: boolean = true;
  let showPreview: boolean = false;
  let availableTags: string[] = [];
  let isUploading: boolean = false;
  let previousNoteId: string | null = null;
  let isDraggingFile: boolean = false;
  let isAttachmentsExpanded: boolean = false;
  let dragCounter: number = 0; // Track nested drag events
  let codeEditor: any = null; // Reference to CodeEditor component

  // Compute preview HTML
  $: previewHtml = showPreview ? getPreviewHtml(content, language) : '';

  // Check if preview is available for current language
  $: canPreview = language === 'markdown' || language === 'html';

  function getPreviewHtml(text: string, lang: string): string {
    if (lang === 'markdown') {
      try {
        // Configure marked for better rendering
        marked.setOptions({
          breaks: true, // Convert \n to <br>
          gfm: true, // GitHub Flavored Markdown
          headerIds: true,
          mangle: false,
          sanitize: false,
        });
        return marked.parse(text);
      } catch (error) {
        console.error('Markdown parsing error:', error);
        return `<p>Error rendering markdown: ${error}</p>`;
      }
    } else if (lang === 'html') {
      return text;
    }
    return '';
  }

  // Update available tags when notes change
  $: availableTags = tagService.getAllTags($notes);

  // Determine if dark mode is active
  $: isDark = $settings.theme === 'dark' ||
    ($settings.theme === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  // Trigger background sync when switching notes
  $: if ($selectedNote) {
    const noteChanged = previousNoteId !== $selectedNote.id;

    // Only reset local state when switching to a different note
    // Don't reset when the same note is reloaded (e.g., after save)
    if (noteChanged) {
      console.log('[EditorPane] Switching to different note');
      console.log('[EditorPane] From note:', previousNoteId, 'to note:', $selectedNote.id);
      console.log('[EditorPane] New note showPreview value from DB:', $selectedNote.showPreview);

      // Flush any pending save before switching
      if (saveTimeout) {
        clearTimeout(saveTimeout);
        saveTimeout = null;
      }

      content = $selectedNote.content;
      tags = [...$selectedNote.tags];
      attachments = [...$selectedNote.attachments];
      language = $selectedNote.syntaxLanguage || 'plain';
      wordWrap = $selectedNote.wordWrap ?? true;
      showPreview = $selectedNote.showPreview ?? false;
      isEditing = true;

      console.log('[EditorPane] Local showPreview set to:', showPreview);

      // Focus the editor after switching notes
      setTimeout(() => {
        if (codeEditor && !showPreview) {
          codeEditor.focus();
        }
      }, 10);

      // Trigger background sync if we had a previous note open
      if (previousNoteId) {
        console.log('[EditorPane] Triggering background sync for previous note');
        triggerBackgroundSync();
      }
    } else {
      console.log('[EditorPane] Same note reloaded (from sync), NOT resetting state');
      console.log('[EditorPane] Current showPreview:', showPreview, 'DB showPreview:', $selectedNote.showPreview);
    }

    previousNoteId = $selectedNote.id;
  } else {
    // Closing editor - flush pending save and trigger sync
    if (previousNoteId) {
      console.log('[EditorPane] Closing editor, flushing save and triggering sync...');
      if (saveTimeout) {
        clearTimeout(saveTimeout);
        saveTimeout = null;
      }
      triggerBackgroundSync();
    }
    previousNoteId = null;

    content = '';
    tags = [];
    attachments = [];
    language = 'plain';
    wordWrap = true;
    showPreview = false;
    isEditing = false;
  }

  /**
   * Trigger background sync without blocking UI
   */
  async function triggerBackgroundSync() {
    try {
      const metadata = await syncRepository.getMetadata();
      if (metadata?.syncEnabled) {
        console.log('[EditorPane] Sync enabled, syncing in background...');
        // Don't await - let it run in background
        syncService.syncNow().then(result => {
          if (result.success) {
            console.log('[EditorPane] Background sync completed successfully');
          } else {
            console.warn('[EditorPane] Background sync failed:', result.error);
          }
        });
      }
    } catch (error) {
      console.error('[EditorPane] Failed to check sync status:', error);
    }
  }

  async function handleSave() {
    if (!$selectedNote) return;

    console.log('[EditorPane] Saving note with showPreview:', showPreview);

    try {
      await noteService.updateNote($selectedNote.id, {
        content,
        tags: tags,
        attachments: attachments,
        syntaxLanguage: language,
        wordWrap,
        showPreview,
      });

      console.log('[EditorPane] Note saved successfully');

      // Reload all notes to refresh the store and UI
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);

      // Re-index notes for search
      searchService.indexNotes(allNotes);

      // Trigger background sync after saving
      console.log('[EditorPane] Note saved, triggering background sync...');
      triggerBackgroundSync();

      // selectedNote will automatically update from the derived store
    } catch (error) {
      console.error('Failed to save note:', error);
    }
  }

  function handleInput() {
    // Auto-save with debounce
    if (saveTimeout) clearTimeout(saveTimeout);
    saveTimeout = window.setTimeout(handleSave, 1000);
  }

  async function handleTogglePin() {
    if (!$selectedNote) return;
    try {
      await noteService.togglePin($selectedNote.id);

      // Reload all notes to refresh the UI
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);

      // selectedNote will automatically update from the derived store
    } catch (error) {
      console.error('Failed to toggle pin:', error);
    }
  }

  async function handleDelete() {
    console.log('[EditorPane] handleDelete called');
    if (!$selectedNote) {
      console.log('[EditorPane] No selected note, returning');
      return;
    }
    console.log('[EditorPane] Selected note ID:', $selectedNote.id);

    try {
      console.log('[EditorPane] Calling noteService.deleteNote...');
      await noteService.deleteNote($selectedNote.id);
      console.log('[EditorPane] Delete successful, clearing selection...');
      handleClose(); // Clear selection and return to list on mobile

      // Reload all notes to refresh the UI
      console.log('[EditorPane] Reloading notes...');
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      console.log('[EditorPane] Loaded notes count:', allNotes.length);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
      console.log('[EditorPane] Delete operation complete');
    } catch (error) {
      console.error('[EditorPane] Failed to delete note:', error);
    }
  }

  function handleLanguageChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    language = target.value as typeof language;
    handleInput();
  }

  function handleWordWrapToggle() {
    wordWrap = !wordWrap;
    handleInput();
  }

  function handlePreviewToggle() {
    showPreview = !showPreview;
    console.log('[EditorPane] Preview toggled to:', showPreview);
    // Save immediately for preview toggle (no debounce)
    if (saveTimeout) clearTimeout(saveTimeout);
    handleSave();
  }

  async function handleFileUpload(files: FileList) {
    if (!files || files.length === 0) return;

    isUploading = true;

    try {
      const newAttachments: Attachment[] = [];

      for (let i = 0; i < files.length; i++) {
        const file = files[i];

        // Validate file
        const validation = attachmentService.validateFile(file);
        if (!validation.valid) {
          alert(`${file.name}: ${validation.error}`);
          continue;
        }

        try {
          const attachment = await attachmentService.addAttachment(file);
          newAttachments.push(attachment);
        } catch (error) {
          console.error(`Failed to upload ${file.name}:`, error);
          alert(`Failed to upload ${file.name}: ${error instanceof Error ? error.message : String(error)}`);
        }
      }

      if (newAttachments.length > 0) {
        attachments = [...attachments, ...newAttachments];
        handleInput(); // Trigger auto-save
      }
    } finally {
      isUploading = false;
    }
  }

  async function handleDeleteAttachment(attachment: Attachment) {
    try {
      // Remove from attachments array
      attachments = attachments.filter(a => a.id !== attachment.id);

      // Delete from storage (this will happen when the note is saved)
      // We'll add cleanup in the noteService
      handleInput(); // Trigger auto-save
    } catch (error) {
      console.error('Failed to delete attachment:', error);
      alert(`Failed to delete attachment: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async function handleCopy() {
    if (!content) return;

    try {
      await navigator.clipboard.writeText(content);
      // Could show a toast notification here
      console.log('Note content copied to clipboard');
    } catch (error) {
      console.error('Failed to copy note:', error);
      // Fallback for older browsers
      try {
        const textArea = document.createElement('textarea');
        textArea.value = content;
        textArea.style.position = 'fixed';
        textArea.style.left = '-999999px';
        document.body.appendChild(textArea);
        textArea.select();
        document.execCommand('copy');
        document.body.removeChild(textArea);
        console.log('Note content copied to clipboard (fallback)');
      } catch (fallbackError) {
        console.error('Failed to copy note (fallback):', fallbackError);
      }
    }
  }

  function handleExport() {
    if (!content || !$selectedNote) return;

    // Map syntax language to file extension
    const extensionMap: Record<string, string> = {
      'plain': 'txt',
      'javascript': 'js',
      'python': 'py',
      'markdown': 'md',
      'json': 'json',
      'html': 'html',
      'css': 'css',
      'sql': 'sql',
      'bash': 'sh',
      'perl': 'pl',
    };

    const extension = extensionMap[language] || 'txt';

    // Generate filename from first line or date
    const firstLine = content.split('\n')[0].trim();
    const sanitizedFirstLine = firstLine
      .substring(0, 50) // Max 50 chars
      .replace(/[^a-z0-9_\-\.]/gi, '_') // Replace invalid chars
      .replace(/_{2,}/g, '_') // Replace multiple underscores
      .replace(/^_|_$/g, ''); // Trim underscores

    const filename = sanitizedFirstLine ||
      new Date($selectedNote.createdAt).toISOString().split('T')[0];

    // Create blob and download
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${filename}.${extension}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    console.log(`Note exported as ${filename}.${extension}`);
  }

  // Handle drag and drop for files
  function handleEditorDragEnter(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter++;
    if (e.dataTransfer?.types.includes('Files')) {
      isDraggingFile = true;
    }
  }

  function handleEditorDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter--;
    if (dragCounter === 0) {
      isDraggingFile = false;
    }
  }

  function handleEditorDragOver(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  function handleEditorDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter = 0;
    isDraggingFile = false;

    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      handleFileUpload(files);
    }
  }

  function toggleAttachments() {
    isAttachmentsExpanded = !isAttachmentsExpanded;
  }
</script>

{#if isEditing && $selectedNote}
  <div
    class="h-full flex flex-col bg-white dark:bg-gray-900"
    on:dragenter={handleEditorDragEnter}
    on:dragleave={handleEditorDragLeave}
    on:dragover={handleEditorDragOver}
    on:drop={handleEditorDrop}
  >
    <!-- Toolbar -->
    <div class="border-b border-gray-200 dark:border-gray-700 p-2 flex flex-wrap items-center justify-between gap-2">
      <!-- Mobile: Back Button -->
      {#if onBackToList}
        <button
          on:click={onBackToList}
          class="tablet:hidden p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors flex-shrink-0"
          title="Back to notes"
          aria-label="Back to notes"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>
      {/if}

      <div class="flex gap-3">
        <button
          on:click={handleTogglePin}
          class="px-4 py-2.5 min-h-11 rounded active:bg-gray-100 dark:active:bg-gray-800 text-sm whitespace-nowrap"
          title={$selectedNote.pinned ? 'Unpin' : 'Pin'}
        >
          {$selectedNote.pinned ? '⭐ Pinned' : '☆ Pin'}
        </button>
        <button
          on:click={handleDelete}
          class="px-4 py-2.5 min-h-11 rounded active:bg-gray-100 dark:active:bg-gray-800 text-sm text-red-600 whitespace-nowrap"
          title="Delete"
        >
          🗑️ Delete
        </button>
        <button
          on:click={handleCopy}
          class="px-4 py-2.5 min-h-11 rounded active:bg-gray-100 dark:active:bg-gray-800 text-sm whitespace-nowrap"
          title="Copy note content"
        >
          📋 Copy
        </button>
        <button
          on:click={handleExport}
          class="px-4 py-2.5 min-h-11 rounded active:bg-gray-100 dark:active:bg-gray-800 text-sm whitespace-nowrap"
          title="Export note to file"
        >
          💾 Export
        </button>
      </div>

      <div class="flex gap-2 items-center flex-1 justify-end">
        <!-- Preview Toggle (only for markdown/html) - Left of dropdown -->
        {#if canPreview}
          <button
            on:click={handlePreviewToggle}
            class="px-3 py-1.5 rounded text-sm whitespace-nowrap flex-shrink-0 {showPreview ? 'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-200' : 'active:bg-gray-100 dark:active:bg-gray-800'}"
            title="Toggle preview"
          >
            {showPreview ? '📝 Edit' : '👁️ Preview'}
          </button>
        {/if}

        <!-- Syntax Language Selector -->
        <select
          value={language}
          on:change={handleLanguageChange}
          class="px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
          title="Syntax highlighting"
        >
          <option value="plain">Plain Text</option>
          <option value="markdown">Markdown</option>
          <option value="javascript">JavaScript</option>
          <option value="python">Python</option>
          <option value="perl">Perl</option>
          <option value="json">JSON</option>
          <option value="html">HTML</option>
          <option value="css">CSS</option>
          <option value="sql">SQL</option>
          <option value="bash">Bash</option>
        </select>

        <!-- Word Wrap Toggle -->
        <button
          on:click={handleWordWrapToggle}
          class="px-3 py-1.5 rounded text-sm whitespace-nowrap flex-shrink-0 {wordWrap ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-200' : 'active:bg-gray-100 dark:active:bg-gray-800'}"
          title="Toggle word wrap"
        >
          {wordWrap ? 'Wrap' : 'No Wrap'}
        </button>

        <button
          on:click={handleClose}
          class="px-4 py-2.5 min-h-11 rounded active:bg-gray-100 dark:active:bg-gray-800 text-sm whitespace-nowrap"
        >
          ✕ Close
        </button>
      </div>
    </div>

    <!-- Tags Input -->
    <div class="border-b border-gray-200 dark:border-gray-700 p-2">
      <TagInput
        bind:tags
        onChange={(newTags) => {
          tags = newTags;
          handleInput();
        }}
        {availableTags}
        placeholder="Add tags..."
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
            onChange={(newValue) => {
              content = newValue;
              handleInput();
            }}
            {language}
            {wordWrap}
            {isDark}
          />
        </div>
      {/if}

      <!-- Preview Panel - Only shown when IN preview mode -->
      {#if showPreview && canPreview}
        <div class="h-full overflow-auto p-8 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100">
          <div class="prose dark:prose-invert max-w-none">
            {@html previewHtml}
          </div>
        </div>
      {/if}
    </div>

    <!-- Attachments Section - Only show if attachments exist or dragging files -->
    {#if attachments.length > 0 || isDraggingFile}
      <div class="border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <!-- Header - Always visible -->
        <button
          on:click={toggleAttachments}
          class="w-full px-3 py-2 flex items-center justify-between text-sm font-medium text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 transition-colors"
        >
          <span>
            📎 Attachments ({attachments.length})
          </span>
          <span class="text-xs transform transition-transform {isAttachmentsExpanded ? 'rotate-180' : ''}">
            ▼
          </span>
        </button>

        <!-- Expanded content or drop zone -->
        {#if isAttachmentsExpanded || isDraggingFile}
          <div class="px-3 pb-3 max-h-64 overflow-y-auto space-y-3">
            <!-- File Upload - Show when expanded or dragging -->
            <FileUpload
              onUpload={handleFileUpload}
              disabled={isUploading}
            />

            <!-- Attachment List -->
            {#if attachments.length > 0}
              <AttachmentList
                {attachments}
                onDelete={handleDeleteAttachment}
                readonly={false}
              />
            {/if}

            {#if isUploading}
              <div class="text-sm text-blue-600 dark:text-blue-400">
                Uploading...
              </div>
            {/if}
          </div>
        {:else if attachments.length > 0}
          <!-- Collapsed view - show attachment count -->
          <div class="px-3 pb-2 text-xs text-gray-500 dark:text-gray-400">
            Click to {isAttachmentsExpanded ? 'hide' : 'view'} attachments
          </div>
        {/if}
      </div>
    {/if}

    <!-- Metadata Footer -->
    <div class="border-t border-gray-200 dark:border-gray-700 p-2 text-xs text-gray-500 dark:text-gray-400">
      <div class="flex justify-between">
        <span>Created: {formatDateTime($selectedNote.createdAt)}</span>
        <span>Modified: {formatDateTime($selectedNote.modifiedAt)}</span>
      </div>
    </div>
  </div>
{:else}
  <div class="h-full flex items-center justify-center text-gray-500 dark:text-gray-400">
    <div class="text-center">
      <p class="text-lg mb-2">No note selected</p>
      <p class="text-sm">Select a note from the list or create a new one</p>
    </div>
  </div>
{/if}
