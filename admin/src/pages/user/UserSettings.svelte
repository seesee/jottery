<script lang="ts">
  import { userApi, ApiError } from '../../lib/userApi';
  import { userAuth } from '../../lib/userAuth.svelte';
  import { toast } from '../../lib/toast.svelte';
  import ConfirmModal from '../../components/ConfirmModal.svelte';
  import { _, locale } from 'svelte-i18n';
  import { AVAILABLE_LOCALES } from '../../lib/services/i18nService';

  // Language setting - stored in localStorage (shared with admin)
  const LANGUAGE_STORAGE_KEY = 'admin_language';
  let selectedLanguage = $state(localStorage.getItem(LANGUAGE_STORAGE_KEY) || $locale || 'en-GB');

  function handleLanguageChange(event: Event) {
    const newLocale = (event.target as HTMLSelectElement).value;
    selectedLanguage = newLocale;
    locale.set(newLocale);
    localStorage.setItem(LANGUAGE_STORAGE_KEY, newLocale);
  }

  // Password change state
  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let passwordError = $state<string | null>(null);
  let passwordLoading = $state(false);

  // Account actions state
  let showDeleteNotesModal = $state(false);
  let showDeleteModal = $state(false);
  let deleteConfirmText = $state('');
  let actionLoading = $state(false);

  async function handlePasswordChange(e: Event) {
    e.preventDefault();
    passwordError = null;

    // Validate passwords match
    if (newPassword !== confirmPassword) {
      passwordError = $_('userPortal.settings.errors.passwordMismatch');
      return;
    }

    // Validate minimum length
    if (newPassword.length < 12) {
      passwordError = $_('userPortal.settings.errors.passwordTooShort');
      return;
    }

    passwordLoading = true;
    try {
      await userApi.changePassword(currentPassword, newPassword);
      toast.success($_('userPortal.settings.passwordChanged'));
      // Clear form
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 401) {
          passwordError = $_('userPortal.settings.errors.wrongPassword');
        } else {
          passwordError = err.message;
        }
      } else {
        passwordError = $_('userPortal.settings.errors.unexpected');
      }
    } finally {
      passwordLoading = false;
    }
  }

  async function handleDeleteNotes() {
    actionLoading = true;
    try {
      await userApi.deleteAllNotes();
      toast.success($_('userPortal.settings.notesDeleted'));
    } catch (err) {
      toast.error($_('userPortal.settings.errors.deleteNotesFailed'));
      console.error('Failed to delete notes:', err);
    } finally {
      actionLoading = false;
      showDeleteNotesModal = false;
    }
  }

  async function handleDelete() {
    if (deleteConfirmText !== 'DELETE') {
      return;
    }

    actionLoading = true;
    try {
      await userApi.deleteAccount();
      userAuth.clearAuth();
      toast.success($_('userPortal.settings.accountDeleted'));
    } catch (err) {
      toast.error($_('userPortal.settings.errors.deleteFailed'));
      console.error('Failed to delete account:', err);
    } finally {
      actionLoading = false;
      showDeleteModal = false;
      deleteConfirmText = '';
    }
  }
</script>

