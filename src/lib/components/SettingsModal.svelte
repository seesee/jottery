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
  import { GeneralTab, EditorTab, KeyboardTab, SyncTab, AdvancedTab, AboutTab } from './settings';
  import { toast } from '../utils/toast.svelte';

  export let show = false;
  export let onClose: () => void = () => {};
  export let onOpenShortcutsHelp: () => void = () => {};

  let theme: Theme = $settings.theme;
  let layoutMode: 'auto' | 'mobile' | 'desktop' = $settings.layoutMode || 'auto';
  let fontSize: 'auto' | 'small' | 'medium' | 'large' = $settings.fontSize || 'auto';
  let autoLockTimeout = $settings.autoLockTimeout;
  let sortOrder = $settings.sortOrder;
  let language = $settings.language;
  let timezone = $settings.timezone || 'local';
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
  let importCredentialsText = '';
  let importing = false;
  let importProgress = { current: 0, total: 0 };
  let importResult: { imported: number; skipped: number; errors: string[]; attachments: number; tags: number } | null = null;
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

  // User account management state
  let showAccountManagement = false;
  let accountEmail = '';
  let accountPassword = '';
  let loggingIn = false;
  let userSession: { sessionId: string; email: string; isAdmin: boolean } | null = null;
  let accountInfo: {
    email: string;
    noteCount: number;
    attachmentCount: number;
    storageUsedBytes: number;
    storageQuotaMb: number;
    createdAt: string;
    lastSyncAt: string | null;
  } | null = null;
  let loadingAccountInfo = false;
  let deletingNotes = false;
  let showDeleteServerNotesConfirm = false;

  // Keyboard shortcut recording
  let recordingShortcut: keyof KeyboardShortcuts | null = null;
  let tempShortcuts: KeyboardShortcuts = { ...$settings.keyboardShortcuts } || { ...DEFAULT_KEYBOARD_SHORTCUTS };

  // Documentation and downloads
  let showDocumentation = false;
  let selectedArchitecture = detectArchitecture();

  // About tab stats
  let noteStats: { total: number; active: number; deleted: number; pinned: number; } | null = null;

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

  // Account management functions
  async function handleAccountLogin() {
    if (!syncEndpoint) {
      syncError = 'Please enter a sync endpoint URL first';
      return;
    }

    if (!accountEmail.trim() || !accountPassword.trim()) {
      syncError = 'Please enter email and password';
      return;
    }

    loggingIn = true;
    syncError = '';
    try {
      const loginUrl = `${syncEndpoint}/api/v1/user/login`;
      console.log('[SettingsModal] Attempting login to:', loginUrl);
      const response = await fetch(loginUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          email: accountEmail,
          password: accountPassword,
        }),
      });
      console.log('[SettingsModal] Login response status:', response.status);

      if (!response.ok) {
        let errorMessage = 'Login failed';
        try {
          const error = await response.json();
          errorMessage = error.error || errorMessage;
        } catch (e) {
          // Response body is not JSON, use status text
          errorMessage = response.statusText || `Login failed (${response.status})`;
        }
        throw new Error(errorMessage);
      }

      const data = await response.json();
      userSession = {
        sessionId: data.sessionId,
        email: data.user.email,
        isAdmin: data.user.isAdmin,
      };

      console.log('[SettingsModal] User logged in successfully:', userSession.email);

      // Fetch account info
      await fetchAccountInfo();
    } catch (error) {
      console.error('Account login failed:', error);
      syncError = error instanceof Error ? error.message : 'Login failed';
    } finally {
      loggingIn = false;
    }
  }

  async function fetchAccountInfo() {
    if (!userSession || !syncEndpoint) return;

    loadingAccountInfo = true;
    syncError = '';
    try {
      const accountUrl = `${syncEndpoint}/api/v1/user/account`;
      console.log('[SettingsModal] Fetching account info from:', accountUrl);
      console.log('[SettingsModal] Using session token:', userSession.sessionId);

      const response = await fetch(accountUrl, {
        headers: {
          'Authorization': `Bearer ${userSession.sessionId}`,
        },
      });

      console.log('[SettingsModal] Account info response status:', response.status);

      if (!response.ok) {
        let errorMessage = `Failed to fetch account info (${response.status})`;
        try {
          const error = await response.json();
          errorMessage = error.error || errorMessage;
        } catch (e) {
          errorMessage = response.statusText || errorMessage;
        }
        throw new Error(errorMessage);
      }

      accountInfo = await response.json();
      console.log('[SettingsModal] Account info loaded:', accountInfo);
    } catch (error) {
      console.error('Failed to fetch account info:', error);
      syncError = error instanceof Error ? error.message : 'Failed to load account info';
    } finally {
      loadingAccountInfo = false;
    }
  }

  async function handleDeleteAllNotes() {
    if (!userSession || !syncEndpoint) return;

    deletingNotes = true;
    syncError = '';
    try {
      const response = await fetch(`${syncEndpoint}/api/v1/user/notes`, {
        method: 'DELETE',
        headers: {
          'Authorization': `Bearer ${userSession.sessionId}`,
        },
      });

      if (!response.ok) {
        throw new Error('Failed to delete notes');
      }

      console.log('[SettingsModal] All notes deleted from server');

      // Refresh account info
      await fetchAccountInfo();

      showDeleteServerNotesConfirm = false;
    } catch (error) {
      console.error('Failed to delete notes:', error);
      syncError = error instanceof Error ? error.message : 'Failed to delete notes';
    } finally {
      deletingNotes = false;
    }
  }

  function handleAccountLogout() {
    userSession = null;
    accountInfo = null;
    accountPassword = '';
    showAccountManagement = false;
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

  async function handleToggleSyncFeature(enabled: boolean) {
    try {
      console.log(`[SettingsModal] Toggling sync feature: ${enabled}`);

      // Update settings
      await settingsRepository.update({
        syncEnabled: enabled,
      });

      // Update local state
      settings.update(s => ({
        ...s,
        syncEnabled: enabled,
      }));

      // If disabling and sync is active, stop auto-sync
      if (!enabled && syncStatus?.isEnabled) {
        syncService.disableAutoSync();
      }

      console.log(`[SettingsModal] Sync feature ${enabled ? 'enabled' : 'disabled'}`);
    } catch (error) {
      console.error('Failed to toggle sync feature:', error);
      syncError = error instanceof Error ? error.message : 'Failed to toggle sync';
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
        timezone,
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
        timezone,
        rememberPassword,
        keyboardShortcuts: tempShortcuts,
        enabledSyntaxLanguages,
      }));

      toast.success($_('settings.settingsSaved'));
      onClose();
    } catch (error) {
      console.error('Failed to save settings:', error);
      toast.error($_('settings.settingsSaveFailed', { values: { error: error instanceof Error ? error.message : String(error) } }));
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
      toast.error($_('settings.deleteDatabaseFailed', { values: { error: error instanceof Error ? error.message : String(error) } }));
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

  async function loadNoteStats() {
    try {
      noteStats = await noteService.getStats();
    } catch (error) {
      console.error('Failed to load note stats:', error);
      noteStats = null;
    }
  }

  // Load stats when switching to About tab
  $: if (currentTab === 'about' && show && !noteStats) {
    loadNoteStats();
  }

  async function handleExport() {
    try {
      const data = await exportAllNotes();
      await downloadExport(data);
    } catch (error) {
      console.error('Failed to export notes:', error);
      toast.error($_('settings.exportFailed', { values: { error: error instanceof Error ? error.message : String(error) } }));
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
      importing = true;
      importProgress = { current: 0, total: 0 };
      importResult = null;

      const data = await parseImportFile(file);
      importProgress.total = data.notes.length;

      const result = await importNotes(data, 'merge', (current, total) => {
        importProgress = { current, total };
      });

      // Reload notes
      const allNotes = await noteService.getAllNotes($settings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);

      // Store result for display in modal
      importResult = {
        imported: result.imported,
        skipped: result.skipped,
        errors: result.errors,
        attachments: result.attachments,
        tags: result.tags.size,
      };
    } catch (error) {
      console.error('Failed to import notes:', error);
      toast.error($_('settings.importFailed', { values: { error: error instanceof Error ? error.message : String(error) } }));
      importing = false;
      importProgress = { current: 0, total: 0 };
    } finally {
      // Clear file input
      target.value = '';
    }
  }

  function closeImportModal() {
    importing = false;
    importProgress = { current: 0, total: 0 };
    importResult = null;
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
        <h2 class="text-xl font-bold text-gray-900 dark:text-white">{$_('settings.title')}</h2>
        <button
          on:click={onClose}
          class="min-h-11 min-w-11 p-3 -m-2 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors text-gray-500 dark:text-gray-400"
          aria-label={$_('settings.closeLabel')}
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
          {$_('settings.tabs.general')}
        </button>
        <button
          on:click={() => currentTab = 'editor'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'editor' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          {$_('settings.tabs.editor')}
        </button>
        <button
          on:click={() => currentTab = 'keyboard'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'keyboard' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          {$_('settings.tabs.keyboard')}
        </button>
        <button
          on:click={() => currentTab = 'sync'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'sync' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          {$_('settings.tabs.sync')}
        </button>
        <button
          on:click={() => currentTab = 'advanced'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'advanced' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          {$_('settings.tabs.advanced')}
        </button>
        <button
          on:click={() => currentTab = 'about'}
          class="px-4 py-3 text-sm font-medium whitespace-nowrap transition-colors {currentTab === 'about' ? 'text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'}"
        >
          {$_('settings.tabs.about')}
        </button>
      </div>

      <!-- Content (scrollable) -->
      <div class="flex-1 overflow-y-auto">

      <!-- Content -->
      <div class="p-6 space-y-6">
        <!-- GENERAL TAB -->
        {#if currentTab === 'general'}
          <GeneralTab
            bind:theme
            bind:layoutMode
            bind:fontSize
            bind:autoLockTimeout
            bind:sortOrder
            bind:language
            bind:timezone
            bind:rememberPassword
            onRememberPasswordToggle={handleRememberPasswordToggle}
          />
        {/if}

        <!-- EDITOR TAB -->
        {#if currentTab === 'editor'}
          <EditorTab bind:enabledSyntaxLanguages />
        {/if}

        <!-- KEYBOARD SHORTCUTS TAB -->
        {#if currentTab === 'keyboard'}
          <KeyboardTab
            bind:tempShortcuts
            bind:recordingShortcut
            onStartRecording={startRecording}
            onResetShortcuts={resetShortcuts}
            {onOpenShortcutsHelp}
          />
        {/if}

        <!-- SYNC TAB -->
        {#if currentTab === 'sync'}
          <SyncTab
            syncFeatureEnabled={$settings.syncEnabled}
            onToggleSyncFeature={handleToggleSyncFeature}
            bind:syncStatus
            bind:syncEndpoint
            bind:deviceName
            bind:syncing
            bind:syncError
            bind:registrationMode
            bind:registrationStep
            bind:userEmail
            bind:userPassword
            bind:registeringUser
            bind:registeringDevice
            bind:registeredUserId
            bind:userRegistrationMessage
            bind:importCredentialsText
            bind:importing
            bind:showCopiedMessage
            bind:showCredentialsModal
            bind:credentialsText
            bind:showAccountManagement
            bind:accountEmail
            bind:accountPassword
            bind:loggingIn
            bind:userSession
            bind:accountInfo
            bind:loadingAccountInfo
            bind:deletingNotes
            onRegisterUser={handleRegisterUser}
            onRegisterDevice={handleRegisterDevice}
            onResetRegistrationFlow={resetRegistrationFlow}
            onImportCredentials={handleImportCredentials}
            onSyncNow={handleSyncNow}
            onDisconnectSync={handleDisconnectSync}
            onCopySyncCredentials={handleCopySyncCredentials}
            onAccountLogin={handleAccountLogin}
            onAccountLogout={handleAccountLogout}
            onShowDeleteServerNotesConfirm={() => showDeleteServerNotesConfirm = true}
          />
        {/if}

        <!-- ADVANCED TAB -->
        {#if currentTab === 'advanced'}
          <AdvancedTab
            bind:selectedArchitecture
            onExport={handleExport}
            onImport={handleImport}
            onDownload={handleDownload}
            onDeleteDatabase={handleDeleteDatabase}
          />
        {/if}

        <!-- ABOUT TAB -->
        {#if currentTab === 'about'}
          <AboutTab
            bind:showDocumentation
            onShowDocumentation={() => showDocumentation = true}
            onClose={onClose}
            stats={noteStats}
            notes={$notes}
          />
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

  <!-- Delete Server Notes Confirmation Modal -->
  <ConfirmModal
    show={showDeleteServerNotesConfirm}
    title="Delete All Notes from Server?"
    message="This will permanently delete ALL your notes from the sync server. This action cannot be undone.{'\n\n'}Your local notes will remain on this device and will NOT be deleted.{'\n\n'}Type DELETE to confirm:"
    confirmText="Delete from Server"
    cancelText="Cancel"
    confirmClass="bg-red-600 hover:bg-red-700"
    requireTextMatch="DELETE"
    onConfirm={handleDeleteAllNotes}
    onCancel={() => showDeleteServerNotesConfirm = false}
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

<!-- Import Progress Modal -->
{#if importing}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50" role="dialog" aria-modal="true">
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-6 max-w-md w-full mx-4">
      <h3 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white">
        {importResult ? $_('settings.importDialog.complete') : $_('settings.importDialog.importing')}
      </h3>

      {#if importResult}
        <!-- Results Summary -->
        <div class="space-y-4">
          <!-- Success icon -->
          <div class="flex justify-center">
            <div class="rounded-full bg-green-100 dark:bg-green-900 p-3">
              <svg class="w-8 h-8 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
              </svg>
            </div>
          </div>

          <!-- Statistics -->
          <div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-gray-600 dark:text-gray-400">{$_('settings.importDialog.notesImported')}</span>
              <span class="font-semibold text-gray-900 dark:text-white">{importResult.imported}</span>
            </div>
            {#if importResult.skipped > 0}
              <div class="flex justify-between text-sm">
                <span class="text-gray-600 dark:text-gray-400">{$_('settings.importDialog.notesSkipped')}</span>
                <span class="font-semibold text-gray-900 dark:text-white">{importResult.skipped}</span>
              </div>
            {/if}
            {#if importResult.attachments > 0}
              <div class="flex justify-between text-sm">
                <span class="text-gray-600 dark:text-gray-400">{$_('settings.importDialog.attachmentsImported')}</span>
                <span class="font-semibold text-gray-900 dark:text-white">{importResult.attachments}</span>
              </div>
            {/if}
            {#if importResult.tags > 0}
              <div class="flex justify-between text-sm">
                <span class="text-gray-600 dark:text-gray-400">{$_('settings.importDialog.uniqueTags')}</span>
                <span class="font-semibold text-gray-900 dark:text-white">{importResult.tags}</span>
              </div>
            {/if}
          </div>

          <!-- Errors -->
          {#if importResult.errors.length > 0}
            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3">
              <p class="text-sm font-medium text-red-800 dark:text-red-300 mb-2">
                {$_('settings.importDialog.errorsOccurred', { values: { count: importResult.errors.length, plural: importResult.errors.length > 1 ? 's' : '' } })}
              </p>
              <div class="max-h-32 overflow-y-auto space-y-1">
                {#each importResult.errors as error}
                  <p class="text-xs text-red-700 dark:text-red-400">{error}</p>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Close button -->
          <button
            on:click={closeImportModal}
            class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-md transition-colors"
          >
            {$_('settings.importDialog.done')}
          </button>
        </div>
      {:else}
        <!-- Progress indicator -->
        <div class="space-y-4">
          <!-- Progress bar -->
          <div class="relative pt-1">
            <div class="flex items-center justify-between mb-2">
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                {#if importProgress.total > 0}
                  {$_('settings.importDialog.processingNotes', { values: { current: importProgress.current, total: importProgress.total } })}
                {:else}
                  {$_('settings.importDialog.readingFile')}
                {/if}
              </span>
              {#if importProgress.total > 0}
                <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                  {Math.round((importProgress.current / importProgress.total) * 100)}%
                </span>
              {/if}
            </div>
            <div class="overflow-hidden h-2 text-xs flex rounded bg-gray-200 dark:bg-gray-700">
              <div
                class="shadow-none flex flex-col text-center whitespace-nowrap text-white justify-center bg-blue-600 transition-all duration-300"
                style="width: {importProgress.total > 0 ? (importProgress.current / importProgress.total) * 100 : 0}%"
              ></div>
            </div>
          </div>

          <!-- Spinner for indeterminate state -->
          {#if importProgress.total === 0}
            <div class="flex justify-center">
              <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
            </div>
          {/if}

          <p class="text-sm text-gray-600 dark:text-gray-400 text-center">
            {$_('settings.importDialog.pleaseWait')}
          </p>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- Documentation Modal -->
<DocumentationModal
  show={showDocumentation}
  onClose={() => showDocumentation = false}
/>
