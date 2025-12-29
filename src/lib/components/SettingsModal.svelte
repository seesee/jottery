<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService, AVAILABLE_LOCALES, syncService, syncRepository, keyManager, cryptoService, encryptionRepository, lock, passwordStorageService, noteRepository } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';
  import { authService } from '../services/authService';
  import { locale, _ } from 'svelte-i18n';
  import type { Theme, SyncStatus, KeyboardShortcut, KeyboardShortcuts } from '../types';
  import { DEFAULT_KEYBOARD_SHORTCUTS } from '../types';
  import { ALL_LANGUAGES, CORE_LANGUAGES, findLanguage, calculateTotalSize, type SyntaxLanguage } from '../utils/syntaxLanguages';
  import ConfirmModal from './ConfirmModal.svelte';
  import DocumentationModal from './DocumentationModal.svelte';

  export let show = false;
  export let onClose: () => void = () => {};
  export let onOpenShortcutsHelp: () => void = () => {};

  let theme: Theme = $settings.theme;
  let layoutMode: 'auto' | 'mobile' | 'desktop' = $settings.layoutMode || 'auto';
  let fontSize: 'auto' | 'small' | 'medium' | 'large' = $settings.fontSize || 'auto';
  let autoLockTimeout = $settings.autoLockTimeout;
  let sortOrder = $settings.sortOrder;
  let language = $settings.language;
  let rememberPassword = $settings.rememberPassword || false;
  let enabledSyntaxLanguages: string[] = $settings.enabledSyntaxLanguages || [];
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

  // Multi-user registration state
  let registrationMode: 'select' | 'newUser' | 'existingUser' = 'select';
  let userEmail = '';
  let userPassword = '';
  let registeringUser = false;
  let registeringDevice = false;
  let registrationStep: 'email' | 'pending' | 'device' | 'complete' = 'email';
  let registeredUserId = '';
  let userRegistrationMessage = '';

  // Keyboard shortcut recording
  let recordingShortcut: keyof KeyboardShortcuts | null = null;
  let tempShortcuts: KeyboardShortcuts = { ...$settings.keyboardShortcuts } || { ...DEFAULT_KEYBOARD_SHORTCUTS };

  // Documentation and downloads
  let showDocumentation = false;
  let selectedArchitecture = detectArchitecture();

  // Tab state
  type Tab = 'general' | 'editor' | 'keyboard' | 'sync' | 'advanced' | 'about';
  let currentTab: Tab = 'general';

  // Detect user's architecture
  function detectArchitecture(): string {
    const userAgent = navigator.userAgent.toLowerCase();
    const platform = navigator.platform?.toLowerCase() || '';

    // macOS
    if (platform.includes('mac') || userAgent.includes('mac')) {
      return 'macos';
    }

    // Windows
    if (platform.includes('win') || userAgent.includes('windows')) {
      return 'windows';
    }

    // Linux - try to detect architecture
    if (platform.includes('linux') || userAgent.includes('linux')) {
      // Check for ARM
      if (userAgent.includes('aarch64') || userAgent.includes('arm64')) {
        return 'linux-arm64';
      }
      if (userAgent.includes('armv7') || userAgent.includes('armhf')) {
        return 'linux-armv7';
      }
      // Default to x64 for Linux
      return 'linux-x64';
    }

    // Can't detect
    return '';
  }

  const githubRepo = 'https://github.com/seesee/jottery';

  const architectures = [
    { value: 'macos', label: 'macOS (Universal)', url: `${githubRepo}/releases/latest/download/jottery-macos` },
    { value: 'windows', label: 'Windows (x64)', url: `${githubRepo}/releases/latest/download/jottery-windows.exe` },
    { value: 'linux-x64', label: 'Linux (x64)', url: `${githubRepo}/releases/latest/download/jottery-linux-x64` },
    { value: 'linux-arm64', label: 'Linux (ARM64)', url: `${githubRepo}/releases/latest/download/jottery-linux-arm64` },
    { value: 'linux-armv7', label: 'Linux (ARMv7)', url: `${githubRepo}/releases/latest/download/jottery-linux-armv7` },
  ];

  function handleDownload() {
    const arch = architectures.find(a => a.value === selectedArchitecture);
    if (arch) {
      window.open(arch.url, '_blank');
    }
  }

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

  // Legacy registration (old single-device flow)
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

  // New multi-user registration flow
  async function handleRegisterUser() {
    if (!syncEndpoint) {
      syncError = 'Please enter a sync endpoint URL';
      return;
    }

    if (!userEmail.trim()) {
      syncError = 'Please enter your email address';
      return;
    }

    if (!userPassword.trim() || userPassword.length < 12) {
      syncError = 'Password must be at least 12 characters';
      return;
    }

    registeringUser = true;
    syncError = '';
    try {
      const response = await authService.registerUser(syncEndpoint, userEmail, userPassword);
      console.log('[SettingsModal] User registration successful:', response);

      registeredUserId = response.userId;
      userRegistrationMessage = response.message;

      if (response.status === 'pending_approval') {
        registrationStep = 'pending';
      } else if (response.status === 'approved') {
        // User is already approved, move to device registration
        registrationStep = 'device';
      }
    } catch (error) {
      console.error('User registration failed:', error);
      const errorMessage = error instanceof Error ? error.message : 'User registration failed';

      // Handle "Email already registered" - allow resuming registration
      if (errorMessage.includes('Email already registered')) {
        console.log('[SettingsModal] Email already registered, proceeding to device registration step');
        // Assume they might be approved and let device registration handle the auth check
        registrationStep = 'device';
        syncError = ''; // Clear error, let device registration validate approval status
      } else {
        syncError = errorMessage;
      }
    } finally {
      registeringUser = false;
    }
  }

  async function handleRegisterDevice() {
    if (!userEmail || !userPassword) {
      syncError = 'Email and password are required';
      return;
    }

    if (!deviceName.trim()) {
      syncError = 'Please provide a device name';
      return;
    }

    registeringDevice = true;
    syncError = '';
    try {
      const response = await authService.registerDevice(
        syncEndpoint,
        userEmail,
        userPassword,
        deviceName.trim()
      );
      console.log('[SettingsModal] Device registration successful:', response);

      // Store the credentials similar to existing flow
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      // Encrypt and store API key
      const encryptedApiKey = await cryptoService.encryptText(response.apiKey, masterKey.key);

      // Save sync settings
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: syncEndpoint,
        apiKey: JSON.stringify(encryptedApiKey),
        clientId: response.clientId,
        userId: response.userId,
        userEmail: userEmail,
      });

      // Update settings store
      settings.update(s => ({
        ...s,
        syncEndpoint,
        syncEnabled: true,
      }));

      await settingsRepository.update({ syncEndpoint, syncEnabled: true });

      // Reload status
      await loadSyncStatus();

      // Trigger full sync automatically
      console.log('[SettingsModal] Triggering full sync of ALL notes after device registration...');
      const syncResult = await syncService.syncNow(true);
      if (syncResult.success) {
        console.log('[SettingsModal] ✓ Initial sync completed successfully');
        console.log('[SettingsModal] All existing notes have been synced to server');
      } else {
        console.warn('[SettingsModal] ⚠️ Initial sync failed, but device is registered. Error:', syncResult.error);
      }

      registrationStep = 'complete';
      syncError = '';
    } catch (error) {
      console.error('Device registration failed:', error);
      syncError = error instanceof Error ? error.message : 'Device registration failed';
    } finally {
      registeringDevice = false;
    }
  }

  function resetRegistrationFlow() {
    registrationMode = 'select';
    registrationStep = 'email';
    userEmail = '';
    userPassword = '';
    registeredUserId = '';
    userRegistrationMessage = '';
    syncError = '';
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
      // Force a full sync to ensure all notes (including imported ones) are pushed
      const result = await syncService.syncNow(true);
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
        fontSize,
        autoLockTimeout,
        sortOrder,
        language,
        rememberPassword,
        keyboardShortcuts: tempShortcuts,
        enabledSyntaxLanguages,
      });

      // Update store
      settings.update(s => ({
        ...s,
        theme,
        layoutMode,
        fontSize,
        autoLockTimeout,
        sortOrder,
        language,
        rememberPassword,
        keyboardShortcuts: tempShortcuts,
        enabledSyntaxLanguages,
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

  // Handle Escape key to close modal
  function handleKeyDown(event: KeyboardEvent) {
    if (show && event.key === 'Escape') {
      // Check if any child modal is open (they have z-50 class and are visible)
      const modals = document.querySelectorAll('.z-50');
      const hasOpenChildModal = Array.from(modals).some(modal => {
        const style = window.getComputedStyle(modal as HTMLElement);
        return style.display !== 'none' && style.visibility !== 'hidden' && modal.getAttribute('role') === 'dialog';
      });

      // Only close settings modal if no child modal is open
      if (!hasOpenChildModal) {
        event.preventDefault();
        onClose();
      }
    }
  }

  onMount(async () => {
    window.addEventListener('keydown', handleKeyDown);

    // Check if current server offers sync and use as default
    if (!syncEndpoint && typeof window !== 'undefined') {
      try {
        const defaultEndpoint = window.location.origin;
        console.log('[SettingsModal] Checking for sync server at:', defaultEndpoint);
        const response = await fetch(`${defaultEndpoint}/api/v1/sync/status`, {
          method: 'HEAD',
        });
        console.log('[SettingsModal] Sync status check response:', response.status);
        // If we get any response other than 404, assume sync is available
        // (401 Unauthorized means sync endpoint exists but needs auth)
        if (response.status !== 404) {
          console.log('[SettingsModal] ✓ Detected sync server at:', defaultEndpoint);
          syncEndpoint = defaultEndpoint;
        } else {
          console.log('[SettingsModal] No sync server at current origin (404)');
        }
      } catch (error) {
        // Network error - user can manually enter endpoint
        console.log('[SettingsModal] Network error checking sync server:', error);
      }
    } else if (syncEndpoint) {
      console.log('[SettingsModal] Sync endpoint already configured:', syncEndpoint);
    }
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });
</script>

{#if show}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={handleBackdropClick}
    on:keydown={(e) => e.key === 'Escape' && onClose()}
    role="dialog"
    aria-modal="true"
    tabindex="0"
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

      <!-- Tab Navigation -->
      <div class="border-b border-gray-200 dark:border-gray-700 flex overflow-x-auto flex-shrink-0">
        <button
          on:click={() => currentTab = 'general'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'general' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          General
        </button>
        <button
          on:click={() => currentTab = 'editor'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'editor' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          Editor
        </button>
        <button
          on:click={() => currentTab = 'keyboard'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'keyboard' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          Keyboard Shortcuts
        </button>
        <button
          on:click={() => currentTab = 'sync'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'sync' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          Sync
        </button>
        <button
          on:click={() => currentTab = 'advanced'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'advanced' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          Advanced
        </button>
        <button
          on:click={() => currentTab = 'about'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'about' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          About
        </button>
      </div>

      <!-- Content (scrollable) -->
      <div class="flex-1 overflow-y-auto">

      <!-- Content -->
      <div class="p-6 space-y-6">
        <!-- GENERAL TAB -->
        {#if currentTab === 'general'}
          <!-- Language -->
          <div>
            <label for="setting-language" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {$_('settings.language')}
            </label>
            <select
              id="setting-language"
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
            <label for="setting-theme" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {$_('settings.theme')}
            </label>
            <select
              id="setting-theme"
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
            <label for="setting-layout-mode" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Layout Mode
            </label>
            <select
              id="setting-layout-mode"
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

          <!-- Font Size -->
          <div>
            <label for="setting-font-size" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Editor Font Size
            </label>
            <select
              id="setting-font-size"
              bind:value={fontSize}
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value="auto">Auto (Mobile-aware: 16px on mobile, 14px desktop)</option>
              <option value="small">Small (12px)</option>
              <option value="medium">Medium (14px)</option>
              <option value="large">Large (16px)</option>
            </select>
            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
              Auto uses larger font on mobile to prevent browser zoom
            </p>
          </div>

          <!-- Auto-lock timeout -->
          <div>
            <label for="setting-autolock" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {$_('settings.autoLockTimeout')}
            </label>
            <input
              id="setting-autolock"
              type="number"
              bind:value={autoLockTimeout}
              min="1"
              max="1440"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <!-- Sort order -->
          <div>
            <label for="setting-sort-order" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {$_('settings.sortOrder')}
            </label>
            <select
              id="setting-sort-order"
              bind:value={sortOrder}
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value="recent">{$_('settings.sortRecent')}</option>
              <option value="created">{$_('settings.sortCreated')}</option>
              <option value="oldest">{$_('settings.sortOldest')}</option>
              <option value="alpha">{$_('settings.sortAlpha')}</option>
            </select>
          </div>

          <!-- Remember Password Toggle -->
          <div class="bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg p-4">
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
        {/if}

        <!-- EDITOR TAB -->
        {#if currentTab === 'editor'}
          <div class="space-y-6">
            <div>
              <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-4">Syntax Highlighting</h3>
              <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">
                Select which programming languages to enable for syntax highlighting in markdown code blocks and previews.
                Core languages are always enabled.
              </p>

              <!-- Summary Stats -->
              <div class="bg-gray-50 dark:bg-gray-700 rounded-lg p-4 mb-4">
                <div class="flex items-center justify-between">
                  <div>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">
                      {enabledSyntaxLanguages.length} languages enabled
                    </p>
                    <p class="text-xs text-gray-500 dark:text-gray-400">
                      Estimated size: ~{calculateTotalSize(enabledSyntaxLanguages)} KB
                    </p>
                  </div>
                  <div class="flex gap-2">
                    <button
                      on:click={() => enabledSyntaxLanguages = CORE_LANGUAGES.map(l => l.id)}
                      class="px-3 py-1 text-xs font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded hover:bg-gray-50 dark:hover:bg-gray-500"
                    >
                      Reset to Core
                    </button>
                    <button
                      on:click={() => enabledSyntaxLanguages = ALL_LANGUAGES.map(l => l.id)}
                      class="px-3 py-1 text-xs font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-600 border border-gray-300 dark:border-gray-500 rounded hover:bg-gray-50 dark:hover:bg-gray-500"
                    >
                      Enable All
                    </button>
                  </div>
                </div>
              </div>

              <!-- Language Groups -->
              <div class="space-y-4">
                <!-- Core Languages (Always Enabled) -->
                <div>
                  <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-2 flex items-center gap-2">
                    Core Languages
                    <span class="text-xs text-gray-500 dark:text-gray-400">(Always enabled)</span>
                  </h4>
                  <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2">
                    {#each CORE_LANGUAGES as lang}
                      <div class="flex items-center gap-2 p-2 bg-gray-50 dark:bg-gray-700 rounded border border-gray-200 dark:border-gray-600">
                        <input
                          type="checkbox"
                          checked={true}
                          disabled={true}
                          class="rounded border-gray-300 dark:border-gray-600 opacity-50"
                        />
                        <div class="flex-1">
                          <span class="text-sm font-medium text-gray-900 dark:text-white">{lang.name}</span>
                          {#if lang.aliases.length > 0}
                            <span class="text-xs text-gray-500 dark:text-gray-400 ml-1">
                              ({lang.aliases.slice(0, 2).join(', ')}{lang.aliases.length > 2 ? '...' : ''})
                            </span>
                          {/if}
                        </div>
                        <span class="text-xs text-gray-500 dark:text-gray-400">{lang.estimatedSize} KB</span>
                      </div>
                    {/each}
                  </div>
                </div>

                <!-- Popular Languages -->
                <div>
                  <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-2">Popular Languages</h4>
                  <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2">
                    {#each ALL_LANGUAGES.filter(l => l.category === 'popular') as lang}
                      <label class="flex items-center gap-2 p-2 bg-white dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={enabledSyntaxLanguages.includes(lang.id)}
                          on:change={(e) => {
                            if (e.currentTarget.checked) {
                              enabledSyntaxLanguages = [...enabledSyntaxLanguages, lang.id];
                            } else {
                              enabledSyntaxLanguages = enabledSyntaxLanguages.filter(id => id !== lang.id);
                            }
                          }}
                          class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
                        />
                        <div class="flex-1">
                          <span class="text-sm font-medium text-gray-900 dark:text-white">{lang.name}</span>
                          {#if lang.aliases.length > 0}
                            <span class="text-xs text-gray-500 dark:text-gray-400 ml-1">
                              ({lang.aliases.slice(0, 2).join(', ')}{lang.aliases.length > 2 ? '...' : ''})
                            </span>
                          {/if}
                        </div>
                        <span class="text-xs text-gray-500 dark:text-gray-400">{lang.estimatedSize} KB</span>
                      </label>
                    {/each}
                  </div>
                </div>

                <!-- Other Language Categories -->
                {#each ['web', 'systems', 'data', 'other'] as category}
                  {@const categoryLangs = ALL_LANGUAGES.filter(l => l.category === category)}
                  {#if categoryLangs.length > 0}
                    <details class="group">
                      <summary class="cursor-pointer list-none">
                        <div class="flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-white mb-2">
                          <span class="group-open:rotate-90 transition-transform">▶</span>
                          <span class="capitalize">{category} Languages</span>
                          <span class="text-xs text-gray-500 dark:text-gray-400">
                            ({categoryLangs.filter(l => enabledSyntaxLanguages.includes(l.id)).length}/{categoryLangs.length} enabled)
                          </span>
                        </div>
                      </summary>
                      <div class="grid grid-cols-1 tablet:grid-cols-2 gap-2 mt-2">
                        {#each categoryLangs as lang}
                          <label class="flex items-center gap-2 p-2 bg-white dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer">
                            <input
                              type="checkbox"
                              checked={enabledSyntaxLanguages.includes(lang.id)}
                              on:change={(e) => {
                                if (e.currentTarget.checked) {
                                  enabledSyntaxLanguages = [...enabledSyntaxLanguages, lang.id];
                                } else {
                                  enabledSyntaxLanguages = enabledSyntaxLanguages.filter(id => id !== lang.id);
                                }
                              }}
                              class="rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-blue-500"
                            />
                            <div class="flex-1">
                              <span class="text-sm font-medium text-gray-900 dark:text-white">{lang.name}</span>
                              {#if lang.aliases.length > 0}
                                <span class="text-xs text-gray-500 dark:text-gray-400 ml-1">
                                  ({lang.aliases.slice(0, 2).join(', ')}{lang.aliases.length > 2 ? '...' : ''})
                                </span>
                              {/if}
                            </div>
                            <span class="text-xs text-gray-500 dark:text-gray-400">{lang.estimatedSize} KB</span>
                          </label>
                        {/each}
                      </div>
                    </details>
                  {/if}
                {/each}
              </div>
            </div>
          </div>
        {/if}

        <!-- KEYBOARD SHORTCUTS TAB -->
        {#if currentTab === 'keyboard'}
          <div class="space-y-4">
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

            <div class="mt-4 space-y-2">
              <button
                on:click={resetShortcuts}
                class="w-full px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors"
              >
                Reset to Defaults
              </button>

              <button
                on:click={onOpenShortcutsHelp}
                class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
              >
                ⌨️ View All Shortcuts
              </button>
              <p class="text-sm text-gray-500 dark:text-gray-400 text-center">
                Quick reference of all keyboard shortcuts
              </p>
            </div>
          </div>
        {/if}

        <!-- SYNC TAB -->
        {#if currentTab === 'sync'}
          <div class="space-y-4">
            {#if !syncStatus?.isEnabled}
              <!-- Setup: Endpoint and Device Name -->
              <div>
                <label for="sync-endpoint" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Sync Server Endpoint
                </label>
                <input
                  id="sync-endpoint"
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
                <label for="sync-device-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Device Name
                </label>
                <input
                  id="sync-device-name"
                  type="text"
                  bind:value={deviceName}
                  placeholder="My Laptop"
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  A name to identify this device
                </p>
              </div>

              <!-- Device Sync Setup -->
              {#if registrationMode === 'select'}
                <div class="space-y-3">
                  <div class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                    Choose how to set up sync:
                  </div>

                  <!-- Option 1: Import Credentials (Most Secure) -->
                  <div class="border-2 border-green-500 dark:border-green-600 rounded-lg p-4 bg-green-50 dark:bg-green-900/20">
                    <div class="flex items-start gap-3 mb-3">
                      <span class="text-2xl">🔐</span>
                      <div class="flex-1">
                        <div class="font-semibold text-green-900 dark:text-green-100 mb-1">
                          Import Sync Credentials
                        </div>
                        <div class="text-xs text-green-800 dark:text-green-200 mb-2">
                          <strong>For additional devices:</strong> Most secure method
                        </div>
                        <div class="text-xs text-green-700 dark:text-green-300 space-y-1">
                          <div>✓ Zero-knowledge encryption (server cannot decrypt your notes)</div>
                          <div>✓ Copy credentials from your first device</div>
                          <div>✓ All devices use the same encryption key</div>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={() => showImportCredentials = !showImportCredentials}
                      class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      📋 Import Credentials from Another Device
                    </button>
                  </div>

                  <!-- Option 2: Register with Server (First Device Only) -->
                  <div class="border-2 border-blue-500 dark:border-blue-600 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20">
                    <div class="flex items-start gap-3 mb-3">
                      <span class="text-2xl">🌐</span>
                      <div class="flex-1">
                        <div class="font-semibold text-blue-900 dark:text-blue-100 mb-1">
                          Register with a Sync Server
                        </div>
                        <div class="text-xs text-blue-800 dark:text-blue-200 mb-2">
                          <strong>For your first device only:</strong> Creates your account
                        </div>
                        <div class="text-xs text-blue-700 dark:text-blue-300 space-y-1">
                          <div>✓ Creates email/password account</div>
                          <div>✓ Enables sync for this device</div>
                          <div>✓ Requires admin approval</div>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={() => registrationMode = 'newUser'}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      🚀 Register New Account
                    </button>
                  </div>

                  <!-- Info Box -->
                  <div class="bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg p-3">
                    <div class="text-xs text-gray-700 dark:text-gray-300 space-y-2">
                      <div>
                        <strong>🔒 Privacy:</strong> All your notes are encrypted end-to-end. The server cannot read your content.
                      </div>
                      <div>
                        <strong>💡 Self-host?</strong> Run your own server - <a href="https://github.com/chand1012/jottery" target="_blank" rel="noopener noreferrer" class="text-blue-600 dark:text-blue-400 hover:underline">see GitHub</a>
                      </div>
                    </div>
                  </div>
                </div>
              {:else if registrationMode === 'newUser'}
                <!-- New User Registration Flow -->
                <div class="border border-blue-200 dark:border-blue-800 rounded-lg p-4 bg-blue-50 dark:bg-blue-900/20 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      Register New User Account
                    </h4>
                    <button
                      on:click={resetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      ← Back
                    </button>
                  </div>

                  {#if registrationStep === 'email'}
                    <div>
                      <label for="user-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Email Address
                      </label>
                      <input
                        id="user-email"
                        type="email"
                        bind:value={userEmail}
                        placeholder="you@example.com"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                    </div>
                    <div>
                      <label for="user-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        Password (min 12 characters)
                      </label>
                      <input
                        id="user-password"
                        type="password"
                        bind:value={userPassword}
                        placeholder="••••••••••••"
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                      />
                      <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                        This password is for server authentication only
                      </p>
                    </div>
                    <button
                      on:click={handleRegisterUser}
                      disabled={!syncEndpoint || !userEmail || !userPassword || registeringUser}
                      class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {registeringUser ? 'Registering...' : 'Register User Account'}
                    </button>
                  {:else if registrationStep === 'pending'}
                    <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded p-3">
                      <div class="flex items-start gap-2">
                        <span class="text-xl">⏳</span>
                        <div>
                          <div class="font-semibold text-sm text-yellow-900 dark:text-yellow-100">
                            Pending Admin Approval
                          </div>
                          <p class="text-xs text-yellow-800 dark:text-yellow-200 mt-1">
                            {userRegistrationMessage}
                          </p>
                          <p class="text-xs text-yellow-700 dark:text-yellow-300 mt-2">
                            Contact your administrator to approve your account. Once approved, click below to complete setup.
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={handleRegisterDevice}
                      disabled={registeringDevice}
                      class="w-full px-4 py-2 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-medium rounded-md transition-colors"
                    >
                      {registeringDevice ? 'Completing Setup...' : 'I\'ve Been Approved - Complete Setup'}
                    </button>
                  {:else if registrationStep === 'device'}
                    <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3 mb-3">
                      <div class="text-xs text-green-800 dark:text-green-200">
                        ⏳ Registering device and syncing...
                      </div>
                    </div>
                  {:else if registrationStep === 'complete'}
                    <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded p-3">
                      <div class="flex items-start gap-2">
                        <span class="text-xl">✅</span>
                        <div>
                          <div class="font-semibold text-sm text-green-900 dark:text-green-100">
                            Sync Enabled Successfully!
                          </div>
                          <p class="text-xs text-green-800 dark:text-green-200 mt-2">
                            Your device is now registered and <strong>all your existing notes have been synced to the server</strong>. Any future changes will sync automatically.
                          </p>
                          <p class="text-xs text-green-700 dark:text-green-300 mt-2 font-medium">
                            📋 To add another device: Click "Export Sync Credentials" below and import them on your other device.
                          </p>
                        </div>
                      </div>
                    </div>
                    <button
                      on:click={resetRegistrationFlow}
                      class="w-full px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors mt-2"
                    >
                      Done
                    </button>
                  {/if}
                </div>
              {:else if registrationMode === 'existingUser'}
                <!-- Link Existing Account (Optional) -->
                <div class="border border-gray-300 dark:border-gray-600 rounded-lg p-4 bg-gray-50 dark:bg-gray-800 space-y-3">
                  <div class="flex items-center justify-between mb-2">
                    <h4 class="font-medium text-sm text-gray-900 dark:text-white">
                      Link Account (Optional)
                    </h4>
                    <button
                      on:click={resetRegistrationFlow}
                      class="text-xs text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
                    >
                      ← Back
                    </button>
                  </div>

                  <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700 rounded p-3 mb-3">
                    <p class="text-xs text-blue-800 dark:text-blue-200">
                      ℹ️ This feature is for future use (admin dashboard access, subscription management). It does <strong>not</strong> enable sync on this device.
                    </p>
                  </div>

                  <div>
                    <label for="existing-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Email Address
                    </label>
                    <input
                      id="existing-email"
                      type="email"
                      bind:value={userEmail}
                      placeholder="you@example.com"
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                  <div>
                    <label for="existing-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Password
                    </label>
                    <input
                      id="existing-password"
                      type="password"
                      bind:value={userPassword}
                      placeholder="••••••••••••"
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                  <button
                    disabled
                    class="w-full px-4 py-2 bg-gray-400 text-white text-sm font-medium rounded-md cursor-not-allowed"
                  >
                    Coming Soon
                  </button>
                  <p class="text-xs text-gray-500 dark:text-gray-400 text-center">
                    Account linking will be enabled in a future update
                  </p>
                </div>
              {/if}

              <!-- Error Display -->
              {#if syncError}
                <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3">
                  <div class="flex items-start gap-2">
                    <span class="text-xl">⚠️</span>
                    <div>
                      <div class="font-semibold text-sm text-red-900 dark:text-red-100">
                        Error
                      </div>
                      <p class="text-xs text-red-800 dark:text-red-200 mt-1">
                        {syncError}
                      </p>
                    </div>
                  </div>
                </div>
              {/if}

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
                  ></textarea>
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

            <!-- Disconnect Sync Server -->
            {#if syncStatus?.isEnabled}
              <div class="bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg p-4">
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
          </div>
        {/if}

        <!-- ADVANCED TAB -->
        {#if currentTab === 'advanced'}
          <div class="space-y-6">
            <!-- Import/Export -->
            <div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Import/Export</h4>
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

            <!-- Download Terminal Client -->
            <div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">💻 Download Terminal Client</h4>
              <div class="flex gap-2 mb-2">
                <select
                  bind:value={selectedArchitecture}
                  class="flex-1 px-3 py-2 min-h-11 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  {#if selectedArchitecture === ''}
                    <option value="">Select platform...</option>
                  {/if}
                  {#each architectures as arch}
                    <option value={arch.value}>{arch.label}</option>
                  {/each}
                </select>
                <button
                  on:click={handleDownload}
                  disabled={!selectedArchitecture}
                  class="px-4 py-2 min-h-11 bg-green-600 hover:bg-green-700 disabled:bg-gray-400 disabled:cursor-not-allowed text-white text-sm font-medium rounded-md transition-colors"
                >
                  Download
                </button>
              </div>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                Use the terminal client to access your notes from the command line
              </p>
            </div>

            <!-- Delete All Data -->
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
        {/if}

        <!-- ABOUT TAB -->
        {#if currentTab === 'about'}
          <div class="space-y-6">
            <!-- Version -->
            <div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Version</h4>
              <p class="text-lg text-gray-900 dark:text-white font-mono">v{__APP_VERSION__}</p>
            </div>

            <!-- Documentation -->
            <div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Documentation</h4>
              <button
                on:click={() => showDocumentation = true}
                class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors flex items-center justify-center gap-2"
              >
                📚 View Documentation
              </button>
              <p class="mt-2 text-sm text-gray-500 dark:text-gray-400">
                Learn how to use Jottery effectively
              </p>
            </div>

            <!-- Terminal Client Info -->
            <div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Terminal Client</h4>
              <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
                Access your notes from the command line with the Jottery terminal client. The TUI provides a fast, keyboard-driven interface for managing your notes.
              </p>
              <p class="text-sm text-gray-600 dark:text-gray-400">
                Download the terminal client from the <strong>Advanced</strong> tab.
              </p>
            </div>

            <!-- About -->
            <div>
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">About Jottery</h4>
              <p class="text-sm text-gray-600 dark:text-gray-400 mb-2">
                Jottery is a privacy-focused, self-hosted scratch pad application for capturing, organizing, and searching notes with rich content, syntax highlighting, and encryption.
              </p>
              <p class="text-sm text-gray-600 dark:text-gray-400">
                Licensed under the MIT License.
              </p>
            </div>
          </div>
        {/if}
      </div>
      </div>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end items-center gap-3 flex-shrink-0">
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
          <label for="remember-password-confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Enter your password to confirm:
          </label>
          <input
            id="remember-password-confirm"
            type="password"
            bind:value={rememberPasswordConfirmInput}
            on:keydown={(e) => e.key === 'Enter' && confirmEnableRememberPassword()}
            placeholder="Your password"
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-orange-500"
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
    <div
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      on:click={() => showCredentialsModal = false}
      on:keydown={(e) => e.key === 'Enter' && (showCredentialsModal = false)}
      role="button"
      tabindex="-1"
      aria-label="Close credentials modal"
    >
      <div
        class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] overflow-hidden"
        on:click|stopPropagation
        on:keydown|stopPropagation
        role="dialog"
        aria-modal="true"
        tabindex="0"
      >
        <!-- Header -->
        <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            📋 Sync Credentials
          </h3>
          <button
            on:click={() => showCredentialsModal = false}
            class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            aria-label="Close credentials modal"
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

<!-- Documentation Modal -->
<DocumentationModal
  show={showDocumentation}
  onClose={() => showDocumentation = false}
/>
