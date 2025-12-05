<script lang="ts">
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService, AVAILABLE_LOCALES, syncService, syncRepository, keyManager, cryptoService } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';
  import { locale, _ } from 'svelte-i18n';
  import type { Theme, SyncStatus } from '../types';
  import ConfirmModal from './ConfirmModal.svelte';

  export let show = false;
  export let onClose: () => void = () => {};
  export let onOpenShortcutsHelp: () => void = () => {};

  let theme: Theme = $settings.theme;
  let autoLockTimeout = $settings.autoLockTimeout;
  let sortOrder = $settings.sortOrder;
  let language = $settings.language;
  let saving = false;
  let fileInput: HTMLInputElement;
  let showDeleteConfirm = false;

  // Sync state
  let syncEndpoint = $settings.syncEndpoint || '';
  let syncStatus: SyncStatus | null = null;
  let syncing = false;
  let syncError = '';
  let registering = false;
  let showManualConfig = false;
  let manualClientId = '';
  let manualApiKey = '';
  let configuringManual = false;
  let showCredentials = false;
  let decryptedApiKey = '';
  let loadingApiKey = false;
  let registrationApiKey = '';  // Store API key from registration to show to user

  // Apply theme when it changes
  $: applyTheme(theme);

  // Load sync status when modal opens
  $: if (show) {
    loadSyncStatus();
  } else {
    // Clear sensitive data when modal closes
    decryptedApiKey = '';
  }

  // Clear sensitive API keys when credentials are hidden
  $: if (!showCredentials) {
    decryptedApiKey = '';
    registrationApiKey = '';
  }

  function applyTheme(selectedTheme: Theme) {
    if (typeof window === 'undefined') return;

    if (selectedTheme === 'dark') {
      document.documentElement.classList.add('dark');
    } else if (selectedTheme === 'light') {
      document.documentElement.classList.remove('dark');
    } else {
      // Auto mode - use system preference
      if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    }
  }

  async function loadSyncStatus() {
    try {
      syncStatus = await syncService.getSyncStatus();
    } catch (error) {
      console.error('Failed to load sync status:', error);
    }
  }

  async function handleRegister() {
    if (!syncEndpoint) {
      syncError = 'Please enter a sync endpoint URL';
      return;
    }

    registering = true;
    syncError = '';
    registrationApiKey = '';  // Clear previous registration key
    try {
      const deviceName = `Jottery Web - ${navigator.platform}`;
      const response = await syncService.register(syncEndpoint, deviceName);

      // Store the API key to show to the user
      registrationApiKey = response.apiKey;

      // Reload status
      await loadSyncStatus();

      // Update local state
      syncEndpoint = $settings.syncEndpoint || '';

      syncError = '';  // Clear any previous errors
      // Show credentials section automatically after registration
      showCredentials = true;
    } catch (error) {
      console.error('Registration failed:', error);
      syncError = error instanceof Error ? error.message : 'Registration failed';
    } finally {
      registering = false;
    }
  }

  async function handleSyncNow() {
    syncing = true;
    syncError = '';
    try {
      const result = await syncService.syncNow();
      if (result.success) {
        await loadSyncStatus();
      } else {
        syncError = result.error || 'Sync failed';
      }
    } catch (error) {
      console.error('Sync failed:', error);
      syncError = error instanceof Error ? error.message : 'Sync failed';
    } finally {
      syncing = false;
    }
  }

  async function handleManualConfig() {
    if (!syncEndpoint || !manualClientId || !manualApiKey) {
      syncError = 'Please provide endpoint, client ID, and API key';
      return;
    }

    configuringManual = true;
    syncError = '';
    try {
      // Use the configureCredentials method from syncService
      await syncService.configureCredentials(syncEndpoint, manualClientId, manualApiKey);

      // Reload status
      await loadSyncStatus();

      // Update local state
      syncEndpoint = $settings.syncEndpoint || '';

      // Clear manual input fields
      manualClientId = '';
      manualApiKey = '';
      showManualConfig = false;

      syncError = '';  // Clear any previous errors
      // Success will be shown by the sync status UI
    } catch (error) {
      console.error('Manual configuration failed:', error);
      syncError = error instanceof Error ? error.message : 'Configuration failed';
    } finally {
      configuringManual = false;
    }
  }

  async function handleShowApiKey() {
    loadingApiKey = true;
    syncError = '';
    try {
      // Get master key
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked. Please unlock first.');
      }

      // Get sync metadata
      const metadata = await syncRepository.getMetadata();
      if (!metadata || !metadata.apiKey) {
        throw new Error('No API key found');
      }

      // Decrypt API key
      const encryptedApiKey = JSON.parse(metadata.apiKey);
      decryptedApiKey = await cryptoService.decryptText(encryptedApiKey, masterKey.key);
    } catch (error) {
      console.error('Failed to show API key:', error);
      syncError = error instanceof Error ? error.message : 'Failed to decrypt API key';
    } finally {
      loadingApiKey = false;
    }
  }

  async function handleSave() {
    saving = true;
    try {
      await settingsRepository.update({
        theme,
        autoLockTimeout,
        sortOrder,
        language,
      });

      // Update store
      settings.update(s => ({
        ...s,
        theme,
        autoLockTimeout,
        sortOrder,
        language,
      }));

      onClose();
    } catch (error) {
      console.error('Failed to save settings:', error);
      alert('Failed to save settings: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      saving = false;
    }
  }

  function handleDeleteDatabase() {
    showDeleteConfirm = true;
  }

  async function confirmDeleteDatabase() {
    showDeleteConfirm = false;
    try {
      // Close database and delete
      await deleteDB();

      // Lock the app and reload
      isLocked.set(true);
      window.location.reload();
    } catch (error) {
      console.error('Failed to delete database:', error);
      alert('Failed to delete database: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }

  async function handleExport() {
    try {
      const data = await exportAllNotes();
      await downloadExport(data);
    } catch (error) {
      console.error('Failed to export notes:', error);
      alert('Failed to export notes: ' + (error instanceof Error ? error.message : String(error)));
    }
  }

  async function handleImport() {
    fileInput.click();
  }

  async function handleFileSelect(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    try {
      const data = await parseImportFile(file);
      const result = await importNotes(data, 'merge');

      alert(`Import complete!\nImported: ${result.imported}\nSkipped: ${result.skipped}\n${result.errors.length > 0 ? 'Errors: ' + result.errors.join('\n') : ''}`);

      // Reload notes
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
    } catch (error) {
      console.error('Failed to import notes:', error);
      alert('Failed to import notes: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      // Clear file input
      target.value = '';
    }
  }
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
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">Settings</h2>
        <button
          on:click={onClose}
          class="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          ✕
        </button>
      </div>

      <!-- Content -->
      <div class="p-6 space-y-6">
        <!-- Language -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('settings.language')}
          </label>
          <select
            bind:value={language}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            {#each AVAILABLE_LOCALES as { code, name }}
              <option value={code}>{name}</option>
            {/each}
          </select>
        </div>

        <!-- Theme -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('settings.theme')}
          </label>
          <select
            bind:value={theme}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="auto">{$_('settings.themeAuto')}</option>
            <option value="light">{$_('settings.themeLight')}</option>
            <option value="dark">{$_('settings.themeDark')}</option>
          </select>
        </div>

        <!-- Auto-lock timeout -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('settings.autoLockTimeout')}
          </label>
          <input
            type="number"
            bind:value={autoLockTimeout}
            min="1"
            max="1440"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <!-- Sort order -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('settings.sortOrder')}
          </label>
          <select
            bind:value={sortOrder}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="recent">{$_('settings.sortRecent')}</option>
            <option value="oldest">{$_('settings.sortOldest')}</option>
            <option value="alpha">{$_('settings.sortAlpha')}</option>
          </select>
        </div>

        <!-- Sync Configuration -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Sync</h3>

          <div class="space-y-4">
            <!-- Sync Endpoint -->
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                Sync Server Endpoint
              </label>
              <input
                type="url"
                bind:value={syncEndpoint}
                placeholder="https://sync.example.com"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                disabled={syncStatus?.isEnabled}
              />
              <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                URL of your self-hosted Jottery sync server
              </p>
            </div>

            <!-- Sync Status & Actions -->
            {#if syncStatus?.isEnabled}
              <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-3">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-sm font-medium text-green-800 dark:text-green-200">
                    ✓ Sync Enabled
                  </span>
                  {#if syncStatus.isSyncing}
                    <span class="text-xs text-green-600 dark:text-green-400">Syncing...</span>
                  {:else if syncStatus.lastSyncAt}
                    <span class="text-xs text-green-600 dark:text-green-400">
                      Last sync: {new Date(syncStatus.lastSyncAt).toLocaleString()}
                    </span>
                  {/if}
                </div>

                <button
                  on:click={handleSyncNow}
                  disabled={syncing || syncStatus.isSyncing}
                  class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                >
                  {syncing || syncStatus.isSyncing ? 'Syncing...' : '🔄 Sync Now'}
                </button>

                {#if syncStatus.pendingNotes > 0}
                  <p class="mt-2 text-xs text-gray-600 dark:text-gray-400">
                    {syncStatus.pendingNotes} note{syncStatus.pendingNotes !== 1 ? 's' : ''} pending sync
                  </p>
                {/if}

                <!-- Show Credentials -->
                <button
                  on:click={() => showCredentials = !showCredentials}
                  class="w-full mt-2 text-xs text-blue-600 dark:text-blue-400 hover:underline text-left"
                >
                  {showCredentials ? '▼' : '▶'} Show Credentials (for other devices)
                </button>

                {#if showCredentials && syncStatus.clientId}
                  <div class="mt-2 p-2 bg-gray-100 dark:bg-gray-800 rounded text-xs space-y-2">
                    <div>
                      <div class="font-medium text-gray-700 dark:text-gray-300">Client ID:</div>
                      <div class="font-mono text-gray-600 dark:text-gray-400 break-all select-all">
                        {syncStatus.clientId}
                      </div>
                    </div>
                    <div>
                      <div class="font-medium text-gray-700 dark:text-gray-300 mb-1">API Key:</div>
                      {#if registrationApiKey || decryptedApiKey}
                        <div class="font-mono text-xs text-gray-600 dark:text-gray-400 break-all select-all bg-yellow-50 dark:bg-yellow-900/20 p-2 rounded border border-yellow-300 dark:border-yellow-700">
                          {registrationApiKey || decryptedApiKey}
                        </div>
                        {#if registrationApiKey}
                          <p class="mt-1 text-xs text-green-600 dark:text-green-400 font-medium">
                            ✓ Registration successful! Copy this API key now - it won't be shown in plaintext again.
                          </p>
                        {/if}
                        <p class="mt-1 text-xs text-orange-600 dark:text-orange-400">
                          ⚠️ Keep this API key secret! Anyone with this key can access your synced notes.
                        </p>
                      {:else}
                        <button
                          on:click={handleShowApiKey}
                          disabled={loadingApiKey}
                          class="px-2 py-1 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-xs rounded transition-colors"
                        >
                          {loadingApiKey ? 'Decrypting...' : '🔓 Show API Key'}
                        </button>
                        <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                          API key is encrypted. Click to decrypt and display (requires app to be unlocked).
                        </p>
                      {/if}
                    </div>
                    <div class="pt-2 border-t border-gray-300 dark:border-gray-600">
                      <p class="text-gray-500 dark:text-gray-400 text-xs mb-2">
                        Copy both the Client ID and API Key to sync the same notes on other devices using "Use Existing Credentials" below.
                      </p>
                      <p class="text-red-600 dark:text-red-400 text-xs font-medium">
                        ⚠️ IMPORTANT: All devices must use the SAME master password! Notes are encrypted with your password and cannot be decrypted if passwords differ.
                      </p>
                    </div>
                  </div>
                {/if}
              </div>
            {:else}
              <button
                on:click={handleRegister}
                disabled={!syncEndpoint || registering}
                class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
              >
                {registering ? 'Registering...' : '🔗 Register New Device'}
              </button>
              <div class="text-xs space-y-1">
                <p class="text-gray-500 dark:text-gray-400">
                  Register this device with your sync server. Auto-sync will be enabled (every 5 minutes).
                </p>
                <p class="text-orange-600 dark:text-orange-400 font-medium">
                  ⚠️ Save the API key shown after registration to sync with other devices!
                </p>
              </div>

              <!-- Manual Configuration -->
              <div class="border-t border-gray-200 dark:border-gray-700 pt-4 mt-4">
                <button
                  on:click={() => showManualConfig = !showManualConfig}
                  class="text-sm text-blue-600 dark:text-blue-400 hover:underline"
                >
                  {showManualConfig ? '▼' : '▶'} Advanced: Use Existing Credentials
                </button>

                {#if showManualConfig}
                  <div class="mt-3 space-y-3 bg-gray-50 dark:bg-gray-800 p-3 rounded-md">
                    <p class="text-xs text-gray-600 dark:text-gray-400">
                      Use this to sync with the same account on multiple devices. Copy credentials from your first device.
                    </p>

                    <div>
                      <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Client ID
                      </label>
                      <input
                        type="text"
                        bind:value={manualClientId}
                        placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                        class="w-full px-2 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono"
                      />
                    </div>

                    <div>
                      <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                        API Key
                      </label>
                      <input
                        type="password"
                        bind:value={manualApiKey}
                        placeholder="64-character hex string"
                        class="w-full px-2 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono"
                      />
                    </div>

                    <button
                      on:click={handleManualConfig}
                      disabled={!manualClientId || !manualApiKey || configuringManual}
                      class="w-full px-3 py-1.5 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded transition-colors"
                    >
                      {configuringManual ? 'Saving...' : '💾 Save Credentials'}
                    </button>
                  </div>
                {/if}
              </div>
            {/if}

            <!-- Error Display -->
            {#if syncError}
              <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3">
                <p class="text-sm text-red-700 dark:text-red-300">
                  {syncError}
                </p>
              </div>
            {/if}
          </div>
        </div>

        <!-- Import/Export -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Import/Export</h3>

          <div class="space-y-3">
            <div class="flex gap-2">
              <button
                on:click={handleExport}
                class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
              >
                📤 {$_('settings.exportNotes')}
              </button>
              <button
                on:click={handleImport}
                class="flex-1 px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
              >
                📥 {$_('settings.importNotes')}
              </button>
            </div>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              Export notes as decrypted JSON. Import will merge notes with existing data.
            </p>
          </div>
        </div>

        <!-- Help & Resources -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Help & Resources</h3>

          <button
            on:click={onOpenShortcutsHelp}
            class="w-full px-4 py-2 bg-gray-100 hover:bg-gray-200 dark:bg-gray-700 dark:hover:bg-gray-600 text-gray-900 dark:text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
          >
            ⌨️ {$_('settings.keyboardShortcuts')}
          </button>
          <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
            View all available keyboard shortcuts
          </p>
        </div>

        <!-- Danger Zone -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-red-600 dark:text-red-400 mb-4">Danger Zone</h3>

          <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
            <h4 class="text-sm font-medium text-red-800 dark:text-red-200 mb-2">
              Delete All Data
            </h4>
            <p class="text-sm text-red-700 dark:text-red-300 mb-3">
              This will permanently delete ALL notes, settings, and encryption keys. This action cannot be undone.
            </p>
            <button
              on:click={handleDeleteDatabase}
              class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-medium rounded-md transition-colors"
            >
              {$_('settings.deleteDatabase')}
            </button>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-2">
        <button
          on:click={onClose}
          class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
        >
          {$_('common.cancel')}
        </button>
        <button
          on:click={handleSave}
          disabled={saving}
          class="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-medium rounded-md transition-colors"
        >
          {saving ? $_('settings.saving') : $_('settings.saveSettings')}
        </button>
      </div>
    </div>

    <!-- Hidden file input for import -->
    <input
      bind:this={fileInput}
      type="file"
      accept=".json"
      on:change={handleFileSelect}
      class="hidden"
    />
  </div>

  <!-- Delete Database Confirmation Modal -->
  <ConfirmModal
    show={showDeleteConfirm}
    title="Delete All Data"
    message="This will permanently delete ALL notes, settings, and encryption keys. This action cannot be undone.{'\n\n'}Type DELETE to confirm:"
    confirmText="Delete Everything"
    cancelText="Cancel"
    confirmClass="bg-red-600 hover:bg-red-700"
    requireTextMatch="DELETE"
    onConfirm={confirmDeleteDatabase}
    onCancel={() => showDeleteConfirm = false}
  />
{/if}
