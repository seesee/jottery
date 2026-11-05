<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { DashboardAuth } from '../lib/dashboardAuth.svelte';
  import { describeError } from '../lib/apiErrors';

  // Shown instead of the dashboard while the server refuses everything else. That's
  // every new install's first sign-in, because the generated password has to be
  // replaced, and any older install still on the published default password.

  interface Props {
    auth: DashboardAuth;
  }

  let { auth }: Props = $props();

  const MIN_LENGTH = 12;

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state<string | null>(null);
  let saving = $state(false);

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }

  function validationError(): string | null {
    if (newPassword.length < MIN_LENGTH) {
      return $_('errors.passwordTooShort');
    }
    if (newPassword !== confirmPassword) {
      return $_('settings.changePassword.errors.mismatch');
    }
    if (newPassword === currentPassword) {
      return $_('errors.passwordSameAsCurrent');
    }
    return null;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = validationError();
    if (error) {
      return;
    }

    saving = true;
    try {
      await auth.changePassword(currentPassword, newPassword);
    } catch (err) {
      error = describeError(err, $_, 'settings.changePassword.errors.failed');
    } finally {
      saving = false;
    }
  }
</script>

<div class="min-h-screen flex items-center justify-center bg-gray-100 p-4">
  <div class="max-w-md w-full bg-white rounded-lg shadow-lg p-8">
    <div class="mb-6">
      <h1 class="text-2xl font-bold text-gray-900">{$_('forcedPasswordChange.title')}</h1>
      {#if auth.user}
        <p class="text-sm text-gray-500 mt-1">{auth.user.email}</p>
      {/if}
      <p class="text-gray-600 mt-3">{$_('forcedPasswordChange.description')}</p>
    </div>

    <form onsubmit={handleSubmit} class="space-y-4">
      {#if error}
        <div role="alert" class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {error}
        </div>
      {/if}

      <div>
        <label for="forced-current-password" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('settings.changePassword.currentPassword')}
        </label>
        <input
          id="forced-current-password"
          type="password"
          required
          autocomplete="current-password"
          bind:value={currentPassword}
          disabled={saving}
          use:focusOnMount
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
      </div>

      <div>
        <label for="forced-new-password" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('settings.changePassword.newPassword')}
        </label>
        <input
          id="forced-new-password"
          type="password"
          required
          autocomplete="new-password"
          bind:value={newPassword}
          disabled={saving}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
        <p class="text-xs text-gray-500 mt-1">{$_('common.passwordRules')}</p>
      </div>

      <div>
        <label for="forced-confirm-password" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('settings.changePassword.confirmPassword')}
        </label>
        <input
          id="forced-confirm-password"
          type="password"
          required
          autocomplete="new-password"
          bind:value={confirmPassword}
          disabled={saving}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
      </div>

      <button
        type="submit"
        disabled={saving}
        class="w-full bg-blue-600 text-white py-2 px-4 rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
      >
        {saving ? $_('settings.changePassword.buttonChanging') : $_('settings.changePassword.button')}
      </button>
    </form>

    <div class="mt-4 text-center">
      <button
        type="button"
        onclick={() => auth.logout()}
        disabled={saving}
        class="text-sm text-gray-600 hover:text-gray-900 hover:underline"
      >
        {$_('nav.logout')}
      </button>
    </div>
  </div>
</div>
