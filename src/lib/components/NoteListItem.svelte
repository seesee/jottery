<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { _ } from 'svelte-i18n';
  import type { DecryptedNote } from '../types';
  import {
    selectNote,
    selectedNoteId,
    searchQuery,
    selectedNoteIds,
    isMultiSelectMode,
    lastSelectedIndex,
    toggleNoteSelection,
    selectRange,
  } from '../stores/appStore';
  import { getColorHex, getTagColor } from '../services/colorService';
  import { formatTimestamp } from '../utils/timezone';
  import { stripMarkdownSignifiers, isMarkdownLanguage } from '../utils/markdownStrip';
  import { getNoteTitle, getTitleTagValue } from '../utils/noteTitle';
  import { getRegularTags } from '../utils/virtualTags';
  import {
    shouldShowCheckbox,
    shouldShowDeleteButton,
  } from '../utils/noteListItemVisibility';
  import NoteContextMenu from './NoteContextMenu.svelte';
  import {
    createSwipeState,
    handleSwipeStart,
    handleSwipeMove,
    handleSwipeEnd,
    getSwipeTransform,
    getActionOpacity,
    triggerHapticFeedback,
    triggerDeleteHapticFeedback,
    type SwipeState,
  } from '../utils/swipeGesture';

  export let note: DecryptedNote;
  export let index: number = 0;
  export let filteredNotes: DecryptedNote[] = [];
  export let onNoteSelect: (() => void) | undefined = undefined;
  export let onDeleteRequest: ((note: DecryptedNote) => void) | undefined = undefined;
  export let hasConflict: boolean = false;
  export let onConflictClick: ((note: DecryptedNote) => void) | undefined = undefined;
  export let forceMobileLayout: boolean = false;
  export let onUpdateTitle: ((noteId: string, title: string) => void) | undefined = undefined;
  export let onContextAction: ((action: string, note: DecryptedNote) => void) | undefined = undefined;

  // Context menu state (desktop only)
  let showContextMenu = false;
  let contextMenuX = 0;
  let contextMenuY = 0;

  $: isSelected = $selectedNoteId === note.id;
  $: isMultiSelected = $selectedNoteIds.has(note.id);
  let isHovered = false;
  let currentTheme: 'light' | 'dark' = 'light'; // Track theme reactively

  // Color support: compute note/tag colors based on theme
  $: noteBackgroundColor = note.color ? getColorHex(note.color, currentTheme) : undefined;

  let themeObserver: MutationObserver | null = null;

  onMount(() => {
    // Set up theme observer to watch for dark mode changes
    const updateTheme = () => {
      // Check actual DOM class instead of settings to handle forced themes
      currentTheme = document.documentElement.classList.contains('dark') ? 'dark' : 'light';
    };

    // Initial check
    updateTheme();

    // Watch for theme changes on document element
    themeObserver = new MutationObserver(updateTheme);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    });
  });

  onDestroy(() => {
    if (themeObserver) {
      themeObserver.disconnect();
      themeObserver = null;
    }
  });

  // Helper function to get tag color (uses reactive currentTheme)
  $: getTagBackgroundColor = (tag: string): string | undefined => {
    const tagColorName = getTagColor(tag);
    return tagColorName ? getColorHex(tagColorName, currentTheme) : undefined;
  };

  // Swipe gesture state
  let swipeState: SwipeState = createSwipeState();
  let wasThresholdCrossed = false; // Track for haptic feedback
  let isDeleting = false; // Track delete animation state

  // Inline title editing state
  let isEditingTitle = false;
  let editingTitleValue = '';
  let titleInputElement: HTMLInputElement;
  let shouldSaveOnBlur = true; // Track whether blur should save (false when cancelled)

  // Strip markdown formatting from the title
  function stripMarkdown(text: string): string {
    return text
      // Remove markdown headers (# ## ### etc.)
      .replace(/^#+\s+/, '')
      // Remove bold (**text** or __text__)
      .replace(/(\*\*|__)(.*?)\1/g, '$2')
      // Remove italic (*text* or _text_)
      .replace(/(\*|_)(.*?)\1/g, '$2')
      // Remove inline code (`text`)
      .replace(/`([^`]+)`/g, '$1')
      // Remove links but keep text [text](url)
      .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
      .trim();
  }

  $: title = stripMarkdown(getNoteTitle(note));
  // Responsive preview length: shorter on mobile, longer on desktop
  $: previewLength = forceMobileLayout ? 60 : 100;

  // Filter out virtual tags for display
  $: displayTags = getRegularTags(note.tags);

  // Build visibility state for the utility functions
  $: visibilityState = {
    isMultiSelectMode: $isMultiSelectMode,
    isSelected,
    isHovered,
    isPinned: note.pinned,
    isLocked: note.locked,
    forceMobileLayout,
  };

  // Use utility functions for visibility logic (tested in noteListItemVisibility.test.ts)
  $: showCheckbox = shouldShowCheckbox(visibilityState);
  $: showDeleteBtn = shouldShowDeleteButton(visibilityState);

  // Strip markdown from preview only for markdown language
  $: rawPreview = note.content.split('\n').slice(1).join(' ');
  $: preview = isMarkdownLanguage(note.syntaxLanguage)
    ? stripMarkdownSignifiers(rawPreview).slice(0, previewLength)
    : rawPreview.slice(0, previewLength);

  $: formattedDateStore = formatTimestamp(note.modifiedAt, 'date');

  // Computed swipe values for rendering
  $: swipeTransform = getSwipeTransform(swipeState);
  $: actionOpacity = getActionOpacity(swipeState);
  $: isSwipeActive = swipeState.isActive && !swipeState.isCancelled && swipeState.distance > 0;

  function handleClick(event: MouseEvent) {
    // Don't handle click if swipe was active
    if (swipeState.distance > 10) {
      return;
    }

    const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
    const ctrlOrCmd = isMac ? event.metaKey : event.ctrlKey;

    if (ctrlOrCmd && !note.pinned && !note.locked) {
      // Ctrl/Cmd+click: toggle selection (not on pinned or locked notes)
      toggleNoteSelection(note.id, index);
    } else if (event.shiftKey && $lastSelectedIndex !== null && !note.pinned && !note.locked) {
      // Shift+click: range selection (not on pinned or locked notes)
      selectRange($lastSelectedIndex, index, filteredNotes);
    } else if ($isMultiSelectMode && !note.pinned && !note.locked) {
      // In multi-select mode, normal click toggles selection (not on pinned or locked notes)
      toggleNoteSelection(note.id, index);
    } else {
      // Normal click: single selection AND open
      // Single tap opens immediately on both mobile and desktop
      // (swipe gestures provide checkbox/delete access on mobile)
      selectNote(note.id);
      if (onNoteSelect) {
        onNoteSelect();
      }
    }
  }

  function handleCheckboxClick(event: MouseEvent | KeyboardEvent) {
    event.stopPropagation();
    if (!note.pinned && !note.locked) {
      toggleNoteSelection(note.id, index);
    }
  }

  function handleTagClick(event: MouseEvent | KeyboardEvent, tag: string) {
    // Stop propagation to prevent note selection
    event.stopPropagation();
    event.preventDefault();

    // Set search query to filter by this tag
    searchQuery.set(`#${tag}`);
  }

  function handleDeleteClick(event: MouseEvent | KeyboardEvent) {
    event.stopPropagation();
    event.preventDefault();

    // Request deletion from parent
    if (onDeleteRequest) {
      onDeleteRequest(note);
    }
  }

  // Touch event handlers for swipe gestures (mobile only)
  function handleTouchStart(event: TouchEvent) {
    if (!forceMobileLayout) return;

    const touch = event.touches[0];
    swipeState = handleSwipeStart(touch.clientX, touch.clientY);
    wasThresholdCrossed = false;
  }

  function handleTouchMove(event: TouchEvent) {
    if (!forceMobileLayout || !swipeState.isActive) return;

    const touch = event.touches[0];
    const previousThreshold = swipeState.isPastThreshold;
    swipeState = handleSwipeMove(swipeState, touch.clientX, touch.clientY);

    // Haptic feedback when crossing threshold
    if (!previousThreshold && swipeState.isPastThreshold && !wasThresholdCrossed) {
      wasThresholdCrossed = true;
      triggerHapticFeedback();
    }

    // Prevent scrolling while actively swiping horizontally
    if (swipeState.isActive && !swipeState.isCancelled && swipeState.distance > 10) {
      event.preventDefault();
    }
  }

  function handleTouchEnd() {
    if (!forceMobileLayout || !swipeState.isActive) return;

    const result = handleSwipeEnd(swipeState);

    // Execute action based on swipe direction
    if (result.action === 'delete') {
      // Swipe left = delete (unless pinned or locked)
      if (!note.pinned && !note.locked && onDeleteRequest) {
        // Trigger stronger haptic feedback for delete action
        triggerDeleteHapticFeedback();

        // Start delete animation - whoosh off screen
        isDeleting = true;

        // Wait for animation to complete before actually deleting
        setTimeout(() => {
          onDeleteRequest(note);
        }, 300); // Match animation duration
      } else {
        swipeState = result.state;
      }
    } else if (result.action === 'select') {
      // Swipe right = toggle multi-select (unless pinned or locked)
      if (!note.pinned && !note.locked) {
        triggerHapticFeedback();
        toggleNoteSelection(note.id, index);
      }
      swipeState = result.state;
    } else {
      swipeState = result.state;
    }

    wasThresholdCrossed = false;
  }

  function handleTouchCancel() {
    swipeState = createSwipeState();
    wasThresholdCrossed = false;
  }

  // Inline title editing handlers
  function handleTitleDoubleClick(e: MouseEvent) {
    e.stopPropagation();
    e.preventDefault();

    // Don't allow title editing on pinned or locked notes
    if (note.pinned || note.locked) {
      return;
    }

    isEditingTitle = true;
    shouldSaveOnBlur = true;
    // If note has custom title tag, use that value; otherwise use the extracted title
    const customTitleValue = getTitleTagValue(note.tags);
    editingTitleValue = customTitleValue !== undefined ? customTitleValue : title;
    // Focus input after render
    tick().then(() => {
      if (titleInputElement) {
        titleInputElement.focus();
        titleInputElement.select();
      }
    });
  }

  function handleTitleSave() {
    if (onUpdateTitle && editingTitleValue.trim()) {
      onUpdateTitle(note.id, editingTitleValue.trim());
    }
    isEditingTitle = false;
  }

  function handleTitleCancel() {
    shouldSaveOnBlur = false;
    isEditingTitle = false;
  }

  function handleTitleBlur() {
    // Only save on blur if not cancelled (Escape key)
    if (shouldSaveOnBlur) {
      handleTitleSave();
    }
  }

  function handleTitleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleTitleSave();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      handleTitleCancel();
    }
  }

  function handleTitleInputClick(e: MouseEvent) {
    // Prevent click from propagating to parent (which would select the note)
    e.stopPropagation();
  }

  function handleContextMenu(e: MouseEvent) {
    // Only show on desktop (non-mobile layout)
    if (forceMobileLayout) return;

    e.preventDefault();
    e.stopPropagation();
    contextMenuX = e.clientX;
    contextMenuY = e.clientY;
    showContextMenu = true;
  }

  function handleContextAction(action: string, contextNote: DecryptedNote) {
    if (action === 'open') {
      selectNote(contextNote.id);
      if (onNoteSelect) onNoteSelect();
    } else if (action === 'delete' && onDeleteRequest) {
      onDeleteRequest(contextNote);
    } else if (onContextAction) {
      onContextAction(action, contextNote);
    }
  }
