<script lang="ts">
  import { _ } from 'svelte-i18n';

  // State props
  export let pinned: boolean;
  export let language: string;
  export let showPreview: boolean;
  export let canPreview: boolean;
  export let attachmentCount: number;
  export let forceMobileLayout: boolean;
  export let isDraftMode: boolean;
  export let wordWrap: boolean;
  export let availableLanguages: Array<{id: string, name: string}>;

  // Callback props
  export let onPin: () => void;
  export let onLanguageChange: (e: Event) => void;
  export let onTogglePreview: () => void;
  export let onShowAttachments: () => void;
  export let onUndo: () => void;
  export let onRedo: () => void;
  export let onWordWrapToggle: () => void;
  export let onCopy: () => void;
  export let onExport: () => void;
  export let onDuplicate: () => void;
  export let onShowInfo: () => void;
  export let onShowVersionHistory: () => void;
  export let onDelete: () => void;
  export let onClose: () => void;
  export let onBackToList: (() => void) | undefined = undefined;

  // Keyboard shortcuts (for tooltips)
  export let copyNoteShortcut: string | null = null;
  export let noteInfoShortcut: string | null = null;
  export let versionHistoryShortcut: string | null = null;

  // Internal state
  let showMoreMenu = false;

  function toggleMoreMenu() {
    showMoreMenu = !showMoreMenu;
  }

  // Wrapper functions that close menu before calling parent callbacks
  function handleUndoClick() {
    showMoreMenu = false;
    onUndo();
  }

  function handleRedoClick() {
    showMoreMenu = false;
    onRedo();
  }

  function handleWordWrapClick() {
    showMoreMenu = false;
    onWordWrapToggle();
  }

  function handleCopyClick() {
    showMoreMenu = false;
    onCopy();
  }

  function handleExportClick() {
    showMoreMenu = false;
    onExport();
  }

  function handleDuplicateClick() {
    showMoreMenu = false;
    onDuplicate();
  }

  function handleShowInfoClick() {
    showMoreMenu = false;
    onShowInfo();
  }

  function handleShowVersionHistoryClick() {
    showMoreMenu = false;
    onShowVersionHistory();
  }

  function handleDeleteClick() {
    showMoreMenu = false;
    onDelete();
  }
</script>

