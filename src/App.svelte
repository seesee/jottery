<script lang="ts">
  import { onMount } from 'svelte';
  import { isLocked, notes, settings, searchQuery, filteredNotes, selectNote } from './lib/stores/appStore';
  import { initDB, noteService, settingsRepository, isLocked as checkLocked, searchService } from './lib/services';
  import { startAutoLock, stopAutoLock, updateAutoLockTimeout } from './lib/services/autoLockService';
  import UnlockScreen from './lib/components/UnlockScreen.svelte';
  import Header from './lib/components/Header.svelte';
  import NoteList from './lib/components/NoteList.svelte';
  import EditorPane from './lib/components/EditorPane.svelte';
  import SettingsModal from './lib/components/SettingsModal.svelte';
  import KeyboardShortcuts from './lib/components/KeyboardShortcuts.svelte';
  import RecycleBin from './lib/components/RecycleBin.svelte';

  let initialized = false;
  let showSettings = false;
  let showRecycleBin = false;

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
      alert('Failed to create note: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleOpenSettings() {
    showSettings = true;
  }

  function handleOpenRecycleBin() {
    showRecycleBin = true;
  }

  function handleFocusSearch() {
    const input = document.getElementById('search-input') as HTMLInputElement;
    input?.focus();
  }

  function applyTheme(theme: 'light' | 'dark' | 'auto') {
    if (theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else if (theme === 'light') {
      document.documentElement.classList.remove('dark');
    } else {
      // Auto mode - use system preference
      if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    }
  }

  onMount(async () => {
    try {
      // Initialize database
      await initDB();

      // Load settings
      const userSettings = await settingsRepository.get();
      settings.set(userSettings);

      // Apply theme
      applyTheme(userSettings.theme);

      // Check lock status
      isLocked.set(checkLocked());

      initialized = true;
    } catch (error) {
      console.error('Failed to initialize app:', error);
    }
  });

  // Watch for theme changes
  $: if ($settings) {
    applyTheme($settings.theme);
  }

  // Watch lock status and load notes when unlocked
  $: if (!$isLocked && initialized) {
    loadNotes();
    // Start auto-lock when unlocked
    startAutoLock($settings.autoLockTimeout);
  }

  // Stop auto-lock when locked
  $: if ($isLocked) {
    stopAutoLock();
  }

  // Update auto-lock timeout when settings change
  $: if (!$isLocked && $settings.autoLockTimeout) {
    updateAutoLockTimeout($settings.autoLockTimeout);
  }

  async function loadNotes() {
    try {
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);

      // Index notes for search
      searchService.indexNotes(allNotes);

      // Update filtered notes
      performSearch();
    } catch (error) {
      console.error('Failed to load notes:', error);
    }
  }

  async function performSearch() {
    const results = await searchService.searchNotes($searchQuery, $notes);
    filteredNotes.set(results);
  }

  // Reload notes when lock status changes
  $: if (!$isLocked) {
    loadNotes();
  } else if ($isLocked) {
    notes.set([]);
    filteredNotes.set([]);
  }

  // Perform search when query or notes change (only if we have notes)
  $: {
    // Reference both to make this block reactive to changes in either
    $searchQuery;
    if ($notes.length > 0) {
      performSearch();
    } else {
      filteredNotes.set([]);
    }
  }
</script>

{#if !initialized}
  <div class="h-screen w-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900">
    <div class="text-center">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
      <p class="text-gray-600 dark:text-gray-400">Loading...</p>
    </div>
  </div>
{:else if $isLocked}
  <UnlockScreen />
{:else}
  <div class="h-screen w-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100">
    <div class="flex h-full flex-col">
      <Header
        onOpenSettings={handleOpenSettings}
        onNewNote={handleNewNote}
        onOpenRecycleBin={handleOpenRecycleBin}
      />

      <main class="flex-1 overflow-hidden flex">
        <!-- Note List Sidebar -->
        <div class="w-80 border-r border-gray-200 dark:border-gray-700">
          <NoteList />
        </div>

        <!-- Editor -->
        <div class="flex-1">
          <EditorPane />
        </div>
      </main>
    </div>

    <!-- Settings Modal -->
    <SettingsModal
      show={showSettings}
      onClose={() => showSettings = false}
    />

    <!-- Recycle Bin Modal -->
    <RecycleBin
      show={showRecycleBin}
      onClose={() => showRecycleBin = false}
    />

    <!-- Keyboard Shortcuts Handler -->
    <KeyboardShortcuts
      onNewNote={handleNewNote}
      onOpenSettings={handleOpenSettings}
      onFocusSearch={handleFocusSearch}
    />
  </div>
{/if}
