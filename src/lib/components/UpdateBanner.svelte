<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { updateAvailable, newVersionInfo, appUpdateService } from '../services';
  import { createBatchedBackup, downloadBackup } from '../services/backupService';

  let isBackingUp = false;

  function handleReload() {
    appUpdateService.reloadApp();
  }

  async function handleBackupAndReload() {
    isBackingUp = true;
    try {
      const backup = await createBatchedBackup();
      downloadBackup(backup);
      // Give the download a moment to start before reloading
      setTimeout(() => {
        appUpdateService.reloadApp();
      }, 500);
    } catch (error) {
      console.error('Failed to create encrypted backup:', error);
      isBackingUp = false;
    }
  }

  function handleDismiss() {
    updateAvailable.set(false);
  }
</script>

{#if $updateAvailable}
  <div class="fixed top-0 left-0 right-0 z-50 bg-blue-600 dark:bg-blue-700 text-white shadow-lg animate-slide-down">
    <div class="max-w-7xl mx-auto px-4 py-3 flex items-center justify-between gap-4">
      <div class="flex items-center gap-3 flex-1 min-w-0">
        <svg class="w-5 h-5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-medium">
            {$_('updateBanner.title')}
            {#if $newVersionInfo}
              <span class="font-normal opacity-90">(v{$newVersionInfo.version})</span>
            {/if}
          </p>
          <p class="text-xs opacity-90 hidden tablet:block">
            {$_('updateBanner.message')}
          </p>
        </div>
      </div>
      
      <div class="flex items-center gap-2 flex-shrink-0">
        <button
          on:click={handleBackupAndReload}
          disabled={isBackingUp}
          class="px-4 py-2 min-h-10 bg-blue-500 dark:bg-blue-600 text-white font-medium text-sm rounded-md hover:bg-blue-400 dark:hover:bg-blue-500 active:bg-blue-300 dark:active:bg-blue-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {#if isBackingUp}
            {$_('updateBanner.backingUp')}
          {:else}
            {$_('updateBanner.backupButton')}
          {/if}
        </button>
        <button
          on:click={handleReload}
          disabled={isBackingUp}
          class="px-4 py-2 min-h-10 bg-white text-blue-600 dark:bg-blue-800 dark:text-white font-medium text-sm rounded-md hover:bg-blue-50 dark:hover:bg-blue-900 active:bg-blue-100 dark:active:bg-blue-950 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {$_('updateBanner.reloadButton')}
        </button>
        <button
          on:click={handleDismiss}
          disabled={isBackingUp}
          class="p-2 min-h-10 min-w-10 hover:bg-blue-700 dark:hover:bg-blue-800 active:bg-blue-800 dark:active:bg-blue-900 rounded-md transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          aria-label={$_('updateBanner.dismissLabel')}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes slide-down {
    from {
      transform: translateY(-100%);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .animate-slide-down {
    animation: slide-down 0.3s ease-out;
  }
</style>
