<script lang="ts">
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
  import { formatTimestamp } from '../utils/timezone';
  import {
    shouldShowCheckbox,
    shouldShowDeleteButton,
  } from '../utils/noteListItemVisibility';

  export let note: DecryptedNote;
  export let index: number = 0;
  export let filteredNotes: DecryptedNote[] = [];
  export let onNoteSelect: (() => void) | undefined = undefined;
  export let onDeleteRequest: ((note: DecryptedNote) => void) | undefined = undefined;
  export let hasConflict: boolean = false;
  export let onConflictClick: ((note: DecryptedNote) => void) | undefined = undefined;
  export let forceMobileLayout: boolean = false;

  $: isSelected = $selectedNoteId === note.id;
  $: isMultiSelected = $selectedNoteIds.has(note.id);
  let isHovered = false;

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

  $: title = stripMarkdown(note.content.split('\n')[0] || 'Untitled');
  // Responsive preview length: shorter on mobile, longer on desktop
  $: previewLength = forceMobileLayout ? 60 : 100;

  // Build visibility state for the utility functions
  $: visibilityState = {
    isMultiSelectMode: $isMultiSelectMode,
    isSelected,
    isHovered,
    isPinned: note.pinned,
    forceMobileLayout,
  };

  // Use utility functions for visibility logic (tested in noteListItemVisibility.test.ts)
  $: showCheckbox = shouldShowCheckbox(visibilityState);
  $: showDeleteBtn = shouldShowDeleteButton(visibilityState);
  $: preview = note.content.split('\n').slice(1).join(' ').slice(0, previewLength);
  $: formattedDateStore = formatTimestamp(note.modifiedAt, 'date');

  function handleClick(event: MouseEvent) {
    const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
    const ctrlOrCmd = isMac ? event.metaKey : event.ctrlKey;

    if (ctrlOrCmd && !note.pinned) {
      // Ctrl/Cmd+click: toggle selection (not on pinned notes)
      toggleNoteSelection(note.id, index);
    } else if (event.shiftKey && $lastSelectedIndex !== null && !note.pinned) {
      // Shift+click: range selection (not on pinned notes)
      selectRange($lastSelectedIndex, index, filteredNotes);
    } else if ($isMultiSelectMode && !note.pinned) {
      // In multi-select mode, normal click toggles selection (not on pinned notes)
      toggleNoteSelection(note.id, index);
    } else {
      // Normal click: single selection
      // On mobile, if note not already selected, just select without navigating
      // This shows the checkbox/delete first; second tap will navigate
      if (onNoteSelect && forceMobileLayout && !isSelected) {
        selectNote(note.id);
        // Don't navigate yet - let user see the controls
      } else {
        selectNote(note.id);
        // Navigate to note (desktop or already-selected on mobile)
        if (onNoteSelect) {
          onNoteSelect();
        }
      }
    }
  }

  function handleCheckboxClick(event: MouseEvent | KeyboardEvent) {
    event.stopPropagation();
    if (!note.pinned) {
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
</script>

<button
  on:click={handleClick}
  on:mouseenter={() => isHovered = true}
  on:mouseleave={() => isHovered = false}
  class="note-list-item w-full text-left p-4 min-h-[60px] border-b border-gray-200 dark:border-gray-700 active:bg-gray-100 dark:active:bg-gray-700 transition-colors relative {isSelected ? 'bg-blue-50 dark:bg-blue-900/20 border-l-4 border-l-blue-500' : ''} {isMultiSelected ? 'bg-blue-100 dark:bg-blue-900/40' : ''}"
>
  <div class="flex items-start justify-between mb-1">
    <div class="flex items-center gap-2 flex-1 min-w-0">
      <!-- Multi-select checkbox: visibility logic is in noteListItemVisibility.ts -->
      {#if showCheckbox}
        <span
          on:click|stopPropagation={handleCheckboxClick}
          on:keydown={(e) => e.key === 'Enter' && handleCheckboxClick(e)}
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
      {#if hasConflict}
        <span
          on:click|stopPropagation={() => onConflictClick && onConflictClick(note)}
          on:keydown|stopPropagation={(e) => e.key === 'Enter' && onConflictClick && onConflictClick(note)}
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
      <h3 class="font-medium text-gray-900 dark:text-gray-100 truncate">
        {title}
      </h3>
    </div>
    {#if showDeleteBtn}
      <span
        on:click={handleDeleteClick}
        on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && handleDeleteClick(e)}
        role="button"
        tabindex="0"
        class="delete-button absolute top-2 right-2 p-0.5 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors cursor-pointer"
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
    {#if note.tags.length > 0}
      <div class="note-tags flex gap-1 flex-wrap justify-end ml-2">
        {#each note.tags.slice(0, 2) as tag}
          <span
            on:click={(e) => handleTagClick(e, tag)}
            on:keydown={(e) => e.key === 'Enter' && handleTagClick(e, tag)}
            role="button"
            tabindex="0"
            class="note-tag bg-gray-200 dark:bg-gray-700 hover:bg-blue-200 dark:hover:bg-blue-800 active:bg-blue-300 dark:active:bg-blue-700 px-2 py-1 rounded text-xs whitespace-nowrap transition-colors cursor-pointer"
            title={$_('noteList.filterByTag', { values: { tag } })}
          >
            #{tag}
          </span>
        {/each}
        {#if note.tags.length > 2}
          <span class="text-gray-400 flex-shrink-0">+{note.tags.length - 2}</span>
        {/if}
      </div>
    {/if}
  </div>
</button>

<style>
  .line-clamp-2 {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
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
</style>
