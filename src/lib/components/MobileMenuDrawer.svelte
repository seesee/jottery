<script lang="ts">
  import { _ } from 'svelte-i18n';

  export let show: boolean;
  export let forceMobileLayout: boolean = false;
  export let rememberPasswordEnabled: boolean = false;
  export let openSettingsShortcut: string | null = null;
  export let lockAppShortcut: string | null = null;

  export let onClose: () => void;
  export let onOpenRecycleBin: () => void;
  export let onOpenSettings: () => void;
  export let onLock: () => void;

  let menuClosing = false;

  export function close() {
    menuClosing = true;
    // Wait for slide-out animation to complete
    setTimeout(() => {
      show = false;
      menuClosing = false;
      onClose();
    }, 300);
  }

  function handleBackdropClick() {
    close();
  }

  function handleItemClick(action: () => void) {
    action();
    close();
  }
</script>

{#if show}
  <!-- Backdrop -->
  <div
    class="{forceMobileLayout ? '' : 'tablet:hidden'} fixed inset-0 bg-black z-40 transition-opacity duration-300 {menuClosing ? 'opacity-0' : 'opacity-50'}"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Enter' && handleBackdropClick()}
    role="button"
    tabindex="-1"
    aria-label="Close menu"
  ></div>

  <!-- Drawer (slides in from right) -->
  <div class="{forceMobileLayout ? '' : 'tablet:hidden'} fixed right-0 top-0 bottom-0 w-64 bg-white dark:bg-gray-800 z-50 shadow-xl {menuClosing ? 'animate-slide-out-right' : 'animate-slide-in-right'}">
    <div class="flex flex-col h-full">
      <!-- Drawer Header -->
      <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <button
          on:click={close}
          class="min-h-11 min-w-11 p-3 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          aria-label="Close menu"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
        <h2 class="text-lg font-bold text-gray-900 dark:text-white">{$_('header.menu')}</h2>
      </div>

      <!-- Menu Items -->
      <nav class="flex-1 overflow-y-auto p-2">
        <button
          on:click={() => handleItemClick(onOpenRecycleBin)}
          class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        >
          <span class="text-xl">🗑️</span>
          <span class="flex-1 text-sm font-medium">{$_('recycleBin.title')}</span>
        </button>

        <button
          on:click={() => handleItemClick(onOpenSettings)}
          class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        >
          <span class="text-xl">⚙️</span>
          <span class="flex-1 text-sm font-medium">{$_('common.settings')}</span>
          {#if openSettingsShortcut}
            <span class="text-xs text-gray-500 dark:text-gray-400">{openSettingsShortcut}</span>
          {/if}
        </button>

        <div class="my-2 border-t border-gray-200 dark:border-gray-700"></div>

        <button
          on:click={() => handleItemClick(onLock)}
          class="w-full flex items-center gap-3 px-4 py-3 min-h-11 text-left {rememberPasswordEnabled ? 'opacity-50' : 'active:bg-gray-100 dark:active:bg-gray-700'} rounded-md transition-colors"
        >
          <span class="text-xl">{rememberPasswordEnabled ? '🔓' : '🔒'}</span>
          <span class="flex-1 text-sm font-medium">{$_('common.lock')}</span>
          {#if lockAppShortcut && !rememberPasswordEnabled}
            <span class="text-xs text-gray-500 dark:text-gray-400">{lockAppShortcut}</span>
          {/if}
        </button>
      </nav>
    </div>
  </div>
{/if}

<style>
  @keyframes slide-in-right {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  @keyframes slide-out-right {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(100%);
    }
  }

  .animate-slide-in-right {
    animation: slide-in-right 0.3s ease-out;
  }

  .animate-slide-out-right {
    animation: slide-out-right 0.3s ease-out forwards;
  }
</style>
