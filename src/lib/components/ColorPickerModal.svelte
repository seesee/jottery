<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settings } from '../stores/appStore';
  import { getColorHex, getColorNames, resolveTheme } from '../services/colorService';

  export let show = false;
  export let currentColor: string | undefined = undefined;
  export let onColorSelect: (color: string | undefined) => void;
  export let onClose: () => void;

  $: currentTheme = resolveTheme($settings.theme);
  $: colorNames = getColorNames();

  function handleColorClick(colorName: string) {
    onColorSelect(colorName);
    onClose();
  }

  function handleRemoveColor() {
    onColorSelect(undefined);
    onClose();
  }

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onClose();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if show}
  <!-- Modal backdrop -->
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    on:click={handleBackdropClick}
    role="presentation"
  >
    <!-- Modal content -->
    <div
      class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby="color-picker-title"
    >
      <h2 id="color-picker-title" class="text-xl font-semibold mb-4 text-gray-900 dark:text-gray-100">
        {$_('editor.selectColor')}
      </h2>

      <!-- Color grid -->
      <div class="grid grid-cols-4 gap-3 mb-4">
        {#each colorNames as colorName}
          {@const colorHex = getColorHex(colorName, currentTheme)}
          {@const isSelected = currentColor === colorName}
          <button
            on:click={() => handleColorClick(colorName)}
            class="aspect-square rounded-lg border-2 transition-all hover:scale-110 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800 {isSelected ? 'border-blue-500 ring-2 ring-blue-500 ring-offset-2 dark:ring-offset-gray-800' : 'border-gray-300 dark:border-gray-600'}"
            style="background-color: {colorHex}"
            title={colorName}
            aria-label="{colorName} color"
            aria-pressed={isSelected}
          >
            {#if isSelected}
              <svg class="w-full h-full p-2 text-gray-900 dark:text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
              </svg>
            {/if}
          </button>
        {/each}
      </div>

      <!-- Actions -->
      <div class="flex gap-2">
        <button
          on:click={handleRemoveColor}
          class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors text-gray-900 dark:text-gray-100"
        >
          {$_('editor.removeColor')}
        </button>
        <button
          on:click={onClose}
          class="flex-1 px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded-lg transition-colors"
        >
          {$_('common.cancel')}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Ensure color buttons are perfectly square */
  .aspect-square {
    aspect-ratio: 1 / 1;
  }
</style>
