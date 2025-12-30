<script lang="ts">
  import type { KeyboardShortcuts, KeyboardShortcut } from '../../types';

  export let tempShortcuts: KeyboardShortcuts;
  export let recordingShortcut: keyof KeyboardShortcuts | null;
  export let onStartRecording: (shortcutName: keyof KeyboardShortcuts) => void;
  export let onResetShortcuts: () => void;
  export let onOpenShortcutsHelp: () => void;

  function formatShortcutDisplay(shortcut: KeyboardShortcut | undefined): string {
    if (!shortcut) return 'Not set';

    const parts: string[] = [];
    if (shortcut.ctrl) parts.push(navigator.platform.includes('Mac') ? 'Cmd' : 'Ctrl');
    if (shortcut.alt) parts.push(navigator.platform.includes('Mac') ? 'Option' : 'Alt');
    if (shortcut.shift) parts.push('Shift');
    if (shortcut.meta) parts.push('Meta');
    parts.push(shortcut.key.toUpperCase());

    return parts.join(' + ');
  }

  const shortcuts: Array<{ name: keyof KeyboardShortcuts; label: string }> = [
    { name: 'focusSearch', label: 'Focus Search' },
    { name: 'newNote', label: 'Create New Note' },
    { name: 'lockApp', label: 'Lock Application' },
    { name: 'openSettings', label: 'Open Settings' },
    { name: 'showShortcuts', label: 'Show Shortcuts Help' },
    { name: 'copyNote', label: 'Copy Note Content' },
    { name: 'undo', label: 'Undo' },
    { name: 'redo', label: 'Redo' },
    { name: 'versionHistory', label: 'Version History' },
    { name: 'noteInfo', label: 'Note Info' },
  ];
</script>

<div class="space-y-4">
  <p class="text-sm text-gray-600 dark:text-gray-400">
    Customize keyboard shortcuts. Click on a shortcut to change it, then press your desired key combination.
  </p>

  <div class="space-y-2">
    {#each shortcuts as { name, label }}
      <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
        <span class="text-sm text-gray-700 dark:text-gray-300">{label}</span>
        <button
          on:click={() => onStartRecording(name)}
          class="px-3 py-1 text-xs font-mono {recordingShortcut === name ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
        >
          {recordingShortcut === name ? 'Press a key...' : formatShortcutDisplay(tempShortcuts[name])}
        </button>
      </div>
    {/each}
  </div>

  <div class="mt-4 space-y-2">
    <button
      on:click={onResetShortcuts}
      class="w-full px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors"
    >
      Reset to Defaults
    </button>

    <button
      on:click={onOpenShortcutsHelp}
      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
    >
      ⌨️ View All Shortcuts
    </button>
    <p class="text-sm text-gray-500 dark:text-gray-400 text-center">
      Quick reference of all keyboard shortcuts
    </p>
  </div>
</div>
