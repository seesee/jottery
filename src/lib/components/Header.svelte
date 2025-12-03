<script lang="ts">
  import { searchQuery, isLocked, notes, settings, selectNote } from '../stores/appStore';
  import { noteService, lock, searchService } from '../services';

  async function handleNewNote() {
    try {
      const newNote = await noteService.createNote('', []);

      // Reload all notes
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);

      // Re-index for search
      searchService.indexNotes(allNotes);

      // Select the newly created note
      selectNote(newNote.id);
    } catch (error) {
      console.error('Failed to create note:', error);
    }
  }

  function handleLock() {
    if (confirm('Lock the application? Unsaved changes will be lost.')) {
      lock();
      isLocked.set(true);
    }
  }
</script>

<header class="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-4 py-3">
  <div class="flex items-center gap-4">
    <!-- Brand -->
    <h1 class="text-xl font-bold text-gray-900 dark:text-white">Jottery</h1>

    <!-- Search Bar -->
    <div class="flex-1 max-w-md">
      <input
        type="text"
        bind:value={$searchQuery}
        placeholder="Search notes..."
        class="w-full px-3 py-1.5 border border-gray-300 dark:border-gray-600 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
      />
    </div>

    <!-- Actions -->
    <div class="flex items-center gap-2">
      <button
        on:click={handleNewNote}
        class="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
        title="Create new note (Ctrl+N)"
      >
        + New
      </button>

      <button
        on:click={handleLock}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Lock application (Ctrl+L)"
      >
        🔒 Lock
      </button>
    </div>
  </div>
</header>
