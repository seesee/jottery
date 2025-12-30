<script lang="ts">
  import { onMount } from 'svelte';
  import { isInitialized as isInitializedStore, isLocked } from '../stores/appStore';
  import { initialize, unlock, isInitialized, deleteDB, passwordStorageService, settingsRepository } from '../services';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from './ConfirmModal.svelte';

  let password = '';
  let confirmPassword = '';
  let error = '';
  let loading = false;
  let needsInit = false;
  let failedAttempts = 0;
  let showDeleteOption = false;
  let showDeleteConfirm = false;
  let passwordInput: HTMLInputElement;

  // Check if needs initialization
  (async () => {
    needsInit = !(await isInitialized());
    isInitializedStore.set(!needsInit);
  })();

  // Auto-focus password input on mount and check for stored password
  onMount(async () => {
    // Try auto-unlock with stored password
    if (!needsInit) {
      try {
        const settings = await settingsRepository.get();
        if (settings.rememberPassword) {
          const storedPassword = passwordStorageService.get();
          if (storedPassword) {
            console.log('[UnlockScreen] Stored password found, attempting auto-unlock...');
            loading = true;
            try {
              await unlock(storedPassword);
              isLocked.set(false);
              console.log('[UnlockScreen] ✓ Auto-unlock successful!');
              // Don't focus input - we're unlocked
              return;
            } catch (err) {
              console.error('[UnlockScreen] Auto-unlock failed:', err);
              // Clear invalid stored password
              passwordStorageService.clear();
              error = 'Stored password is invalid. Please enter your password.';
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

      // Store password if rememberPassword is enabled
      try {
        const settings = await settingsRepository.get();
        if (settings.rememberPassword) {
          passwordStorageService.store(passwordToStore);
          console.log('[UnlockScreen] Password stored for future auto-unlock');
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
</script>

<div class="flex min-h-screen items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4">
  <div class="w-full max-w-lg">
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-8">
      <div class="text-center mb-8">
        <h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
          {needsInit ? $_('unlock.welcome') : $_('app.name')}
        </h1>
        <p class="text-gray-600 dark:text-gray-400">
          {needsInit ? $_('unlock.setupPassword') : $_('unlock.enterPassword')}
        </p>
      </div>

      {#if needsInit}
        <!-- Landing page content for first-time users -->
        <div class="mb-6 space-y-4 text-sm">
          <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-md p-4">
            <h2 class="font-semibold text-blue-900 dark:text-blue-200 mb-2">
              🔒 Private & Encrypted
            </h2>
            <p class="text-blue-800 dark:text-blue-300 leading-relaxed">
              Jottery is a privacy-focused note-taking app. All your notes are <strong>encrypted locally</strong> on your device using your master password. No one can read your notes without your password—not even if they access your device's storage.
            </p>
          </div>

          <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-md p-4">
            <h2 class="font-semibold text-green-900 dark:text-green-200 mb-2">
              💾 Local-First Storage
            </h2>
            <p class="text-green-800 dark:text-green-300 leading-relaxed">
              Your notes are stored directly in your browser's local database. Jottery works completely <strong>offline</strong> and doesn't require an internet connection or account to use.
            </p>
          </div>

          <div class="bg-purple-50 dark:bg-purple-900/20 border border-purple-200 dark:border-purple-800 rounded-md p-4">
            <h2 class="font-semibold text-purple-900 dark:text-purple-200 mb-2">
              ☁️ Optional Sync Server
            </h2>
            <p class="text-purple-800 dark:text-purple-300 leading-relaxed">
              Want to sync across devices? Jottery can optionally sync with a self-hosted server. Your notes and tags are <strong>encrypted before syncing</strong>—the server only stores encrypted data it cannot read.
            </p>
          </div>

          <div class="bg-gray-50 dark:bg-gray-700/50 border border-gray-200 dark:border-gray-600 rounded-md p-4">
            <h2 class="font-semibold text-gray-900 dark:text-gray-200 mb-2">
              🏠 Self-Hosting
            </h2>
            <p class="text-gray-700 dark:text-gray-300 leading-relaxed mb-2">
              Want to run your own sync server? Jottery is open source and easy to self-host.
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
              View on GitHub
            </a>
          </div>
        </div>
      {/if}

      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <div>
          <label for="password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {$_('unlock.password')}
          </label>
          <input
            id="password"
            type="password"
            bind:value={password}
            bind:this={passwordInput}
            disabled={loading}
            required
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            placeholder={$_('unlock.password')}
          />
        </div>

        {#if needsInit}
          <div>
            <label for="confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {$_('unlock.confirmPassword')}
            </label>
            <input
              id="confirm"
              type="password"
              bind:value={confirmPassword}
              disabled={loading}
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
              placeholder={$_('unlock.confirmPassword')}
            />
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

      <div class="mt-6 text-center text-xs text-gray-500 dark:text-gray-400">
        <p>{$_('app.tagline')}</p>
      </div>
    </div>
  </div>
</div>

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
