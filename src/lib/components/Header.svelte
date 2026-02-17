<script lang="ts">
  import { searchQuery, isLocked, isLocking, settings, isDraftMode, searchResultCount, notes, isSyncing, syncProgress, archiveMode, toggleArchiveMode } from '../stores/appStore';
  import { lock, passwordStorageService, settingsRepository, syncService, syncRepository } from '../services';
  import { getCurrentNotebook } from '../utils/notebookPath';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from './ConfirmModal.svelte';
  import MobileMenuDrawer from './MobileMenuDrawer.svelte';
  import SavedSearchesPanel from './SavedSearchesPanel.svelte';
  import { formatShortcutForTooltip } from '../utils/keyboardShortcuts';
  import { getFontSizeCSS } from '../utils/fontSize';
  import { localeIncludes } from '../utils/stringUtils';

  export let onOpenSettings: () => void = () => {};
  export let onNewNote: () => void = () => {};
  export let onOpenRecycleBin: () => void = () => {};
  export let onBackToList: (() => void) | undefined = undefined; // Mobile: back to list handler
  export let forceMobileLayout: boolean = false;
  export let disableNewNote: boolean = false;
  export let loadingNotes: boolean = false;
  export let loadingProgress: { current: number; total: number } = { current: 0, total: 0 };

  let showDisableRememberPasswordConfirm = false;
  let showMobileMenu = false;
  let showSavedSearches = false;

  // Tag autocomplete state
  let tagSuggestions: string[] = [];
  let showTagSuggestions = false;
  let selectedSuggestionIndex = -1;

  // Get current notebook info for display
  const notebook = getCurrentNotebook();

  // Sync progress - show percentage if >= 5 items, otherwise just spinner
  $: showSyncPercentage = $syncProgress.total >= 5;
  $: syncPercentage = $syncProgress.total > 0
    ? Math.round(($syncProgress.completed / $syncProgress.total) * 100)
    : 0;

  // Check if remember password is enabled
  $: rememberPasswordEnabled = $settings.rememberPassword || false;

  // Compute font size from settings (prevents Safari zoom on mobile)
  $: searchFontSize = getFontSizeCSS($settings.fontSize);

  // Format keyboard shortcuts for display
  $: shortcuts = $settings.keyboardShortcuts;
  $: newNoteShortcut = formatShortcutForTooltip(shortcuts?.newNote);
  $: openSettingsShortcut = formatShortcutForTooltip(shortcuts?.openSettings);
  $: lockAppShortcut = formatShortcutForTooltip(shortcuts?.lockApp);

  // Update placeholder text based on archive mode
  $: searchPlaceholder = $archiveMode ? $_('search.placeholderArchive') : $_('search.placeholder');

  function handleNewNoteClick() {
    // Call parent handler
    onNewNote();
  }

  async function handleLockRequest() {
    if (rememberPasswordEnabled) {
      // If remember password is enabled, ask if they want to disable it
      showDisableRememberPasswordConfirm = true;
    } else {
      // Auto-save and sync before locking (no confirmation needed)
      await handleLockNow();
    }
  }

  async function handleLockNow() {
    isLocking.set(true);

    try {
      // Give any pending auto-saves time to complete (EditorPane has 1s debounce)
      await new Promise(resolve => setTimeout(resolve, 1200));

      // Trigger sync if enabled (will save all notes to server)
      if ($settings.syncEnabled) {
        const syncMetadata = await syncRepository.getMetadata();
        if (syncMetadata?.apiKey) {
          await syncService.syncNow();
        }
      }
    } catch (error) {
      console.error('[Header] Error during pre-lock save/sync:', error);
      // Continue with lock even if sync fails
    }

    // Lock the application
    lock();
    isLocked.set(true);
    isLocking.set(false);
  }

  async function handleDisableRememberPasswordConfirm() {
    showDisableRememberPasswordConfirm = false;

    // Clear stored password
    await passwordStorageService.clear();

    // Save the setting
    try {
      await settingsRepository.update({ rememberPassword: false });
      settings.update(s => ({ ...s, rememberPassword: false }));
    } catch (error) {
      console.error('Failed to save disabled setting:', error);
    }

    // Lock the application (with auto-save/sync)
    await handleLockNow();
  }

  function handleDisableRememberPasswordCancel() {
    showDisableRememberPasswordConfirm = false;
  }

  function toggleMobileMenu() {
    showMobileMenu = !showMobileMenu;
  }

  function closeMobileMenu() {
    showMobileMenu = false;
  }

  // Tag autocomplete functions
  function getTagPartial(query: string): { partial: string; startIndex: number } | null {
    // Find the last # in the query that's being typed (not followed by space)
    const lastHashIndex = query.lastIndexOf('#');
    if (lastHashIndex === -1) return null;

    // Extract text after the last #
    const afterHash = query.slice(lastHashIndex + 1);

    // If there's a space after the partial tag, user finished typing that tag
    if (afterHash.includes(' ')) return null;

    return { partial: afterHash, startIndex: lastHashIndex };
  }

  function handleSearchInput() {
    const tagInfo = getTagPartial($searchQuery);
    if (tagInfo && tagInfo.partial.length > 0) {
      // Get suggestions for the partial tag
      const partial = tagInfo.partial;
      const allTags = new Set<string>();

      $notes.forEach((note) => {
        note.tags.forEach((tag) => {
          if (localeIncludes(tag, partial)) {
            allTags.add(tag);
          }
        });
      });

      tagSuggestions = Array.from(allTags).slice(0, 5);
      showTagSuggestions = tagSuggestions.length > 0;
      selectedSuggestionIndex = -1;
    } else {
      showTagSuggestions = false;
      tagSuggestions = [];
    }
  }

  function selectTagSuggestion(tag: string) {
    const tagInfo = getTagPartial($searchQuery);
    if (tagInfo) {
      // Replace the partial tag with the full tag
      const before = $searchQuery.slice(0, tagInfo.startIndex);
      searchQuery.set(before + '#' + tag + ' ');
    }
    showTagSuggestions = false;
    tagSuggestions = [];
    selectedSuggestionIndex = -1;
  }

  function handleSearchKeyDown(e: KeyboardEvent) {
    if (!showTagSuggestions) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedSuggestionIndex = Math.min(selectedSuggestionIndex + 1, tagSuggestions.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedSuggestionIndex = Math.max(selectedSuggestionIndex - 1, -1);
    } else if (e.key === 'Tab' || e.key === 'Enter') {
      if (selectedSuggestionIndex >= 0 && tagSuggestions[selectedSuggestionIndex]) {
        e.preventDefault();
        selectTagSuggestion(tagSuggestions[selectedSuggestionIndex]);
      } else if (tagSuggestions.length > 0) {
        e.preventDefault();
        selectTagSuggestion(tagSuggestions[0]);
      }
    } else if (e.key === 'Escape') {
      showTagSuggestions = false;
      selectedSuggestionIndex = -1;
    }
  }

  function handleSearchBlur() {
    // Delay to allow click on suggestion
    setTimeout(() => {
      showTagSuggestions = false;
      selectedSuggestionIndex = -1;
    }, 200);
  }

  function toggleSavedSearches() {
    showSavedSearches = !showSavedSearches;
  }

  function closeSavedSearches() {
    showSavedSearches = false;
  }

  function handleApplySavedSearch(query: string) {
    searchQuery.set(query);
    closeSavedSearches();
  }
