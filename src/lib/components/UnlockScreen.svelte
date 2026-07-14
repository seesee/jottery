<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { isInitialized as isInitializedStore, isLocked, settings } from '../stores/appStore';
  import { initialize, unlock, isInitialized, deleteDB, passwordStorageService, sessionStorageService, settingsRepository, restoreFromBackup, keyManager, cryptoService, syncRepository, syncService, onboardFromServer } from '../services';
  import { authService } from '../services/authService';
  import { validateBackup, getBackupStats } from '../services/backupService';
  import type { BackupData } from '../services/backupService';
  import { getCurrentNotebook } from '../utils/notebookPath';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from './ConfirmModal.svelte';
  import LandingPage from './LandingPage.svelte';
  import LanguagePicker from './LanguagePicker.svelte';
  import PasswordInput from './PasswordInput.svelte';
  import type { UserSettings } from '../types';

  let password = '';
  let confirmPassword = '';
  let error = '';
  let loading = false;
  let needsInit = false;
  let failedAttempts = 0;
  let showDeleteOption = false;
  let showDeleteConfirm = false;
  let passwordInput: { focus: () => void } | undefined;

  // Landing page state for first-time users
  let showLandingPage = false;

  // Sync configuration state
  let showSyncConfig = false;
  let syncEndpoint = '';
  let deviceName = '';
  let syncSetupMode: 'select' | 'newUser' | null = null;

  // Connect Existing Account (full-page view) state
  let showConnectExistingAccount = false;
  let connectEndpoint = typeof window !== 'undefined' ? window.location.origin : '';
  let connectDeviceName = '';
  let connectEmail = '';
  let connectServerPassword = '';
  let connectNotesPassword = '';
  let connectError = '';
  let connectLoading = false;
  let connectStep = '';
  let defaultDeviceName = '';

  // Random device name generator
  const FUN_ADJECTIVES = [
    'Brave', 'Swift', 'Clever', 'Bright', 'Calm', 'Bold', 'Eager', 'Fair',
    'Gentle', 'Happy', 'Jolly', 'Keen', 'Lively', 'Merry', 'Noble', 'Plucky',
    'Quick', 'Rosy', 'Snug', 'Trusty', 'Vivid', 'Warm', 'Witty', 'Zesty', 'Dapper'
  ];
  const FUN_ANIMALS = [
    'Penguin', 'Otter', 'Fox', 'Owl', 'Panda', 'Rabbit', 'Badger', 'Dolphin',
    'Falcon', 'Hedgehog', 'Koala', 'Lynx', 'Meerkat', 'Narwhal', 'Octopus',
    'Parrot', 'Quail', 'Robin', 'Seal', 'Toucan', 'Wren', 'Wombat', 'Ibis', 'Newt', 'Gecko'
  ];

  function generateFunDeviceName(): string {
    const adj = FUN_ADJECTIVES[Math.floor(Math.random() * FUN_ADJECTIVES.length)];
    const animal = FUN_ANIMALS[Math.floor(Math.random() * FUN_ANIMALS.length)];
    return `${adj} ${animal}`;
  }

  function handleDeviceNameFocus() {
    if (connectDeviceName === defaultDeviceName) {
      connectDeviceName = '';
    }
  }

  function handleDeviceNameBlur() {
    if (!connectDeviceName.trim()) {
      connectDeviceName = defaultDeviceName;
    }
  }

  // Backup restore state
  let showBackupRestore = false;
  let backupFile: File | null = null;
  let backupData: BackupData | null = null;
  let backupStats: { createdAt?: string; recordCount?: number; summary?: { notes: number; attachments: number; versions: number } } | null = null;
  let backupPassword = '';
  let validatingBackup = false;
  let validationProgress: { percent: number; bytesRead: number; totalBytes: number } | null = null;
  let restoring = false;
  let restoreProgress: { phase: string; current?: number; total?: number } | null = null;
  let backupFileInput: HTMLInputElement;

  // Get current notebook info for display
  const notebook = getCurrentNotebook();

  // Check if needs initialization
  (async () => {
    needsInit = !(await isInitialized());
    isInitializedStore.set(!needsInit);

    // Show landing page for first-time users ONLY at root path
    // Non-root paths (e.g., /test, /recipes) skip the landing page
    if (needsInit && notebook.id === 'main') {
      showLandingPage = true;
    }
  })();

  // Auto-focus password input on mount and check for stored password
  onMount(async () => {
    // Try auto-unlock with stored password
    if (!needsInit) {
      try {
        const settings = await settingsRepository.get();

        // First try rememberPassword (localStorage - permanent)
        if (settings.rememberPassword) {
          const storedPassword = await passwordStorageService.get();
          if (storedPassword) {
            loading = true;
            try {
              await unlock(storedPassword);
              isLocked.set(false);
              // Don't focus input - we're unlocked
              return;
            } catch (err) {
              console.error('[UnlockScreen] Auto-unlock failed:', err);
              // Clear invalid stored password
              await passwordStorageService.clear();
              error = get(_)('unlock.storedPasswordInvalid');
            } finally {
              loading = false;
            }
          }
        }

        // Then try persistSession (sessionStorage - tab-scoped with expiry)
        if (!settings.rememberPassword && settings.persistSession && sessionStorageService.isAvailable()) {
          const sessionTimeout = settings.persistSessionTimeout ?? 30;
          const sessionPassword = sessionStorageService.get(sessionTimeout);
          if (sessionPassword) {
            loading = true;
            try {
              await unlock(sessionPassword);
              // Refresh the session timestamp on successful unlock
              sessionStorageService.refresh();
              isLocked.set(false);
              // Don't focus input - we're unlocked
              return;
            } catch (err) {
              console.error('[UnlockScreen] Session auto-unlock failed:', err);
              // Clear invalid session
              sessionStorageService.clear();
              error = get(_)('unlock.sessionExpiredOrInvalid');
            } finally {
              loading = false;
            }
          }
        }
      } catch (err) {
        console.error('[UnlockScreen] Failed to check stored password:', err);
      }
    }

    // Focus password input if we didn't auto-unlock
    if (passwordInput) {
      passwordInput.focus();
    }
  });

  async function handleSubmit() {
    error = '';
    loading = true;

    const passwordToStore = password; // Store before clearing

    try {
      if (needsInit) {
        // First time setup
        if (password.length < 8) {
          error = $_('unlock.passwordMinLength');
          return;
        }
        if (password !== confirmPassword) {
          error = $_('unlock.passwordMismatch');
          return;
        }
        await initialize(passwordToStore);
      } else {
        // Unlock existing
        await unlock(passwordToStore);
      }

      // Update store to trigger UI change
      isLocked.set(false);

      // Apply optional sync configuration (first-time setup only)
      if (needsInit && (syncEndpoint || deviceName)) {
        try {
          const updates: Partial<UserSettings> = {};

          if (syncEndpoint) {
            updates.syncEndpoint = syncEndpoint.trim();
            // Enable sync feature if endpoint was provided
            updates.syncEnabled = true;
          }

          if (deviceName) {
            updates.deviceName = deviceName.trim();
          }

          if (Object.keys(updates).length > 0) {
            await settingsRepository.update(updates);
            console.log('[UnlockScreen] Applied sync configuration during initialization');
          }
        } catch (err) {
          console.error('[UnlockScreen] Failed to apply sync configuration:', err);
          // Don't fail the whole setup if sync config fails - just log it
        }
      }

      // Store password if rememberPassword or persistSession is enabled
      try {
        const settings = await settingsRepository.get();
        if (settings.rememberPassword) {
          await passwordStorageService.store(passwordToStore);
        } else if (settings.persistSession && sessionStorageService.isAvailable()) {
          // Store session password (tab-scoped, will expire)
          sessionStorageService.store(passwordToStore);
        }
      } catch (err) {
        console.error('[UnlockScreen] Failed to store password:', err);
      }

      // Clear password fields and reset attempts
      password = '';
      confirmPassword = '';
      failedAttempts = 0;
      showDeleteOption = false;
    } catch (err) {
      error = err instanceof Error ? err.message : $_('unlock.incorrectPassword');

      // Track failed attempts (only for unlock, not init)
      if (!needsInit && error.includes('Incorrect password')) {
        failedAttempts++;
        if (failedAttempts >= 3) {
          showDeleteOption = true;
          error = $_('unlock.failedAttempts');
        }
      }

      // Clear password field on error
      password = '';
    } finally {
      loading = false;
    }
  }

  function handleDeleteRequest() {
    showDeleteConfirm = true;
  }

  async function handleDeleteConfirm() {
    showDeleteConfirm = false;
    loading = true;
    try {
      await deleteDB();
      // Reload the page to reinitialize
      window.location.reload();
    } catch (err) {
      error = 'Failed to delete database: ' + (err instanceof Error ? err.message : 'Unknown error');
      loading = false;
    }
  }

  function handleDeleteCancel() {
    showDeleteConfirm = false;
  }

  function resetSyncSetupMode() {
    syncSetupMode = null;
    error = '';
  }

  function handleShowConnectExistingAccount() {
    defaultDeviceName = generateFunDeviceName();
    connectDeviceName = defaultDeviceName;
    connectEndpoint = typeof window !== 'undefined' ? window.location.origin : '';
    connectEmail = '';
    connectServerPassword = '';
    connectNotesPassword = '';
    connectError = '';
    connectStep = '';
    showConnectExistingAccount = true;
    showLandingPage = false;
    showSyncConfig = false;
    syncSetupMode = null;
  }

  function handleCancelConnectExistingAccount() {
    showConnectExistingAccount = false;
    connectError = '';
    connectStep = '';
    connectLoading = false;
  }

  async function handleConnectExistingAccount() {
    connectError = '';
    connectStep = '';

    // Validate all fields
    if (!connectEndpoint.trim()) {
      connectError = $_('unlock.connectExisting.errors.endpointRequired');
      return;
    }
    if (!connectDeviceName.trim()) {
      connectError = $_('unlock.connectExisting.errors.deviceNameRequired');
      return;
    }
    if (!connectEmail.trim()) {
      connectError = $_('unlock.connectExisting.errors.emailRequired');
      return;
    }
    if (!connectServerPassword) {
      connectError = $_('unlock.connectExisting.errors.serverPasswordRequired');
      return;
    }
    if (!connectNotesPassword || connectNotesPassword.length < 8) {
      connectError = $_('unlock.connectExisting.errors.notesPasswordMinLength');
      return;
    }

    connectLoading = true;

    try {
      // Step 1: Initialise local encryption with notes password
      connectStep = $_('unlock.connectExisting.steps.initialising');
      await initialize(connectNotesPassword);

      // Step 2: Register device with the server
      connectStep = $_('unlock.connectExisting.steps.registering');
      const response = await authService.registerDevice(
        connectEndpoint.trim(),
        connectEmail.trim(),
        connectServerPassword,
        connectDeviceName.trim()
      );

      // Step 3: Onboard from server — download the shared master key, replace local random key
      // The server blob is wrapped with the NOTES password (all upload paths —
      // envelope migration and password change on web and iOS — derive the
      // wrapping key from it), so the notes password must be used to unwrap.
      connectStep = $_('unlock.connectExisting.steps.onboarding');
      try {
        await onboardFromServer(
          connectNotesPassword,
          connectEndpoint.trim(),
          response.userId,
          response.apiKey,
          connectNotesPassword
        );
        console.log('[UnlockScreen] ✓ Onboarded from server via envelope encryption');
      } catch (onboardError) {
        // CRITICAL: Secondary device with wrong master key is useless
        console.error('[UnlockScreen] Onboarding from server failed:', onboardError);
        throw new Error(
          $_('unlock.connectExisting.errors.onboardingFailed') +
          (onboardError instanceof Error ? ': ' + onboardError.message : '')
        );
      }

      // Step 4: Encrypt API key with the CORRECT (server's) master key
      connectStep = $_('unlock.connectExisting.steps.encrypting');
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked after onboarding');
      }
      const encryptedApiKey = await cryptoService.encryptText(response.apiKey, masterKey.key);

      // Step 5: Save sync metadata
      await syncRepository.updateMetadata({
        syncEnabled: true,
        syncEndpoint: connectEndpoint.trim(),
        apiKey: JSON.stringify(encryptedApiKey),
        clientId: response.clientId,
        userId: response.userId,
        userEmail: connectEmail.trim(),
      });

      // Save settings to IndexedDB
      await settingsRepository.update({
        syncEndpoint: connectEndpoint.trim(),
        syncEnabled: true,
        deviceName: connectDeviceName.trim(),
        welcomeNoteCreated: true,
      });

      // Update the in-memory Svelte store BEFORE unlocking so that
      // App.svelte's reactive block sees syncEnabled=true (for startAutoSync)
      // and welcomeNoteCreated=true (to suppress welcome note creation)
      settings.update(s => ({
        ...s,
        syncEndpoint: connectEndpoint.trim(),
        syncEnabled: true,
        deviceName: connectDeviceName.trim(),
        welcomeNoteCreated: true,
      }));

      // Step 6: Unlock
      isInitializedStore.set(true);
      isLocked.set(false);

      // Store password for session if applicable
      try {
        const currentSettings = await settingsRepository.get();
        if (currentSettings.rememberPassword) {
          await passwordStorageService.store(connectNotesPassword);
        } else if (currentSettings.persistSession && sessionStorageService.isAvailable()) {
          sessionStorageService.store(connectNotesPassword);
        }
      } catch (err) {
        console.error('[UnlockScreen] Failed to store password:', err);
      }

      // Step 7: Trigger initial full sync (non-blocking)
      connectStep = $_('unlock.connectExisting.steps.syncing');
      syncService.syncNow(true).then(syncResult => {
        if (syncResult.success) {
          console.log('[UnlockScreen] Initial sync completed successfully');
        } else {
          console.warn('[UnlockScreen] Initial sync failed:', syncResult.error);
        }
      }).catch(err => {
        console.warn('[UnlockScreen] Initial sync error:', err);
      });

      console.log('[UnlockScreen] ✓ Connected to existing account successfully');

      // Clear state
      showConnectExistingAccount = false;
      connectStep = '';
    } catch (err) {
      console.error('[UnlockScreen] Connect existing account failed:', err);
      connectError = err instanceof Error ? err.message : String(err);
      connectStep = '';
    } finally {
      connectLoading = false;
    }
  }

  function handleShowBackupRestore() {
    showBackupRestore = true;
    showLandingPage = false;
    error = '';
  }

  function handleCancelBackupRestore() {
    showBackupRestore = false;
    backupFile = null;
    backupData = null;
    backupStats = null;
    backupPassword = '';
    restoreProgress = null;
    error = '';
  }

  async function handleBackupFileSelect(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;

    error = '';
    backupFile = file;
    backupData = null;
    backupStats = null;
    validatingBackup = true;
    validationProgress = { percent: 0, bytesRead: 0, totalBytes: file.size };

    try {
      // Get quick stats first (fast, uses streaming)
      const stats = await getBackupStats(file);
      if (!stats.valid) {
        error = stats.error || $_('backup.restore.invalidFile');
        backupFile = null;
        validatingBackup = false;
        validationProgress = null;
        return;
      }
      backupStats = stats;

      // Validate the full backup structure (can be slow for large files)
      const validation = await validateBackup(file, (progress) => {
        validationProgress = {
          percent: progress.percent,
          bytesRead: progress.bytesRead,
          totalBytes: progress.totalBytes,
        };
      });
      if (!validation.valid || !validation.backup) {
        error = validation.error || $_('backup.restore.invalidFile');
        backupFile = null;
        backupStats = null;
        validatingBackup = false;
        validationProgress = null;
        return;
      }

      backupData = validation.backup;
    } finally {
      validatingBackup = false;
      validationProgress = null;
    }
  }

  async function handleRestoreBackup() {
    if (!backupData || !backupPassword) {
      error = $_('backup.restore.passwordHint');
      return;
    }

    restoring = true;
    error = '';
    restoreProgress = null;

    try {
      await restoreFromBackup(backupData, backupPassword, (progress) => {
        restoreProgress = progress;
      });

      // Restore successful - update UI state
      needsInit = false;
      isInitializedStore.set(true);
      isLocked.set(false);

      // Clear restore state
      handleCancelBackupRestore();
    } catch (err) {
      console.error('[UnlockScreen] Backup restore failed:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);

      if (errorMessage.includes('Incorrect password')) {
        error = $_('backup.restore.wrongPassword');
      } else {
        error = errorMessage;
      }

      backupPassword = '';
      restoreProgress = null;
    } finally {
      restoring = false;
    }
  }

  function formatBackupDate(isoDate?: string): string {
    if (!isoDate) return '';
    try {
      return new Date(isoDate).toLocaleDateString(undefined, {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      });
    } catch {
      return isoDate;
    }
  }

  // Theme toggle for unlock/setup screen
  let isDark = typeof document !== 'undefined' && document.documentElement.classList.contains('dark');

  function toggleTheme() {
    isDark = !isDark;
    document.documentElement.classList.toggle('dark', isDark);
    try {
      localStorage.setItem('jottery-theme', isDark ? 'dark' : 'light');
    } catch {}
  }

  function handleGetStarted() {
    showLandingPage = false;
    // Focus password input after a short delay to allow form to render
    setTimeout(() => {
      if (passwordInput) {
        passwordInput.focus();
      }
    }, 100);
  }
</script>

{#if showBackupRestore}
  <!-- Backup Restore Screen -->
  <div class="bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4 py-8" style="min-height: 100vh; overflow-y: auto;">
    <div class="w-full max-w-lg mx-auto">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-8">
        <div class="text-center mb-6">
          <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">
            {$_('backup.restore.title')}
          </h1>
          <p class="text-gray-600 dark:text-gray-400 text-sm">
            {$_('backup.restore.selectFileHint')}
          </p>
        </div>

        <!-- Back Button -->
        <button
          type="button"
          on:click={handleCancelBackupRestore}
          disabled={restoring}
          class="mb-6 flex items-center text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors disabled:opacity-50"
        >
          <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
          {$_('backup.restore.cancel')}
        </button>

        <!-- File Selection -->
        <div class="mb-6">
          <label for="backup-file-select" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {$_('backup.restore.selectFile')}
          </label>
          <div class="flex gap-2">
            <button
              id="backup-file-select"
              type="button"
              on:click={() => backupFileInput.click()}
              disabled={restoring}
              class="flex-1 px-4 py-3 border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-lg hover:border-blue-500 dark:hover:border-blue-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <div class="text-center">
                {#if backupFile}
                  <span class="text-sm text-gray-900 dark:text-white font-medium">
                    {backupFile.name}
                  </span>
                {:else}
                  <span class="text-sm text-gray-500 dark:text-gray-400">
                    {$_('backup.restore.noFileSelected')}
                  </span>
                {/if}
              </div>
            </button>
          </div>
          <input
            bind:this={backupFileInput}
            type="file"
            accept=".jottery-backup,.json"
            on:change={handleBackupFileSelect}
            class="hidden"
            aria-hidden="true"
          />
        </div>

        <!-- Backup Info (shown after file selected) -->
        {#if backupStats}
          <div class="mb-6 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
            <h3 class="text-sm font-medium text-blue-800 dark:text-blue-200 mb-2">
              {$_('backup.restore.stats.title')}
            </h3>
            <div class="space-y-1 text-sm text-blue-700 dark:text-blue-300">
              {#if backupStats.createdAt}
                <p>
                  <span class="font-medium">{$_('backup.restore.stats.created')}:</span>
                  {formatBackupDate(backupStats.createdAt)}
                </p>
              {/if}
              <p>
                <span class="font-medium">{$_('backup.restore.stats.records')}:</span>
                {backupStats.recordCount ?? 0}
              </p>
            </div>
          </div>
        {/if}

        <!-- Validation Progress (shown while parsing large backup files) -->
        {#if validatingBackup}
          <div class="mb-6 bg-gray-50 dark:bg-gray-800 rounded-lg p-4">
            <div class="flex items-center gap-3 mb-2">
              <div class="animate-spin rounded-full h-5 w-5 border-2 border-blue-600 border-t-transparent"></div>
              <div class="flex-1">
                <p class="text-sm font-medium text-gray-900 dark:text-white">
                  {$_('backup.restore.progress.validating')}
                </p>
              </div>
              {#if validationProgress}
                <span class="text-sm font-medium text-blue-600 dark:text-blue-400">
                  {validationProgress.percent}%
                </span>
              {/if}
            </div>
            <!-- Progress bar -->
            <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
              <div
                class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                style="width: {validationProgress?.percent ?? 0}%"
              ></div>
            </div>
            {#if validationProgress && backupFile}
              <p class="text-xs text-gray-500 dark:text-gray-400 mt-2">
                {(validationProgress.bytesRead / (1024 * 1024)).toFixed(1)} / {(backupFile.size / (1024 * 1024)).toFixed(1)} MB
              </p>
            {/if}
          </div>
        {/if}

        <!-- Password Input (shown after file selected and validated) -->
        {#if backupData}
          <div class="mb-6">
            <label for="backup-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {$_('backup.restore.password')}
            </label>
            <PasswordInput
              id="backup-password"
              bind:value={backupPassword}
              disabled={restoring}
              on:keydown={(e) => e.detail.key === 'Enter' && handleRestoreBackup()}
              placeholder={$_('backup.restore.password')}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {$_('backup.restore.passwordHint')}
            </p>
          </div>
        {/if}

        <!-- Error Message -->
        {#if error}
          <div class="mb-6 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-md p-3">
            <p class="text-sm text-red-800 dark:text-red-200">{error}</p>
          </div>
        {/if}

        <!-- Restore Progress -->
        {#if restoreProgress}
          <div class="mb-6 bg-gray-50 dark:bg-gray-900 rounded-lg p-4">
            <div class="flex items-center gap-3">
              <div class="animate-spin rounded-full h-5 w-5 border-2 border-blue-600 border-t-transparent"></div>
              <div class="flex-1">
                <p class="text-sm font-medium text-gray-900 dark:text-white">
                  {$_(`backup.restore.progress.${restoreProgress.phase}`)}
                </p>
                {#if restoreProgress.total && restoreProgress.current !== undefined}
                  <p class="text-xs text-gray-500 dark:text-gray-400">
                    {restoreProgress.current} / {restoreProgress.total}
                  </p>
                {/if}
              </div>
            </div>
          </div>
        {/if}

        <!-- Restore Button -->
        {#if backupData}
          <button
            type="button"
            on:click={handleRestoreBackup}
            disabled={restoring || !backupPassword}
            class="w-full bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 disabled:cursor-not-allowed text-white font-medium py-3 px-4 rounded-md transition-colors duration-200"
          >
            {restoring ? $_('backup.restore.restoring') : $_('backup.restore.restoreButton')}
          </button>
        {/if}
      </div>

      <div class="mt-6 text-center text-xs text-gray-500 dark:text-gray-400">
        <p>{$_('app.tagline')}</p>
      </div>
    </div>
  </div>
{:else if showConnectExistingAccount}
  <!-- Connect Existing Account - Full Page -->
  <div class="bg-gradient-to-br from-purple-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4 py-8" style="min-height: 100vh; overflow-y: auto;">
    <div class="w-full max-w-lg mx-auto">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-8">
        <div class="text-center mb-6">
          <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">
            {$_('unlock.connectExisting.title')}
          </h1>
          <p class="text-gray-600 dark:text-gray-400 text-sm">
            {$_('unlock.connectExisting.subtitle')}
          </p>
        </div>

        <!-- Back Button -->
        <button
          type="button"
          on:click={handleCancelConnectExistingAccount}
          disabled={connectLoading}
          class="mb-6 flex items-center text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors disabled:opacity-50"
        >
          <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
          {$_('common.back')}
        </button>

        <form on:submit|preventDefault={handleConnectExistingAccount} class="space-y-4">
          <!-- Sync Server URL -->
          <div>
            <label for="connect-endpoint" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.connectExisting.endpoint')}
            </label>
            <input
              id="connect-endpoint"
              type="url"
              bind:value={connectEndpoint}
              disabled={connectLoading}
              placeholder="https://example.com"
              class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {$_('unlock.connectExisting.endpointHint')}
            </p>
          </div>

          <!-- Device Name -->
          <div>
            <label for="connect-device-name" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.connectExisting.deviceName')}
            </label>
            <input
              id="connect-device-name"
              type="text"
              bind:value={connectDeviceName}
              disabled={connectLoading}
              on:focus={handleDeviceNameFocus}
              on:blur={handleDeviceNameBlur}
              placeholder={$_('unlock.connectExisting.deviceNamePlaceholder')}
              class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {$_('unlock.connectExisting.deviceNameHint')}
            </p>
          </div>

          <!-- Email -->
          <div>
            <label for="connect-email" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.connectExisting.email')}
            </label>
            <input
              id="connect-email"
              type="email"
              bind:value={connectEmail}
              disabled={connectLoading}
              placeholder={$_('unlock.connectExisting.emailPlaceholder')}
              class="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
          </div>

          <!-- Server Password -->
          <div>
            <label for="connect-server-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.connectExisting.serverPassword')}
            </label>
            <PasswordInput
              id="connect-server-password"
              bind:value={connectServerPassword}
              disabled={connectLoading}
              placeholder={$_('unlock.connectExisting.serverPasswordPlaceholder')}
              className="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {$_('unlock.connectExisting.serverPasswordHint')}
            </p>
          </div>

          <!-- Notes Password -->
          <div>
            <label for="connect-notes-password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.connectExisting.notesPassword')}
            </label>
            <PasswordInput
              id="connect-notes-password"
              bind:value={connectNotesPassword}
              disabled={connectLoading}
              placeholder={$_('unlock.connectExisting.notesPasswordPlaceholder')}
              className="w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
            <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {$_('unlock.connectExisting.notesPasswordHint')}
            </p>
          </div>

          <!-- Step Progress -->
          {#if connectStep}
            <div class="bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-800 rounded-md p-3">
              <div class="flex items-center gap-3">
                <div class="animate-spin rounded-full h-5 w-5 border-2 border-purple-600 border-t-transparent"></div>
                <p class="text-sm text-purple-800 dark:text-purple-200">{connectStep}</p>
              </div>
            </div>
          {/if}

          <!-- Error Message -->
          {#if connectError}
            <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-md p-3">
              <p class="text-sm text-red-800 dark:text-red-200">{connectError}</p>
            </div>
          {/if}

          <!-- Submit Button -->
          <button
            type="submit"
            disabled={connectLoading}
            class="w-full bg-purple-600 hover:bg-purple-700 disabled:bg-purple-400 disabled:cursor-not-allowed text-white font-medium py-3 px-4 rounded-md transition-colors duration-200"
          >
            {connectLoading ? connectStep || $_('unlock.processing') : $_('unlock.connectExisting.submitButton')}
          </button>
        </form>
      </div>

      <div class="mt-6 text-center text-xs text-gray-500 dark:text-gray-400">
        <p>{$_('app.tagline')}</p>
      </div>
    </div>
  </div>
{:else if showLandingPage}
  <!-- Landing Page for First-Time Users -->
  <LandingPage onGetStarted={handleGetStarted} />
{:else}
  <!-- Password Setup/Unlock Form -->
  <div class="bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4 py-8" style="min-height: 100vh; overflow-y: auto;">
    <div class="w-full max-w-4xl mx-auto">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-8 max-w-lg mx-auto">
      <div class="text-center mb-8">
        <h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
          {needsInit ? $_('unlock.welcome') : $_('app.name')}
        </h1>
        {#if notebook.id.startsWith('_')}
          <p class="text-sm font-medium text-blue-600 dark:text-blue-400 mb-2">
            {notebook.displayName}
          </p>
        {/if}
        <p class="text-gray-600 dark:text-gray-400">
          {needsInit ? $_('unlock.setupPassword') : $_('unlock.enterPassword')}
        </p>
      </div>

      {#if needsInit}
        <!-- Language/theme controls and back link -->
        <div class="flex items-center justify-between mb-4">
          <button
            type="button"
            on:click={() => showLandingPage = true}
            class="flex items-center text-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white transition-colors"
          >
            <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
            {$_('common.back')}
          </button>
          <div class="flex items-center gap-2">
            <LanguagePicker />
            <button
              type="button"
              on:click={toggleTheme}
              class="p-2 rounded-lg border border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
              aria-label={$_('settings.appearance.theme', { default: 'Toggle theme' })}
            >
              {#if isDark}
                <svg class="w-[18px] h-[18px] text-gray-600 dark:text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
                </svg>
              {:else}
                <svg class="w-[18px] h-[18px] text-gray-600 dark:text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
                </svg>
              {/if}
            </button>
          </div>
        </div>
      {/if}

      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <div>
          <label for="password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {$_('unlock.password')}
          </label>
          <PasswordInput
            id="password"
            bind:value={password}
            bind:this={passwordInput}
            disabled={loading}
            placeholder={$_('unlock.password')}
            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
          />
        </div>

        {#if needsInit}
          <div>
            <label for="confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.confirmPassword')}
            </label>
            <PasswordInput
              id="confirm"
              bind:value={confirmPassword}
              disabled={loading}
              placeholder={$_('unlock.confirmPassword')}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            />
          </div>

          <!-- Already using Jottery? -->
          <div class="border border-gray-300 dark:border-gray-600 rounded-md">
            <button
              type="button"
              on:click={() => { showSyncConfig = !showSyncConfig; if (!showSyncConfig) resetSyncSetupMode(); }}
              class="w-full px-4 py-3 flex items-center justify-between text-left hover:bg-gray-50 dark:hover:bg-gray-700 rounded-md transition-colors"
            >
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                {$_('unlock.alreadyUsingJottery')}
              </span>
              <svg
                class="w-5 h-5 text-gray-500 dark:text-gray-400 transform transition-transform {showSyncConfig ? 'rotate-180' : ''}"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>

            {#if showSyncConfig}
              <div class="px-4 pb-4 space-y-3">
                <p class="text-xs text-gray-600 dark:text-gray-400">
                  {$_('unlock.alreadyUsingJotteryDescription')}
                </p>

                {#if syncSetupMode === null}
                  <!-- Setup Method Selection -->
                  <div class="space-y-2">
                    <!-- Option 1: Restore from Backup -->
                    <button
                      type="button"
                      on:click={handleShowBackupRestore}
                      class="w-full text-left border-2 border-amber-500 dark:border-amber-600 rounded-lg p-3 bg-amber-50 dark:bg-amber-900/20 hover:bg-amber-100 dark:hover:bg-amber-900/30 transition-colors"
                    >
                      <div class="flex items-start gap-2">
                        <span class="text-lg">💾</span>
                        <div class="flex-1">
                          <div class="font-semibold text-sm text-amber-900 dark:text-amber-100">
                            {$_('unlock.restoreFromBackup')}
                          </div>
                          <div class="text-xs text-amber-700 dark:text-amber-300 mt-1">
                            {$_('unlock.restoreFromBackupSubtitle')}
                          </div>
                        </div>
                      </div>
                    </button>

                    <!-- Option 2: Connect Existing Account -->
                    <button
                      type="button"
                      on:click={handleShowConnectExistingAccount}
                      class="w-full text-left border-2 border-purple-500 dark:border-purple-600 rounded-lg p-3 bg-purple-50 dark:bg-purple-900/20 hover:bg-purple-100 dark:hover:bg-purple-900/30 transition-colors"
                    >
                      <div class="flex items-start gap-2">
                        <span class="text-lg">🔗</span>
                        <div class="flex-1">
                          <div class="font-semibold text-sm text-purple-900 dark:text-purple-100">
                            {$_('unlock.syncSetup.existingAccount.title')}
                          </div>
                          <div class="text-xs text-purple-700 dark:text-purple-300 mt-1">
                            {$_('unlock.syncSetup.existingAccount.subtitle')}
                          </div>
                        </div>
                      </div>
                    </button>
                  </div>
                {/if}
              </div>
            {/if}
          </div>

          <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-md p-3">
            <p class="text-sm text-yellow-800 dark:text-yellow-200">
              {$_('unlock.warning')}
            </p>
          </div>
        {/if}

        {#if error}
          <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-md p-3">
            <p class="text-sm text-red-800 dark:text-red-200">{error}</p>
          </div>
        {/if}

        <button
          type="submit"
          disabled={loading}
          class="w-full bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-medium py-2 px-4 rounded-md transition-colors duration-200"
        >
          {loading ? $_('unlock.processing') : needsInit ? $_('unlock.createPassword') : $_('unlock.unlock')}
        </button>

        {#if showDeleteOption && !needsInit}
          <button
            type="button"
            on:click={handleDeleteRequest}
            disabled={loading}
            class="w-full bg-red-600 hover:bg-red-700 disabled:bg-red-400 text-white font-medium py-2 px-4 rounded-md transition-colors duration-200"
          >
            {$_('unlock.deleteAndStartOver')}
          </button>
        {/if}
      </form>
      </div>

      {#if needsInit}
        <!-- Landing page content for first-time users -->
        <div class="mt-8 pt-6 border-t border-gray-200 dark:border-gray-700 text-sm">
          <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-md p-4">
              <h2 class="font-semibold text-blue-900 dark:text-blue-200 mb-2">
                {$_('unlock.landingPage.privateEncrypted.title')}
              </h2>
              <p class="text-blue-800 dark:text-blue-300 leading-relaxed">
                {@html $_('unlock.landingPage.privateEncrypted.description')}
              </p>
            </div>

            <div class="bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-800 rounded-md p-4">
              <h2 class="font-semibold text-purple-900 dark:text-purple-200 mb-2">
                {$_('unlock.landingPage.optionalSync.title')}
              </h2>
              <p class="text-purple-800 dark:text-purple-300 leading-relaxed">
                {@html $_('unlock.landingPage.optionalSync.description')}
              </p>
            </div>

            <div class="bg-gray-50 dark:bg-gray-700/50 border border-gray-200 dark:border-gray-600 rounded-md p-4">
              <h2 class="font-semibold text-gray-900 dark:text-gray-200 mb-2">
                {$_('unlock.landingPage.selfHosting.title')}
              </h2>
              <p class="text-gray-700 dark:text-gray-300 leading-relaxed mb-2">
                {$_('unlock.landingPage.selfHosting.description')}
              </p>
              <a
                href="https://github.com/seesee/jottery"
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center text-blue-600 dark:text-blue-400 hover:text-blue-800 dark:hover:text-blue-300 font-medium"
              >
                <svg class="w-4 h-4 mr-1" fill="currentColor" viewBox="0 0 24 24">
                  <path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/>
                </svg>
                {$_('unlock.landingPage.selfHosting.viewOnGitHub')}
              </a>
            </div>
          </div>
        </div>
      {/if}

      <div class="mt-6 text-center text-xs text-gray-500 dark:text-gray-400">
        <p>{$_('app.tagline')}</p>
      </div>
    </div>
  </div>
{/if}

<ConfirmModal
  show={showDeleteConfirm}
  title={$_('confirm.deleteDatabase.title')}
  message={$_('confirm.deleteDatabase.message')}
  confirmText={$_('confirm.deleteDatabase.confirmButton')}
  cancelText={$_('common.cancel')}
  confirmClass="bg-red-600 hover:bg-red-700"
  onConfirm={handleDeleteConfirm}
  onCancel={handleDeleteCancel}
/>
