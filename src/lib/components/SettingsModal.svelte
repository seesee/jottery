<script lang="ts">
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService, AVAILABLE_LOCALES, syncService, syncRepository, keyManager, cryptoService, encryptionRepository, lock, passwordStorageService, noteRepository } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';
  import { locale, _ } from 'svelte-i18n';
  import type { Theme, SyncStatus, KeyboardShortcut, KeyboardShortcuts } from '../types';
  import { DEFAULT_KEYBOARD_SHORTCUTS } from '../types';
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
  let rememberPasswordConfirmInput = '';
  let rememberPasswordError = '';

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
  let showDisconnectSyncConfirm = false;

  // Keyboard shortcut recording
  let recordingShortcut: keyof KeyboardShortcuts | null = null;
  let tempShortcuts: KeyboardShortcuts = { ...$settings.keyboardShortcuts } || { ...DEFAULT_KEYBOARD_SHORTCUTS };

  // Apply theme when it changes
  $: applyTheme(theme);

  // Load sync status when modal opens
  $: if (show) {
    loadSyncStatus();
    // Reset temp shortcuts to current settings
    tempShortcuts = { ...$settings.keyboardShortcuts } || { ...DEFAULT_KEYBOARD_SHORTCUTS };
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

  function handleDisconnectSync() {
    showDisconnectSyncConfirm = true;
  }

  async function confirmDisconnectSync() {
    showDisconnectSyncConfirm = false;
    try {
      console.log('[SettingsModal] Disconnecting from sync server...');

      // Clear sync metadata
      await syncRepository.updateMetadata({
        clientId: undefined,
        syncEndpoint: undefined,
        syncEnabled: false,
        apiKey: undefined,
      });

      // Update settings
      await settingsRepository.update({
        syncEndpoint: undefined,
        syncEnabled: false,
      });

      // Update local state
      syncEndpoint = '';
      await loadSyncStatus();

      console.log('[SettingsModal] Successfully disconnected from sync server');
    } catch (error) {
      console.error('Failed to disconnect from sync:', error);
      syncError = error instanceof Error ? error.message : 'Failed to disconnect';
    }
  }


  function formatShortcutDisplay(shortcut: KeyboardShortcut | undefined): string {
    if (!shortcut) return 'Not set';

    const parts: string[] = [];
    if (shortcut.ctrl) parts.push('Ctrl/Cmd');
    if (shortcut.alt) parts.push('Alt');
    if (shortcut.shift) parts.push('Shift');

    const key = shortcut.key.length === 1 && shortcut.key.match(/[a-z]/i)
      ? shortcut.key.toUpperCase()
      : shortcut.key;
    parts.push(key);

    return parts.join(' + ');
  }

  function startRecording(shortcutName: keyof KeyboardShortcuts) {
    recordingShortcut = shortcutName;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Prevent default browser behavior
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();

      // Ignore just modifier keys by themselves
      if (['Control', 'Alt', 'Shift', 'Meta', 'Command'].includes(e.key)) {
        return;
      }

      const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;

      // Capture the exact modifiers that are pressed
      // On Mac, support both Cmd (metaKey) and Ctrl (ctrlKey) as "Ctrl"
      // On Windows/Linux, just use Ctrl (ctrlKey)
      const hasCtrl = e.metaKey || e.ctrlKey;
      const hasAlt = e.altKey;
      const hasShift = e.shiftKey;

      // Normalize key to lowercase to handle Shift properly
      // e.g., Shift+R gives 'R', but we want to store 'r'
      const normalizedKey = e.key.length === 1 ? e.key.toLowerCase() : e.key;

      // Explicitly set all modifiers
      // This ensures Ctrl+N is different from Ctrl+Alt+N
      const newShortcut: KeyboardShortcut = {
        key: normalizedKey,
        ctrl: hasCtrl || undefined,
        alt: hasAlt || undefined,
        shift: hasShift || undefined,
      };

      console.log('[SettingsModal] Recorded shortcut:', {
        raw: e.key,
        normalized: normalizedKey,
        platform: navigator.platform,
        isMac: isMac,
        eventModifiers: {
          ctrlKey: e.ctrlKey,
          metaKey: e.metaKey,
          altKey: e.altKey,
          shiftKey: e.shiftKey
        },
        computed: { ctrl: hasCtrl, alt: hasAlt, shift: hasShift },
        result: newShortcut
      });

      tempShortcuts = {
        ...tempShortcuts,
        [shortcutName]: newShortcut,
      };

      recordingShortcut = null;
      window.removeEventListener('keydown', handleKeyDown, { capture: true });
      window.removeEventListener('keyup', handleKeyUp, { capture: true });
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      // Cancel recording on Escape
      if (e.key === 'Escape') {
        e.preventDefault();
        recordingShortcut = null;
        window.removeEventListener('keydown', handleKeyDown, { capture: true });
        window.removeEventListener('keyup', handleKeyUp, { capture: true });
      }
    };

    // Use capture phase to intercept before browser shortcuts
    window.addEventListener('keydown', handleKeyDown, { capture: true, passive: false });
    window.addEventListener('keyup', handleKeyUp, { capture: true, passive: false });
  }

  function resetShortcuts() {
    tempShortcuts = { ...DEFAULT_KEYBOARD_SHORTCUTS };
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
        keyboardShortcuts: tempShortcuts,
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
        keyboardShortcuts: tempShortcuts,
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
    if (!rememberPasswordConfirmInput) {
      rememberPasswordError = 'Please enter your password';
      return;
    }

    rememberPasswordError = '';

    try {
      // Verify the password by attempting to unlock with it
      const metadata = await encryptionRepository.getMetadata();
      if (!metadata) {
        throw new Error('Encryption not initialized');
      }

      const salt = base64ToUint8Array(metadata.salt);
      const derivedKey = await cryptoService.deriveKey({
        password: rememberPasswordConfirmInput,
        salt,
        iterations: metadata.iterations,
        algorithm: 'PBKDF2',
      });

      // Try to decrypt a note to verify password is correct
      const notes = await noteRepository.getAllActive();
      if (notes.length > 0) {
        const testNote = notes[0];
        const encryptedContent = JSON.parse(testNote.content);
        await cryptoService.decryptText(encryptedContent, derivedKey);
      }

      // Password is correct! Store it
      passwordStorageService.store(rememberPasswordConfirmInput);
      console.log('[SettingsModal] Password verified and stored successfully');

      // Save the setting immediately to persist it
      await settingsRepository.update({ rememberPassword: true });
      settings.update(s => ({ ...s, rememberPassword: true }));
      console.log('[SettingsModal] rememberPassword setting saved to database');

      // Close modal and clear input
      showRememberPasswordWarning = false;
      rememberPasswordConfirmInput = '';
      rememberPassword = true;
    } catch (error) {
      console.error('Password verification failed:', error);
      rememberPasswordError = 'Incorrect password';
      rememberPasswordConfirmInput = '';
      rememberPassword = false;
    }
  }

  function cancelEnableRememberPassword() {
    showRememberPasswordWarning = false;
    rememberPassword = false;
    rememberPasswordConfirmInput = '';
    rememberPasswordError = '';
  }

  // Helper function to convert base64 to Uint8Array
  function base64ToUint8Array(base64: string): Uint8Array {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
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
                  placeholder={typeof window !== 'undefined' ? window.location.origin : 'https://example.com'}
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

        <!-- Keyboard Shortcuts -->
        <div class="border-t border-gray-200 dark:border-gray-700 pt-6">
          <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Keyboard Shortcuts</h3>

          <div class="space-y-3">
            <p class="text-sm text-gray-600 dark:text-gray-400">
              Customize keyboard shortcuts. Click on a shortcut to change it, then press your desired key combination.
            </p>

            <div class="space-y-2">
              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Focus Search</span>
                <button
                  on:click={() => startRecording('focusSearch')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'focusSearch' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'focusSearch' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.focusSearch)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Create New Note</span>
                <button
                  on:click={() => startRecording('newNote')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'newNote' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'newNote' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.newNote)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Lock Application</span>
                <button
                  on:click={() => startRecording('lockApp')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'lockApp' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'lockApp' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.lockApp)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Open Settings</span>
                <button
                  on:click={() => startRecording('openSettings')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'openSettings' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'openSettings' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.openSettings)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Show Shortcuts Help</span>
                <button
                  on:click={() => startRecording('showShortcuts')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'showShortcuts' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'showShortcuts' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.showShortcuts)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Copy Note Content</span>
                <button
                  on:click={() => startRecording('copyNote')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'copyNote' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'copyNote' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.copyNote)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Undo</span>
                <button
                  on:click={() => startRecording('undo')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'undo' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'undo' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.undo)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Redo</span>
                <button
                  on:click={() => startRecording('redo')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'redo' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'redo' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.redo)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Version History</span>
                <button
                  on:click={() => startRecording('versionHistory')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'versionHistory' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'versionHistory' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.versionHistory)}
                </button>
              </div>

              <div class="flex items-center justify-between py-2 border-b border-gray-100 dark:border-gray-700">
                <span class="text-sm text-gray-700 dark:text-gray-300">Note Info</span>
                <button
                  on:click={() => startRecording('noteInfo')}
                  class="px-3 py-1 text-xs font-mono {recordingShortcut === 'noteInfo' ? 'bg-blue-100 dark:bg-blue-900 border-blue-500' : 'bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600'} border border-gray-300 dark:border-gray-600 rounded transition-colors"
                >
                  {recordingShortcut === 'noteInfo' ? 'Press a key...' : formatShortcutDisplay(tempShortcuts.noteInfo)}
                </button>
              </div>
            </div>

            <button
              on:click={resetShortcuts}
              class="w-full px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors"
            >
              Reset to Defaults
            </button>
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

          <!-- Disconnect Sync Server -->
          {#if syncStatus?.isEnabled}
            <div class="bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg p-4 mb-4">
              <h4 class="text-sm font-medium text-orange-800 dark:text-orange-200 mb-2">
                🔌 Disconnect Sync Server
              </h4>
              <p class="text-sm text-orange-700 dark:text-orange-300 mb-3">
                Disconnect this device from the sync server. This will clear sync credentials but will NOT delete your notes.
              </p>
              <button
                on:click={handleDisconnectSync}
                class="px-4 py-2 bg-orange-600 hover:bg-orange-700 text-white text-sm font-medium rounded-md transition-colors"
              >
                Disconnect from Sync Server
              </button>
            </div>
          {/if}

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
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-between items-center gap-3 flex-shrink-0">
        <div class="text-xs text-gray-500 dark:text-gray-400">
          v{__APP_VERSION__}
        </div>
        <div class="flex gap-3">
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

  <!-- Disconnect Sync Confirmation Modal -->
  <ConfirmModal
    show={showDisconnectSyncConfirm}
    title="Disconnect from Sync Server?"
    message="This will disconnect this device from the sync server and clear all sync credentials.{'\n\n'}Your notes will NOT be deleted and will remain on this device.{'\n\n'}You can reconnect to a sync server later."
    confirmText="Disconnect"
    cancelText="Cancel"
    confirmClass="bg-orange-600 hover:bg-orange-700"
    onConfirm={confirmDisconnectSync}
    onCancel={() => showDisconnectSyncConfirm = false}
  />

  <!-- Remember Password Warning Modal -->
  {#if showRememberPasswordWarning}
    <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6">
        <h3 class="text-xl font-bold text-orange-600 dark:text-orange-400 mb-4">
          ⚠️ Security Warning
        </h3>

        <div class="mb-4 text-sm text-gray-700 dark:text-gray-300 space-y-2">
          <p class="font-semibold">
            Enabling this feature will store your password in plain text in localStorage, which is HIGHLY INSECURE.
          </p>
          <ul class="list-disc list-inside space-y-1 text-gray-600 dark:text-gray-400">
            <li>Anyone with access to your device can read it</li>
            <li>Browser extensions can access it</li>
            <li>It may survive browser cache clearing</li>
            <li>Auto-lock will be disabled</li>
          </ul>
          <p class="font-semibold text-orange-700 dark:text-orange-300">
            Only enable this if you fully understand the security risks.
          </p>
        </div>

        <div class="mb-4">
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Enter your password to confirm:
          </label>
          <input
            type="password"
            bind:value={rememberPasswordConfirmInput}
            on:keydown={(e) => e.key === 'Enter' && confirmEnableRememberPassword()}
            placeholder="Your password"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-orange-500"
            autofocus
          />
          {#if rememberPasswordError}
            <p class="mt-2 text-sm text-red-600 dark:text-red-400">{rememberPasswordError}</p>
          {/if}
        </div>

        <div class="flex gap-3 justify-end">
          <button
            on:click={cancelEnableRememberPassword}
            class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
          >
            Cancel
          </button>
          <button
            on:click={confirmEnableRememberPassword}
            class="px-4 py-2 bg-orange-600 hover:bg-orange-700 text-white font-medium rounded-md transition-colors"
          >
            I Understand, Enable Anyway
          </button>
        </div>
      </div>
    </div>
  {/if}

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
