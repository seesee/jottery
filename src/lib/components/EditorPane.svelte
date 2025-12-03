<script lang="ts">
  import { selectedNote, clearSelection } from '../stores/appStore';
  import { noteService } from '../services';

  let content = '';
  let tags = '';
  let isEditing = false;
  let saveTimeout: number | null = null;

  $: if ($selectedNote) {
    content = $selectedNote.content;
    tags = $selectedNote.tags.join(', ');
    isEditing = true;
  } else {
    content = '';
    tags = '';
    isEditing = false;
  }

  async function handleSave() {
    if (!$selectedNote) return;

    const tagArray = tags
      .split(',')
      .map(t => t.trim())
      .filter(t => t.length > 0);

    try {
      await noteService.updateNote($selectedNote.id, {
        content,
        tags: tagArray,
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
</script>

{#if isEditing && $selectedNote}
  <div class="h-full flex flex-col bg-white dark:bg-gray-900">
    <!-- Toolbar -->
    <div class="border-b border-gray-200 dark:border-gray-700 p-2 flex items-center justify-between">
      <div class="flex gap-2">
        <button
          on:click={handleTogglePin}
          class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm"
          title={$selectedNote.pinned ? 'Unpin' : 'Pin'}
        >
          {$selectedNote.pinned ? '⭐ Pinned' : '☆ Pin'}
        </button>
        <button
          on:click={handleDelete}
          class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm text-red-600"
          title="Delete"
        >
          🗑️ Delete
        </button>
      </div>
      <button
        on:click={clearSelection}
        class="px-3 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-sm"
      >
        ✕ Close
      </button>
    </div>

    <!-- Tags Input -->
    <div class="border-b border-gray-200 dark:border-gray-700 p-3">
      <input
        type="text"
        bind:value={tags}
        on:input={handleInput}
        placeholder="Tags (comma-separated)"
        class="w-full text-sm bg-transparent border-none focus:outline-none text-gray-700 dark:text-gray-300"
      />
    </div>

    <!-- Content Editor (basic textarea for now, will be replaced with CodeMirror) -->
    <div class="flex-1 overflow-hidden">
      <textarea
        bind:value={content}
        on:input={handleInput}
        placeholder="Start writing..."
        class="w-full h-full p-4 bg-transparent border-none focus:outline-none resize-none text-gray-900 dark:text-gray-100 font-mono"
      ></textarea>
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
