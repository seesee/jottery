<script lang="ts">
  import { searchQuery, isLocked, isLocking, settings, isDraftMode, searchResultCount, notes } from '../stores/appStore';
  import { lock, passwordStorageService, settingsRepository, syncService, syncRepository } from '../services';
  import { getCurrentNotebook } from '../utils/notebookPath';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from './ConfirmModal.svelte';
  import { formatShortcutForTooltip } from '../utils/keyboardShortcuts';
  import { getFontSizeCSS } from '../utils/fontSize';

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
  let menuClosing = false;
  let showMobileSearch = false;

  // Tag autocomplete state
  let tagSuggestions: string[] = [];
  let showTagSuggestions = false;
  let selectedSuggestionIndex = -1;

  // Get current notebook info for display
  const notebook = getCurrentNotebook();

  // Check if remember password is enabled
  $: rememberPasswordEnabled = $settings.rememberPassword || false;

  // Compute font size from settings (prevents Safari zoom on mobile)
  $: searchFontSize = getFontSizeCSS($settings.fontSize);

  // Format keyboard shortcuts for display
  $: shortcuts = $settings.keyboardShortcuts;
  $: newNoteShortcut = formatShortcutForTooltip(shortcuts?.newNote);
  $: focusSearchShortcut = formatShortcutForTooltip(shortcuts?.focusSearch);
  $: openSettingsShortcut = formatShortcutForTooltip(shortcuts?.openSettings);
  $: lockAppShortcut = formatShortcutForTooltip(shortcuts?.lockApp);

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
    passwordStorageService.clear();

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

  function toggleMobileSearch() {
    showMobileSearch = !showMobileSearch;
    if (showMobileSearch) {
      // Focus search input after showing
      setTimeout(() => {
        document.getElementById('search-input-mobile')?.focus();
      }, 100);
    }
  }

  function closeMobileMenu() {
    menuClosing = true;
    // Wait for slide-out animation to complete
    setTimeout(() => {
      showMobileMenu = false;
      menuClosing = false;
    }, 300);
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
      const partial = tagInfo.partial.toLowerCase();
      const allTags = new Set<string>();

      $notes.forEach((note) => {
        note.tags.forEach((tag) => {
          if (tag.toLowerCase().includes(partial)) {
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
</script>

<header class="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-4 py-3 relative">
  <div class="flex items-center gap-2 {forceMobileLayout ? 'gap-2' : 'tablet:gap-4'}">
    {#if forceMobileLayout}
      <!-- Mobile: Back Button (active when viewing note, greyed placeholder on list) -->
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
        class="tablet:hidden min-h-11 min-w-11 p-2.5 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        title={$_('header.menu')}
        aria-label={$_('header.menu')}
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>
    {/if}

    <!-- Brand -->
    <h1 class="text-lg {forceMobileLayout ? '' : 'tablet:text-xl'} font-bold text-gray-900 dark:text-white">{$_('app.name')}</h1>

    {#if !forceMobileLayout}
      <!-- Desktop: Search Bar -->
      <div class="hidden tablet:flex items-center gap-2 flex-1 max-w-md">
        <div class="relative flex-1">
          <input
            id="search-input"
            type="text"
            bind:value={$searchQuery}
            on:input={handleSearchInput}
            on:keydown={handleSearchKeyDown}
            on:blur={handleSearchBlur}
            placeholder={loadingNotes ? $_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } }) : $_('search.placeholder')}
            disabled={loadingNotes}
            class="w-full px-3 py-1.5 pr-8 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-60 disabled:cursor-wait"
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
            <div class="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto">
              {#each tagSuggestions as suggestion, index}
                <button
                  on:click={() => selectTagSuggestion(suggestion)}
                  class="w-full text-left px-3 py-2 text-sm hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
                >
                  #{suggestion}
                </button>
              {/each}
            </div>
          {/if}
        </div>
        {#if $searchResultCount.isSearching}
          <span class="text-xs text-gray-500 dark:text-gray-400 tabular-nums whitespace-nowrap" title={$_('search.resultCount')}>
            {$searchResultCount.matches}/{$searchResultCount.total}
          </span>
        {/if}
      </div>

      <div class="flex-1 tablet:hidden"></div>
    {/if}

    {#if forceMobileLayout}
      <!-- Mobile: Always-visible search bar -->
      <div class="flex-1 ml-1 mr-2">
        <div class="relative">
          <input
            id="search-input-mobile"
            type="text"
            bind:value={$searchQuery}
            on:input={handleSearchInput}
            on:keydown={handleSearchKeyDown}
            on:blur={handleSearchBlur}
            placeholder={loadingNotes ? $_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } }) : $_('search.placeholder')}
            disabled={loadingNotes}
            class="w-full px-3 py-2 pr-8 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-60 disabled:cursor-wait text-sm"
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
          <!-- Tag suggestions dropdown (mobile) -->
          {#if showTagSuggestions}
            <div class="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto">
              {#each tagSuggestions as suggestion, index}
                <button
                  on:click={() => selectTagSuggestion(suggestion)}
                  class="w-full text-left px-3 py-3 min-h-11 text-sm hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
                >
                  #{suggestion}
                </button>
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
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    {:else}
      <!-- Mobile: Essential Actions (responsive) -->
      <div class="flex tablet:hidden items-center gap-3">
        <button
          on:click={toggleMobileSearch}
          class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          title={focusSearchShortcut ? `${$_('search.placeholder')} (${focusSearchShortcut})` : $_('search.placeholder')}
          aria-label="Search"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </button>

        <button
          on:click={handleNewNoteClick}
          disabled={disableNewNote}
          class="min-h-11 min-w-11 p-3 bg-blue-600 active:bg-blue-700 text-white rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title={disableNewNote ? $_('header.creatingNote') : (newNoteShortcut ? `${$_('note.create')} (${newNoteShortcut})` : $_('note.create'))}
          aria-label="New note"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    {/if}

    {#if !forceMobileLayout}
      <!-- Desktop: Full Actions -->
      <div class="hidden tablet:flex items-center gap-2">
      <button
        on:click={handleNewNoteClick}
        disabled={$isDraftMode}
        class="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        title={$isDraftMode ? $_('header.addContentFirst') : (newNoteShortcut ? `${$_('note.create')} (${newNoteShortcut})` : $_('note.create'))}
      >
        + {$_('note.new')}
      </button>

      <button
        on:click={onOpenRecycleBin}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={$_('recycleBin.title')}
      >
        🗑️ {$_('recycleBin.title')}
      </button>

      <button
        on:click={onOpenSettings}
        class="px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm rounded-md transition-colors"
        title={openSettingsShortcut ? `${$_('common.settings')} (${openSettingsShortcut})` : $_('common.settings')}
      >
        ⚙️ {$_('common.settings')}
      </button>

      <button
        on:click={handleLockRequest}
        class="px-3 py-1.5 {rememberPasswordEnabled ? 'opacity-50 cursor-default' : 'hover:bg-gray-100 dark:hover:bg-gray-700'} text-sm rounded-md transition-colors"
        title={rememberPasswordEnabled ? 'Password remembered - click to disable' : lockAppShortcut ? `${$_('keyboard.lockApp')} (${lockAppShortcut})` : $_('keyboard.lockApp')}
      >
        {rememberPasswordEnabled ? '🔓' : '🔒'} {$_('common.lock')}
      </button>
    </div>
    {/if}

    {#if notebook.id !== 'main'}
      <div class="hidden tablet:block ml-auto">
        <span class="text-xs text-blue-600 dark:text-blue-400 font-medium opacity-75 truncate max-w-[200px] inline-block" title={notebook.displayName}>
          {notebook.displayName}
        </span>
      </div>
    {/if}
  </div>

  <!-- Mobile: Expandable Search Bar -->
  {#if showMobileSearch}
    <div class="{forceMobileLayout ? '' : 'tablet:hidden'} mt-3 animate-slide-down">
      <div class="flex items-center gap-2">
        <div class="relative flex-1">
          <input
            id="search-input-mobile"
            type="text"
            bind:value={$searchQuery}
            on:input={handleSearchInput}
            on:keydown={handleSearchKeyDown}
            on:blur={handleSearchBlur}
            placeholder={loadingNotes ? $_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } }) : $_('search.placeholder')}
            disabled={loadingNotes}
            class="w-full px-3 py-2 pr-8 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white dark:bg-gray-700 text-gray-900 dark:text-white disabled:opacity-60 disabled:cursor-wait"
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
          <!-- Tag suggestions dropdown (mobile) -->
          {#if showTagSuggestions}
            <div class="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-md shadow-lg max-h-48 overflow-y-auto">
              {#each tagSuggestions as suggestion, index}
                <button
                  on:click={() => selectTagSuggestion(suggestion)}
                  class="w-full text-left px-3 py-3 min-h-11 text-sm hover:bg-gray-100 dark:hover:bg-gray-700 {index === selectedSuggestionIndex ? 'bg-gray-100 dark:bg-gray-700' : ''}"
                >
                  #{suggestion}
                </button>
              {/each}
            </div>
          {/if}
        </div>
        {#if $searchResultCount.isSearching}
          <span class="text-xs text-gray-500 dark:text-gray-400 tabular-nums whitespace-nowrap">
            {$searchResultCount.matches}/{$searchResultCount.total}
          </span>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Mobile Menu Drawer -->
  {#if showMobileMenu}
    <!-- Backdrop -->
    <div
      class="{forceMobileLayout ? '' : 'tablet:hidden'} fixed inset-0 bg-black z-40 transition-opacity duration-300 {menuClosing ? 'opacity-0' : 'opacity-50'}"
      on:click={closeMobileMenu}
      on:keydown={(e) => e.key === 'Enter' && closeMobileMenu()}
      role="button"
      tabindex="-1"
      aria-label="Close menu"
    ></div>

    <!-- Drawer (slides in from right) -->
    <div class="{forceMobileLayout ? '' : 'tablet:hidden'} fixed right-0 top-0 bottom-0 w-64 bg-white dark:bg-gray-800 z-50 shadow-xl {menuClosing ? 'animate-slide-out-right' : 'animate-slide-in-right'}">
      <div class="flex flex-col h-full">
        <!-- Drawer Header -->
        <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
          <button
            on:click={closeMobileMenu}
            class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
            aria-label="Close menu"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
          <h2 class="text-lg font-bold text-gray-900 dark:text-white">{$_('header.menu')}</h2>
        </div>

        <!-- Menu Items -->
        <nav class="flex-1 overflow-y-auto p-2">
          <button
            on:click={() => { onOpenRecycleBin(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            <span class="text-xl">🗑️</span>
            <span class="flex-1 text-sm font-medium">{$_('recycleBin.title')}</span>
          </button>

          <button
            on:click={() => { onOpenSettings(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          >
            <span class="text-xl">⚙️</span>
            <span class="flex-1 text-sm font-medium">{$_('common.settings')}</span>
            {#if openSettingsShortcut}
              <span class="text-xs text-gray-500 dark:text-gray-400">{openSettingsShortcut}</span>
            {/if}
          </button>

          <div class="my-2 border-t border-gray-200 dark:border-gray-700"></div>

          <button
            on:click={() => { handleLockRequest(); closeMobileMenu(); }}
            class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left {rememberPasswordEnabled ? 'opacity-50' : 'active:bg-gray-100 dark:active:bg-gray-700'} rounded-md transition-colors"
          >
            <span class="text-xl">{rememberPasswordEnabled ? '🔓' : '🔒'}</span>
            <span class="flex-1 text-sm font-medium">{$_('common.lock')}</span>
            {#if lockAppShortcut && !rememberPasswordEnabled}
              <span class="text-xs text-gray-500 dark:text-gray-400">{lockAppShortcut}</span>
            {/if}
          </button>
        </nav>
      </div>
    </div>
  {/if}
</header>

<style>
  @keyframes slide-down {
    from {
      opacity: 0;
      transform: translateY(-10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes slide-in-left {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(0);
    }
  }

  @keyframes slide-in-right {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  @keyframes slide-out-right {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(100%);
    }
  }

  .animate-slide-down {
    animation: slide-down 0.2s ease-out;
  }

  .animate-slide-in-left {
    animation: slide-in-left 0.3s ease-out;
  }

  .animate-slide-in-right {
    animation: slide-in-right 0.3s ease-out;
  }

  .animate-slide-out-right {
    animation: slide-out-right 0.3s ease-out forwards;
  }
</style>

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
