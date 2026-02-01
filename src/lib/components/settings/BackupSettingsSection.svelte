<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsRepository } from '../../services/settingsRepository';
  import { settings } from '../../stores/appStore';

  export let onCreateBackup: () => void;
  export let isCreatingBackup: boolean = false;

  // Local state bound to parent via settings store
  let backupReminderEnabled = $settings.backupReminderEnabled ?? false;
  let backupReminderFrequency: 'daily' | 'weekly' | 'monthly' = $settings.backupReminderFrequency ?? 'weekly';
  let autoBackupEnabled = $settings.autoBackupEnabled ?? false;
  let autoBackupFrequency: 'daily' | 'weekly' | 'on-changes' = $settings.autoBackupFrequency ?? 'weekly';
  let autoBackupAfterChanges = $settings.autoBackupAfterChanges ?? 10;
  let lastBackupAt = $settings.lastBackupAt ?? null;

  // Sync with settings store
  $: {
    backupReminderEnabled = $settings.backupReminderEnabled ?? false;
    backupReminderFrequency = $settings.backupReminderFrequency ?? 'weekly';
    autoBackupEnabled = $settings.autoBackupEnabled ?? false;
    autoBackupFrequency = $settings.autoBackupFrequency ?? 'weekly';
    autoBackupAfterChanges = $settings.autoBackupAfterChanges ?? 10;
    lastBackupAt = $settings.lastBackupAt ?? null;
  }

  /**
   * Format the last backup date for display
   */
  function formatLastBackup(isoDate: string | null): string {
    if (!isoDate) {
      return $_('backupSettings.never');
    }

    try {
      const date = new Date(isoDate);
      const tz = $settings.timezone === 'local' ? undefined : $settings.timezone;
      return date.toLocaleDateString(undefined, {
        day: 'numeric',
        month: 'long',
        year: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        timeZone: tz,
      });
    } catch {
      return $_('backupSettings.never');
    }
  }

  async function updateSetting(key: string, value: unknown) {
    try {
      const update = { [key]: value };
      await settingsRepository.update(update);
      settings.update(s => ({ ...s, ...update }));
    } catch (error) {
      console.error('Failed to update backup setting:', error);
    }
  }

  function handleReminderToggle() {
    updateSetting('backupReminderEnabled', backupReminderEnabled);
  }

  function handleReminderFrequencyChange() {
    updateSetting('backupReminderFrequency', backupReminderFrequency);
  }

  function handleAutoBackupToggle() {
    updateSetting('autoBackupEnabled', autoBackupEnabled);
  }

  function handleAutoBackupFrequencyChange() {
    updateSetting('autoBackupFrequency', autoBackupFrequency);
  }

  function handleChangesThresholdChange() {
    // Ensure valid range
    if (autoBackupAfterChanges < 1) autoBackupAfterChanges = 1;
    if (autoBackupAfterChanges > 1000) autoBackupAfterChanges = 1000;
    updateSetting('autoBackupAfterChanges', autoBackupAfterChanges);
  }
</script>

<div class="space-y-4">
  <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">
    {$_('backupSettings.title')}
  </h4>

  <!-- Last backup info -->
  <div class="text-sm text-gray-600 dark:text-gray-400">
    <span class="font-medium">{$_('backupSettings.lastBackup')}:</span>
    {formatLastBackup(lastBackupAt)}
  </div>

  <!-- Backup reminders -->
  <div class="space-y-2">
    <label class="flex items-center gap-3">
      <input
        type="checkbox"
        bind:checked={backupReminderEnabled}
        on:change={handleReminderToggle}
        class="w-4 h-4 rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
      />
      <span class="text-sm text-gray-700 dark:text-gray-300">
        {$_('backupSettings.enableReminders')}
      </span>
    </label>

    {#if backupReminderEnabled}
      <div class="ml-7">
        <select
          bind:value={backupReminderFrequency}
          on:change={handleReminderFrequencyChange}
          class="w-full max-w-xs px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          <option value="daily">{$_('backupSettings.frequencyDaily')}</option>
          <option value="weekly">{$_('backupSettings.frequencyWeekly')}</option>
          <option value="monthly">{$_('backupSettings.frequencyMonthly')}</option>
        </select>
      </div>
    {/if}
  </div>

  <!-- Auto backups -->
  <div class="space-y-2">
    <label class="flex items-center gap-3">
      <input
        type="checkbox"
        bind:checked={autoBackupEnabled}
        on:change={handleAutoBackupToggle}
        class="w-4 h-4 rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
      />
      <span class="text-sm text-gray-700 dark:text-gray-300">
        {$_('backupSettings.enableAutoBackup')}
      </span>
    </label>

    {#if autoBackupEnabled}
      <div class="ml-7 space-y-3">
        <select
          bind:value={autoBackupFrequency}
          on:change={handleAutoBackupFrequencyChange}
          class="w-full max-w-xs px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          <option value="daily">{$_('backupSettings.frequencyDaily')}</option>
          <option value="weekly">{$_('backupSettings.frequencyWeekly')}</option>
          <option value="on-changes">{$_('backupSettings.frequencyOnChanges')}</option>
        </select>

        {#if autoBackupFrequency === 'on-changes'}
          <div class="flex items-center gap-2">
            <label for="changes-threshold" class="text-sm text-gray-600 dark:text-gray-400">
              {$_('backupSettings.changesThreshold')}:
            </label>
            <input
              id="changes-threshold"
              type="number"
              bind:value={autoBackupAfterChanges}
              on:change={handleChangesThresholdChange}
              min="1"
              max="1000"
              class="w-20 px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Description -->
  <p class="text-xs text-gray-500 dark:text-gray-400">
    {$_('backupSettings.description')}
  </p>

  <!-- Create backup button -->
  <button
    on:click={onCreateBackup}
    disabled={isCreatingBackup}
    class="px-4 py-2 min-h-11 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 disabled:cursor-not-allowed text-white text-sm font-medium rounded-md transition-colors"
  >
    {isCreatingBackup ? $_('backup.creating') : $_('backup.createButton')}
  </button>
</div>
