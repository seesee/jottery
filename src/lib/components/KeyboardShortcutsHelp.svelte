<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settings } from '../stores/appStore';
  import type { KeyboardShortcut } from '../types';

  export let show: boolean = false;
  export let onClose: () => void;

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  function formatShortcut(shortcut: KeyboardShortcut): string[] {
    const keys: string[] = [];

    if (shortcut.ctrl) keys.push('Ctrl/Cmd');
    if (shortcut.alt) keys.push('Alt');
    if (shortcut.shift) keys.push('Shift');

    // Capitalize single letters, but keep symbols as-is
    const key = shortcut.key.length === 1 && shortcut.key.match(/[a-z]/i)
      ? shortcut.key.toUpperCase()
      : shortcut.key;
    keys.push(key);

    return keys;
  }

  $: shortcuts = [
    {
      category: $_('keyboard.global'),
      items: [
        { keys: formatShortcut($settings.keyboardShortcuts?.focusSearch || { key: 'k', ctrl: true }), description: $_('keyboard.focusSearch') },
        { keys: formatShortcut($settings.keyboardShortcuts?.newNote || { key: 'n', alt: true }), description: $_('keyboard.createNote') },
        { keys: formatShortcut($settings.keyboardShortcuts?.lockApp || { key: 'l', alt: true }), description: $_('keyboard.lockApp') },
        { keys: formatShortcut($settings.keyboardShortcuts?.openSettings || { key: ',', ctrl: true }), description: $_('keyboard.openSettings') },
        { keys: formatShortcut($settings.keyboardShortcuts?.showShortcuts || { key: '/', alt: true }), description: $_('keyboard.showShortcuts') },
      ]
    },
    {
      category: $_('keyboard.noteList'),
      items: [
        { keys: ['↑/↓'], description: $_('keyboard.navigateNotes') },
        { keys: ['J/K'], description: $_('keyboard.navigateVim') },
        { keys: ['Enter'], description: $_('keyboard.openNote') },
        { keys: ['Delete'], description: $_('keyboard.deleteNote') },
        { keys: ['P'], description: $_('keyboard.pinNote') },
      ]
    },
    {
      category: $_('keyboard.editor'),
      items: [
        { keys: ['Esc'], description: $_('keyboard.closeNote') },
        { keys: formatShortcut($settings.keyboardShortcuts?.copyNote || { key: 'c', alt: true }), description: $_('keyboard.copyNote') },
        { keys: ['Ctrl/Cmd', 'F'], description: $_('keyboard.findInNote') },
        { keys: ['Ctrl/Cmd', 'H'], description: $_('keyboard.replaceInNote') },
      ]
    }
  ];
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="0"
  >
    <div class="bg-white dark:bg-gray-800 w-full h-full tablet:h-auto tablet:max-w-2xl tablet:rounded-lg shadow-xl tablet:max-h-[90vh] flex flex-col">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between flex-shrink-0">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('keyboard.title')}</h2>
        <button
          on:click={onClose}
          class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
          aria-label={$_('keyboard.closeLabel')}
        >
          ✕
        </button>
      </div>

      <!-- Content (scrollable) -->
      <div class="p-6 space-y-6 flex-1 overflow-y-auto">
        {#each shortcuts as section}
          <div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-3">
              {section.category}
            </h3>
            <div class="space-y-2">
              {#each section.items as item}
                <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700 last:border-0">
                  <span class="text-sm text-gray-700 dark:text-gray-300">
                    {item.description}
                  </span>
                  <div class="flex gap-1">
                    {#each item.keys as key}
                      <kbd class="px-2 py-1 text-xs font-semibold text-gray-800 dark:text-gray-200 bg-gray-100 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded">
                        {key}
                      </kbd>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/each}

        <!-- Additional Info -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-4 mt-6">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            <strong>{$_('keyboard.note')}</strong> {$_('keyboard.macNote')}
          </p>
        </div>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end flex-shrink-0">
        <button
          on:click={onClose}
          class="px-4 py-2.5 min-h-11 bg-blue-600 active:bg-blue-700 text-white font-medium rounded-md transition-colors"
        >
          {$_('common.close')}
        </button>
      </div>
    </div>
  </div>
{/if}
