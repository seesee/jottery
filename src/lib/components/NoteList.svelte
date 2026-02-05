<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { filteredNotes, notes, searchQuery, selectedNoteId, settings, isMultiSelectMode, selectAllFiltered, clearMultiSelection, noteListScrollPosition, archiveMode, toggleArchiveMode, isSyncing, inboxCount } from '../stores/appStore';
  import NoteListItem from './NoteListItem.svelte';
  import PullToRefresh from './PullToRefresh.svelte';
  import ConflictResolutionModal from './ConflictResolutionModal.svelte';
  import { beforeUpdate, afterUpdate, onMount } from 'svelte';
  import { noteRepository } from '../services/noteRepository';
  import { noteService } from '../services/noteService';
  import { keyManager } from '../services/keyManager';
  import { syncService } from '../services/syncService';
  import { syncRepository } from '../services/syncRepository';
  import { searchService } from '../services/searchService';
  import { isMobileTouchDevice } from '../utils/device';
  import type { DecryptedNote, KeyboardShortcut } from '../types';
  import {
    VIRTUAL_SCROLL_ESTIMATED_ITEM_HEIGHT,
    VIRTUAL_SCROLL_OVERSCAN,
    VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS,
    CONFLICT_CHECK_BATCH_SIZE,
    SEARCH_MAX_RESULTS_FOR_CROSS_MODE_HINT,
  } from '../constants';

  export let onNoteSelect: (() => void) | undefined = undefined;
  export let onOpenInbox: (() => void) | undefined = undefined;
  export let loadingNotes: boolean = false;
  export let loadingProgress: { current: number; total: number } = { current: 0, total: 0 };
  export let forceMobileLayout: boolean = false;

  let scrollContainer: HTMLDivElement;
  let savedScrollTop = 0;

  // Pull-to-refresh support
  let isMobile = false;

  // Handle pull-to-refresh
  async function handleRefresh() {
    // Always force a full re-render to recover from any corrupted virtual scroll state
    // This provides a recovery mechanism if the list gets into a bad state
    forceFullRender();

    // Only sync if enabled
    if ($settings.syncEnabled) {
      await syncService.syncNow();
      // Refresh conflict list after sync
      await loadConflictNotes();
    }
  }

  // Conflict resolution state
  let conflictNoteIds: Set<string> = new Set();
  let showConflictModal = false;
  let conflictNoteId: string | undefined = undefined;

  // Load notes with conflicts - optimised to avoid blocking UI
  async function loadConflictNotes() {
    // Only check for conflicts if sync is enabled
    if (!$settings.syncEnabled) {
      conflictNoteIds = new Set();
      return;
    }

    // Get all sync metadata in parallel batches to avoid blocking
    const allNotes = await noteRepository.getAllActive();
    const conflicts = new Set<string>();

    // Process in batches to avoid overwhelming the browser
    for (let i = 0; i < allNotes.length; i += CONFLICT_CHECK_BATCH_SIZE) {
      const batch = allNotes.slice(i, i + CONFLICT_CHECK_BATCH_SIZE);
      const results = await Promise.all(
        batch.map(async (note) => {
          const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
          return { id: note.id, isConflict: syncMeta?.lastSyncStatus === 'conflict' && !!syncMeta.conflictData };
        })
      );
      for (const result of results) {
        if (result.isConflict) {
          conflicts.add(result.id);
        }
      }
    }

    conflictNoteIds = conflicts;
  }

  function handleConflictClick(note: DecryptedNote) {
    conflictNoteId = note.id;
    showConflictModal = true;
  }

  function handleConflictResolved() {
    // Refresh conflict list and notes
    loadConflictNotes();
  }

  // Auto-trigger conflict resolution when selecting a conflicted note
  $: if ($selectedNoteId && conflictNoteIds.has($selectedNoteId) && !showConflictModal) {
    // Delay slightly to ensure the note selection completes first
    setTimeout(() => {
      if ($selectedNoteId && conflictNoteIds.has($selectedNoteId)) {
        conflictNoteId = $selectedNoteId;
        showConflictModal = true;
      }
    }, 100);
  }

  // Subscribe to sync state changes to reload conflicts after sync completes
  let wasSyncing = false;
  $: {
    const currentlySyncing = $isSyncing;
    if (wasSyncing && !currentlySyncing && $settings.syncEnabled) {
      // Sync just completed - reload conflict indicators
      loadConflictNotes();
    }
    wasSyncing = currentlySyncing;
  }

  // Virtual scrolling state
  let viewportHeight = 0;
  let scrollTop = 0;
  let startIndex = 0;
  let endIndex = 0;
  let visibleNotes: DecryptedNote[] = [];
  let totalHeight = 0;
  let offsetY = 0;

  // Cross-mode match indicator
  let crossModeMatchCount = 0;

  // Calculate cross-mode matches when search query or archive mode changes
  $: if ($searchQuery && $searchQuery.trim()) {
    searchService.countCrossModeMatches($searchQuery, $notes, $archiveMode).then(count => {
      crossModeMatchCount = count;
    });
  } else {
    // Count all notes in opposite mode when no search query
    crossModeMatchCount = $notes.filter(n => n.archived === !$archiveMode).length;
  }

  // Show cross-mode hint only when searching with few results
  $: showCrossModeHint = crossModeMatchCount > 0 && $searchQuery.trim() !== '' && $filteredNotes.length <= SEARCH_MAX_RESULTS_FOR_CROSS_MODE_HINT;

  // Height cache: stores measured heights for each note ID
  let heightCache = new Map<string, number>();
  let itemElements: (HTMLElement | null)[] = [];

  // Prefix sum cache for O(1) offset lookups
  // prefixSums[i] = sum of heights from index 0 to i-1 (prefixSums[0] = 0)
  let prefixSums: number[] = [];
  let cachedTotalHeight = 0;
  let prefixSumsValid = false;

  // Get height for a note (measured or estimated)
  function getItemHeight(index: number): number {
    if (index < 0 || index >= $filteredNotes.length) return VIRTUAL_SCROLL_ESTIMATED_ITEM_HEIGHT;
    const noteId = $filteredNotes[index].id;
    return heightCache.get(noteId) || VIRTUAL_SCROLL_ESTIMATED_ITEM_HEIGHT;
  }

  // Rebuild prefix sums array (called when heights change or notes change)
  function rebuildPrefixSums(): void {
    const len = $filteredNotes.length;
    prefixSums = new Array(len + 1);
    prefixSums[0] = 0;

    for (let i = 0; i < len; i++) {
      prefixSums[i + 1] = prefixSums[i] + getItemHeight(i);
    }

    cachedTotalHeight = prefixSums[len] || 0;
    prefixSumsValid = true;
  }

  // Calculate total height - O(1) with cache
  function calculateTotalHeight(): number {
    if (!prefixSumsValid) rebuildPrefixSums();
    return cachedTotalHeight;
  }

  // Calculate offset for items before startIndex - O(1) with prefix sums
  function calculateOffset(upToIndex: number): number {
    if (!prefixSumsValid) rebuildPrefixSums();
    if (upToIndex <= 0) return 0;
    if (upToIndex >= prefixSums.length) return cachedTotalHeight;
    return prefixSums[upToIndex];
  }

  // Find which item index contains a given scroll position - O(log n) with binary search
  function findIndexAtPosition(position: number): number {
    if (!prefixSumsValid) rebuildPrefixSums();
    if ($filteredNotes.length === 0) return 0;
    if (position <= 0) return 0;

    // Binary search for the largest index where prefixSums[index] <= position
    let left = 0;
    let right = $filteredNotes.length;

    while (left < right) {
      const mid = Math.floor((left + right) / 2);
      if (prefixSums[mid + 1] <= position) {
        left = mid + 1;
      } else {
        right = mid;
      }
    }

    return Math.min(left, $filteredNotes.length - 1);
  }

  // Invalidate prefix sums when notes change
  function invalidatePrefixSums(): void {
    prefixSumsValid = false;
  }

  // Calculate visible range based on scroll position
  function updateVisibleRange() {
    if (!scrollContainer) return;

    viewportHeight = scrollContainer.clientHeight;
    scrollTop = scrollContainer.scrollTop;

    // Guard against zero viewport height (happens when element is hidden)
    // Use a sensible default to prevent rendering too few items
    const effectiveViewportHeight = viewportHeight > 0 ? viewportHeight : 800;

    // Find start index based on scroll position (subtract VIRTUAL_SCROLL_OVERSCAN for buffer above)
    startIndex = Math.max(0, findIndexAtPosition(scrollTop) - VIRTUAL_SCROLL_OVERSCAN);

    // Find end index based on scroll position + viewport height (add VIRTUAL_SCROLL_OVERSCAN for buffer below)
    const endPosition = scrollTop + effectiveViewportHeight;
    let calculatedEndIndex = Math.min($filteredNotes.length, findIndexAtPosition(endPosition) + 1 + VIRTUAL_SCROLL_OVERSCAN);

    // Ensure we always render at least VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS (or all notes if fewer)
    const minEndIndex = Math.min(startIndex + VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS, $filteredNotes.length);
    endIndex = Math.max(calculatedEndIndex, minEndIndex);

    visibleNotes = $filteredNotes.slice(startIndex, endIndex);
    totalHeight = calculateTotalHeight();
    offsetY = calculateOffset(startIndex);
  }

  // Force a complete re-render of the virtual list
  // Call this to recover from corrupted states
  function forceFullRender() {
    // Reset all virtual scrolling state
    heightCache.clear();
    prefixSumsValid = false;
    startIndex = 0;
    endIndex = 0;
    visibleNotes = [];
    totalHeight = 0;
    offsetY = 0;

    // Rebuild everything
    invalidatePrefixSums();

    // Use requestAnimationFrame to ensure DOM is ready
    requestAnimationFrame(() => {
      if (scrollContainer) {
        updateVisibleRange();
      }
    });
  }

  // Measure heights of rendered items
  function measureHeights() {
    let heightsChanged = false;

    // Measure each visible item
    for (let i = 0; i < visibleNotes.length; i++) {
      const element = itemElements[i];
      const note = visibleNotes[i];

      if (element && note) {
        const height = element.offsetHeight;
        const cachedHeight = heightCache.get(note.id);

        if (height > 0 && height !== cachedHeight) {
          heightCache.set(note.id, height);
          heightsChanged = true;
        }
      }
    }

    // If heights changed, invalidate prefix sums and recalculate visible range
    if (heightsChanged) {
      invalidatePrefixSums();
      updateVisibleRange();
    }
  }

  // Handle scroll events
  function handleScroll() {
    updateVisibleRange();
    // Save scroll position to global store for preservation across mobile view switches
    if (scrollContainer) {
      noteListScrollPosition.set(scrollContainer.scrollTop);

      // Detect corrupted state during scroll: if we have many notes but very few visible
      const expectedMinVisible = Math.min($filteredNotes.length, VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS);
      if ($filteredNotes.length > VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS && visibleNotes.length < expectedMinVisible) {
        console.warn('[NoteList] Detected corrupted virtual scroll state during scroll, forcing full render');
        forceFullRender();
      }
    }
  }

  // Update visible range when filtered notes change
  $: if ($filteredNotes) {
    invalidatePrefixSums();
    updateVisibleRange();
  }

  let previousSelectedNoteId: string | null = null;

  // Scroll selected note into view when selection changes (not during manual scrolling)
  $: if ($selectedNoteId && scrollContainer && $selectedNoteId !== previousSelectedNoteId) {
    scrollToNote($selectedNoteId);
    previousSelectedNoteId = $selectedNoteId;
  }

  function scrollToNote(noteId: string) {
    const noteIndex = $filteredNotes.findIndex(n => n.id === noteId);
    if (noteIndex === -1) return;

    // Calculate the position of the note
    const notePosition = calculateOffset(noteIndex);
    const noteHeight = getItemHeight(noteIndex);

    const scrollTop = scrollContainer.scrollTop;
    const viewportHeight = scrollContainer.clientHeight;

    // Check if note is outside the viewport
    const isAboveViewport = notePosition < scrollTop;
    const isBelowViewport = notePosition + noteHeight > scrollTop + viewportHeight;

    if (isAboveViewport) {
      // Scroll to show note at top of viewport
      scrollContainer.scrollTop = notePosition;
    } else if (isBelowViewport) {
      // Scroll to show note at bottom of viewport
      scrollContainer.scrollTop = notePosition + noteHeight - viewportHeight;
    }
    // If note is already visible, don't scroll
  }

  // Initialize on mount
  onMount(() => {
    isMobile = isMobileTouchDevice();
    updateVisibleRange();
    // Load conflict notes on mount
    loadConflictNotes();

    // Restore scroll position from global store (preserved across mobile view switches)
    // iOS/WebKit needs multiple animation frames for virtual scrolling to settle
    const savedPosition = $noteListScrollPosition;
    if (savedPosition > 0 && scrollContainer) {
      // First frame: ensure initial DOM is rendered
      requestAnimationFrame(() => {
        if (!scrollContainer) return;

        // Second frame: virtual scrolling calculations have settled
        requestAnimationFrame(() => {
          if (!scrollContainer) return;

          scrollContainer.scrollTop = savedPosition;
          updateVisibleRange();

          // Third frame: verify scroll was applied (iOS sometimes needs this)
          requestAnimationFrame(() => {
            if (!scrollContainer) return;

            // If scroll wasn't applied (can happen on iOS with virtual scroll),
            // try again with a small delay
            if (Math.abs(scrollContainer.scrollTop - savedPosition) > 10) {
              scrollContainer.scrollTop = savedPosition;
              updateVisibleRange();
            }
          });
        });
      });
    }
  });

  // Track if we were visible before the update
  let wasVisible = false;

  // Save scroll position before DOM updates
  beforeUpdate(() => {
    if (scrollContainer) {
      // Only save if we're currently visible (offsetParent is non-null when visible)
      wasVisible = scrollContainer.offsetParent !== null;
      if (wasVisible) {
        savedScrollTop = scrollContainer.scrollTop;
      }
    }
  });

  // Restore scroll position after DOM updates and measure heights
  afterUpdate(() => {
    // Measure heights of rendered items
    measureHeights();

    if (scrollContainer) {
      const isVisible = scrollContainer.offsetParent !== null;

      // If we just became visible (was hidden, now visible), restore from global store
      if (isVisible && !wasVisible) {
        const savedPosition = $noteListScrollPosition;
        // Use requestAnimationFrame to ensure layout is settled
        requestAnimationFrame(() => {
          if (!scrollContainer) return;

          // Restore scroll position if we had one
          if (savedPosition > 0) {
            scrollContainer.scrollTop = savedPosition;
          }

          // Force recalculation when becoming visible
          updateVisibleRange();

          // Detect corrupted state: if we have many notes but very few visible, force full render
          const expectedMinVisible = Math.min($filteredNotes.length, VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS);
          if ($filteredNotes.length > VIRTUAL_SCROLL_MIN_VISIBLE_ITEMS && visibleNotes.length < expectedMinVisible) {
            console.warn('[NoteList] Detected corrupted virtual scroll state, forcing full render');
            forceFullRender();
          }
        });
      } else if (savedScrollTop >= 0) {
        // Normal case: restore scroll position after DOM update
        scrollContainer.scrollTop = savedScrollTop;
      }
    }
  });

  async function reloadNotes() {
    // Reload notes after deletion
    try {
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
    } catch (error) {
      console.error('Failed to reload notes:', error);
    }
  }

  async function requestDelete(note: DecryptedNote) {
    // Pinned notes should not reach here (UI prevents it), but double-check
    if (note.pinned) {
      console.warn('Attempted to delete pinned note');
      return;
    }

    // Soft delete - notes go to recycle bin
    try {
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      await noteRepository.softDelete(note.id);
      await reloadNotes();
    } catch (error) {
      console.error('Failed to delete note:', error);
    }
  }

  function matchesShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut): boolean {
    const ctrlOrCmd = event.ctrlKey || event.metaKey;
    return (
      event.key === shortcut.key &&
      (shortcut.ctrl ? ctrlOrCmd : !ctrlOrCmd) &&
      (shortcut.alt ? event.altKey : !event.altKey) &&
      (shortcut.shift ? event.shiftKey : !event.shiftKey)
    );
  }

  function handleKeydown(event: KeyboardEvent) {
    const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
    const ctrlOrCmd = isMac ? event.metaKey : event.ctrlKey;

    // Ctrl/Cmd+A: Select all filtered notes
    if (ctrlOrCmd && event.key.toLowerCase() === 'a') {
      event.preventDefault();
      selectAllFiltered($filteredNotes);
      return;
    }

    // Escape: Clear multi-selection
    if (event.key === 'Escape' && $isMultiSelectMode) {
      event.preventDefault();
      clearMultiSelection();
      return;
    }

    // Handle delete shortcut when a note is selected
    if ($selectedNoteId && $settings.keyboardShortcuts) {
      const deleteShortcut = $settings.keyboardShortcuts.deleteNote;
      if (matchesShortcut(event, deleteShortcut)) {
        const selectedNote = $filteredNotes.find(n => n.id === $selectedNoteId);
        if (selectedNote) {
          event.preventDefault();
          requestDelete(selectedNote);
        }
      }
    }
  }
