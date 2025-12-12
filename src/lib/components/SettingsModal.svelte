<script lang="ts">
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService, AVAILABLE_LOCALES, syncService, syncRepository, keyManager, cryptoService, encryptionRepository, lock, passwordStorageService } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';
  import { locale, _ } from 'svelte-i18n';
  import type { Theme, SyncStatus } from '../types';
  import ConfirmModal from './ConfirmModal.svelte';

  export let show = false;
  export let onClose: () => void = () => {};
  export let onOpenShortcutsHelp: () => void = () => {};

  let theme: Theme = $settings.theme;
  let layoutMode: 'auto' | 'mobile' | 'desktop' = $settings.layoutMode || 'auto';
  let autoLockTimeout = $settings.autoLockTimeout;
  let sortOrder = $settings.sortOrder;
  let language = $settings.language;
  let rememberPassword = $settings.rememberPassword || false;
  let saving = false;
  let fileInput: HTMLInputElement;
  let showDeleteConfirm = false;
  let showRememberPasswordWarning = false;

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
  let showCredentialsModal = false;
  let credentialsText = '';

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

      // Try to copy to clipboard (best effort - may fail over SSH or in some browsers)
      try {
        await navigator.clipboard.writeText(base64);
        console.log('[SettingsModal] Credentials copied to clipboard');
      } catch (clipboardError) {
        console.warn('[SettingsModal] Clipboard copy failed (this is OK):', clipboardError);
      }

      // Always show the modal with text for manual copy
      credentialsText = base64;
      showCredentialsModal = true;

      console.log('[SettingsModal] Showing credentials modal');
    } catch (error) {
      console.error('Failed to generate credentials:', error);
      syncError = error instanceof Error ? error.message : 'Failed to generate credentials';
    }
  }

  async function handleImportCredentials() {
    console.log('[Import] Starting credential import...');

    if (!importCredentialsText.trim()) {
      console.error('[Import] No credentials text provided');
      syncError = 'Please paste the credentials';
      return;
    }

    importing = true;
    syncError = '';

    try {
      console.log('[Import] Step 1: Decoding base64...');
      const json = atob(importCredentialsText.trim());
      console.log('[Import] Base64 decoded, parsing JSON...');

      const credentials = JSON.parse(json);
      console.log('[Import] Credentials parsed:', {
        hasEndpoint: !!credentials.endpoint,
        hasClientId: !!credentials.clientId,
        hasApiKey: !!credentials.apiKey,
        hasSalt: !!credentials.salt,
        endpoint: credentials.endpoint,
        clientId: credentials.clientId,
      });

      // Validate structure
      if (!credentials.endpoint || !credentials.clientId || !credentials.apiKey || !credentials.salt) {
        console.error('[Import] Invalid credentials structure');
        throw new Error('Invalid credentials format - missing required fields');
      }

      console.log('[Import] Step 2: Storing encryption salt...');
      await encryptionRepository.setMetadata({
        salt: credentials.salt,
        iterations: 100000,
        createdAt: new Date().toISOString(),
        algorithm: 'AES-256-GCM',
      });
      console.log('[Import] ✓ Encryption salt stored');

      console.log('[Import] Step 3: Storing sync metadata...');
      await syncRepository.updateMetadata({
        clientId: credentials.clientId,
        syncEndpoint: credentials.endpoint,
        syncEnabled: false,
        apiKey: `IMPORT:${credentials.apiKey}`,
      });
      console.log('[Import] ✓ Sync metadata stored');

      console.log('[Import] Step 4: Updating settings...');
      await settingsRepository.update({
        syncEndpoint: credentials.endpoint,
        syncEnabled: false,
      });
      console.log('[Import] ✓ Settings updated');

      console.log('[Import] Step 5: Locking app and forcing UI update...');

      // Close the modal first
      onClose();

      // Lock and update UI state
      lock();
      isLocked.set(true);

      console.log('[Import] ✓ Import complete! App locked. Please unlock with your password.');

      // Clear the import text
      importCredentialsText = '';
      showImportCredentials = false;

    } catch (error) {
      console.error('[Import] ERROR:', error);
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
        layoutMode,
        autoLockTimeout,
        sortOrder,
        language,
        rememberPassword,
      });

      // Update store
      settings.update(s => ({
        ...s,
        theme,
        layoutMode,
        autoLockTimeout,
        sortOrder,
        language,
        rememberPassword,
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

  function handleRememberPasswordToggle() {
    if (rememberPassword) {
      // Just enabled (value is now true) - show warning
      showRememberPasswordWarning = true;
    } else {
      // Just disabled (value is now false) - clear stored password and lock
      handleDisableRememberPassword();
    }
  }

  async function confirmEnableRememberPassword() {
    showRememberPasswordWarning = false;
    rememberPassword = true;

    // Get current password from keyManager (user is already unlocked)
    const masterKey = keyManager.getMasterKey();
    if (masterKey && masterKey.password) {
      try {
        // Store the password
        passwordStorageService.store(masterKey.password);
        console.log('[SettingsModal] Password stored successfully');

        // Save the setting immediately to persist it
        await settingsRepository.update({ rememberPassword: true });
        settings.update(s => ({ ...s, rememberPassword: true }));
        console.log('[SettingsModal] rememberPassword setting saved to database');
      } catch (error) {
        console.error('Failed to store password:', error);
        alert('Failed to store password: ' + (error instanceof Error ? error.message : String(error)));
        rememberPassword = false;
        return;
      }
    } else {
      console.warn('[SettingsModal] No password available to store');
      rememberPassword = false;
    }
  }

  function cancelEnableRememberPassword() {
    showRememberPasswordWarning = false;
    rememberPassword = false;
  }

  async function handleDisableRememberPassword() {
    rememberPassword = false;

    // Clear stored password
    passwordStorageService.clear();
    console.log('[SettingsModal] Stored password cleared');

    // Save the setting immediately to persist it
    try {
      await settingsRepository.update({ rememberPassword: false });
      settings.update(s => ({ ...s, rememberPassword: false }));
      console.log('[SettingsModal] rememberPassword setting disabled in database');
    } catch (error) {
      console.error('Failed to save disabled setting:', error);
    }

    // Lock the application
    lock();
    isLocked.set(true);
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
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
  >
    <div class="bg-white dark:bg-gray-800 w-full h-full tablet:h-auto tablet:max-w-2xl tablet:rounded-lg shadow-xl tablet:max-h-[90vh] flex flex-col">
      <!-- Header -->
      <div class="border-b border-gray-200 dark:border-gray-700 p-4 flex items-center justify-between flex-shrink-0">
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">Settings</h2>
        <button
          on:click={onClose}
          class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
          aria-label="Close settings"
        >
          ✕
        </button>
      </div>

      <!-- Content (scrollable) -->
      <div class="flex-1 overflow-y-auto">

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

        <!-- Layout Mode -->
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Layout Mode
          </label>
          <select
            bind:value={layoutMode}
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="auto">Auto (Responsive)</option>
            <option value="mobile">Force Mobile Layout</option>
            <option value="desktop">Force Desktop Layout</option>
          </select>
          <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
            Override the automatic layout detection for this device
          </p>
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

                  <div class="mb-3 bg-orange-100 dark:bg-orange-900/30 border border-orange-300 dark:border-orange-700 rounded p-2">
                    <p class="text-xs text-orange-800 dark:text-orange-200 font-medium">
                      ⚠️ IMPORTANT: You must use the SAME password on all devices!
                    </p>
                    <p class="text-xs text-orange-700 dark:text-orange-300 mt-1">
                      If you use a different password, notes will not decrypt.
                    </p>
                  </div>

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
                    📋 Show Credentials for Other Devices
                  </button>

                  <p class="mt-2 text-xs text-gray-600 dark:text-gray-400">
                    Click to display credentials as text. Use "Use Existing Credentials" on other devices to import.
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

          <!-- Remember Password Toggle -->
          <div class="bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg p-4 mb-4">
            <div class="flex items-start justify-between">
              <div class="flex-1">
                <h4 class="text-sm font-medium text-orange-800 dark:text-orange-200 mb-2">
                  🔓 Remember Password (Insecure)
                </h4>
                <p class="text-sm text-orange-700 dark:text-orange-300 mb-2">
                  Store your password on this device to skip entering it on every visit. <strong>WARNING:</strong> This stores your password in plain text in localStorage, which is highly insecure.
                </p>
                <p class="text-xs text-orange-600 dark:text-orange-400">
                  Auto-lock will be disabled when this is enabled. Disabling this will immediately lock the application.
                </p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer ml-4">
                <input
                  type="checkbox"
                  bind:checked={rememberPassword}
                  on:change={handleRememberPasswordToggle}
                  class="sr-only peer"
                />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-orange-300 dark:peer-focus:ring-orange-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-orange-600"></div>
              </label>
            </div>
          </div>

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
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end gap-3 flex-shrink-0">
        <button
          on:click={onClose}
          class="px-4 py-2.5 min-h-11 text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
        >
          {$_('common.cancel')}
        </button>
        <button
          on:click={handleSave}
          disabled={saving}
          class="px-4 py-2.5 min-h-11 bg-blue-600 active:bg-blue-700 disabled:bg-blue-400 text-white font-medium rounded-md transition-colors"
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

  <!-- Remember Password Warning Modal -->
  <ConfirmModal
    show={showRememberPasswordWarning}
    title="⚠️ Security Warning"
    message="Enabling this feature will store your password in plain text in localStorage, which is HIGHLY INSECURE.\n\n• Anyone with access to your device can read it\n• Browser extensions can access it\n• It may survive browser cache clearing\n• Auto-lock will be disabled\n\nOnly enable this if you fully understand the security risks.\n\nType ENABLE to confirm:"
    confirmText="I Understand, Enable Anyway"
    cancelText="Cancel"
    confirmClass="bg-orange-600 hover:bg-orange-700"
    requireTextMatch="ENABLE"
    onConfirm={confirmEnableRememberPassword}
    onCancel={cancelEnableRememberPassword}
  />

  <!-- Sync Credentials Display Modal -->
  {#if showCredentialsModal}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" on:click={() => showCredentialsModal = false}>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] overflow-hidden" on:click|stopPropagation>
        <!-- Header -->
        <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            📋 Sync Credentials
          </h3>
          <button
            on:click={() => showCredentialsModal = false}
            class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <!-- Content -->
        <div class="p-6 overflow-y-auto max-h-[calc(80vh-140px)]">
          <p class="text-sm text-gray-700 dark:text-gray-300 mb-4">
            Copy the text below and paste it into another Jottery client using "Use Existing Credentials":
          </p>

          <div class="bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded p-4">
            <pre class="text-xs font-mono text-gray-900 dark:text-gray-100 break-all whitespace-pre-wrap">{credentialsText}</pre>
          </div>

          <div class="mt-4 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded p-3">
            <p class="text-sm text-orange-800 dark:text-orange-200">
              ⚠️ <strong>Important:</strong> All devices must use the SAME password to decrypt notes!
            </p>
          </div>

          <p class="mt-3 text-xs text-gray-600 dark:text-gray-400">
            The clipboard copy may have failed (especially over SSH or in some browsers). You can manually select and copy the text above.
          </p>
        </div>

        <!-- Footer -->
        <div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
          <button
            on:click={() => showCredentialsModal = false}
            class="px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  {/if}
{/if}
