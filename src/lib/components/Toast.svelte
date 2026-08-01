<script lang="ts">
  import { toast } from '../utils/toast.svelte';
  import type { Toast } from '../utils/toast.svelte';

  function getToastClasses(type: Toast['type']) {
    switch (type) {
      case 'success':
        return 'bg-green-50 dark:bg-green-900 border-green-200 dark:border-green-700 text-green-800 dark:text-green-100';
      case 'error':
        return 'bg-red-50 dark:bg-red-900 border-red-200 dark:border-red-700 text-red-800 dark:text-red-100';
      default:
        return 'bg-blue-50 dark:bg-blue-900 border-blue-200 dark:border-blue-700 text-blue-800 dark:text-blue-100';
    }
  }

  function getIcon(type: Toast['type']) {
    switch (type) {
      case 'success':
        return '✓';
      case 'error':
        return '✕';
      default:
        return 'ℹ';
    }
  }
</script>

<div class="fixed top-4 right-4 z-50 space-y-2" aria-live="polite" aria-atomic="false">
  {#each toast.items as item (item.id)}
    <div
      class="animate-slide-in border rounded-lg shadow-lg p-4 max-w-sm {getToastClasses(item.type)}"
      role={item.type === 'error' ? 'alert' : 'status'}
    >
      <div class="flex items-start justify-between gap-3">
        <div class="flex items-start gap-2">
          <span class="text-lg font-bold">{getIcon(item.type)}</span>
          <div>
            <p class="text-sm whitespace-pre-line">{item.message}</p>
            {#if item.action}
              <button
                onclick={() => { item.action?.onClick(); toast.remove(item.id); }}
                class="mt-1 text-sm font-medium underline hover:no-underline"
              >
                {item.action.label}
              </button>
            {/if}
          </div>
        </div>
        <button
          onclick={() => toast.remove(item.id)}
          class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 flex-shrink-0"
        >
          ✕
        </button>
      </div>
    </div>
  {/each}
</div>

<style>
  @keyframes slide-in {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  .animate-slide-in {
    animation: slide-in 0.3s ease-out;
  }
</style>
