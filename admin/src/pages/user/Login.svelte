<script lang="ts">
  import { userAuth } from '../../lib/userAuth.svelte';
  import { userApi, ApiError } from '../../lib/userApi';
  import { _ } from 'svelte-i18n';

  let email = $state('');
  let password = $state('');
  let error = $state<string | null>(null);
  let loading = $state(false);

  // Status check state
  let showStatusCheck = $state(false);
  let statusEmail = $state('');
  let statusLoading = $state(false);
  let statusResult = $state<{ exists: boolean; isApproved: boolean; isActive: boolean } | null>(null);
  let statusError = $state<string | null>(null);

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

  async function handleCheckStatus(e: Event) {
    e.preventDefault();
    statusError = null;
    statusResult = null;
    statusLoading = true;

    try {
      statusResult = await userApi.checkStatus(statusEmail);
    } catch (err) {
      statusError = $_('userPortal.login.errors.statusCheckFailed');
    } finally {
      statusLoading = false;
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

    <!-- Check Status Link -->
    <div class="mt-6 text-center">
      <button
        onclick={() => showStatusCheck = !showStatusCheck}
        class="text-sm text-blue-600 hover:text-blue-800 hover:underline"
      >
        {$_('userPortal.login.checkStatus')}
      </button>
    </div>

    <!-- Status Check Form -->
    {#if showStatusCheck}
      <div class="mt-4 p-4 bg-gray-50 rounded-lg border border-gray-200">
        <h3 class="text-sm font-medium text-gray-900 mb-3">{$_('userPortal.login.statusCheck.title')}</h3>

        <form onsubmit={handleCheckStatus} class="space-y-3">
          <div>
            <input
              type="email"
              required
              bind:value={statusEmail}
              disabled={statusLoading}
              class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100 text-sm"
              placeholder={$_('userPortal.login.placeholder.email')}
            />
          </div>
          <button
            type="submit"
            disabled={statusLoading || !statusEmail}
            class="w-full bg-gray-600 text-white py-2 px-4 rounded-md hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors text-sm"
          >
            {statusLoading ? $_('common.loading') : $_('userPortal.login.statusCheck.checkButton')}
          </button>
        </form>

        {#if statusError}
          <div class="mt-3 bg-red-50 border border-red-200 text-red-700 px-3 py-2 rounded text-sm">
            {statusError}
          </div>
        {/if}

        {#if statusResult}
          <div class="mt-3 text-sm">
            {#if !statusResult.exists}
              <div class="bg-gray-100 border border-gray-300 text-gray-700 px-3 py-2 rounded">
                {$_('userPortal.login.statusCheck.notFound')}
              </div>
            {:else if statusResult.isApproved && statusResult.isActive}
              <div class="bg-green-50 border border-green-200 text-green-700 px-3 py-2 rounded">
                {$_('userPortal.login.statusCheck.approved')}
              </div>
            {:else if !statusResult.isApproved}
              <div class="bg-yellow-50 border border-yellow-200 text-yellow-700 px-3 py-2 rounded">
                {$_('userPortal.login.statusCheck.pending')}
              </div>
            {:else if !statusResult.isActive}
              <div class="bg-red-50 border border-red-200 text-red-700 px-3 py-2 rounded">
                {$_('userPortal.login.statusCheck.inactive')}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
