<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { KeyboardShortcuts, KeyboardShortcut } from '../../types';

  export let tempShortcuts: KeyboardShortcuts;
  export let recordingShortcut: keyof KeyboardShortcuts | null;
  export let onStartRecording: (shortcutName: keyof KeyboardShortcuts) => void;
  export let onResetShortcuts: () => void;
  export let onOpenShortcutsHelp: () => void;

  function formatShortcutDisplay(shortcut: KeyboardShortcut | undefined): string {
    if (!shortcut) return $_('settings.keyboardTab.notSet');

    const parts: string[] = [];
    if (shortcut.ctrl) parts.push(navigator.platform.includes('Mac') ? 'Cmd' : 'Ctrl');
    if (shortcut.alt) parts.push(navigator.platform.includes('Mac') ? 'Option' : 'Alt');
    if (shortcut.shift) parts.push('Shift');
    parts.push(shortcut.key.toUpperCase());

    return parts.join(' + ');
  }

  // Grouped shortcuts for better organisation
  $: globalShortcuts = [
    { name: 'focusSearch' as const, label: $_('settings.keyboardTab.focusSearch') },
    { name: 'newNote' as const, label: $_('settings.keyboardTab.createNewNote') },
    { name: 'lockApp' as const, label: $_('settings.keyboardTab.lockApplication') },
    { name: 'openSettings' as const, label: $_('settings.keyboardTab.openSettings') },
    { name: 'showShortcuts' as const, label: $_('settings.keyboardTab.showShortcutsHelp') },
    { name: 'openRecycleBin' as const, label: $_('settings.keyboardTab.openRecycleBin') },
  ];

  $: noteListShortcuts = [
    { name: 'navigateUp' as const, label: $_('settings.keyboardTab.navigateUp') },
    { name: 'navigateDown' as const, label: $_('settings.keyboardTab.navigateDown') },
    { name: 'openNote' as const, label: $_('settings.keyboardTab.openNote') },
    { name: 'deleteNote' as const, label: $_('settings.keyboardTab.deleteNote') },
    { name: 'pinNote' as const, label: $_('settings.keyboardTab.pinNote') },
    { name: 'selectAll' as const, label: $_('settings.keyboardTab.selectAll') },
  ];

  $: editorShortcuts = [
    { name: 'copyNote' as const, label: $_('settings.keyboardTab.copyNoteContent') },
    { name: 'undo' as const, label: $_('settings.keyboardTab.undo') },
    { name: 'redo' as const, label: $_('settings.keyboardTab.redo') },
    { name: 'versionHistory' as const, label: $_('settings.keyboardTab.versionHistory') },
    { name: 'noteInfo' as const, label: $_('settings.keyboardTab.noteInfo') },
    { name: 'toggleWordWrap' as const, label: $_('settings.keyboardTab.toggleWordWrap') },
    { name: 'togglePreview' as const, label: $_('settings.keyboardTab.togglePreview') },
    { name: 'duplicateNote' as const, label: $_('settings.keyboardTab.duplicateNote') },
    { name: 'exportNote' as const, label: $_('settings.keyboardTab.exportNote') },
  ];
</script>

<div class="space-y-4">
  <p class="text-sm text-gray-600 dark:text-gray-400">
    {$_('settings.keyboardTab.description')}
  </p>

  <!-- Global Shortcuts -->
  <div>
    <h4 class="text-sm font-medium text-gray-800 dark:text-gray-200 mb-2">{$_('settings.keyboardTab.globalShortcuts')}</h4>
    <div class="space-y-1">
      {#each globalShortcuts as { name, label }}
        <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
          <span class="text-sm text-gray-700 dark:text-gray-300">{label}</span>
          <button
            on:click={() => onStartRecording(name)}
            class="px-3 py-1 text-xs font-mono {recordingShortcut === name ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
          >
            {recordingShortcut === name ? $_('settings.keyboardTab.pressKey') : formatShortcutDisplay(tempShortcuts[name])}
          </button>
        </div>
      {/each}
    </div>
  </div>

  <!-- Note List Shortcuts -->
  <div>
    <h4 class="text-sm font-medium text-gray-800 dark:text-gray-200 mb-2">{$_('settings.keyboardTab.noteListShortcuts')}</h4>
    <div class="space-y-1">
      {#each noteListShortcuts as { name, label }}
        <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
          <span class="text-sm text-gray-700 dark:text-gray-300">{label}</span>
          <button
            on:click={() => onStartRecording(name)}
            class="px-3 py-1 text-xs font-mono {recordingShortcut === name ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
          >
            {recordingShortcut === name ? $_('settings.keyboardTab.pressKey') : formatShortcutDisplay(tempShortcuts[name])}
          </button>
        </div>
      {/each}
    </div>
  </div>

  <!-- Editor Shortcuts -->
  <div>
    <h4 class="text-sm font-medium text-gray-800 dark:text-gray-200 mb-2">{$_('settings.keyboardTab.editorShortcuts')}</h4>
    <div class="space-y-1">
      {#each editorShortcuts as { name, label }}
        <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
          <span class="text-sm text-gray-700 dark:text-gray-300">{label}</span>
          <button
            on:click={() => onStartRecording(name)}
            class="px-3 py-1 text-xs font-mono {recordingShortcut === name ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
          >
            {recordingShortcut === name ? $_('settings.keyboardTab.pressKey') : formatShortcutDisplay(tempShortcuts[name])}
          </button>
        </div>
      {/each}
    </div>
  </div>

  <div class="mt-4 space-y-2">
    <button
      on:click={onResetShortcuts}
      class="w-full px-4 py-2 min-h-11 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors"
    >
      {$_('settings.keyboardTab.resetToDefaults')}
    </button>

    <button
      on:click={onOpenShortcutsHelp}
      class="w-full px-4 py-2 min-h-11 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
    >
      {$_('settings.keyboardTab.viewAllShortcuts')}
    </button>
    <p class="text-sm text-gray-500 dark:text-gray-400 text-center">
      {$_('settings.keyboardTab.quickReference')}
    </p>
  </div>
</div>