</script>

{#if isMobile && $settings.syncEnabled}
  <PullToRefresh onRefresh={handleRefresh} enabled={$settings.syncEnabled}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      bind:this={scrollContainer}
      class="h-full overflow-y-auto bg-white dark:bg-gray-900"
      on:scroll={handleScroll}
      on:keydown={handleKeydown}
      role="list"
      tabindex="-1"
    >
      {#if $filteredNotes.length === 0}
        <div class="flex items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4 text-center">
          <div>
            {#if loadingNotes}
              <!-- Loading state with progress -->
              <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
              <p class="text-lg mb-2">{$_('common.loading')}</p>
              {#if loadingProgress.total > 0}
                <p class="text-sm">
                  {$_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } })}
                </p>
              {/if}
            {:else if $notes.length === 0}
              <p class="text-lg mb-2">{$_('note.noNotesYet')}</p>
              <p class="text-sm">{$_('note.createFirstNote')}</p>
            {:else if $searchQuery.trim()}
              <p class="text-lg mb-2">{$_('note.noResultsFound')}</p>
              <p class="text-sm">{$_('note.tryDifferentSearch')}</p>
            {:else if $archiveMode}
              <p class="text-lg mb-2">{$_('archive.empty')}</p>
              <p class="text-sm">{$_('archive.emptyDescription')}</p>
            {:else}
              <p class="text-lg mb-2">{$_('note.noNotes')}</p>
              <p class="text-sm">{$_('note.somethingWentWrong')}</p>
            {/if}
          </div>
        </div>
      {:else}
        <!-- Virtual scrolling container -->
        <div style="height: {totalHeight}px; position: relative;">
          <!-- Offset for items before viewport -->
          <div style="height: {offsetY}px;"></div>

          <!-- Render only visible items -->
          {#each visibleNotes as note, i (note.id)}
            <div bind:this={itemElements[i]}>
              <NoteListItem {note} index={startIndex + i} filteredNotes={$filteredNotes} {onNoteSelect} onDeleteRequest={requestDelete} hasConflict={conflictNoteIds.has(note.id)} onConflictClick={handleConflictClick} {forceMobileLayout} />
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </PullToRefresh>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={scrollContainer}
    class="h-full overflow-y-auto bg-white dark:bg-gray-900"
    on:scroll={handleScroll}
    on:keydown={handleKeydown}
    role="list"
    tabindex="-1"
  >
    {#if $filteredNotes.length === 0}
      <div class="flex items-center justify-center h-full text-gray-500 dark:text-gray-400 p-4 text-center">
        <div>
          {#if loadingNotes}
            <!-- Loading state with progress -->
            <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto mb-4"></div>
            <p class="text-lg mb-2">{$_('common.loading')}</p>
            {#if loadingProgress.total > 0}
              <p class="text-sm">
                {$_('header.loadingNotes', { values: { current: loadingProgress.current, total: loadingProgress.total } })}
              </p>
            {/if}
          {:else if $notes.length === 0}
            <p class="text-lg mb-2">{$_('note.noNotesYet')}</p>
            <p class="text-sm">{$_('note.createFirstNote')}</p>
          {:else if $searchQuery.trim()}
            <p class="text-lg mb-2">{$_('note.noResultsFound')}</p>
            <p class="text-sm">{$_('note.tryDifferentSearch')}</p>
          {:else if $archiveMode}
            <p class="text-lg mb-2">{$_('archive.empty')}</p>
            <p class="text-sm">{$_('archive.emptyDescription')}</p>
          {:else}
            <p class="text-lg mb-2">{$_('note.noNotes')}</p>
            <p class="text-sm">{$_('note.somethingWentWrong')}</p>
          {/if}
        </div>
      </div>
    {:else}
      <!-- Virtual scrolling container -->
      <div style="height: {totalHeight}px; position: relative;">
        <!-- Offset for items before viewport -->
        <div style="height: {offsetY}px;"></div>

        <!-- Render only visible items -->
        {#each visibleNotes as note, i (note.id)}
          <div bind:this={itemElements[i]}>
            <NoteListItem {note} index={startIndex + i} filteredNotes={$filteredNotes} {onNoteSelect} onDeleteRequest={requestDelete} hasConflict={conflictNoteIds.has(note.id)} onConflictClick={handleConflictClick} {forceMobileLayout} />
          </div>
        {/each}
      </div>

      <!-- Inbox indicator (only shown when inbox has items) -->
      {#if $inboxCount > 0 && onOpenInbox}
        <div class="px-4 py-3 border-t border-gray-200 dark:border-gray-700 bg-amber-50 dark:bg-amber-900/20 text-center">
          <button
            on:click={onOpenInbox}
            class="text-sm font-medium text-amber-700 dark:text-amber-400 hover:text-amber-800 dark:hover:text-amber-300"
          >
            {$_('inbox.indicator', { values: { count: $inboxCount } })}
          </button>
        </div>
      {/if}

      <!-- Cross-mode match indicator (only shown when searching with few results) -->
      {#if showCrossModeHint}
        <div class="px-4 py-3 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-center">
          <button
            on:click={toggleArchiveMode}
            class="text-sm text-gray-600 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 underline"
            title={$archiveMode ? $_('archive.crossModeHintActive') : $_('archive.crossModeHintArchive')}
          >
            {crossModeMatchCount} {crossModeMatchCount === 1 ? $_('note.matchSingular') : $_('note.matchPlural')} {$archiveMode ? $_('archive.inActive') : $_('archive.inArchive')}
          </button>
        </div>
      {/if}
    {/if}
  </div>
{/if}

<!-- Conflict Resolution Modal -->
<ConflictResolutionModal
  show={showConflictModal}
  noteId={conflictNoteId}
  onClose={() => { showConflictModal = false; conflictNoteId = undefined; }}
  onResolved={handleConflictResolved}
/>
