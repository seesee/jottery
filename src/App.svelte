<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { isLocked, isLocking, notes, settings, searchQuery, filteredNotes, selectNote, isContentOnlyUpdate, archiveMode } from './lib/stores/appStore';
  import { addNoteToStoreAndSearch } from './lib/stores/storeHelpers';
  import { initDB, noteService, settingsRepository, isLocked as checkLocked, searchService, initI18n, getInitialLocale, syncService, syncRepository, appUpdateService, versionRepository } from './lib/services';
  import { startAutoLock, stopAutoLock } from './lib/services/autoLockService';
  import { locale, _ } from 'svelte-i18n';
  import type { DecryptedNote } from './lib/types';
  import { DEFAULT_SETTINGS } from './lib/types';
  import { getCurrentNotebook } from './lib/utils/notebookPath';
  import UnlockScreen from './lib/components/UnlockScreen.svelte';
  import Header from './lib/components/Header.svelte';
  import NoteList from './lib/components/NoteList.svelte';
  import EditorPane from './lib/components/EditorPane.svelte';
  import SettingsModal from './lib/components/SettingsModal.svelte';
  import KeyboardShortcuts from './lib/components/KeyboardShortcuts.svelte';
  import RecycleBin from './lib/components/RecycleBin.svelte';
  import InboxPanel from './lib/components/InboxPanel.svelte';
  import KeyboardShortcutsHelp from './lib/components/KeyboardShortcutsHelp.svelte';
  import UpdateBanner from './lib/components/UpdateBanner.svelte';
  import Toast from './lib/components/Toast.svelte';
  import BulkOperationsToolbar from './lib/components/BulkOperationsToolbar.svelte';
  import BackupReminderBanner from './lib/components/BackupReminderBanner.svelte';

  let initialized = false;
  let loadingNotes = false;
  let loadingProgress = { current: 0, total: 0 };
  let showSettings = false;
  let showRecycleBin = false;
  let showInboxPanel = false;
  let showShortcutsHelp = false;
  let mobileView: 'list' | 'editor' = 'list'; // Mobile navigation state
  let wasUnlocked = false; // Track if we were previously unlocked (to detect lock transitions)

  // Determine which layout to use based on layoutMode setting
  // For auto mode: only use mobile layout for narrow screens (phones)
  // Tablets and other wide touch devices get the regular desktop layout
  $: useMobileLayout = $settings.layoutMode === 'mobile' ||
    ($settings.layoutMode === 'auto' && window.matchMedia('(max-width: 767px)').matches);

  let creatingNote = false;

  /**
   * Check if a note is "blank" (empty content and no tags)
   */
  function isBlankNote(note: DecryptedNote): boolean {
    return note.content.trim() === '' && note.tags.length === 0;
  }

  async function handleNewNote() {
    if (creatingNote) return; // Prevent multiple clicks

    creatingNote = true;
    try {
      // Check for existing blank note first
      const existingBlank = $notes.find(n => !n.deleted && isBlankNote(n));

      if (existingBlank) {
        // Reuse existing blank note: update timestamps and reorder
        const now = new Date().toISOString();
        const updatedNote: DecryptedNote = {
          ...existingBlank,
          createdAt: now,
          modifiedAt: now,
        };

        // Update in database (this will update timestamps)
        await noteService.updateNote(existingBlank.id, {
          content: '',
          tags: [],
        });

        // Update in store and reorder
        notes.update(allNotes => {
          // Remove the blank note from its current position
          const filtered = allNotes.filter(n => n.id !== existingBlank.id);
          // Insert at the top (after pinned notes)
          const pinnedCount = filtered.filter(n => n.pinned).length;
          const reordered = [...filtered];
          reordered.splice(pinnedCount, 0, updatedNote);
          return reordered;
        });

        searchService.updateNote(updatedNote);
        selectNote(existingBlank.id);
      } else {
        // Create new note
        const newNote = await noteService.createNote('', []);

        // Decrypt the note before adding to store
        const decryptedNote = await noteService.getNote(newNote.id);
        if (!decryptedNote) {
          throw new Error('Failed to retrieve newly created note');
        }

        // Add decrypted note to store and search index
        addNoteToStoreAndSearch(decryptedNote);
        selectNote(decryptedNote.id);

        // Trigger background sync for new note
        syncService.triggerBackgroundSync();
      }

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

  function handleOpenInbox() {
    showInboxPanel = true;
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

  // Reference to EditorPane for calling close() method
  let editorPaneRef: { close: () => void } | undefined;
  let showBackSpinner = false;
  let backSpinnerTimeout: number | null = null;

  function handleBackToList() {
    // Show spinner immediately
    showBackSpinner = true;

    // Minimum 200ms delay before hiding editor
    backSpinnerTimeout = window.setTimeout(() => {
      mobileView = 'list';
      showBackSpinner = false;
      backSpinnerTimeout = null;
    }, 200);
  }

  // Handler for Header back button - triggers EditorPane close with version creation
  function handleHeaderBackToList() {
    if (editorPaneRef) {
      editorPaneRef.close();
    } else {
      // Fallback if ref not available
      handleBackToList();
    }
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
      // Update document title based on notebook
      const notebook = getCurrentNotebook();
      if (notebook.id === 'main') {
        document.title = 'Jottery';
      } else {
        document.title = `Jottery: ${notebook.displayName}`;
      }

      // Initialize database
      await initDB();

      // Load settings (merge with defaults to ensure new fields exist)
      let userSettings = DEFAULT_SETTINGS;
      try {
        userSettings = await settingsRepository.get();
      } catch (error) {
        // If settings read fails (e.g., QuotaExceededError), use defaults
        console.warn('Failed to load settings, using defaults:', error);
      }
      settings.set({ ...DEFAULT_SETTINGS, ...userSettings });

      // Expose app context for E2E testing
      if (typeof window !== 'undefined') {
        (window as any).__appContext = {
          settingsRepository,
          settings,
          DEFAULT_SETTINGS,
          noteService,
          versionRepository,
          syncService,
        };
      }

      // Initialize i18n with user's language preference
      const initialLocale = getInitialLocale(userSettings.language);
      initI18n(initialLocale);
      locale.set(initialLocale);

      // If language was auto-detected (empty string), try to save the detected locale
      // (wrapped in try-catch to handle QuotaExceededError gracefully)
      if (!userSettings.language || userSettings.language === '') {
        try {
          const updated = await settingsRepository.update({ language: initialLocale });
          settings.set({ ...DEFAULT_SETTINGS, ...updated });
        } catch (error) {
          // Ignore write failures (e.g., QuotaExceededError) - user can still use the app
          console.warn('Failed to save language setting:', error);
        }
      }

      // Check for theme override from URL parameter (?theme=light or ?theme=dark)
      const urlParams = new URLSearchParams(window.location.search);
      const themeParam = urlParams.get('theme');
      const themeToApply = (themeParam === 'light' || themeParam === 'dark') ? themeParam : userSettings.theme;

      // Apply theme
      applyTheme(themeToApply);

      // Check lock status
      isLocked.set(checkLocked());

      // Start version checking (only in production)
      appUpdateService.startChecking();

      initialized = true;
    } catch (error) {
      console.error('Failed to initialize app:', error);
      // Even on error, mark as initialized so the UI renders
      // This allows users to access settings/delete database when quota is exceeded
      if (error instanceof Error && error.name === 'QuotaExceededError') {
        console.warn('Storage quota exceeded. You may need to delete data or clear storage.');
        settings.set(DEFAULT_SETTINGS);
        initI18n(getInitialLocale(''));
        isLocked.set(true);
        initialized = true;
      }
    }
  });

  onDestroy(() => {
    if (backSpinnerTimeout !== null) {
      clearTimeout(backSpinnerTimeout);
    }
  });

  // Watch for theme changes (but respect URL parameter override)
  $: if ($settings && initialized) {
    const urlParams = new URLSearchParams(window.location.search);
    const themeParam = urlParams.get('theme');
    const themeToApply = (themeParam === 'light' || themeParam === 'dark') ? themeParam : $settings.theme;
    applyTheme(themeToApply);
  }

  // Watch for language changes (only after initialization to avoid race condition)
  $: if ($settings && initialized) {
    const newLocale = getInitialLocale($settings.language);
    locale.set(newLocale);
  }

  // Watch lock status and load notes when unlocked
  $: if (!$isLocked && initialized) {
    wasUnlocked = true;
    // Reload settings from IndexedDB (may have been updated during unlock, e.g., credential import)
    refreshSettings();
    loadNotes();
    // Start auto-sync if enabled
    startAutoSync();
  }

  // Stop auto-lock and auto-sync when locked
  $: if ($isLocked && initialized) {
    stopAutoLock();
    syncService.disableAutoSync();
    syncService.disconnectFromSyncEvents();
    // Only sync on lock if we were previously unlocked (not on app start)
    if (wasUnlocked) {
      syncOnLock();
      wasUnlocked = false;
    }
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

  /**
   * Refresh settings from IndexedDB
   * Called after unlock to pick up any changes made during credential import
   */
  async function refreshSettings() {
    try {
      const userSettings = await settingsRepository.get();
      settings.set({ ...DEFAULT_SETTINGS, ...userSettings });
    } catch (error) {
      console.error('Failed to refresh settings:', error);
    }
  }

  async function loadNotes() {
    try {
      loadingNotes = true;

      // Accumulate all batches before updating UI
      const allBatches: DecryptedNote[] = [];
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

        // Connect to SSE for real-time sync notifications
        // This runs in background - errors are handled internally
        syncService.connectToSyncEvents();

        // Trigger sync on unlock to get latest changes
        syncService.syncNow();
      }
    } catch (error) {
      console.error('Failed to start auto-sync:', error);
    }
  }

  async function performSearch() {
    const results = await searchService.searchNotes($searchQuery, $notes, $settings.sortOrder, $archiveMode);
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
      // Skip full search for mobile back navigation only
      // This flag is only set when closing a note on mobile (via back button)
      // Desktop always runs full search to maintain proper sort order
      if ($isContentOnlyUpdate) {
        isContentOnlyUpdate.set(false);
        // Update the note in filteredNotes to reflect the content change
        // without running a full search (which is expensive with many notes)
        // Use a Map for O(1) lookups instead of O(n) find() calls
        const notesMap = new Map($notes.map(n => [n.id, n]));
        filteredNotes.update(current => {
          return current.map(note => notesMap.get(note.id) || note);
        });
      } else {
        // Only search, don't re-index (indexing happens once in loadNotes)
        performSearch();
      }
    } else {
      filteredNotes.set([]);
    }
    // Make reactive to searchQuery and archiveMode
    $searchQuery;
    $archiveMode;
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

  <!-- Backup reminder banner (appears when backup is due) -->
  <BackupReminderBanner />

  <!-- Skip link for keyboard accessibility -->
  <a
    href="#main-content"
    class="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:px-4 focus:py-2 focus:bg-blue-600 focus:text-white focus:rounded-md focus:shadow-lg focus:outline-none"
  >
    {$_('a11y.skipToContent')}
  </a>

  <div class="h-screen w-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100 overflow-hidden">
    <div class="flex h-full flex-col">
      <Header
        onOpenSettings={handleOpenSettings}
        onNewNote={handleNewNote}
        onOpenRecycleBin={handleOpenRecycleBin}
        onBackToList={useMobileLayout && mobileView === 'editor' ? handleHeaderBackToList : undefined}
        forceMobileLayout={useMobileLayout}
        disableNewNote={creatingNote}
        {loadingNotes}
        {loadingProgress}
      />

      <main id="main-content" class="flex-1 min-h-0 overflow-hidden flex" tabindex="-1">
        {#if useMobileLayout}
          <!-- Mobile: Keep both components mounted, toggle visibility with CSS -->
          <!-- This preserves NoteList state (scroll position, height cache) across navigation -->
          <div class="w-full h-full relative">
            <div class="absolute inset-0" class:hidden={mobileView !== 'list'}>
              <NoteList onNoteSelect={handleNoteSelect} onOpenInbox={handleOpenInbox} {loadingNotes} {loadingProgress} forceMobileLayout={true} />
            </div>
            <div class="absolute inset-0" class:hidden={mobileView !== 'editor'}>
              <EditorPane bind:this={editorPaneRef} onBackToList={handleBackToList} forceMobileLayout={true} />
            </div>
            <!-- Loading spinner overlay -->
            {#if showBackSpinner}
              <div class="absolute inset-0 bg-gray-900/20 dark:bg-black/40 flex items-center justify-center z-50">
                <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-4">
                  <svg class="w-8 h-8 animate-spin text-blue-600 dark:text-blue-400" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                  </svg>
                </div>
              </div>
            {/if}
          </div>
        {:else}
          <!-- Desktop: Side-by-side layout -->
          <div class="flex w-full h-full min-h-0">
            <!-- Note List Sidebar -->
            <div class="w-80 h-full min-h-0 border-r border-gray-200 dark:border-gray-700 overflow-hidden">
              <NoteList onOpenInbox={handleOpenInbox} {loadingNotes} {loadingProgress} />
            </div>

            <!-- Editor -->
            <div class="flex-1 h-full min-h-0 overflow-hidden">
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

    <!-- Inbox Panel Modal -->
    <InboxPanel
      show={showInboxPanel}
      onClose={() => showInboxPanel = false}
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
      onOpenRecycleBin={handleOpenRecycleBin}
    />

    <!-- Bulk Operations Toolbar -->
    <BulkOperationsToolbar />

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