<div class="space-y-8">
  <h2 class="text-2xl font-bold text-gray-900">{$_('userPortal.settings.title')}</h2>

  <!-- Language Section -->
  <div class="bg-white rounded-lg shadow p-6">
    <h3 class="text-lg font-semibold text-gray-900 mb-4">{$_('userPortal.settings.language.title')}</h3>
    <p class="text-sm text-gray-600 mb-4">
      {$_('userPortal.settings.language.description')}
    </p>

    <div class="max-w-xs">
      <label for="language" class="block text-sm font-medium text-gray-700 mb-1">
        {$_('userPortal.settings.language.label')}
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

  <!-- Change Password Section -->
  <div class="bg-white rounded-lg shadow p-6">
    <h3 class="text-lg font-semibold text-gray-900 mb-4">{$_('userPortal.settings.changePassword')}</h3>

    <form onsubmit={handlePasswordChange} class="space-y-4 max-w-md">
      {#if passwordError}
        <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {passwordError}
        </div>
      {/if}

      <div>
        <label for="currentPassword" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('userPortal.settings.currentPassword')}
        </label>
        <input
          id="currentPassword"
          type="password"
          required
          bind:value={currentPassword}
          disabled={passwordLoading}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
      </div>

      <div>
        <label for="newPassword" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('userPortal.settings.newPassword')}
        </label>
        <input
          id="newPassword"
          type="password"
          required
          minlength="12"
          bind:value={newPassword}
          disabled={passwordLoading}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
        <p class="text-xs text-gray-500 mt-1">{$_('userPortal.settings.passwordHint')}</p>
      </div>

      <div>
        <label for="confirmPassword" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('userPortal.settings.confirmPassword')}
        </label>
        <input
          id="confirmPassword"
          type="password"
          required
          minlength="12"
          bind:value={confirmPassword}
          disabled={passwordLoading}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-100"
        />
      </div>

      <button
        type="submit"
        disabled={passwordLoading}
        class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
      >
        {passwordLoading ? $_('common.saving') : $_('userPortal.settings.updatePassword')}
      </button>
    </form>
  </div>

  <!-- Danger Zone -->
  <div class="bg-white rounded-lg shadow p-6 border-2 border-red-200">
    <h3 class="text-lg font-semibold text-red-600 mb-4">{$_('userPortal.settings.dangerZone')}</h3>

    <div class="space-y-4">
      <!-- Delete Notes -->
      <div class="flex items-center justify-between p-4 bg-gray-50 rounded-lg">
        <div>
          <h4 class="font-medium text-gray-900">{$_('userPortal.settings.deleteNotes')}</h4>
          <p class="text-sm text-gray-600">{$_('userPortal.settings.deleteNotesDescription')}</p>
        </div>
        <button
          onclick={() => showDeleteNotesModal = true}
          class="px-4 py-2 bg-yellow-600 text-white rounded-md hover:bg-yellow-700 focus:outline-none focus:ring-2 focus:ring-yellow-500 focus:ring-offset-2 transition-colors"
        >
          {$_('userPortal.settings.deleteNotesButton')}
        </button>
      </div>

      <!-- Delete Account -->
      <div class="flex items-center justify-between p-4 bg-gray-50 rounded-lg">
        <div>
          <h4 class="font-medium text-gray-900">{$_('userPortal.settings.deleteAccount')}</h4>
          <p class="text-sm text-gray-600">{$_('userPortal.settings.deleteDescription')}</p>
        </div>
        <button
          onclick={() => showDeleteModal = true}
          class="px-4 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 transition-colors"
        >
          {$_('userPortal.settings.delete')}
        </button>
      </div>
    </div>
  </div>
</div>

<!-- Delete Notes Confirmation Modal -->
{#if showDeleteNotesModal}
  <ConfirmModal
    title={$_('userPortal.settings.deleteNotesConfirmTitle')}
    message={$_('userPortal.settings.deleteNotesConfirmMessage')}
    confirmText={$_('userPortal.settings.deleteNotesButton')}
    cancelText={$_('common.cancel')}
    onConfirm={handleDeleteNotes}
    onCancel={() => showDeleteNotesModal = false}
    danger={true}
  />
{/if}

<!-- Delete Confirmation Modal -->
{#if showDeleteModal}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
    <div class="bg-white rounded-lg shadow-xl max-w-md w-full mx-4 p-6">
      <h3 class="text-lg font-semibold text-red-600 mb-2">{$_('userPortal.settings.deleteConfirmTitle')}</h3>
      <p class="text-gray-600 mb-4">{$_('userPortal.settings.deleteConfirmMessage')}</p>

      <div class="mb-4">
        <label for="deleteConfirm" class="block text-sm font-medium text-gray-700 mb-1">
          {$_('userPortal.settings.deleteConfirmLabel')}
        </label>
        <input
          id="deleteConfirm"
          type="text"
          bind:value={deleteConfirmText}
          disabled={actionLoading}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-red-500 disabled:bg-gray-100"
          placeholder="DELETE"
        />
      </div>

      <div class="flex justify-end space-x-3">
        <button
          onclick={() => { showDeleteModal = false; deleteConfirmText = ''; }}
          disabled={actionLoading}
          class="px-4 py-2 bg-gray-200 text-gray-800 rounded-md hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 transition-colors"
        >
          {$_('common.cancel')}
        </button>
        <button
          onclick={handleDelete}
          disabled={actionLoading || deleteConfirmText !== 'DELETE'}
          class="px-4 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
        >
          {actionLoading ? $_('common.deleting') : $_('userPortal.settings.deleteForever')}
        </button>
      </div>
    </div>
  </div>
{/if}