<!-- Toolbar -->
<div class="border-b border-gray-200 dark:border-gray-700 p-2 flex items-center gap-2">
  <!-- Mobile: Back Button -->
  {#if onBackToList}
    <button
      on:click={onBackToList}
      class="tablet:hidden p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors flex-shrink-0"
      title={$_('editor.backToNotes')}
      aria-label={$_('editor.backToNotes')}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
      </svg>
    </button>
  {/if}

  <!-- Pin Button (icon only) -->
  <button
    on:click={onPin}
    disabled={isDraftMode}
    class="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors flex-shrink-0 disabled:opacity-50 disabled:cursor-not-allowed"
    title={isDraftMode ? $_('editor.tooltips.saveNoteFirst') : (pinned ? $_('editor.tooltips.unpinNote') : $_('editor.tooltips.pinNote'))}
    aria-label={isDraftMode ? $_('editor.tooltips.saveNoteFirst') : (pinned ? $_('editor.tooltips.unpinNote') : $_('editor.tooltips.pinNote'))}
  >
    {#if pinned}
      <svg class="w-5 h-5 text-yellow-500" fill="currentColor" viewBox="0 0 24 24">
        <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
      </svg>
    {:else}
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
      </svg>
    {/if}
  </button>

  <!-- Syntax Language Selector -->
  <select
    value={language}
    on:change={onLanguageChange}
    class="px-2 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500 flex-shrink-0"
    title={$_('editor.syntaxHighlight')}
  >
    {#each availableLanguages as lang}
      <option value={lang.id}>{lang.name}</option>
    {/each}
  </select>

  <!-- Preview/Edit Toggle (shown when contextually appropriate) -->
  {#if canPreview}
    <button
      on:click={onTogglePreview}
      class="px-2 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 flex-shrink-0 flex items-center gap-1.5"
      title={showPreview ? $_('editor.edit') : $_('editor.preview')}
    >
      {#if showPreview}
        <span>📝</span>
        <span>{$_('editor.edit')}</span>
      {:else}
        <span>👁️</span>
        <span>{$_('editor.preview')}</span>
      {/if}
    </button>
  {/if}

  <!-- Attachments Button (Mobile only) -->
  {#if forceMobileLayout}
    <button
      on:click={onShowAttachments}
      class="px-2 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 flex-shrink-0 flex items-center gap-1.5"
      title={$_('editor.attachments')}
    >
      <span>📎</span>
      <span>{$_('editor.attachments')}</span>
      {#if attachmentCount > 0}
        <span class="px-1.5 py-0.5 text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-200 rounded">
          {attachmentCount}
        </span>
      {/if}
    </button>
  {/if}

  <div class="flex-1"></div>

  <!-- More Menu -->
  <div class="more-menu-container relative flex-shrink-0">
    <button
      on:click={toggleMoreMenu}
      class="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors"
      title={$_('editor.moreActions')}
      aria-label="More actions"
    >
      <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
        <path d="M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z" />
      </svg>
    </button>

    {#if showMoreMenu}
      <div class="absolute right-0 mt-1 w-48 bg-white dark:bg-gray-800 rounded-md shadow-lg border border-gray-200 dark:border-gray-700 z-50">
        <div class="py-1">
          <button
            on:click={handleUndoClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>↶</span>
            <span>{$_('editor.undo')}</span>
          </button>

          <button
            on:click={handleRedoClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>↷</span>
            <span>{$_('editor.redo')}</span>
          </button>

          <div class="border-t border-gray-200 dark:border-gray-700 my-1"></div>

          <button
            on:click={handleWordWrapClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            {#if wordWrap}
              <span>↩️</span>
              <span>{$_('editor.wordWrapOn')}</span>
            {:else}
              <span>➡️</span>
              <span>{$_('editor.wordWrapOff')}</span>
            {/if}
          </button>

          <button
            on:click={handleCopyClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>📋</span>
            <span class="flex-1">{$_('editor.copyContent')}</span>
            {#if copyNoteShortcut}
              <span class="text-xs text-gray-500 dark:text-gray-400">{copyNoteShortcut}</span>
            {/if}
          </button>

          <button
            on:click={handleExportClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>💾</span>
            <span class="flex-1">{$_('editor.exportToFile')}</span>
          </button>

          <button
            on:click={handleDuplicateClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>📄</span>
            <span class="flex-1">{$_('editor.duplicateNote')}</span>
          </button>

          <button
            on:click={handleShowInfoClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>ℹ️</span>
            <span class="flex-1">{$_('editor.info')}</span>
            {#if noteInfoShortcut}
              <span class="text-xs text-gray-500 dark:text-gray-400">{noteInfoShortcut}</span>
            {/if}
          </button>

          <button
            on:click={handleShowVersionHistoryClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 flex items-center gap-2"
          >
            <span>📜</span>
            <span class="flex-1">{$_('editor.versionHistory')}</span>
            {#if versionHistoryShortcut}
              <span class="text-xs text-gray-500 dark:text-gray-400">{versionHistoryShortcut}</span>
            {/if}
          </button>

          <div class="border-t border-gray-200 dark:border-gray-700 my-1"></div>

          <button
            on:click={handleDeleteClick}
            class="w-full px-4 py-2 text-left text-sm hover:bg-red-50 dark:hover:bg-red-900/20 text-red-600 dark:text-red-400 flex items-center gap-2"
          >
            <span>🗑️</span>
            <span>{$_('editor.deleteNote')}</span>
          </button>
        </div>
      </div>
    {/if}
  </div>

  <!-- Close Button (icon only) -->
  <button
    on:click={onClose}
    class="p-2 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors flex-shrink-0"
    title={$_('editor.closeNote')}
    aria-label="Close note"
  >
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
    </svg>
  </button>
</div>
