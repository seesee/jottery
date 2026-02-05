<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { showBackupReminder, lastBackupDate, isAutoBackingUp, backupSchedulerService } from '../services/backupSchedulerService';
  import { createBatchedBackup, downloadBackup } from '../services/backupService';
  import { toast } from '../utils/toast.svelte';
  import { settings } from '../stores/appStore';

  let isBackingUp = false;

  /**
   * Format the last backup date for display
   */
  function formatLastBackup(isoDate: string | null): string {
    if (!isoDate) {
      return $_('backupReminder.never');
    }

    try {
      const date = new Date(isoDate);
      // Use the user's timezone setting
      const tz = $settings.timezone === 'local' ? undefined : $settings.timezone;
      return date.toLocaleDateString(undefined, {
        day: 'numeric',
        month: 'long',
        year: 'numeric',
        timeZone: tz,
      });
    } catch {
      return $_('backupReminder.never');
    }
  }

  async function handleBackupNow() {
    if (isBackingUp) return;

    isBackingUp = true;
    try {
      const backup = await createBatchedBackup();
      downloadBackup(backup);
      await backupSchedulerService.recordBackup();
      toast.success($_('backup.created'));
    } catch (error) {
      console.error('Failed to create backup:', error);
      toast.error($_('backup.failed', { values: { error: error instanceof Error ? error.message : String(error) } }));
    } finally {
      isBackingUp = false;
    }
  }

  function handleDismiss() {
    backupSchedulerService.dismissReminder();
  }
</script>

{#if $showBackupReminder}
  <div
    class="bg-amber-50 dark:bg-amber-900/30 border-b border-amber-200 dark:border-amber-800 px-4 py-3"
    role="alert"
  >
    <div class="flex items-center justify-between gap-4 max-w-4xl mx-auto">
      <div class="flex items-center gap-3 min-w-0">
        <span class="text-amber-600 dark:text-amber-400 text-lg flex-shrink-0" aria-hidden="true">
          ⚠
        </span>
        <div class="min-w-0">
          <p class="text-sm font-medium text-amber-800 dark:text-amber-200">
            {$_('backupReminder.title')}
          </p>
          <p class="text-xs text-amber-700 dark:text-amber-300 truncate">
            {$_('backupReminder.lastBackup', { values: { date: formatLastBackup($lastBackupDate) } })}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2 flex-shrink-0">
        <button
          on:click={handleBackupNow}
          disabled={isBackingUp || $isAutoBackingUp}
          class="px-3 py-1.5 text-sm font-medium bg-amber-600 hover:bg-amber-700 disabled:bg-amber-400 disabled:cursor-not-allowed text-white rounded-md transition-colors"
        >
          {#if isBackingUp || $isAutoBackingUp}
            {$_('backupReminder.backingUp')}
          {:else}
            {$_('backupReminder.backupNow')}
          {/if}
        </button>
        <button
          on:click={handleDismiss}
          class="p-1.5 text-amber-600 dark:text-amber-400 hover:bg-amber-100 dark:hover:bg-amber-800/50 rounded-md transition-colors"
          aria-label={$_('backupReminder.dismissLabel')}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  </div>
{/if}
