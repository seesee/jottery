<script lang="ts">
  import { _ } from 'svelte-i18n';
  import BackupSettingsSection from './BackupSettingsSection.svelte';

  export let selectedArchitecture: string;
  export let onExport: () => void;
  export let onImport: () => void;
  export let onDownload: () => void;
  export let onDeleteDatabase: () => void;
  export let onCreateBackup: () => void;
  export let isCreatingBackup: boolean = false;

  const githubRepo = 'https://github.com/seesee/jottery';

  const architectures = [
    { value: 'macos', label: 'macOS (Universal)', url: `${githubRepo}/releases/latest/download/jottery-macos` },
    { value: 'windows', label: 'Windows (x64)', url: `${githubRepo}/releases/latest/download/jottery-windows.exe` },
    { value: 'linux-x64', label: 'Linux (x64)', url: `${githubRepo}/releases/latest/download/jottery-linux-x64` },
    { value: 'linux-arm64', label: 'Linux (ARM64)', url: `${githubRepo}/releases/latest/download/jottery-linux-arm64` },
    { value: 'linux-armv7', label: 'Linux (ARMv7)', url: `${githubRepo}/releases/latest/download/jottery-linux-armv7` },
  ];
</script>

<div class="space-y-6">
  <!-- Import/Export -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">{$_('settings.advancedTab.importExport')}</h4>
    <div class="space-y-3">
      <div class="flex gap-2">
        <button
          on:click={onExport}
          class="flex-1 px-4 py-2 min-h-11 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
        >
          📤 {$_('settings.exportNotes')}
        </button>
        <button
          on:click={onImport}
          class="flex-1 px-4 py-2 min-h-11 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
        >
          📥 {$_('settings.importNotes')}
        </button>
      </div>
      <p class="text-sm text-gray-500 dark:text-gray-400">
        {$_('settings.advancedTab.exportDescription')}
      </p>
    </div>
  </div>

  <!-- Automatic Backups -->
  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
    <BackupSettingsSection {onCreateBackup} {isCreatingBackup} />
  </div>

  <!-- Download Terminal Client -->
  <div>
    <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">💻 {$_('settings.advancedTab.downloadTerminalClient')}</h4>
    <div class="flex gap-2 mb-2">
      <select
        bind:value={selectedArchitecture}
        class="flex-1 px-3 py-2 min-h-11 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        {#if selectedArchitecture === ''}
          <option value="">{$_('settings.advancedTab.selectPlatform')}</option>
        {/if}
        {#each architectures as arch}
          <option value={arch.value}>{arch.label}</option>
        {/each}
      </select>
      <button
        on:click={onDownload}
        disabled={!selectedArchitecture}
        class="px-4 py-2 min-h-11 bg-green-600 hover:bg-green-700 disabled:bg-gray-400 disabled:cursor-not-allowed text-white text-sm font-medium rounded-md transition-colors"
      >
        {$_('settings.advancedTab.download')}
      </button>
    </div>
    <p class="text-sm text-gray-500 dark:text-gray-400">
      {$_('settings.advancedTab.terminalClientDescription')}
    </p>
  </div>

  <!-- Delete All Data -->
  <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
    <h4 class="text-sm font-medium text-red-800 dark:text-red-200 mb-2">
      {$_('settings.advancedTab.deleteAllData')}
    </h4>
    <p class="text-sm text-red-700 dark:text-red-300 mb-3">
      {$_('settings.advancedTab.deleteAllDataWarning')}
    </p>
    <button
      on:click={onDeleteDatabase}
      class="px-4 py-2 min-h-11 bg-red-600 hover:bg-red-700 text-white text-sm font-medium rounded-md transition-colors"
    >
      {$_('settings.deleteDatabase')}
    </button>
  </div>
</div>