</script>

<!-- Swipe container wrapper (mobile only) -->
{#if forceMobileLayout}
  <div class="swipe-container relative overflow-hidden {isDeleting ? 'deleting' : ''}">
    <!-- Multi-select action (left side, revealed when swiping right) - not for pinned or locked notes -->
    {#if !note.pinned && !note.locked}
      <div
        class="swipe-action-left absolute inset-y-0 left-0 w-20 bg-blue-500 flex items-center justify-center"
        style="opacity: {swipeState.direction === 'right' ? actionOpacity : 0}"
      >
        <svg class="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
        </svg>
      </div>
    {/if}

    <!-- Delete action (right side, revealed when swiping left) -->
    {#if !note.pinned && !note.locked}
      <div
        class="swipe-action-right absolute inset-y-0 right-0 w-20 bg-red-500 flex items-center justify-center"
        style="opacity: {swipeState.direction === 'left' ? actionOpacity : 0}"
      >
        <svg class="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
      </div>
    {/if}

    <!-- Note content (slides with swipe) -->
    <button
      on:click={handleClick}
      on:touchstart={handleTouchStart}
      on:touchmove={handleTouchMove}
      on:touchend={handleTouchEnd}
      on:touchcancel={handleTouchCancel}
      class="note-list-item w-full text-left p-4 min-h-[60px] border-b border-gray-200 dark:border-gray-700 relative {isSelected ? 'border-l-4 border-l-blue-500' : ''} {isMultiSelected ? 'bg-blue-100 dark:bg-blue-900/40' : noteBackgroundColor ? '' : 'bg-white dark:bg-gray-900'}"
      style="transform: translateX({swipeTransform}px); transition: {isSwipeActive ? 'none' : 'transform 0.2s ease-out'}; {noteBackgroundColor && !isMultiSelected ? `background-color: ${noteBackgroundColor};` : ''}"
      data-testid="note-list-item"
    >
      <div class="flex items-start justify-between mb-1">
        <div class="flex items-center gap-2 flex-1 min-w-0">
          <!-- Multi-select checkbox: visibility logic is in noteListItemVisibility.ts -->
          {#if showCheckbox}
            <span
              on:click|stopPropagation={handleCheckboxClick}
              on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), handleCheckboxClick(e))}
              role="checkbox"
              aria-checked={isMultiSelected}
              tabindex="0"
              class="flex-shrink-0 w-5 h-5 rounded border-2 flex items-center justify-center cursor-pointer transition-colors {isMultiSelected ? 'bg-blue-500 border-blue-500' : 'border-gray-400 dark:border-gray-500 hover:border-blue-400'}"
              title={$_('bulk.toggleSelect')}
            >
              {#if isMultiSelected}
                <svg class="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                </svg>
              {/if}
            </span>
          {/if}
          {#if note.pinned}
            <span class="text-yellow-500 flex-shrink-0">⭐</span>
          {/if}
          {#if note.locked}
            <span class="text-gray-500 dark:text-gray-400 flex-shrink-0" title={$_('editor.lock.locked')}>
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
              </svg>
            </span>
          {/if}
          {#if hasConflict}
            <span
              on:click|stopPropagation={() => onConflictClick && onConflictClick(note)}
              on:keydown|stopPropagation={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onConflictClick && onConflictClick(note))}
              role="button"
              tabindex="0"
              class="text-amber-500 flex-shrink-0 cursor-pointer hover:text-amber-600"
              title={$_('conflict.indicator')}
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </span>
          {/if}
          {#if isEditingTitle}
            <input
              bind:this={titleInputElement}
              bind:value={editingTitleValue}
              on:blur={handleTitleBlur}
              on:keydown={handleTitleKeydown}
              on:click={handleTitleInputClick}
              class="font-medium text-gray-900 dark:text-gray-100 bg-transparent border-b-2 border-blue-500 outline-none flex-1 min-w-0"
              type="text"
            />
          {:else}
            <h3
              on:dblclick={handleTitleDoubleClick}
              class="font-medium text-gray-900 dark:text-gray-100 truncate {!note.pinned && !note.locked ? 'cursor-text' : ''}"
              title={!note.pinned && !note.locked ? $_('note.editTitleTooltip') : undefined}
            >
              {title}
            </h3>
          {/if}
        </div>
      </div>

      {#if preview}
        <p class="text-sm text-gray-600 dark:text-gray-400 line-clamp-2 mb-1">
          {preview}
        </p>
      {/if}

      <div class="note-footer flex items-center justify-between text-xs text-gray-500 dark:text-gray-500">
        <span class="flex-shrink-0">{$formattedDateStore}</span>
        {#if displayTags.length > 0}
          <div class="note-tags flex gap-1 flex-wrap justify-end ml-2">
            {#each displayTags.slice(0, 2) as tag}
              {@const tagBgColor = getTagBackgroundColor(tag)}
              <span
                on:click={(e) => handleTagClick(e, tag)}
                on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), handleTagClick(e, tag))}
                role="button"
                tabindex="0"
                class="note-tag px-2 py-1 rounded text-xs whitespace-nowrap transition-colors cursor-pointer hover:underline {tagBgColor ? 'text-gray-900 dark:text-gray-100' : 'bg-gray-200 dark:bg-gray-600 text-gray-800 dark:text-gray-300 hover:bg-blue-200 dark:hover:bg-blue-700 active:bg-blue-300 dark:active:bg-blue-600'}"
                style:background-color={tagBgColor}
                title={$_('noteList.filterByTag', { values: { tag } })}
              >
                #{tag}
              </span>
            {/each}
            {#if displayTags.length > 2}
              <span class="text-gray-500 dark:text-gray-400 flex-shrink-0">+{displayTags.length - 2}</span>
            {/if}
          </div>
        {/if}
      </div>
    </button>
  </div>
{:else}
  <!-- Desktop: No swipe container -->
  <button
    on:click={handleClick}
    on:contextmenu={handleContextMenu}
    on:mouseenter={() => isHovered = true}
    on:mouseleave={() => isHovered = false}
    class="note-list-item w-full text-left p-4 min-h-[60px] border-b border-gray-200 dark:border-gray-700 active:bg-gray-100 dark:active:bg-gray-700 transition-colors relative {isSelected ? 'border-l-4 border-l-blue-500' : ''} {isMultiSelected ? 'bg-blue-100 dark:bg-blue-900/40' : ''}"
    style:background-color={!isMultiSelected && noteBackgroundColor ? noteBackgroundColor : undefined}
    data-testid="note-list-item"
  >
    <div class="flex items-start justify-between mb-1">
      <div class="flex items-center gap-2 flex-1 min-w-0">
        <!-- Multi-select checkbox: visibility logic is in noteListItemVisibility.ts -->
        {#if showCheckbox}
          <span
            on:click|stopPropagation={handleCheckboxClick}
            on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), handleCheckboxClick(e))}
            role="checkbox"
            aria-checked={isMultiSelected}
            tabindex="0"
            class="flex-shrink-0 w-5 h-5 rounded border-2 flex items-center justify-center cursor-pointer transition-colors {isMultiSelected ? 'bg-blue-500 border-blue-500' : 'border-gray-400 dark:border-gray-500 hover:border-blue-400'}"
            title={$_('bulk.toggleSelect')}
          >
            {#if isMultiSelected}
              <svg class="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
              </svg>
            {/if}
          </span>
        {/if}
        {#if note.pinned}
          <span class="text-yellow-500 flex-shrink-0">⭐</span>
        {/if}
        {#if note.locked}
          <span class="text-gray-500 dark:text-gray-400 flex-shrink-0" title={$_('editor.lock.locked')}>
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
            </svg>
          </span>
        {/if}
        {#if hasConflict}
          <span
            on:click|stopPropagation={() => onConflictClick && onConflictClick(note)}
            on:keydown|stopPropagation={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onConflictClick && onConflictClick(note))}
            role="button"
            tabindex="0"
            class="text-amber-500 flex-shrink-0 cursor-pointer hover:text-amber-600"
            title={$_('conflict.indicator')}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </span>
        {/if}
        {#if isEditingTitle}
          <input
            bind:this={titleInputElement}
            bind:value={editingTitleValue}
            on:blur={handleTitleBlur}
            on:keydown={handleTitleKeydown}
            on:click={handleTitleInputClick}
            class="font-medium text-gray-900 dark:text-gray-100 bg-transparent border-b-2 border-blue-500 outline-none flex-1 min-w-0"
            type="text"
          />
        {:else}
          <h3
            on:dblclick={handleTitleDoubleClick}
            class="font-medium text-gray-900 dark:text-gray-100 truncate {!note.pinned && !note.locked ? 'cursor-text' : ''}"
            title={!note.pinned && !note.locked ? $_('note.editTitleTooltip') : undefined}
          >
            {title}
          </h3>
        {/if}
      </div>
      {#if showDeleteBtn}
        <span
          on:click={handleDeleteClick}
          on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && handleDeleteClick(e)}
          role="button"
          tabindex="0"
          class="delete-button absolute top-2 right-2 p-0.5 text-gray-500 hover:text-red-600 dark:text-gray-400 dark:hover:text-red-400 transition-colors cursor-pointer"
          title={$_('noteList.deleteNote')}
          aria-label={$_('noteList.deleteNote')}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </span>
      {/if}
    </div>

    {#if preview}
      <p class="text-sm text-gray-600 dark:text-gray-400 line-clamp-2 mb-1">
        {preview}
      </p>
    {/if}

    <div class="note-footer flex items-center justify-between text-xs text-gray-500 dark:text-gray-500">
      <span class="flex-shrink-0">{$formattedDateStore}</span>
      {#if displayTags.length > 0}
        <div class="note-tags flex gap-1 flex-wrap justify-end ml-2">
          {#each displayTags.slice(0, 2) as tag}
            {@const tagBgColor = getTagBackgroundColor(tag)}
            <span
              on:click={(e) => handleTagClick(e, tag)}
              on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), handleTagClick(e, tag))}
              role="button"
              tabindex="0"
              class="note-tag px-2 py-1 rounded text-xs whitespace-nowrap transition-colors cursor-pointer hover:underline {tagBgColor ? 'text-gray-900 dark:text-gray-100' : 'bg-gray-200 dark:bg-gray-600 text-gray-800 dark:text-gray-300 hover:bg-blue-200 dark:hover:bg-blue-700 active:bg-blue-300 dark:active:bg-blue-600'}"
              style:background-color={tagBgColor}
              title={$_('noteList.filterByTag', { values: { tag } })}
            >
              #{tag}
            </span>
          {/each}
          {#if displayTags.length > 2}
            <span class="text-gray-500 dark:text-gray-400 flex-shrink-0">+{displayTags.length - 2}</span>
          {/if}
        </div>
      {/if}
    </div>
  </button>

  <!-- Context menu (desktop only) -->
  <NoteContextMenu
    show={showContextMenu}
    x={contextMenuX}
    y={contextMenuY}
    {note}
    onClose={() => showContextMenu = false}
    onAction={handleContextAction}
  />
{/if}

<style>
  .line-clamp-2 {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .swipe-container {
    touch-action: pan-y;
  }

  .swipe-action-left,
  .swipe-action-right {
    pointer-events: none;
  }

  /* Mobile improvements for better touch targets and spacing */
  @media (max-width: 768px) {
    .note-list-item {
      padding: 12px 16px;
      min-height: 80px;
    }

    .delete-button {
      top: 8px;
      right: 8px;
      padding: 8px;
      min-width: 44px;
      min-height: 44px;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .note-footer {
      gap: 12px;
      flex-wrap: wrap;
    }

    .note-tags {
      gap: 8px;
    }

    .note-tag {
      padding: 8px 12px;
      min-height: 36px;
      display: inline-flex;
      align-items: center;
    }
  }

  /* Delete animation - whoosh off screen */
  .swipe-container.deleting {
    animation: whoosh-delete 0.3s ease-out forwards;
  }

  @keyframes whoosh-delete {
    0% {
      transform: translateX(0);
      opacity: 1;
      max-height: 200px;
    }
    50% {
      transform: translateX(-100%);
      opacity: 0.5;
      max-height: 200px;
    }
    100% {
      transform: translateX(-100%);
      opacity: 0;
      max-height: 0;
      margin-bottom: 0;
      padding: 0;
      border: none;
    }
  }
</style>
