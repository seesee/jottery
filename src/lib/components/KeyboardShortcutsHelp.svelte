<script lang="ts">
  export let show: boolean = false;
  export let onClose: () => void;

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  const shortcuts = [
    {
      category: 'Global',
      items: [
        { keys: ['Ctrl/Cmd', 'K'], description: 'Focus search' },
        { keys: ['Ctrl/Cmd', 'N'], description: 'Create new note' },
        { keys: ['Ctrl/Cmd', 'L'], description: 'Lock application' },
        { keys: ['Ctrl/Cmd', ','], description: 'Open settings' },
        { keys: ['Ctrl/Cmd', '/'], description: 'Show keyboard shortcuts' },
      ]
    },
    {
      category: 'Note List',
      items: [
        { keys: ['↑/↓'], description: 'Navigate notes' },
        { keys: ['J/K'], description: 'Navigate notes (vim-style)' },
        { keys: ['Enter'], description: 'Open selected note' },
        { keys: ['Delete'], description: 'Delete selected note' },
        { keys: ['P'], description: 'Pin/unpin selected note' },
      ]
    },
    {
      category: 'Editor',
      items: [
        { keys: ['Esc'], description: 'Close note' },
        { keys: ['Ctrl/Cmd', 'Shift', 'C'], description: 'Copy note content' },
        { keys: ['Ctrl/Cmd', 'F'], description: 'Find in note (CodeMirror)' },
        { keys: ['Ctrl/Cmd', 'H'], description: 'Replace in note (CodeMirror)' },
      ]
    }
  ];
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">Keyboard Shortcuts</h2>
        <button
          on:click={onClose}
          class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          ✕
        </button>
      </div>

      <!-- Content -->
      <div class="p-6 space-y-6">
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
            <strong>Note:</strong> On macOS, use <kbd class="px-1.5 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded">Cmd</kbd> instead of <kbd class="px-1.5 py-0.5 text-xs bg-gray-100 dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded">Ctrl</kbd>.
          </p>
        </div>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end">
        <button
          on:click={onClose}
          class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-md transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
