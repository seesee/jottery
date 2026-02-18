<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { selectedNoteId, settings } from '../stores/appStore';
  import { tick } from 'svelte';
  import type { KeyboardShortcut } from '../types';

  export let show: boolean = false;
  export let onClose: () => void;
  export let onAction: (actionId: string) => void;

  let searchText = '';
  let selectedIndex = 0;
  let searchInput: HTMLInputElement;
  let listContainer: HTMLDivElement;

  interface CommandAction {
    id: string;
    labelKey: string;
    shortcut?: KeyboardShortcut;
    editorOnly?: boolean;
  }

  interface CommandGroup {
    labelKey: string;
    actions: CommandAction[];
  }

  function getGroups(): CommandGroup[] {
    const shortcuts = $settings.keyboardShortcuts;

    return [
      {
        labelKey: 'commandPalette.navigation',
        actions: [
          { id: 'focusSearch', labelKey: 'commandPalette.focusSearch', shortcut: shortcuts?.focusSearch },
          { id: 'newNote', labelKey: 'commandPalette.newNote', shortcut: shortcuts?.newNote },
          { id: 'openSettings', labelKey: 'commandPalette.openSettings', shortcut: shortcuts?.openSettings },
          { id: 'openRecycleBin', labelKey: 'commandPalette.openRecycleBin', shortcut: shortcuts?.openRecycleBin },
          { id: 'openArchive', labelKey: 'commandPalette.openArchive' },
          { id: 'lockApp', labelKey: 'commandPalette.lockApp', shortcut: shortcuts?.lockApp },
          { id: 'showShortcuts', labelKey: 'commandPalette.showShortcuts', shortcut: shortcuts?.showShortcuts },
        ],
      },
      {
        labelKey: 'commandPalette.editorActions',
        actions: [
          { id: 'togglePreview', labelKey: 'commandPalette.togglePreview', shortcut: shortcuts?.togglePreview, editorOnly: true },
          { id: 'toggleWordWrap', labelKey: 'commandPalette.toggleWordWrap', shortcut: shortcuts?.toggleWordWrap, editorOnly: true },
          { id: 'copyContent', labelKey: 'commandPalette.copyContent', shortcut: shortcuts?.copyNote, editorOnly: true },
          { id: 'copyLink', labelKey: 'commandPalette.copyLink', editorOnly: true },
          { id: 'duplicateNote', labelKey: 'commandPalette.duplicateNote', shortcut: shortcuts?.duplicateNote, editorOnly: true },
          { id: 'exportNote', labelKey: 'commandPalette.exportNote', shortcut: shortcuts?.exportNote, editorOnly: true },
          { id: 'printNote', labelKey: 'commandPalette.printNote', editorOnly: true },
          { id: 'noteInfo', labelKey: 'commandPalette.noteInfo', shortcut: shortcuts?.noteInfo, editorOnly: true },
          { id: 'versionHistory', labelKey: 'commandPalette.versionHistory', shortcut: shortcuts?.versionHistory, editorOnly: true },
          { id: 'deleteNote', labelKey: 'commandPalette.deleteNote', shortcut: shortcuts?.deleteNote, editorOnly: true },
          { id: 'lockNote', labelKey: 'commandPalette.lockNote', editorOnly: true },
          { id: 'setColor', labelKey: 'commandPalette.setColor', editorOnly: true },
          { id: 'archiveNote', labelKey: 'commandPalette.archiveNote', editorOnly: true },
        ],
      },
    ];
  }

  function formatShortcutDisplay(shortcut: KeyboardShortcut): string {
    const parts: string[] = [];
    if (shortcut.ctrl) parts.push('Ctrl');
    if (shortcut.alt) parts.push('Alt');
    if (shortcut.shift) parts.push('Shift');

    const key = shortcut.key.length === 1 && shortcut.key.match(/[a-z]/i)
      ? shortcut.key.toUpperCase()
      : shortcut.key;
    parts.push(key);

    return parts.join('+');
  }

  function fuzzyMatch(text: string, query: string): boolean {
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    // Simple substring match for fuzzy-ish behaviour
    const words = lowerQuery.split(/\s+/).filter(Boolean);
    return words.every(word => lowerText.includes(word));
  }

  interface FlatAction {
    id: string;
    label: string;
    groupLabel: string;
    shortcut?: KeyboardShortcut;
  }

  $: groups = getGroups();

  $: filteredGroups = (() => {
    const hasNote = !!$selectedNoteId;
    const query = searchText.trim();

    const result: { groupLabel: string; actions: FlatAction[] }[] = [];

    for (const group of groups) {
      const groupLabel = $_(group.labelKey);
      const filteredActions: FlatAction[] = [];

      for (const action of group.actions) {
        // Skip editor-only actions when no note is selected
        if (action.editorOnly && !hasNote) continue;

        const label = $_(action.labelKey);

        if (query === '' || fuzzyMatch(label, query) || fuzzyMatch(groupLabel, query)) {
          filteredActions.push({
            id: action.id,
            label,
            groupLabel,
            shortcut: action.shortcut,
          });
        }
      }

      if (filteredActions.length > 0) {
        result.push({ groupLabel, actions: filteredActions });
      }
    }

    return result;
  })();

  $: flatActions = filteredGroups.flatMap(g => g.actions);

  // Clamp selected index when results change
  $: if (selectedIndex >= flatActions.length) {
    selectedIndex = Math.max(0, flatActions.length - 1);
  }

  function executeAction(actionId: string) {
    onAction(actionId);
    resetState();
  }

  function resetState() {
    searchText = '';
    selectedIndex = 0;
  }

  function scrollSelectedIntoView() {
    if (!listContainer) return;
    const selected = listContainer.querySelector('[data-selected="true"]');
    if (selected) {
      selected.scrollIntoView({ block: 'nearest' });
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!show) return;

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, flatActions.length - 1);
        tick().then(scrollSelectedIntoView);
        break;
      case 'ArrowUp':
        event.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        tick().then(scrollSelectedIntoView);
        break;
      case 'Enter':
        event.preventDefault();
        if (flatActions[selectedIndex]) {
          executeAction(flatActions[selectedIndex].id);
        }
        break;
      case 'Escape':
        event.preventDefault();
        onClose();
        resetState();
        break;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
      resetState();
    }
  }

  // Auto-focus input when shown
  $: if (show) {
    tick().then(() => {
      if (searchInput) {
        searchInput.focus();
      }
    });
  }

  // Reset state when hidden
  $: if (!show) {
    resetState();
  }
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-start justify-center z-50 pt-[15vh]"
    on:click={handleBackdropClick}
    on:keydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-label={$_('commandPalette.title')}
    tabindex="-1"
  >
    <div class="bg-white dark:bg-gray-800 w-full max-w-lg rounded-lg shadow-2xl border border-gray-200 dark:border-gray-700 flex flex-col max-h-[60vh] mx-4">
      <!-- Search input -->
      <div class="border-b border-gray-200 dark:border-gray-700 px-4 py-3 flex items-center gap-3">
        <svg class="w-5 h-5 text-gray-400 dark:text-gray-500 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <input
          bind:this={searchInput}
          bind:value={searchText}
          type="text"
          placeholder={$_('commandPalette.placeholder')}
          class="flex-1 bg-transparent border-none outline-none text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 text-base"
          on:keydown={handleKeydown}
        />
        <kbd class="hidden sm:inline-block px-1.5 py-0.5 text-xs font-mono text-gray-400 dark:text-gray-500 bg-gray-100 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded">
          Esc
        </kbd>
      </div>

      <!-- Action list -->
      <div bind:this={listContainer} class="overflow-y-auto flex-1 py-2">
        {#if flatActions.length === 0}
          <div class="px-4 py-8 text-center text-sm text-gray-500 dark:text-gray-400">
            {$_('commandPalette.noResults')}
          </div>
        {:else}
          {#each filteredGroups as group}
            <div class="px-3 pt-2 pb-1">
              <span class="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
                {group.groupLabel}
              </span>
            </div>
            {#each group.actions as action}
              {@const globalIdx = flatActions.findIndex(a => a.id === action.id)}
              <button
                class="w-full flex items-center justify-between px-4 py-2 text-sm text-left transition-colors
                  {globalIdx === selectedIndex
                    ? 'bg-indigo-600 text-white'
                    : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'}"
                data-selected={globalIdx === selectedIndex}
                on:click={() => executeAction(action.id)}
                on:mouseenter={() => { selectedIndex = globalIdx; }}
              >
                <span class="truncate">{action.label}</span>
                {#if action.shortcut}
                  <kbd
                    class="ml-4 flex-shrink-0 px-1.5 py-0.5 text-xs font-mono rounded
                      {globalIdx === selectedIndex
                        ? 'bg-indigo-500 text-indigo-100 border border-indigo-400'
                        : 'text-gray-400 dark:text-gray-500 bg-gray-100 dark:bg-gray-700 border border-gray-300 dark:border-gray-600'}"
                  >
                    {formatShortcutDisplay(action.shortcut)}
                  </kbd>
                {/if}
              </button>
            {/each}
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
