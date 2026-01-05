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

  $: shortcuts = [
    { name: 'focusSearch' as const, label: $_('settings.keyboardTab.focusSearch') },
    { name: 'newNote' as const, label: $_('settings.keyboardTab.createNewNote') },
    { name: 'lockApp' as const, label: $_('settings.keyboardTab.lockApplication') },
    { name: 'openSettings' as const, label: $_('settings.keyboardTab.openSettings') },
    { name: 'showShortcuts' as const, label: $_('settings.keyboardTab.showShortcutsHelp') },
    { name: 'copyNote' as const, label: $_('settings.keyboardTab.copyNoteContent') },
    { name: 'undo' as const, label: $_('settings.keyboardTab.undo') },
    { name: 'redo' as const, label: $_('settings.keyboardTab.redo') },
    { name: 'versionHistory' as const, label: $_('settings.keyboardTab.versionHistory') },
    { name: 'noteInfo' as const, label: $_('settings.keyboardTab.noteInfo') },
  ];
</script>

<div class="space-y-4">
  <p class="text-sm text-gray-600 dark:text-gray-400">
    {$_('settings.keyboardTab.description')}
  </p>

  <div class="space-y-2">
    {#each shortcuts as { name, label }}
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
      ⌨️ {$_('settings.keyboardTab.viewAllShortcuts')}
    </button>
    <p class="text-sm text-gray-500 dark:text-gray-400 text-center">
      {$_('settings.keyboardTab.quickReference')}
    </p>
  </div>
</div>
