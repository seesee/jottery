<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onDestroy } from 'svelte';

  export let show: boolean = false;
  export let title: string = '';
  export let message: string = '';
  export let current: number = 0;
  export let total: number = 0;
  export let showAfterMs: number = 500; // Only show if operation takes longer than this

  let actuallyShow = false;
  let showTimeout: ReturnType<typeof setTimeout> | null = null;

  $: progress = total > 0 ? Math.round((current / total) * 100) : 0;

  // Delay showing the modal to avoid flashing for quick operations
  $: if (show) {
    if (!showTimeout) {
      showTimeout = setTimeout(() => {
        actuallyShow = true;
      }, showAfterMs);
    }
  } else {
    if (showTimeout) {
      clearTimeout(showTimeout);
      showTimeout = null;
    }
    actuallyShow = false;
  }

  onDestroy(() => {
    if (showTimeout) {
      clearTimeout(showTimeout);
    }
  });
</script>

{#if actuallyShow}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
    role="dialog"
    aria-modal="true"
    aria-labelledby="progress-title"
  >
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4">
        <h2 id="progress-title" class="text-xl font-bold text-gray-900 dark:text-white">
          {title || $_('progress.title')}
        </h2>
      </div>

      <!-- Content -->
      <div class="p-6">
        {#if message}
          <p class="text-gray-700 dark:text-gray-300 mb-4">
            {message}
          </p>
        {/if}

        <!-- Progress bar -->
        <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3 overflow-hidden">
          <div
            class="bg-blue-600 h-3 rounded-full transition-all duration-150 ease-out"
            style="width: {progress}%"
          ></div>
        </div>

        <!-- Progress text -->
        <div class="mt-2 text-sm text-gray-600 dark:text-gray-400 text-center">
          {#if total > 0}
            {current} / {total} ({progress}%)
          {:else}
            <span class="animate-pulse">{$_('progress.processing')}</span>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}
