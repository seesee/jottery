<script lang="ts">
  import { toast } from '../lib/toast.svelte';
  import type { Toast } from '../lib/toast.svelte';

  function getToastClasses(type: Toast['type']) {
    switch (type) {
      case 'success':
        return 'bg-green-50 border-green-200 text-green-800';
      case 'error':
        return 'bg-red-50 border-red-200 text-red-800';
      default:
        return 'bg-blue-50 border-blue-200 text-blue-800';
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

<div class="fixed top-4 right-4 z-50 space-y-2">
  {#each toast.items as item (item.id)}
    <div class="animate-slide-in border rounded-lg shadow-lg p-4 max-w-sm {getToastClasses(item.type)}">
      <div class="flex items-start justify-between gap-3">
        <div class="flex items-start gap-2">
          <span class="text-lg font-bold">{getIcon(item.type)}</span>
          <p class="text-sm">{item.message}</p>
        </div>
        <button
          onclick={() => toast.remove(item.id)}
          class="text-gray-500 hover:text-gray-700"
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
