<script lang="ts">
  import { api } from '../lib/api';
  import { toast } from '../lib/toast.svelte';
  import { _, locale } from 'svelte-i18n';
  import { AVAILABLE_LOCALES } from '../lib/services/i18nService';

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let changingPassword = $state(false);
  let passwordError = $state('');

  // Language setting - stored in localStorage
  const LANGUAGE_STORAGE_KEY = 'admin_language';
  let selectedLanguage = $state(localStorage.getItem(LANGUAGE_STORAGE_KEY) || $locale || 'en-GB');

  function handleLanguageChange(event: Event) {
    const newLocale = (event.target as HTMLSelectElement).value;
    selectedLanguage = newLocale;
    locale.set(newLocale);
    localStorage.setItem(LANGUAGE_STORAGE_KEY, newLocale);
  }

  async function handleChangePassword() {
    // Validate inputs
    if (!currentPassword) {
      passwordError = $_('settings.changePassword.errors.currentRequired');
      return;
    }

    if (!newPassword) {
      passwordError = $_('settings.changePassword.errors.newRequired');
      return;
    }

    if (newPassword.length < 12) {
      passwordError = $_('settings.changePassword.errors.minLength');
      return;
    }

    if (newPassword !== confirmPassword) {
      passwordError = $_('settings.changePassword.errors.mismatch');
      return;
    }

    changingPassword = true;
    passwordError = '';

    try {
      await api.changePassword(currentPassword, newPassword);
      toast.success($_('settings.changePassword.success'));

      // Clear form
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : $_('settings.changePassword.errors.failed');
      passwordError = errorMessage;
      toast.error(errorMessage);
    } finally {
      changingPassword = false;
    }
  }
</script>

<div class="space-y-6">
  <h1 class="text-3xl font-bold text-gray-900">{$_('settings.title')}</h1>

  <!-- Language Setting Card -->
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-xl font-semibold text-gray-900 mb-4">{$_('settings.language.title')}</h2>
    <p class="text-sm text-gray-600 mb-6">
      {$_('settings.language.description')}
    </p>

    <div class="max-w-xs">
      <label for="language" class="block text-sm font-medium text-gray-700 mb-1">
        {$_('settings.language.label')}
      </label>
      <select
        id="language"
        value={selectedLanguage}
        onchange={handleLanguageChange}
        class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white"
      >
        {#each AVAILABLE_LOCALES as loc}
          <option value={loc.code}>{loc.name}</option>
        {/each}
      </select>
    </div>
  </div>

  <!-- Change Password Card -->
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-xl font-semibold text-gray-900 mb-4">{$_('settings.changePassword.title')}</h2>
    <p class="text-sm text-gray-600 mb-6">
      {$_('settings.changePassword.description')}
    </p>

    <form onsubmit={(e) => { e.preventDefault(); handleChangePassword(); }} class="space-y-4 max-w-md">
      <div>
        <label for="currentPassword" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('settings.changePassword.currentPassword')}
        </label>
        <input
          id="currentPassword"
          type="password"
          bind:value={currentPassword}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder={$_('settings.changePassword.placeholder.current')}
          disabled={changingPassword}
        />
      </div>

      <div>
        <label for="newPassword" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('settings.changePassword.newPassword')}
        </label>
        <input
          id="newPassword"
          type="password"
          bind:value={newPassword}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder={$_('settings.changePassword.placeholder.new')}
          disabled={changingPassword}
        />
      </div>

      <div>
        <label for="confirmPassword" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('settings.changePassword.confirmPassword')}
        </label>
        <input
          id="confirmPassword"
          type="password"
          bind:value={confirmPassword}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder={$_('settings.changePassword.placeholder.confirm')}
          disabled={changingPassword}
        />
      </div>

      {#if passwordError}
        <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {passwordError}
        </div>
      {/if}

      <div class="flex gap-3">
        <button
          type="submit"
          disabled={changingPassword}
          class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
        >
          {changingPassword ? $_('settings.changePassword.buttonChanging') : $_('settings.changePassword.button')}
        </button>

        {#if currentPassword || newPassword || confirmPassword}
          <button
            type="button"
            onclick={() => {
              currentPassword = '';
              newPassword = '';
              confirmPassword = '';
              passwordError = '';
            }}
            disabled={changingPassword}
            class="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 disabled:bg-gray-100 disabled:cursor-not-allowed transition-colors"
          >
            {$_('settings.changePassword.cancel')}
          </button>
        {/if}
      </div>
    </form>
  </div>
</div>
