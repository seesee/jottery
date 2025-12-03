<script lang="ts">
  import { searchQuery, isLocked, notes, settings, selectNote } from '../stores/appStore';
  import { noteService, lock, searchService } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';

  export let onOpenSettings: () => void = () => {};
  export let onNewNote: () => void = () => {};

  let fileInput: HTMLInputElement;

  async function handleNewNoteClick() {
    try {
      const newNote = await noteService.createNote('', []);

      // Reload all notes
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);

      // Re-index for search
      searchService.indexNotes(allNotes);

      // Select the newly created note
      selectNote(newNote.id);

      // Call parent handler if provided
      if (onNewNote) onNewNote();
    } catch (error) {
      console.error('Failed to create note:', error);
      alert('Failed to create note: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleLock() {
    if (confirm('Lock the application? Unsaved changes will be lost.')) {
      lock();
      isLocked.set(true);
    }
  }

  async function handleExport() {
    try {
      const data = await exportAllNotes();
      await downloadExport(data);
    } catch (error) {
      console.error('Failed to export notes:', error);
      alert('Failed to export notes: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  async function handleImport() {
    fileInput.click();
  }

  async function handleFileSelect(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    try {
      const data = await parseImportFile(file);
      const result = await importNotes(data, 'merge');

      alert(`Import complete!\nImported: ${result.imported}\nSkipped: ${result.skipped}\n${result.errors.length > 0 ? 'Errors: ' + result.errors.join('\n') : ''}`);

      // Reload notes
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
    } catch (error) {
      console.error('Failed to import notes:', error);
      alert('Failed to import notes: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      // Clear file input
      target.value = '';
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
        id="search-input"
        type="text"
        bind:value={$searchQuery}
        placeholder="Search notes..."
        class="w-full px-3 py-1.5 border border-gray-300 dark:border-gray-600 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-700 dark:text-white"
      />
    </div>

    <!-- Actions -->
    <div class="flex items-center gap-2">
      <button
        on:click={handleNewNoteClick}
        class="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
        title="Create new note (Ctrl+N)"
      >
        + New
      </button>

      <button
        on:click={handleExport}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Export all notes"
      >
        📤 Export
      </button>

      <button
        on:click={handleImport}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Import notes"
      >
        📥 Import
      </button>

      <button
        on:click={onOpenSettings}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title="Settings"
      >
        ⚙️ Settings
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

  <!-- Hidden file input for import -->
  <input
    bind:this={fileInput}
    type="file"
    accept=".json"
    on:change={handleFileSelect}
    class="hidden"
  />
</header>
