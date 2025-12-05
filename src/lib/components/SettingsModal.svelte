<script lang="ts">
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService, AVAILABLE_LOCALES, syncService, syncRepository, keyManager, cryptoService, encryptionRepository } from '../services';
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
  let deviceName = 'My Device';
  let showImportCredentials = false;
  let importCredentialsText = '';
  let importing = false;
  let showCopiedMessage = false;

  // Apply theme when it changes
  $: applyTheme(theme);

  // Load sync status when modal opens
  $: if (show) {
    loadSyncStatus();
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

    if (!deviceName.trim()) {
      syncError = 'Please provide a device name';
      return;
    }

    registering = true;
    syncError = '';
    try {
      const response = await syncService.register(syncEndpoint, deviceName.trim());
      console.log('[SettingsModal] Registration successful');

      // Reload status
      await loadSyncStatus();
      syncEndpoint = $settings.syncEndpoint || '';
      syncError = '';
    } catch (error) {
      console.error('Registration failed:', error);
      syncError = error instanceof Error ? error.message : 'Registration failed';
    } finally {
      registering = false;
    }
  }

  async function handleCopySyncCredentials() {
    try {
      // Get all required data
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      const metadata = await syncRepository.getMetadata();
      if (!metadata || !metadata.apiKey || !metadata.clientId) {
        throw new Error('Sync not configured');
      }

      const encryptionMeta = await encryptionRepository.getMetadata();
      if (!encryptionMeta) {
        throw new Error('Encryption not initialized');
      }

      // Decrypt API key
      const encryptedApiKey = JSON.parse(metadata.apiKey);
      const apiKey = await cryptoService.decryptText(encryptedApiKey, masterKey.key);

      // Create credentials object
      const credentials = {
        endpoint: metadata.syncEndpoint,
        clientId: metadata.clientId,
        apiKey: apiKey,
        salt: encryptionMeta.salt,
      };

      // Encode as base64
      const json = JSON.stringify(credentials);
      const base64 = btoa(json);

      // Copy to clipboard
      await navigator.clipboard.writeText(base64);

      // Show success message
      showCopiedMessage = true;
      setTimeout(() => showCopiedMessage = false, 3000);

      console.log('[SettingsModal] Credentials copied to clipboard');
    } catch (error) {
      console.error('Failed to copy credentials:', error);
      syncError = error instanceof Error ? error.message : 'Failed to copy credentials';
    }
  }

  async function handleImportCredentials() {
    if (!importCredentialsText.trim()) {
      syncError = 'Please paste the credentials';
      return;
    }

    importing = true;
    syncError = '';

    try {
      // Decode base64
      const json = atob(importCredentialsText.trim());
      const credentials = JSON.parse(json);

      // Validate structure
      if (!credentials.endpoint || !credentials.clientId || !credentials.apiKey || !credentials.salt) {
        throw new Error('Invalid credentials format');
      }

      console.log('[SettingsModal] Importing credentials...');

      // Step 1: Import encryption salt
      await encryptionRepository.setMetadata({
        salt: credentials.salt,
        iterations: 100000,
        createdAt: new Date().toISOString(),
        algorithm: 'AES-256-GCM',
      });
      console.log('[SettingsModal] Encryption salt imported');

      // Step 2: Store sync metadata with PLAINTEXT API key temporarily
      // It will be encrypted after unlock with the correct master key
      await syncRepository.updateMetadata({
        clientId: credentials.clientId,
        syncEndpoint: credentials.endpoint,
        syncEnabled: false,  // Will be enabled after encrypting API key
        apiKey: `IMPORT:${credentials.apiKey}`,  // Temporary plaintext marker
      });

      await settingsRepository.update({
        syncEndpoint: credentials.endpoint,
      });

      console.log('[SettingsModal] Credentials stored. Locking app...');

      // Step 3: Lock the app to force re-unlock with new salt
      const { lock } = await import('../services');
      lock();
    } catch (error) {
      console.error('Import failed:', error);
      syncError = error instanceof Error ? error.message : 'Failed to import credentials';
    } finally {
      importing = false;
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
            {#if !syncStatus?.isEnabled}
              <!-- Setup: Endpoint and Device Name -->
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Sync Server Endpoint
                </label>
                <input
                  type="url"
                  bind:value={syncEndpoint}
                  placeholder="http://localhost:3030"
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  URL of your self-hosted Jottery sync server
                </p>
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Device Name
                </label>
                <input
                  type="text"
                  bind:value={deviceName}
                  placeholder="My Laptop"
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  A name to identify this device
                </p>
              </div>

              <!-- Setup Buttons -->
              <div class="flex gap-3">
                <button
                  on:click={handleRegister}
                  disabled={!syncEndpoint || !deviceName.trim() || registering}
                  class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                >
                  {registering ? 'Registering...' : '🔗 Register New Device'}
                </button>
                <button
                  on:click={() => showImportCredentials = !showImportCredentials}
                  class="flex-1 px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
                >
                  📋 Use Existing Credentials
                </button>
              </div>

              <div class="text-xs text-gray-500 dark:text-gray-400">
                <p>• <strong>Register</strong> if this is your first device</p>
                <p>• <strong>Use Existing</strong> if you already set up sync on another device</p>
              </div>

              <!-- Import Credentials Box -->
              {#if showImportCredentials}
                <div class="border border-blue-200 dark:border-blue-800 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20">
                  <h4 class="font-medium text-sm text-gray-900 dark:text-white mb-2">
                    Import Credentials
                  </h4>
                  <p class="text-xs text-gray-600 dark:text-gray-400 mb-3">
                    Paste the credentials from your first device. The app will lock and you'll need to unlock with your password.
                  </p>
                  <textarea
                    bind:value={importCredentialsText}
                    placeholder="Paste base64 credentials here..."
                    rows="4"
                    class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-xs"
                  />
                  <button
                    on:click={handleImportCredentials}
                    disabled={!importCredentialsText.trim() || importing}
                    class="w-full mt-3 px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded-md transition-colors"
                  >
                    {importing ? 'Importing...' : '📥 Import and Lock'}
                  </button>
                </div>
              {/if}
            {:else}
              <!-- Sync Enabled - Show Status & Copy Credentials -->
              <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-3 space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-medium text-green-800 dark:text-green-200">
                    ✓ Sync Enabled
                  </span>
                  {#if syncStatus?.isSyncing}
                    <span class="text-xs text-green-600 dark:text-green-400">Syncing...</span>
                  {:else if syncStatus?.lastSyncAt}
                    <span class="text-xs text-green-600 dark:text-green-400">
                      Last sync: {new Date(syncStatus.lastSyncAt).toLocaleString()}
                    </span>
                  {/if}
                </div>

                <button
                  on:click={handleSyncNow}
                  disabled={syncing || syncStatus?.isSyncing}
                  class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                >
                  {syncing || syncStatus?.isSyncing ? 'Syncing...' : '🔄 Sync Now'}
                </button>

                {#if syncStatus?.pendingNotes > 0}
                  <p class="text-xs text-gray-600 dark:text-gray-400">
                    {syncStatus.pendingNotes} note{syncStatus.pendingNotes !== 1 ? 's' : ''} pending sync
                  </p>
                {/if}

                <div class="border-t border-green-200 dark:border-green-700 pt-3">
                  <button
                    on:click={handleCopySyncCredentials}
                    class="w-full px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium rounded-md transition-colors"
                  >
                    📋 Copy Credentials for Other Devices
                  </button>

                  {#if showCopiedMessage}
                    <div class="mt-2 bg-green-100 dark:bg-green-800/30 border border-green-300 dark:border-green-600 rounded p-2">
                      <p class="text-xs text-green-700 dark:text-green-300">
                        ✓ Credentials copied to clipboard! Paste them in "Use Existing Credentials" on your other device.
                      </p>
                    </div>
                  {/if}

                  <p class="mt-2 text-xs text-gray-600 dark:text-gray-400">
                    Click to copy all credentials as a single base64 string. Use "Use Existing Credentials" on other devices to import.
                  </p>
                  <p class="mt-1 text-xs text-orange-600 dark:text-orange-400 font-medium">
                    ⚠️ All devices must use the SAME password to decrypt notes!
                  </p>
                </div>
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
