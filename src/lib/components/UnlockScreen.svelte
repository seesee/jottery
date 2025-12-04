<script lang="ts">
  import { isInitialized as isInitializedStore, isLocked } from '../stores/appStore';
  import { initialize, unlock, isInitialized, deleteDB } from '../services';
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

  // Check if needs initialization
  (async () => {
    needsInit = !(await isInitialized());
    isInitializedStore.set(!needsInit);
  })();

  async function handleSubmit() {
    error = '';
    loading = true;

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
        await initialize(password);
      } else {
        // Unlock existing
        await unlock(password);
      }

      // Update store to trigger UI change
      isLocked.set(false);

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
  <div class="w-full max-w-md">
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-8">
      <div class="text-center mb-8">
        <h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
          {needsInit ? $_('unlock.welcome') : $_('app.name')}
        </h1>
        <p class="text-gray-600 dark:text-gray-400">
          {needsInit ? $_('unlock.setupPassword') : $_('unlock.enterPassword')}
        </p>
      </div>

      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <div>
          <label for="password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            {$_('unlock.password')}
          </label>
          <input
            id="password"
            type="password"
            bind:value={password}
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