</script>

<header class="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-4 py-3 relative">
  <div class="flex items-center gap-2 {forceMobileLayout ? 'gap-2' : 'tablet:gap-4'}">
    {#if forceMobileLayout}
      <!-- Mobile: Back Button OR Sync indicator (when syncing and on list view) -->
      {#if onBackToList}
        <button
          on:click={onBackToList}
          class="min-h-11 min-w-11 p-2.5 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          title={$_('common.back')}
          aria-label={$_('common.back')}
        >
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>
      {:else if $isSyncing && $settings.syncEnabled}
        <!-- Sync indicator in back button space -->
        <div
          class="min-h-11 min-w-11 p-2.5 flex items-center justify-center text-blue-600 dark:text-blue-400"
          title={$_('sync.syncing')}
          role="status"
          aria-live="polite"
          aria-label={showSyncPercentage ? $_('sync.syncProgress', { values: { percentage: syncPercentage } }) : $_('sync.syncing')}
        >
          {#if showSyncPercentage}
            <!-- Percentage indicator for >= 5 items -->
            <div class="relative w-6 h-6" aria-hidden="true">
              <svg class="w-6 h-6 transform -rotate-90" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none"></circle>
                <circle
                  cx="12" cy="12" r="10"
                  stroke="currentColor"
                  stroke-width="3"
                  fill="none"
                  stroke-dasharray="62.83"
                  stroke-dashoffset={62.83 * (1 - syncPercentage / 100)}
                  stroke-linecap="round"
                  class="transition-all duration-300"
                ></circle>
              </svg>
              <span class="absolute inset-0 flex items-center justify-center text-[8px] font-bold">{syncPercentage}</span>
            </div>
          {:else}
            <!-- Spinner for < 5 items -->
            <svg class="w-6 h-6 animate-spin" fill="none" viewBox="0 0 24 24" aria-hidden="true">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
          {/if}
        </div>
      {:else}
        <!-- Greyed-out placeholder to prevent layout shift -->
        <div class="min-h-11 min-w-11 p-2.5 opacity-30">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
        </div>
      {/if}
    {:else}
      <button
        on:click={toggleMobileMenu}
        class="{forceMobileLayout ? '' : 'hidden'} min-h-11 min-w-11 p-2.5 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        title={$_('header.menu')}
        aria-label={$_('header.menu')}
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>
    {/if}

    <!-- Brand with sync indicator (desktop only - mobile shows in back button space) -->
    <div class="flex items-center gap-2">
      <h1 class="text-lg {forceMobileLayout ? '' : 'tablet:text-xl'} font-bold text-gray-900 dark:text-white">{$_('app.name')}</h1>
      {#if !forceMobileLayout && $isSyncing && $settings.syncEnabled}
        <div
          class="flex items-center gap-1 text-blue-600 dark:text-blue-400"
          title={$_('sync.syncing')}
          role="status"
          aria-live="polite"
          aria-label={showSyncPercentage ? $_('sync.syncProgress', { values: { percentage: syncPercentage } }) : $_('sync.syncing')}
        >
          {#if showSyncPercentage}
            <!-- Percentage indicator for >= 5 items -->
            <div class="relative w-4 h-4" aria-hidden="true">
              <svg class="w-4 h-4 transform -rotate-90" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none"></circle>
                <circle
                  cx="12" cy="12" r="10"
                  stroke="currentColor"
                  stroke-width="3"
                  fill="none"
                  stroke-dasharray="62.83"
                  stroke-dashoffset={62.83 * (1 - syncPercentage / 100)}
                  stroke-linecap="round"
                  class="transition-all duration-300"
                ></circle>
              </svg>
            </div>
            <span class="text-xs" aria-hidden="true">{syncPercentage}%</span>
          {:else}
            <!-- Spinner for < 5 items -->
            <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24" aria-hidden="true">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <span class="text-xs" aria-hidden="true">{$_('sync.syncing')}</span>
          {/if}
        </div>
      {/if}
    </div>

    {#if !forceMobileLayout}
      <!-- Desktop: Search Bar (always visible in desktop mode) -->
      <div class="flex items-center gap-2 flex-1 max-w-md">
        <div class="relative flex-1">
          <input
            id="search-input"
            data-testid="search-input"
            type="text"
            bind:value={$searchQuery}
            on:input={handleSearchInput}
            on:keydown={handleSearchKeyDown}
            on:blur={handleSearchBlur}
            placeholder={loadingNotes ? $_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } }) : searchPlaceholder}
            disabled={loadingNotes}
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={showTagSuggestions}
            aria-haspopup="listbox"
            aria-controls={showTagSuggestions ? 'tag-suggestions-desktop' : undefined}
            aria-activedescendant={showTagSuggestions && selectedSuggestionIndex >= 0 ? `tag-suggestion-desktop-${selectedSuggestionIndex}` : undefined}
            class="w-full px-3 py-1.5 pr-8 border rounded-md focus:outline-none focus:ring-2 disabled:opacity-60 disabled:cursor-wait {$archiveMode ? 'border-amber-400 dark:border-amber-600 bg-amber-50 dark:bg-amber-900/20 text-gray-900 dark:text-white focus:ring-amber-500' : 'border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-blue-500'}"
            style="font-size: {searchFontSize}"
          />
          {#if loadingNotes}
            <div class="absolute right-2 top-1/2 -translate-y-1/2">
              <div class="animate-spin rounded-full h-4 w-4 border-2 border-gray-300 dark:border-gray-600 border-t-blue-600"></div>
            </div>
          {:else if $searchQuery}
            <button
              on:click={() => searchQuery.set('')}
              class="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
              title={$_('search.clear')}
            >
              ✕
            </button>
          {/if}
          <!-- Tag suggestions dropdown -->
          {#if showTagSuggestions}
            <div
              id="tag-suggestions-desktop"
              role="listbox"
              aria-label={$_('search.tagSuggestions')}
              class="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto"
            >
              {#each tagSuggestions as suggestion, index}
                <div
                  id="tag-suggestion-desktop-{index}"
                  role="option"
                  aria-selected={index === selectedSuggestionIndex}
                  on:click={() => selectTagSuggestion(suggestion)}
                  on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && selectTagSuggestion(suggestion)}
                  tabindex="-1"
                  class="w-full text-left px-3 py-2 text-sm cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
                >
                  #{suggestion}
                </div>
              {/each}
            </div>
          {/if}
        </div>
        <button
          on:click={toggleSavedSearches}
          class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors flex items-center gap-1"
          title={$_('search.savedSearches')}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
          </svg>
        </button>
        {#if $searchResultCount.isSearching}
          <span
            class="text-xs text-gray-500 dark:text-gray-400 tabular-nums whitespace-nowrap"
            title={$_('search.resultCount')}
            aria-live="polite"
            aria-atomic="true"
          >
            <span class="sr-only">{$_('search.resultsFound', { values: { matches: $searchResultCount.matches, total: $searchResultCount.total } })}</span>
            <span aria-hidden="true">{$searchResultCount.matches}/{$searchResultCount.total}</span>
          </span>
        {/if}
      </div>
    {/if}

    {#if forceMobileLayout}
      <!-- Mobile: Always-visible search bar -->
      <div class="flex-1 ml-3 mr-1">
        <div class="relative">
          <input
            id="search-input-mobile"
            data-testid="search-input-mobile"
            type="text"
            bind:value={$searchQuery}
            on:input={handleSearchInput}
            on:keydown={handleSearchKeyDown}
            on:blur={handleSearchBlur}
            placeholder={loadingNotes ? $_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } }) : searchPlaceholder}
            disabled={loadingNotes}
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={showTagSuggestions}
            aria-haspopup="listbox"
            aria-controls={showTagSuggestions ? 'tag-suggestions-mobile' : undefined}
            aria-activedescendant={showTagSuggestions && selectedSuggestionIndex >= 0 ? `tag-suggestion-mobile-${selectedSuggestionIndex}` : undefined}
            class="w-full px-3 py-2 {$searchResultCount.isSearching ? 'pr-24' : 'pr-12'} border rounded-md focus:outline-none focus:ring-2 disabled:opacity-60 disabled:cursor-wait text-sm {$archiveMode ? 'border-amber-400 dark:border-amber-600 bg-amber-50 dark:bg-amber-900/20 text-gray-900 dark:text-white focus:ring-amber-500' : 'border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-blue-500'}"
            style="font-size: {searchFontSize}"
          />
          {#if loadingNotes}
            <div class="absolute right-2 top-1/2 -translate-y-1/2">
              <div class="animate-spin rounded-full h-4 w-4 border-2 border-gray-300 dark:border-gray-600 border-t-blue-600"></div>
            </div>
          {:else}
            <div class="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
              {#if $searchResultCount.isSearching}
                <span
                  class="text-xs text-gray-500 dark:text-gray-400 tabular-nums whitespace-nowrap"
                  aria-live="polite"
                  aria-atomic="true"
                >
                  <span class="sr-only">{$_('search.resultsFound', { values: { matches: $searchResultCount.matches, total: $searchResultCount.total } })}</span>
                  <span aria-hidden="true">{$searchResultCount.matches}/{$searchResultCount.total}</span>
                </span>
              {/if}
              <button
                on:click={toggleSavedSearches}
                class="text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 p-1"
                title={$_('search.savedSearches')}
                aria-label={$_('search.savedSearches')}
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                </svg>
              </button>
              {#if $searchQuery}
                <button
                  on:click={() => searchQuery.set('')}
                  class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 ml-1"
                  title={$_('search.clear')}
                >
                  ✕
                </button>
              {/if}
            </div>
          {/if}
          <!-- Tag suggestions dropdown (mobile) -->
          {#if showTagSuggestions}
            <div
              id="tag-suggestions-mobile"
              role="listbox"
              aria-label={$_('search.tagSuggestions')}
              class="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto"
            >
              {#each tagSuggestions as suggestion, index}
                <div
                  id="tag-suggestion-mobile-{index}"
                  role="option"
                  aria-selected={index === selectedSuggestionIndex}
                  on:click={() => selectTagSuggestion(suggestion)}
                  on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && selectTagSuggestion(suggestion)}
                  tabindex="-1"
                  class="w-full text-left px-3 py-3 min-h-11 text-sm cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
                >
                  #{suggestion}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Mobile: Menu and New Note buttons -->
      <div class="flex items-center gap-2">
        <!-- Hamburger menu (settings/recycle bin/lock) -->
        <button
          on:click={toggleMobileMenu}
          class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          title={$_('header.menu')}
          aria-label={$_('header.menu')}
          data-testid="btn-hamburger-menu"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>

        <button
          on:click={handleNewNoteClick}
          disabled={disableNewNote}
          class="min-h-11 min-w-11 p-3 bg-blue-600 active:bg-blue-700 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title={disableNewNote ? $_('header.creatingNote') : (newNoteShortcut ? `${$_('note.create')} (${newNoteShortcut})` : $_('note.create'))}
          aria-label="New note"
          data-testid="btn-new-note-mobile"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    {/if}

    {#if !forceMobileLayout}
      <!-- Desktop: Full Actions (always visible in desktop mode) -->
      <div class="flex items-center gap-2">
      <button
        on:click={handleNewNoteClick}
        disabled={$isDraftMode}
        class="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        title={$isDraftMode ? $_('header.addContentFirst') : (newNoteShortcut ? `${$_('note.create')} (${newNoteShortcut})` : $_('note.create'))}
        data-testid="btn-new-note"
      >
        + {$_('note.new')}
      </button>

      <button
        on:click={onOpenRecycleBin}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={$_('recycleBin.title')}
        data-testid="btn-recycle-bin"
      >
        🗑️ {$_('recycleBin.title')}
      </button>

      <button
        on:click={toggleArchiveMode}
        class="px-3 py-1.5 {$archiveMode ? 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400' : 'hover:bg-gray-100 dark:hover:bg-gray-700'} text-sm rounded-md transition-colors"
        title={$archiveMode ? $_('archive.exitMode') : $_('archive.enterMode')}
        data-testid="btn-archive"
      >
        📦 {$archiveMode ? $_('archive.exitMode') : $_('archive.mode')}
      </button>

      <button
        on:click={onOpenSettings}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={openSettingsShortcut ? `${$_('common.settings')} (${openSettingsShortcut})` : $_('common.settings')}
        data-testid="btn-settings"
      >
        ⚙️ {$_('common.settings')}
      </button>
    </div>

      <!-- Lock button (right-aligned) -->
      <button
        on:click={handleLockRequest}
        class="ml-auto px-3 py-1.5 {rememberPasswordEnabled ? 'opacity-50 cursor-default' : 'hover:bg-gray-100 dark:hover:bg-gray-700'} text-sm rounded-md transition-colors"
        title={rememberPasswordEnabled ? 'Password remembered - click to disable' : lockAppShortcut ? `${$_('keyboard.lockApp')} (${lockAppShortcut})` : $_('keyboard.lockApp')}
        data-testid="btn-lock"
      >
        {rememberPasswordEnabled ? '🔓' : '🔒'} {$_('common.lock')}
      </button>
    {/if}

    {#if notebook.id.startsWith('_') && !forceMobileLayout}
      <div class="block ml-auto">
        <span class="text-xs text-blue-600 dark:text-blue-400 font-medium opacity-75 truncate max-w-[200px] inline-block" title={notebook.displayName}>
          {notebook.displayName}
        </span>
      </div>
    {/if}
  </div>

  <!-- Mobile Menu Drawer -->
  <MobileMenuDrawer
    bind:show={showMobileMenu}
    {forceMobileLayout}
    {rememberPasswordEnabled}
    {openSettingsShortcut}
    {lockAppShortcut}
    archiveMode={$archiveMode}
    onClose={closeMobileMenu}
    onOpenRecycleBin={onOpenRecycleBin}
    onToggleArchiveMode={toggleArchiveMode}
    onOpenSettings={onOpenSettings}
    onLock={handleLockRequest}
  />
</header>

<ConfirmModal
  show={showDisableRememberPasswordConfirm}
  title={$_('confirm.disableRememberPassword.title')}
  message={$_('confirm.disableRememberPassword.message')}
  confirmText={$_('confirm.disableRememberPassword.confirmButton')}
  cancelText={$_('confirm.disableRememberPassword.cancelButton')}
  confirmClass="bg-orange-600 hover:bg-orange-700"
  onConfirm={handleDisableRememberPasswordConfirm}
  onCancel={handleDisableRememberPasswordCancel}
/>

<SavedSearchesPanel
  show={showSavedSearches}
  currentQuery={$searchQuery}
  onClose={closeSavedSearches}
  onApply={handleApplySavedSearch}
/>
