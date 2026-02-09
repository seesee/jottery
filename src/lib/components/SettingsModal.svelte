<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { settings, isLocked, notes } from '../stores/appStore';
  import { settingsRepository, deleteDB, noteService, searchService, syncService, syncRepository, keyManager, cryptoService, encryptionRepository, lock, passwordStorageService, sessionStorageService, noteRepository, createSyncRecoveryNote } from '../services';
  import { exportAllNotes, downloadExport, parseImportFile, importNotes } from '../services/exportService';
  import { createBatchedBackup, downloadBackup } from '../services/backupService';
  import { backupSchedulerService } from '../services/backupSchedulerService';
  import { authService } from '../services/authService';
  import { exportCredentials, parseAndStoreImportedCredentials, copyToClipboard } from '../utils/syncCredentials';
  import { _ } from 'svelte-i18n';
  import { modal, createBackdropHandler } from '../actions';
  import type { Theme, SyncStatus, KeyboardShortcut, KeyboardShortcuts, QuickCommandConfig } from '../types';
  import { DEFAULT_KEYBOARD_SHORTCUTS, DEFAULT_QUICK_COMMANDS } from '../types';
  import ConfirmModal from './ConfirmModal.svelte';
  import DocumentationModal from './DocumentationModal.svelte';
  import ProgressModal from './ProgressModal.svelte';
  import PasswordInput from './PasswordInput.svelte';
  import TabContainer from './TabContainer.svelte';
  import { GeneralTab, EditorTab, KeyboardTab, SyncTab, AdvancedTab, AboutTab, ColorsTab } from './settings';
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
  let defaultSyntaxLanguage: string = $settings.defaultSyntaxLanguage || 'markdown';
  let openLinksInNewTab: boolean = $settings.openLinksInNewTab ?? true;
  let vimMode: boolean = $settings.vimMode || false;
  let quickCommandsEnabled: boolean = $settings.quickCommandsEnabled ?? true;
  let quickCommandsList: QuickCommandConfig[] = $settings.quickCommandsList ?? DEFAULT_QUICK_COMMANDS;
  let persistSession: boolean = $settings.persistSession ?? false;
  let persistSessionTimeout: number = $settings.persistSessionTimeout ?? 30;
  let colorPalette = $settings.colorPalette || {};
  let tagColors = $settings.tagColors || {};
  let saving = false;
  let fileInput: HTMLInputElement;
  let showDeleteConfirm = false;
  let showRememberPasswordWarning = false;
  let rememberPasswordConfirmInput = '';
  let rememberPasswordError = '';
  let showPersistSessionConfirm = false;
  let persistSessionConfirmInput = '';
  let persistSessionError = '';

  // Sync state
  let syncEndpoint = $settings.syncEndpoint || '';
  let syncStatus: SyncStatus | null = null;
  let syncing = false;
  let syncError = '';
  let deviceName = $settings.deviceName || 'My Device';
  let importCredentialsText = '';
  let importDeviceName = 'My Device';
  let importing = false;
  let importProgress = { current: 0, total: 0 };
  let importResult: { imported: number; skipped: number; errors: string[]; attachments: number; tags: number } | null = null;
  let showCredentialsModal = false;
  let credentialsText = '';

  // Multi-user registration state
  let registrationMode: 'select' | 'newUser' | 'existingUser' = 'select';
  let userEmail = '';
  let userPassword = '';
  let userPasswordConfirm = '';
  let registeringUser = false;
  let registeringDevice = false;
  let registrationStep: 'email' | 'pending' | 'device' | 'complete' = 'email';
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
  let showDeleteServerNotesConfirm = false;
  let showDisconnectConfirm = false;

  // Keyboard shortcut recording
  let recordingShortcut: keyof KeyboardShortcuts | null = null;
  let tempShortcuts: KeyboardShortcuts = { ...DEFAULT_KEYBOARD_SHORTCUTS, ...$settings.keyboardShortcuts };

  // Documentation and downloads
  let showDocumentation = false;
  let selectedArchitecture = detectArchitecture();
  let isCreatingBackup = false;

  // Progress modal state
  let showProgress = false;
  let progressTitle = '';
  let progressMessage = '';
  let progressCurrent = 0;
  let progressTotal = 0;

  // About tab stats
  let noteStats: { total: number; active: number; deleted: number; pinned: number; } | null = null;

  // Tab state
  type Tab = 'general' | 'editor' | 'colors' | 'keyboard' | 'sync' | 'advanced' | 'about';
  let currentTab: Tab = 'general';

  // Tabs configuration (reactive to pick up translations)
  $: settingsTabs = [
    { id: 'general', label: $_('settings.tabs.general') },
    { id: 'editor', label: $_('settings.tabs.editor') },
    { id: 'colors', label: $_('settings.tabs.colors') },
    { id: 'keyboard', label: $_('settings.tabs.keyboard') },
    { id: 'sync', label: $_('settings.tabs.sync') },
    { id: 'advanced', label: $_('settings.tabs.advanced') },
    { id: 'about', label: $_('settings.tabs.about') },
  ];

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
    { value: 'html', label: 'HTML Client (zip)', url: `${githubRepo}/releases/latest/download/jottery-web.zip` },
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
    tempShortcuts = { ...DEFAULT_KEYBOARD_SHORTCUTS, ...$settings.keyboardShortcuts };
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

      // If sync is enabled, fetch device name from server
      if (syncStatus?.isEnabled && syncStatus.syncEndpoint) {
        try {
          const metadata = await syncRepository.getMetadata();
          if (metadata?.apiKey && !metadata.apiKey.startsWith('IMPORT:') && !metadata.apiKey.startsWith('ENCRYPTED:') && !metadata.apiKey.startsWith('RESTORE:')) {
            const masterKey = keyManager.getMasterKey();
            if (masterKey) {
              const apiKeyEncrypted = JSON.parse(metadata.apiKey);
              const apiKey = await cryptoService.decryptText(apiKeyEncrypted, masterKey.key);
              const { getServerStatus } = await import('../services/syncClient');
              const serverStatus = await getServerStatus(syncStatus.syncEndpoint, apiKey);
              // Update local device name with server value
              if (serverStatus.deviceName) {
                deviceName = serverStatus.deviceName;
              }
            }
          }
        } catch (error) {
          console.warn('Failed to fetch device name from server:', error);
          // Fall back to local device name - not critical
        }
      }
    } catch (error) {
      console.error('Failed to load sync status:', error);
    }
  }

  // Multi-user registration flow
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

      // Update settings store (including device name)
      settings.update(s => ({
        ...s,
        syncEndpoint,
        syncEnabled: true,
        deviceName: deviceName.trim(),
      }));

      await settingsRepository.update({ syncEndpoint, syncEnabled: true, deviceName: deviceName.trim() });

      // Reload status
      await loadSyncStatus();

      // Trigger full sync automatically
      const syncResult = await syncService.syncNow(true);
      if (syncResult.success) {
      } else {
        console.warn('[SettingsModal] ⚠️ Initial sync failed, but device is registered. Error:', syncResult.error);
      }

      registrationStep = 'complete';
      syncError = '';

      // Create sync recovery note (non-blocking)
      createSyncRecoveryNote().catch(err =>
        console.warn('[SettingsModal] Failed to create recovery note:', err)
      );
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
    userPasswordConfirm = '';
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

      const response = await fetch(accountUrl, {
        headers: {
          'Authorization': `Bearer ${userSession.sessionId}`,
        },
      });


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
    } catch (error) {
      console.error('Failed to fetch account info:', error);
      syncError = error instanceof Error ? error.message : 'Failed to load account info';
    } finally {
      loadingAccountInfo = false;
    }
  }

  async function handleDeleteAllNotes() {
    if (!userSession || !syncEndpoint) return;

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


      // Refresh account info
      await fetchAccountInfo();

      showDeleteServerNotesConfirm = false;
    } catch (error) {
      console.error('Failed to delete notes:', error);
      syncError = error instanceof Error ? error.message : 'Failed to delete notes';
    }
  }

  async function handleDisconnect() {
    try {
      await syncService.disconnect();
      // Reset UI state
      syncStatus = null;
      syncEndpoint = '';
      userSession = null;
      accountInfo = null;
      registrationMode = 'select';
      registrationStep = 'email';
      showDisconnectConfirm = false;
      // Refresh sync status
      syncStatus = await syncService.getSyncStatus();
    } catch (error) {
      console.error('Failed to disconnect:', error);
      syncError = error instanceof Error ? error.message : 'Failed to disconnect';
    }
  }

  function handleAccountLogout() {
    userSession = null;
    accountInfo = null;
    accountPassword = '';
    showAccountManagement = false;
    syncError = '';
  }

  async function handleCopySyncCredentials(useLegacyFormat: boolean = false) {
    const result = await exportCredentials(useLegacyFormat);

    if (result.success && result.credentials) {
      // Try to copy to clipboard (best effort)
      await copyToClipboard(result.credentials);

      // Always show the modal with text for manual copy
      credentialsText = result.credentials;
      showCredentialsModal = true;
    } else {
      syncError = result.error || 'Failed to generate credentials';
    }
  }

  async function handleImportCredentials() {
    if (!importCredentialsText.trim()) {
      syncError = 'Please paste the credentials';
      return;
    }

    if (!importDeviceName.trim()) {
      syncError = 'Please enter a device name';
      return;
    }

    importing = true;
    syncError = '';

    try {
      const result = await parseAndStoreImportedCredentials(importCredentialsText, importDeviceName.trim());

      if (!result.success) {
        syncError = result.error || 'Failed to import credentials';
        return;
      }

      // Update settings to disable sync (will be enabled after unlock)
      await settingsRepository.update({
        syncEnabled: false,
      });

      // Close the modal first
      onClose();

      // Lock and update UI state
      lock();
      isLocked.set(true);

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
      // Normal sync - only push notes that need syncing
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

  async function handleFullSync() {
    syncing = true;
    syncError = '';
    try {
      // Full sync - push ALL notes regardless of sync status
      const result = await syncService.syncNow(true);
      if (result.success) {
        await loadSyncStatus();
      } else {
        syncError = result.error || 'Sync failed';
      }
    } catch (error) {
      console.error('Full sync failed:', error);
      syncError = error instanceof Error ? error.message : 'Full sync failed';
    } finally {
      syncing = false;
    }
  }

  async function handleToggleSyncFeature(enabled: boolean) {
    try {

      // Update settings
      await settingsRepository.update({
        syncEnabled: enabled,
      });

      // Update sync metadata to keep in sync with settings
      await syncRepository.updateMetadata({
        syncEnabled: enabled,
      });

      // Update local state
      settings.update(s => ({
        ...s,
        syncEnabled: enabled,
      }));

      // If disabling, stop auto-sync and disconnect SSE
      if (!enabled) {
        syncService.disableAutoSync();
        syncService.disconnectFromSyncEvents();
      }

    } catch (error) {
      console.error('Failed to toggle sync feature:', error);
      syncError = error instanceof Error ? error.message : 'Failed to toggle sync';
    }
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
        defaultSyntaxLanguage,
        openLinksInNewTab,
        vimMode,
        quickCommandsEnabled,
        quickCommandsList,
        persistSession,
        persistSessionTimeout,
        colorPalette,
        tagColors,
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
        defaultSyntaxLanguage,
        openLinksInNewTab,
        vimMode,
        quickCommandsEnabled,
        quickCommandsList,
        persistSession,
        persistSessionTimeout,
        colorPalette,
        tagColors,
      }));

      // Wait for Svelte to process reactive updates (e.g., locale change) before showing toast
      await tick();
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

  function handlePersistSessionToggle() {
    if (persistSession) {
      // Just enabled (value is now true) - show confirmation to get password
      showPersistSessionConfirm = true;
    } else {
      // Just disabled (value is now false) - clear session storage
      sessionStorageService.clear();
    }
  }

  async function confirmEnablePersistSession() {
    if (!persistSessionConfirmInput) {
      persistSessionError = $_('settings.rememberPasswordModal.passwordPlaceholder');
      return;
    }

    persistSessionError = '';

    try {
      // Verify the password by attempting to derive key and decrypt
      const metadata = await encryptionRepository.getMetadata();
      if (!metadata) {
        throw new Error('Encryption not initialized');
      }

      const salt = base64ToUint8Array(metadata.salt);
      const derivedKey = await cryptoService.deriveKey({
        password: persistSessionConfirmInput,
        salt,
        iterations: metadata.iterations,
        algorithm: 'PBKDF2',
      });

      // Try to decrypt a note to verify password is correct
      const allNotes = await noteRepository.getAllActive();
      if (allNotes.length > 0) {
        const testNote = allNotes[0];
        const encryptedContent = JSON.parse(testNote.content);
        await cryptoService.decryptText(encryptedContent, derivedKey);
      }

      // Password is correct! Store it in session storage
      sessionStorageService.store(persistSessionConfirmInput);

      // Save the setting immediately to persist it
      await settingsRepository.update({ persistSession: true, persistSessionTimeout });
      settings.update(s => ({ ...s, persistSession: true, persistSessionTimeout }));

      // Close modal and clear input
      showPersistSessionConfirm = false;
      persistSessionConfirmInput = '';
      persistSession = true;
    } catch (error) {
      console.error('Password verification failed:', error);
      persistSessionError = $_('unlock.incorrectPassword');
      persistSessionConfirmInput = '';
      persistSession = false;
    }
  }

  function cancelEnablePersistSession() {
    showPersistSessionConfirm = false;
    persistSession = false;
    persistSessionConfirmInput = '';
    persistSessionError = '';
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
      await passwordStorageService.store(rememberPasswordConfirmInput);

      // Save the setting immediately to persist it
      await settingsRepository.update({ rememberPassword: true });
      settings.update(s => ({ ...s, rememberPassword: true }));

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
    await passwordStorageService.clear();

    // Save the setting immediately to persist it
    try {
      await settingsRepository.update({ rememberPassword: false });
      settings.update(s => ({ ...s, rememberPassword: false }));
    } catch (error) {
      console.error('Failed to save disabled setting:', error);
    }

    // Lock the application
    lock();
    isLocked.set(true);
  }

  $: backdropHandler = createBackdropHandler(onClose);

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
      showProgress = true;
      progressTitle = $_('progress.export');
      progressMessage = '';
      progressCurrent = 0;
      progressTotal = 0;

      const data = await exportAllNotes((progress) => {
        if (progress.total) progressTotal = progress.total;
        if (progress.current) progressCurrent = progress.current;
      });
      await downloadExport(data);
    } catch (error) {
      console.error('Failed to export notes:', error);
      toast.error($_('settings.exportFailed', { values: { error: error instanceof Error ? error.message : String(error) } }));
    } finally {
      showProgress = false;
    }
  }

  async function handleCreateBackup() {
    isCreatingBackup = true;
    showProgress = true;
    progressTitle = $_('progress.backup');
    progressMessage = '';
    progressCurrent = 0;
    progressTotal = 0;

    try {
      const backup = await createBatchedBackup((progress) => {
        if (progress.total) progressTotal = progress.total;
        if (progress.current) progressCurrent = progress.current;
        // Show phase-specific message
        if (progress.phase === 'loading' && progress.item === 'attachments') {
          progressMessage = $_('backup.loadingAttachments');
        } else if (progress.phase === 'encrypting') {
          progressMessage = $_('backup.encrypting');
        }
      });
      downloadBackup(backup);
      // Record the backup to reset counters and hide reminder
      await backupSchedulerService.recordBackup();
      toast.success($_('backup.created'));
    } catch (error) {
      console.error('Failed to create backup:', error);
      toast.error($_('backup.failed', { values: { error: error instanceof Error ? error.message : String(error) } }));
    } finally {
      isCreatingBackup = false;
      showProgress = false;
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

      const result = await importNotes(data, 'skip', (current, total) => {
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
      // Check if any child modal is open by checking our state flags
      const hasOpenChildModal = showDeleteConfirm ||
        showRememberPasswordWarning ||
        showPersistSessionConfirm ||
        showCredentialsModal ||
        showDeleteServerNotesConfirm ||
        showDisconnectConfirm ||
        showDocumentation ||
        importing;

      // Only close settings modal if no child modal is open
      if (!hasOpenChildModal) {
        event.preventDefault();
        onClose();
      }
    }
  }

  onMount(async () => {
    // Check if current server offers sync and use as default
    if (!syncEndpoint && typeof window !== 'undefined') {
      try {
        const defaultEndpoint = window.location.origin;
        const response = await fetch(`${defaultEndpoint}/api/v1/sync/status`, {
          method: 'HEAD',
        });
        // If we get any response other than 404, assume sync is available
        // (401 Unauthorized means sync endpoint exists but needs auth)
        if (response.status !== 404) {
          syncEndpoint = defaultEndpoint;
        } else {
        }
      } catch (error) {
        // Network error - user can manually enter endpoint
      }
    } else if (syncEndpoint) {
    }
  });

</script>

{#if show}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-0 tablet:p-4"
    on:click={backdropHandler}
    on:keydown={handleKeyDown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: onClose }}
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

      <!-- Tab Navigation and Content -->
      <TabContainer tabs={settingsTabs} bind:currentTab ariaLabel={$_('settings.title')}>
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
            bind:openLinksInNewTab
            bind:persistSession
            bind:persistSessionTimeout
            onRememberPasswordToggle={handleRememberPasswordToggle}
            onPersistSessionToggle={handlePersistSessionToggle}
          />
        {/if}

        <!-- EDITOR TAB -->
        {#if currentTab === 'editor'}
          <EditorTab bind:enabledSyntaxLanguages bind:defaultSyntaxLanguage bind:vimMode bind:quickCommandsEnabled bind:quickCommandsList />
        {/if}

        <!-- COLORS TAB -->
        {#if currentTab === 'colors'}
          <ColorsTab bind:colorPalette bind:tagColors />
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
            bind:userPasswordConfirm
            bind:registeringUser
            bind:registeringDevice
            bind:userRegistrationMessage
            bind:importCredentialsText
            bind:importDeviceName
            bind:importing
            bind:showAccountManagement
            bind:accountEmail
            bind:accountPassword
            bind:loggingIn
            bind:userSession
            bind:accountInfo
            bind:loadingAccountInfo
            onRegisterUser={handleRegisterUser}
            onRegisterDevice={handleRegisterDevice}
            onResetRegistrationFlow={resetRegistrationFlow}
            onImportCredentials={handleImportCredentials}
            onSyncNow={handleSyncNow}
            onFullSync={handleFullSync}
            onCopySyncCredentials={handleCopySyncCredentials}
            onAccountLogin={handleAccountLogin}
            onAccountLogout={handleAccountLogout}
            onShowDeleteServerNotesConfirm={() => showDeleteServerNotesConfirm = true}
            onShowDisconnectConfirm={() => showDisconnectConfirm = true}
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
            onCreateBackup={handleCreateBackup}
            {isCreatingBackup}
          />
        {/if}

        <!-- ABOUT TAB -->
        {#if currentTab === 'about'}
          <AboutTab
            onShowDocumentation={() => showDocumentation = true}
            onClose={onClose}
            stats={noteStats}
            notes={$notes}
          />
        {/if}
        </div>
      </TabContainer>

      <!-- Footer -->
      <div class="border-t border-gray-200 dark:border-gray-700 p-4 flex justify-end items-center gap-3 flex-shrink-0">
        <button
          on:click={onClose}
          class="px-4 py-2.5 min-h-11 text-gray-700 dark:text-gray-300 active:bg-gray-100 dark:active:bg-gray-700 rounded-md transition-colors"
          data-testid="btn-settings-cancel"
        >
          {$_('common.cancel')}
        </button>
        <button
          on:click={handleSave}
          disabled={saving}
          class="px-4 py-2.5 min-h-11 bg-blue-600 active:bg-blue-700 disabled:bg-blue-400 text-white font-medium rounded-md transition-colors"
          data-testid="btn-settings-save"
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
    title={$_('confirm.deleteAllData.title')}
    message={$_('confirm.deleteAllData.message')}
    confirmText={$_('confirm.deleteAllData.confirmButton')}
    cancelText={$_('confirm.deleteAllData.cancelButton')}
    confirmClass="bg-red-600 hover:bg-red-700"
    requireTextMatch={$_('confirm.deleteKeyword')}
    onConfirm={confirmDeleteDatabase}
    onCancel={() => showDeleteConfirm = false}
  />

  <!-- Delete Server Notes Confirmation Modal -->
  <ConfirmModal
    show={showDeleteServerNotesConfirm}
    title={$_('confirm.deleteServerNotes.title')}
    message={$_('confirm.deleteServerNotes.message')}
    confirmText={$_('confirm.deleteServerNotes.confirmButton')}
    cancelText={$_('confirm.deleteServerNotes.cancelButton')}
    confirmClass="bg-red-600 hover:bg-red-700"
    requireTextMatch={$_('confirm.deleteKeyword')}
    onConfirm={handleDeleteAllNotes}
    onCancel={() => showDeleteServerNotesConfirm = false}
  />

  <!-- Disconnect Sync Server Confirmation Modal -->
  <ConfirmModal
    show={showDisconnectConfirm}
    title={$_('confirm.disconnectSync.title')}
    message={$_('confirm.disconnectSync.message')}
    confirmText={$_('confirm.disconnectSync.confirmButton')}
    cancelText={$_('confirm.disconnectSync.cancelButton')}
    confirmClass="bg-red-600 hover:bg-red-700"
    onConfirm={handleDisconnect}
    onCancel={() => showDisconnectConfirm = false}
  />

  <!-- Remember Password Warning Modal -->
  {#if showRememberPasswordWarning}
    <div
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      use:modal={{ onEscape: cancelEnableRememberPassword }}
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6">
        <h3 class="text-xl font-bold text-orange-600 dark:text-orange-400 mb-4">
          {$_('settings.rememberPasswordModal.title')}
        </h3>

        <div class="mb-4 text-sm text-gray-700 dark:text-gray-300 space-y-2">
          <p class="font-semibold">
            {$_('settings.rememberPasswordModal.insecureWarning')}
          </p>
          <ul class="list-disc list-inside space-y-1 text-gray-600 dark:text-gray-400">
            <li>{$_('settings.rememberPasswordModal.riskDevice')}</li>
            <li>{$_('settings.rememberPasswordModal.riskExtensions')}</li>
            <li>{$_('settings.rememberPasswordModal.riskCache')}</li>
            <li>{$_('settings.rememberPasswordModal.riskAutolock')}</li>
          </ul>
          <p class="font-semibold text-orange-700 dark:text-orange-300">
            {$_('settings.rememberPasswordModal.confirmWarning')}
          </p>
        </div>

        <div class="mb-4">
          <label for="remember-password-confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('settings.rememberPasswordModal.confirmPassword')}
          </label>
          <PasswordInput
            id="remember-password-confirm"
            bind:value={rememberPasswordConfirmInput}
            on:keydown={(e) => e.detail.key === 'Enter' && confirmEnableRememberPassword()}
            placeholder="{$_('settings.rememberPasswordModal.passwordPlaceholder')}"
            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-orange-500"
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
            {$_('settings.rememberPasswordModal.cancelButton')}
          </button>
          <button
            on:click={confirmEnableRememberPassword}
            class="px-4 py-2 bg-orange-600 hover:bg-orange-700 text-white font-medium rounded-md transition-colors"
          >
            {$_('settings.rememberPasswordModal.enableButton')}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Persist Session Confirmation Modal -->
  {#if showPersistSessionConfirm}
    <div
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      use:modal={{ onEscape: cancelEnablePersistSession }}
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6">
        <h3 class="text-xl font-bold text-blue-600 dark:text-blue-400 mb-4">
          🔄 {$_('settings.persistSession')}
        </h3>

        <div class="mb-4 text-sm text-gray-700 dark:text-gray-300 space-y-2">
          <p>
            {$_('settings.persistSessionDescription')}
          </p>
          <p class="text-blue-700 dark:text-blue-300">
            {$_('settings.persistSessionNote')}
          </p>
        </div>

        <div class="mb-4">
          <label for="persist-session-confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('settings.persistSessionModal.confirmPassword')}
          </label>
          <PasswordInput
            id="persist-session-confirm"
            bind:value={persistSessionConfirmInput}
            on:keydown={(e) => e.detail.key === 'Enter' && confirmEnablePersistSession()}
            placeholder="{$_('settings.rememberPasswordModal.passwordPlaceholder')}"
            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          {#if persistSessionError}
            <p class="mt-2 text-sm text-red-600 dark:text-red-400">{persistSessionError}</p>
          {/if}
        </div>

        <div class="flex gap-3 justify-end">
          <button
            on:click={cancelEnablePersistSession}
            class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
          >
            {$_('common.cancel')}
          </button>
          <button
            on:click={confirmEnablePersistSession}
            class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-md transition-colors"
          >
            {$_('settings.persistSessionModal.enableButton')}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Sync Credentials Display Modal -->
  {#if showCredentialsModal}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
    <div
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      on:click={createBackdropHandler(() => showCredentialsModal = false)}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      use:modal={{ onEscape: () => showCredentialsModal = false }}
    >
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
      <div
        class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-2xl w-full max-h-[80vh] overflow-hidden"
        on:click|stopPropagation
        role="document"
      >
        <!-- Header -->
        <div class="px-6 py-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            {$_('settings.syncCredentials.title')}
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
            {$_('settings.syncCredentials.instructions')}
          </p>

          <div class="bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded p-4">
            <pre class="text-xs font-mono text-gray-900 dark:text-gray-100 break-all whitespace-pre-wrap">{credentialsText}</pre>
          </div>

          <div class="mt-4 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded p-3">
            <p class="text-sm text-orange-800 dark:text-orange-200">
              {@html $_('settings.syncCredentials.samePasswordWarning')}
            </p>
          </div>

          <p class="mt-3 text-xs text-gray-600 dark:text-gray-400">
            {$_('settings.syncCredentials.clipboardNote')}
          </p>
        </div>

        <!-- Footer -->
        <div class="px-6 py-4 border-t border-gray-200 dark:border-gray-700 flex justify-end">
          <button
            on:click={() => showCredentialsModal = false}
            class="px-4 py-2 bg-gray-600 hover:bg-gray-700 text-white text-sm font-medium rounded-md transition-colors"
          >
            {$_('settings.syncCredentials.closeButton')}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Progress Modal for backup/export operations -->
  <ProgressModal
    show={showProgress}
    title={progressTitle}
    message={progressMessage}
    current={progressCurrent}
    total={progressTotal}
  />
{/if}

<!-- Import Progress Modal -->
{#if importing}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:modal={{ onEscape: importResult ? closeImportModal : undefined }}
  >
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
            data-testid="btn-import-done"
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
