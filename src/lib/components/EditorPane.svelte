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
  import hljs from 'highlight.js';

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
  let showMoreMenu: boolean = false;
  let showInfoModal: boolean = false;

  // Compute preview HTML
  $: previewHtml = showPreview ? getPreviewHtml(content, language) : '';

  // Check if preview is available for current language
  $: canPreview = language === 'markdown' || language === 'html';

  // Compute note statistics
  $: noteStats = {
    characters: content.length,
    charactersNoSpaces: content.replace(/\s/g, '').length,
    words: content.trim().split(/\s+/).filter(s => s.length > 0).length,
    lines: content.split('\n').length,
    tags: tags.length,
    attachments: attachments.length,
  };

  function getPreviewHtml(text: string, lang: string): string {
    if (lang === 'markdown') {
      try {
        // Configure marked for better rendering with syntax highlighting
        marked.setOptions({
          breaks: true, // Convert \n to <br>
          gfm: true, // GitHub Flavored Markdown
          headerIds: true,
          mangle: false,
          sanitize: false,
          highlight: function(code, language) {
            if (language && hljs.getLanguage(language)) {
              try {
                return hljs.highlight(code, { language }).value;
              } catch (err) {
                console.error('Syntax highlighting error:', err);
              }
            }
            return hljs.highlightAuto(code).value;
          }
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
        triggerBackgroundSync();
      }
    } else {
      // Same note reloaded (from sync), not resetting state
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
        // Don't await - let it run in background
        syncService.syncNow().then(result => {
          if (!result.success) {
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

    try {
      await noteService.updateNote($selectedNote.id, {
        content,
        tags: tags,
        attachments: attachments,
        syntaxLanguage: language,
        wordWrap,
        showPreview,
      });

      // Reload all notes to refresh the store and UI
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);

      // Re-index notes for search
      searchService.indexNotes(allNotes);

      // Trigger background sync after saving
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

    // Close menu first
    showMoreMenu = false;

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
    showMoreMenu = false;
    handleInput();
  }

  function handlePreviewToggle() {
    showPreview = !showPreview;
    showMoreMenu = false;
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

    showMoreMenu = false;

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

    showMoreMenu = false;

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

  function handleShowInfo() {
    showMoreMenu = false;
    showInfoModal = true;
  }

  function toggleMoreMenu() {
    showMoreMenu = !showMoreMenu;
  }

  // Close menu when clicking outside
  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest('.more-menu-container')) {
      showMoreMenu = false;
    }
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

<svelte:window on:click={handleClickOutside} />

{#if isEditing && $selectedNote}
  <div
    class="h-full flex flex-col bg-white dark:bg-gray-900"
    on:dragenter={handleEditorDragEnter}
    on:dragleave={handleEditorDragLeave}
    on:dragover={handleEditorDragOver}
    on:drop={handleEditorDrop}
  >
    <!-- Toolbar -->
    <div class="border-b border-gray-200 dark:border-gray-700 p-2 flex items-center gap-2">
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

      <!-- Pin Button (icon only) -->
      <button
        on:click={handleTogglePin}
        class="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors flex-shrink-0"
        title={$selectedNote.pinned ? 'Unpin note' : 'Pin note'}
        aria-label={$selectedNote.pinned ? 'Unpin note' : 'Pin note'}
      >
        {#if $selectedNote.pinned}
          <svg class="w-5 h-5 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
        {:else}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
          </svg>
        {/if}
      </button>

      <!-- Syntax Language Selector -->
      <select
        value={language}
        on:change={handleLanguageChange}
        class="px-2 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500 flex-shrink-0"
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

      <div class="flex-1"></div>

      <!-- More Menu -->
      <div class="more-menu-container relative flex-shrink-0">
        <button
          on:click={toggleMoreMenu}
          class="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors"
          title="More actions"
          aria-label="More actions"
        >
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z" />
          </svg>
        </button>

        {#if showMoreMenu}
          <div class="absolute right-0 mt-1 w-48 bg-white dark:bg-gray-800 rounded-md shadow-lg border border-gray-200 dark:border-gray-700 z-50">
            <div class="py-1">
              {#if canPreview}
                <button
                  on:click={handlePreviewToggle}
                  class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
                >
                  {#if showPreview}
                    <span>📝</span>
                    <span>Edit Mode</span>
                  {:else}
                    <span>👁️</span>
                    <span>Preview</span>
                  {/if}
                </button>
              {/if}

              <button
                on:click={handleWordWrapToggle}
                class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
              >
                {#if wordWrap}
                  <span>↩️</span>
                  <span>Word Wrap: On</span>
                {:else}
                  <span>➡️</span>
                  <span>Word Wrap: Off</span>
                {/if}
              </button>

              <button
                on:click={handleCopy}
                class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
              >
                <span>📋</span>
                <span>Copy Content</span>
              </button>

              <button
                on:click={handleExport}
                class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
              >
                <span>💾</span>
                <span>Export to File</span>
              </button>

              <button
                on:click={handleShowInfo}
                class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
              >
                <span>ℹ️</span>
                <span>Info</span>
              </button>

              <div class="border-t border-gray-200 dark:border-gray-700 my-1"></div>

              <button
                on:click={handleDelete}
                class="w-full px-4 py-2 text-left text-sm hover:bg-red-50 dark:hover:bg-red-900/20 text-red-600 dark:text-red-400 flex items-center gap-2"
              >
                <span>🗑️</span>
                <span>Delete Note</span>
              </button>
            </div>
          </div>
        {/if}
      </div>

      <!-- Close Button (icon only) -->
      <button
        on:click={handleClose}
        class="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors flex-shrink-0"
        title="Close note"
        aria-label="Close note"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
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

<!-- Info Modal -->
{#if showInfoModal && $selectedNote}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" on:click={() => showInfoModal = false}>
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6" on:click|stopPropagation>
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-xl font-semibold text-gray-900 dark:text-gray-100">Note Information</h2>
        <button
          on:click={() => showInfoModal = false}
          class="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
          aria-label="Close"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div class="space-y-3 text-sm">
        <div class="grid grid-cols-2 gap-2">
          <div class="text-gray-600 dark:text-gray-400">Characters:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.characters.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">Characters (no spaces):</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.charactersNoSpaces.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">Words:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.words.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">Lines:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.lines.toLocaleString()}</div>

          <div class="text-gray-600 dark:text-gray-400">Tags:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.tags}</div>

          <div class="text-gray-600 dark:text-gray-400">Attachments:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{noteStats.attachments}</div>
        </div>

        <div class="border-t border-gray-200 dark:border-gray-700 pt-3 mt-3">
          <div class="text-gray-600 dark:text-gray-400 mb-2">Syntax:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100 capitalize">{language.replace('-', ' ')}</div>
        </div>

        <div class="border-t border-gray-200 dark:border-gray-700 pt-3 mt-3">
          <div class="text-gray-600 dark:text-gray-400 mb-2">Created:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{formatDateTime($selectedNote.createdAt)}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">Modified:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">{formatDateTime($selectedNote.modifiedAt)}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">Version:</div>
          <div class="font-medium text-gray-900 dark:text-gray-100">v{$selectedNote.version}</div>
        </div>

        <div>
          <div class="text-gray-600 dark:text-gray-400 mb-2">Note ID:</div>
          <div class="font-mono text-xs text-gray-900 dark:text-gray-100 break-all">{$selectedNote.id}</div>
        </div>
      </div>

      <div class="mt-6 flex justify-end">
        <button
          on:click={() => showInfoModal = false}
          class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
