<script lang="ts">
  import { selectedNote, clearSelection, notes, settings } from '../stores/appStore';
  import { noteService, tagService } from '../services';
  import CodeEditor from './CodeEditor.svelte';
  import TagInput from './TagInput.svelte';

  let content = '';
  let tags: string[] = [];
  let isEditing = false;
  let saveTimeout: number | null = null;
  let language: 'plain' | 'javascript' | 'python' | 'markdown' | 'json' | 'html' | 'css' | 'sql' | 'bash' = 'plain';
  let wordWrap: boolean = true;
  let availableTags: string[] = [];

  // Update available tags when notes change
  $: availableTags = tagService.getAllTags($notes);

  // Determine if dark mode is active
  $: isDark = $settings.theme === 'dark' ||
    ($settings.theme === 'auto' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  $: if ($selectedNote) {
    content = $selectedNote.content;
    tags = [...$selectedNote.tags];
    language = $selectedNote.syntaxLanguage || 'plain';
    wordWrap = $selectedNote.wordWrap ?? true;
    isEditing = true;
  } else {
    content = '';
    tags = [];
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
        syntaxLanguage: language,
        wordWrap,
      });
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
      // Refresh would happen via store update in real implementation
    } catch (error) {
      console.error('Failed to toggle pin:', error);
    }
  }

  async function handleDelete() {
    if (!$selectedNote) return;
    if (confirm('Move this note to recycle bin?')) {
      try {
        await noteService.deleteNote($selectedNote.id);
        clearSelection();
        // Refresh would happen via store update
      } catch (error) {
        console.error('Failed to delete note:', error);
      }
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
