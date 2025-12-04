<script lang="ts">
  import { selectedNote, clearSelection, notes, settings } from '../stores/appStore';
  import { noteService, tagService, searchService, attachmentService } from '../services';
  import type { Attachment } from '../types';
  import CodeEditor from './CodeEditor.svelte';
  import TagInput from './TagInput.svelte';
  import AttachmentList from './AttachmentList.svelte';
  import FileUpload from './FileUpload.svelte';

  let content = '';
  let tags: string[] = [];
  let attachments: Attachment[] = [];
  let isEditing = false;
  let saveTimeout: number | null = null;
  let language: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash' = 'plain';
  let wordWrap: boolean = true;
  let availableTags: string[] = [];
  let isUploading: boolean = false;

  // Update available tags when notes change
  $: availableTags = tagService.getAllTags($notes);

  // Determine if dark mode is active
  $: isDark = $settings.theme === 'dark' ||
    ($settings.theme === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  $: if ($selectedNote) {
    content = $selectedNote.content;
    tags = [...$selectedNote.tags];
    attachments = [...$selectedNote.attachments];
    language = $selectedNote.syntaxLanguage || 'plain';
    wordWrap = $selectedNote.wordWrap ?? true;
    isEditing = true;
  } else {
    content = '';
    tags = [];
    attachments = [];
    language = 'plain';
    wordWrap = true;
    isEditing = false;
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
      });

      // Reload all notes to refresh the store and UI
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);

      // Re-index notes for search
      searchService.indexNotes(allNotes);

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
      clearSelection();

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
</script>

{#if isEditing && $selectedNote}
  <div class="h-full flex flex-col bg-white dark:bg-gray-900">
    <!-- Toolbar -->
    <div class="border-b border-gray-200 dark:border-gray-700 p-2 flex items-center justify-between gap-2">
      <div class="flex gap-2">
        <button
          on:click={handleTogglePin}
          class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm whitespace-nowrap"
          title={$selectedNote.pinned ? 'Unpin' : 'Pin'}
        >
          {$selectedNote.pinned ? '⭐ Pinned' : '☆ Pin'}
        </button>
        <button
          on:click={handleDelete}
          class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm text-red-600 whitespace-nowrap"
          title="Delete"
        >
          🗑️ Delete
        </button>
        <button
          on:click={handleCopy}
          class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm whitespace-nowrap"
          title="Copy note content"
        >
          📋 Copy
        </button>
      </div>

      <div class="flex gap-2 items-center flex-1 justify-end">
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
          <option value="json">JSON</option>
          <option value="html">HTML</option>
          <option value="css">CSS</option>
          <option value="sql">SQL</option>
          <option value="bash">Bash</option>
        </select>

        <!-- Word Wrap Toggle -->
        <button
          on:click={handleWordWrapToggle}
          class="px-3 py-1 rounded text-sm whitespace-nowrap {wordWrap ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-200' : 'hover:bg-gray-100 dark:hover:bg-gray-800'}"
          title="Toggle word wrap"
        >
          {wordWrap ? '↩ Wrap' : '→ No Wrap'}
        </button>

        <button
          on:click={clearSelection}
          class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm whitespace-nowrap"
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

    <!-- Content Editor with CodeMirror -->
    <div class="flex-1 overflow-hidden bg-white dark:bg-gray-900">
      <CodeEditor
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

    <!-- Attachments Section -->
    <div class="border-t border-gray-200 dark:border-gray-700 p-3 bg-gray-50 dark:bg-gray-800/50 max-h-64 overflow-y-auto">
      <div class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
        Attachments ({attachments.length})
      </div>

      <div class="space-y-3">
        <!-- File Upload -->
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
      </div>

      {#if isUploading}
        <div class="mt-2 text-sm text-blue-600 dark:text-blue-400">
          Uploading...
        </div>
      {/if}
    </div>

    <!-- Metadata Footer -->
    <div class="border-t border-gray-200 dark:border-gray-700 p-2 text-xs text-gray-500 dark:text-gray-400">
      <div class="flex justify-between">
        <span>Created: {new Date($selectedNote.createdAt).toLocaleString('en-GB')}</span>
        <span>Modified: {new Date($selectedNote.modifiedAt).toLocaleString('en-GB')}</span>
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
