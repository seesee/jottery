<script lang="ts">
  import { isInitialized as isInitializedStore } from '../stores/appStore';
  import { initialize, unlock, isInitialized } from '../services';

  let password = '';
  let confirmPassword = '';
  let error = '';
  let loading = false;
  let needsInit = false;

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
          error = 'Password must be at least 8 characters';
          return;
        }
        if (password !== confirmPassword) {
          error = 'Passwords do not match';
          return;
        }
        await initialize(password);
      } else {
        // Unlock existing
        await unlock(password);
      }

      // Success - will be handled by parent component
      password = '';
      confirmPassword = '';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to unlock';
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex min-h-screen items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4">
  <div class="w-full max-w-md">
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl p-8">
      <div class="text-center mb-8">
        <h1 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">
          {needsInit ? 'Welcome to Jottery' : 'Jottery'}
        </h1>
        <p class="text-gray-600 dark:text-gray-400">
          {needsInit ? 'Set up your master password' : 'Enter your password to unlock'}
        </p>
      </div>

      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <div>
          <label for="password" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Password
          </label>
          <input
            id="password"
            type="password"
            bind:value={password}
            disabled={loading}
            required
            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
            placeholder="Enter password"
          />
        </div>

        {#if needsInit}
          <div>
            <label for="confirm" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Confirm Password
            </label>
            <input
              id="confirm"
              type="password"
              bind:value={confirmPassword}
              disabled={loading}
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
              placeholder="Confirm password"
            />
          </div>

          <div class="bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-md p-3">
            <p class="text-sm text-yellow-800 dark:text-yellow-200">
              ⚠️ <strong>Important:</strong> Your password cannot be recovered. If you lose it, your data will be permanently inaccessible.
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
          {loading ? 'Processing...' : needsInit ? 'Create Password' : 'Unlock'}
        </button>
      </form>

      <div class="mt-6 text-center text-xs text-gray-500 dark:text-gray-400">
        <p>🔒 Privacy-focused • Local-first • Encrypted</p>
      </div>
    </div>
  </div>
</div>
