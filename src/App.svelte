<script lang="ts">
  import { onMount } from 'svelte';
  import { isLocked, isLocking, notes, settings, searchQuery, filteredNotes, selectNote, isDraftMode, enterDraftMode } from './lib/stores/appStore';
  import { initDB, noteService, settingsRepository, isLocked as checkLocked, searchService, initI18n, getInitialLocale, syncService, syncRepository, appUpdateService } from './lib/services';
  import { startAutoLock, stopAutoLock, updateAutoLockTimeout } from './lib/services/autoLockService';
  import { locale, _ } from 'svelte-i18n';
  import UnlockScreen from './lib/components/UnlockScreen.svelte';
  import Header from './lib/components/Header.svelte';
  import NoteList from './lib/components/NoteList.svelte';
  import EditorPane from './lib/components/EditorPane.svelte';
  import SettingsModal from './lib/components/SettingsModal.svelte';
  import KeyboardShortcuts from './lib/components/KeyboardShortcuts.svelte';
  import RecycleBin from './lib/components/RecycleBin.svelte';
  import KeyboardShortcutsHelp from './lib/components/KeyboardShortcutsHelp.svelte';
  import UpdateBanner from './lib/components/UpdateBanner.svelte';
  import Toast from './lib/components/Toast.svelte';

  let initialized = false;
  let loadingNotes = false;
  let loadingProgress = { current: 0, total: 0 };
  let showSettings = false;
  let showRecycleBin = false;
  let showShortcutsHelp = false;
  let mobileView: 'list' | 'editor' = 'list'; // Mobile navigation state

  // Determine which layout to use based on layoutMode setting
  $: useMobileLayout = $settings.layoutMode === 'mobile' ||
    ($settings.layoutMode === 'auto' && window.matchMedia('(max-width: 767px)').matches);

  let creatingNote = false;

  async function handleNewNote() {
    if (creatingNote) return; // Prevent multiple clicks

    creatingNote = true;
    try {
      // Create empty note immediately
      const newNote = await noteService.createNote('', []);

      // Decrypt the note before adding to store
      const decryptedNote = await noteService.getNote(newNote.id);
      if (!decryptedNote) {
        throw new Error('Failed to retrieve newly created note');
      }

      // Add decrypted note to store (incremental update)
      notes.update(allNotes => {
        // Add to beginning if sorted by recent, otherwise add and let virtual list handle it
        return [decryptedNote, ...allNotes];
      });
      searchService.updateNote(decryptedNote);
      selectNote(decryptedNote.id);

      // Switch to editor view on mobile
      mobileView = 'editor';
    } catch (error) {
      console.error('Failed to create note:', error);
    } finally {
      creatingNote = false;
    }
  }

  function handleOpenSettings() {
    showSettings = true;
  }

  function handleOpenRecycleBin() {
    showRecycleBin = true;
  }

  function handleOpenShortcutsHelp() {
    showShortcutsHelp = true;
  }

  function handleFocusSearch() {
    // Try desktop search first, fallback to mobile
    const desktopInput = document.getElementById('search-input') as HTMLInputElement;
    const mobileInput = document.getElementById('search-input-mobile') as HTMLInputElement;

    if (desktopInput && window.getComputedStyle(desktopInput).display !== 'none') {
      desktopInput.focus();
    } else if (mobileInput) {
      mobileInput.focus();
    }
  }

  function handleBackToList() {
    mobileView = 'list';
  }

  function handleNoteSelect() {
    // Switch to editor view on mobile when a note is selected
    mobileView = 'editor';
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

      // Initialize i18n with user's language preference
      const initialLocale = getInitialLocale(userSettings.language);
      initI18n(initialLocale);
      locale.set(initialLocale);

      // If language was auto-detected (empty string), save the detected locale
      if (!userSettings.language || userSettings.language === '') {
        const updated = await settingsRepository.update({ language: initialLocale });
        settings.set(updated);
      }

      // Apply theme
      applyTheme(userSettings.theme);

      // Check lock status
      isLocked.set(checkLocked());

      // Start version checking (only in production)
      appUpdateService.startChecking();

      initialized = true;
    } catch (error) {
      console.error('Failed to initialize app:', error);
    }
  });

  // Watch for theme changes
  $: if ($settings) {
    applyTheme($settings.theme);
  }

  // Watch for language changes (only after initialization to avoid race condition)
  $: if ($settings && initialized) {
    const newLocale = getInitialLocale($settings.language);
    locale.set(newLocale);
  }

  // Watch lock status and load notes when unlocked
  $: if (!$isLocked && initialized) {
    loadNotes();
    // Start auto-sync if enabled
    startAutoSync();
  }

  // Stop auto-lock and auto-sync when locked
  $: if ($isLocked && initialized) {
    stopAutoLock();
    syncService.disableAutoSync();
    // Sync on lock to push any changes before going away
    syncOnLock();
  }

  async function syncOnLock() {
    try {
      const syncMetadata = await syncRepository.getMetadata();
      if (syncMetadata?.syncEnabled) {
        await syncService.syncNow();
      }
    } catch (error) {
      console.error('Failed to sync on lock:', error);
    }
  }

  // Manage auto-lock based on lock state and remember password setting
  $: if (!$isLocked && initialized && $settings) {
    if ($settings.rememberPassword) {
      // Remember password enabled - disable auto-lock
      stopAutoLock();
    } else {
      // Remember password disabled - enable auto-lock
      startAutoLock($settings.autoLockTimeout);
    }
  }

  async function loadNotes() {
    try {
      loadingNotes = true;
      const startTime = Date.now();

      // Accumulate all batches before updating UI
      const allBatches: typeof notes.$state = [];
      const loadedNoteIds = new Set<string>();

      // Get total count for progress reporting
      const totalCount = await noteService.getStats().then(s => s.active);
      loadingProgress = { current: 0, total: totalCount };

      // Load all notes in batches, accumulating them
      const firstBatch = await noteService.getAllNotesBatched(
        $settings.sortOrder,
        200, // First 200 notes
        (moreBatch) => {
          // Callback for subsequent batches - just accumulate, don't update UI
          const newNotes = moreBatch.filter(note => {
            if (loadedNoteIds.has(note.id)) {
              return false;
            }
            loadedNoteIds.add(note.id);
            return true;
          });

          allBatches.push(...newNotes);
          loadingProgress = { current: firstBatch.length + allBatches.length, total: totalCount };
        }
      );

      // Track first batch IDs
      firstBatch.forEach(note => loadedNoteIds.add(note.id));
      loadingProgress = { current: firstBatch.length, total: totalCount };

      // Wait for all batches to complete by checking if we have all notes
      // Poll every 50ms until we have all notes or timeout after 10s
      const pollInterval = 50;
      const maxWait = 10000;
      let waited = 0;

      while (firstBatch.length + allBatches.length < totalCount && waited < maxWait) {
        await new Promise(resolve => setTimeout(resolve, pollInterval));
        waited += pollInterval;
      }

      // Combine all batches
      const allNotes = [...firstBatch, ...allBatches];

      // Now set notes once and index once
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
      await performSearch();

      // Clear loading state
      loadingNotes = false;
      loadingProgress = { current: 0, total: 0 };
    } catch (error) {
      console.error('Failed to load notes:', error);
      loadingNotes = false;
      loadingProgress = { current: 0, total: 0 };
    }
  }

  async function startAutoSync() {
    try {
      const syncMetadata = await syncRepository.getMetadata();
      if (syncMetadata?.syncEnabled) {
        const interval = syncMetadata.autoSyncInterval || 5;
        syncService.enableAutoSync(interval);

        // Trigger sync on unlock to get latest changes
        syncService.syncNow();
      }
    } catch (error) {
      console.error('Failed to start auto-sync:', error);
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

  // Perform search when query changes (indexing happens in loadNotes)
  $: {
    if ($notes.length > 0 && !loadingNotes) {
      // Only search, don't re-index (indexing happens once in loadNotes)
      performSearch();
    } else {
      filteredNotes.set([]);
    }
    // Make reactive to searchQuery
    $searchQuery;
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
  <!-- Update banner (appears at top when new version available) -->
  <UpdateBanner />

  <div class="h-screen w-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100">
    <div class="flex h-full flex-col">
      <Header
        onOpenSettings={handleOpenSettings}
        onNewNote={handleNewNote}
        onOpenRecycleBin={handleOpenRecycleBin}
        onBackToList={useMobileLayout && mobileView === 'editor' ? handleBackToList : undefined}
        forceMobileLayout={useMobileLayout}
        disableNewNote={creatingNote}
        {loadingNotes}
        {loadingProgress}
      />

      <main class="flex-1 overflow-hidden flex">
        {#if useMobileLayout}
          <!-- Mobile: Single view (list OR editor) -->
          <div class="w-full">
            {#if mobileView === 'list'}
              <NoteList onNoteSelect={handleNoteSelect} {loadingNotes} {loadingProgress} />
            {:else}
              <EditorPane onBackToList={handleBackToList} forceMobileLayout={true} />
            {/if}
          </div>
        {:else}
          <!-- Desktop: Side-by-side layout -->
          <div class="flex w-full">
            <!-- Note List Sidebar -->
            <div class="w-80 border-r border-gray-200 dark:border-gray-700">
              <NoteList {loadingNotes} {loadingProgress} />
            </div>

            <!-- Editor -->
            <div class="flex-1">
              <EditorPane />
            </div>
          </div>
        {/if}
      </main>
    </div>

    <!-- Settings Modal -->
    <SettingsModal
      show={showSettings}
      onClose={() => showSettings = false}
      onOpenShortcutsHelp={handleOpenShortcutsHelp}
    />

    <!-- Recycle Bin Modal -->
    <RecycleBin
      show={showRecycleBin}
      onClose={() => showRecycleBin = false}
    />

    <!-- Keyboard Shortcuts Help Modal -->
    <KeyboardShortcutsHelp
      show={showShortcutsHelp}
      onClose={() => showShortcutsHelp = false}
    />

    <!-- Keyboard Shortcuts Handler -->
    <KeyboardShortcuts
      onNewNote={handleNewNote}
      onOpenSettings={handleOpenSettings}
      onFocusSearch={handleFocusSearch}
      onOpenShortcutsHelp={handleOpenShortcutsHelp}
    />

    <!-- Toast Notifications -->
    <Toast />
  </div>
{/if}

<!-- Locking Interstitial -->
{#if $isLocking}
  <div class="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-[100]" role="status" aria-live="polite">
    <div class="text-center">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
      <p class="text-white text-lg font-medium">Locking...</p>
      <p class="text-gray-300 text-sm mt-2">Saving and syncing your notes</p>
    </div>
  </div>
{/if}
