<script lang="ts">
  import { userAuth } from '../../lib/userAuth.svelte';
  import { ApiError } from '../../lib/userApi';
  import { _ } from 'svelte-i18n';

  let email = $state('');
  let password = $state('');
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = null;
    loading = true;

    try {
      await userAuth.login(email, password);
      // Navigation will be handled by UserApp.svelte watching auth state
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 401) {
          error = $_('userPortal.login.errors.invalidCredentials');
        } else if (err.status === 403) {
          error = $_('userPortal.login.errors.accountInactive');
        } else {
          error = err.message;
        }
      } else {
        error = $_('userPortal.login.errors.unexpected');
      }
    } finally {
      loading = false;
    }
  }
</script>

<div class="min-h-screen flex items-center justify-center bg-gray-100">
  <div class="max-w-md w-full bg-white rounded-lg shadow-lg p-8">
    <div class="text-center mb-8">
      <h1 class="text-3xl font-bold text-gray-900">{$_('userPortal.login.title')}</h1>
      <p class="text-gray-600 mt-2">{$_('userPortal.login.subtitle')}</p>
    </div>

    <form onsubmit={handleSubmit} class="space-y-6">
      {#if error}
        <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {error}
        </div>
      {/if}

      <div>
        <label for="email" class="block text-sm font-medium text-gray-700 mb-2">
          {$_('userPortal.login.email')}
        </label>
        <input
          id="email"
          type="email"
          required
          bind:value={email}
          disabled={loading}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
          placeholder={$_('userPortal.login.placeholder.email')}
        />
      </div>

      <div>
        <label for="password" class="block text-sm font-medium text-gray-700 mb-2">
          {$_('userPortal.login.password')}
        </label>
        <input
          id="password"
          type="password"
          required
          bind:value={password}
          disabled={loading}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
          placeholder={$_('userPortal.login.placeholder.password')}
        />
      </div>

      <button
        type="submit"
        disabled={loading}
        class="w-full bg-blue-600 text-white py-2 px-4 rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
      >
        {loading ? $_('userPortal.login.signingIn') : $_('userPortal.login.signIn')}
      </button>
    </form>
  </div>
</div>
