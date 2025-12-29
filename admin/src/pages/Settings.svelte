<script lang="ts">
  import { api } from '../lib/api';
  import { toast } from '../lib/toast.svelte';

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let changingPassword = $state(false);
  let passwordError = $state('');

  async function handleChangePassword() {
    // Validate inputs
    if (!currentPassword) {
      passwordError = 'Current password is required';
      return;
    }

    if (!newPassword) {
      passwordError = 'New password is required';
      return;
    }

    if (newPassword.length < 12) {
      passwordError = 'New password must be at least 12 characters';
      return;
    }

    if (newPassword !== confirmPassword) {
      passwordError = 'Passwords do not match';
      return;
    }

    changingPassword = true;
    passwordError = '';

    try {
      await api.changePassword(currentPassword, newPassword);
      toast.success('Password changed successfully');

      // Clear form
      currentPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Failed to change password';
      passwordError = errorMessage;
      toast.error(errorMessage);
    } finally {
      changingPassword = false;
    }
  }
</script>

<div class="space-y-6">
  <h1 class="text-3xl font-bold text-gray-900">Settings</h1>

  <!-- Change Password Card -->
  <div class="bg-white rounded-lg shadow p-6">
    <h2 class="text-xl font-semibold text-gray-900 mb-4">Change Password</h2>
    <p class="text-sm text-gray-600 mb-6">
      Update your admin password. Password must be at least 12 characters long.
    </p>

    <form onsubmit={(e) => { e.preventDefault(); handleChangePassword(); }} class="space-y-4 max-w-md">
      <div>
        <label for="currentPassword" class="block text-sm font-medium text-gray-700 mb-1">
          Current Password
        </label>
        <input
          id="currentPassword"
          type="password"
          bind:value={currentPassword}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Enter current password"
          disabled={changingPassword}
        />
      </div>

      <div>
        <label for="newPassword" class="block text-sm font-medium text-gray-700 mb-1">
          New Password
        </label>
        <input
          id="newPassword"
          type="password"
          bind:value={newPassword}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Enter new password (min 12 characters)"
          disabled={changingPassword}
        />
      </div>

      <div>
        <label for="confirmPassword" class="block text-sm font-medium text-gray-700 mb-1">
          Confirm New Password
        </label>
        <input
          id="confirmPassword"
          type="password"
          bind:value={confirmPassword}
          class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Confirm new password"
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
          {changingPassword ? 'Changing...' : 'Change Password'}
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
            Cancel
          </button>
        {/if}
      </div>
    </form>
  </div>
</div>
