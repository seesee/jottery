<script lang="ts">
  import { api, ApiError } from '../lib/api';
  import type { UserListItem } from '../lib/api';
  import { onMount } from 'svelte';
  import { toast } from '../lib/toast.svelte';
  import ConfirmModal from '../components/ConfirmModal.svelte';

  let users = $state<UserListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state<'all' | 'pending' | 'approved' | 'inactive'>('all');

  // Confirmation modal state
  type ConfirmAction = { type: 'approve' | 'deactivate' | 'activate'; userId: string; email: string } | null;
  let confirmAction = $state<ConfirmAction>(null);

  async function loadUsers() {
    try {
      loading = true;
      error = null;
      const response = await api.listUsers();
      users = response.users;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load users';
    } finally {
      loading = false;
    }
  }

  function requestApprove(userId: string, email: string) {
    confirmAction = { type: 'approve', userId, email };
  }

  function requestDeactivate(userId: string, email: string) {
    confirmAction = { type: 'deactivate', userId, email };
  }

  function requestActivate(userId: string, email: string) {
    confirmAction = { type: 'activate', userId, email };
  }

  async function executeAction() {
    if (!confirmAction) return;

    const { type, userId, email } = confirmAction;
    confirmAction = null;

    try {
      if (type === 'approve') {
        await api.approveUser(userId);
        toast.success(`User ${email} approved successfully`);
      } else if (type === 'deactivate') {
        await api.deactivateUser(userId);
        toast.success(`User ${email} deactivated`);
      } else if (type === 'activate') {
        await api.activateUser(userId);
        toast.success(`User ${email} activated`);
      }
      await loadUsers();
    } catch (err) {
      if (err instanceof ApiError) {
        toast.error(`Failed to ${type} user: ${err.message}`);
      } else {
        toast.error(`Failed to ${type} user`);
      }
    }
  }

  function getFilteredUsers() {
    switch (filter) {
      case 'pending':
        return users.filter(u => !u.approved);
      case 'approved':
        return users.filter(u => u.approved && u.isActive);
      case 'inactive':
        return users.filter(u => !u.isActive);
      default:
        return users;
    }
  }

  function getStatusBadge(user: UserListItem) {
    if (!user.approved) {
      return { text: 'Pending', class: 'bg-yellow-100 text-yellow-800' };
    }
    if (!user.isActive) {
      return { text: 'Inactive', class: 'bg-gray-100 text-gray-800' };
    }
    if (user.isAdmin) {
      return { text: 'Admin', class: 'bg-purple-100 text-purple-800' };
    }
    return { text: 'Active', class: 'bg-green-100 text-green-800' };
  }

  onMount(() => {
    loadUsers();
  });

  $effect(() => {
    // React to filter changes
    filter;
  });

  const filteredUsers = $derived(getFilteredUsers());
</script>

<div class="space-y-6">
  <div class="flex justify-between items-center">
    <h1 class="text-3xl font-bold text-gray-900">Users</h1>
    <button
      onclick={loadUsers}
      class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-400"
      disabled={loading}
    >
      {loading ? 'Refreshing...' : 'Refresh'}
    </button>
  </div>

  {#if error}
    <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
      {error}
    </div>
  {/if}

  <!-- Filters -->
  <div class="flex space-x-2">
    <button
      onclick={() => filter = 'all'}
      class="px-4 py-2 rounded-md {filter === 'all' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      All ({users.length})
    </button>
    <button
      onclick={() => filter = 'pending'}
      class="px-4 py-2 rounded-md {filter === 'pending' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      Pending ({users.filter(u => !u.approved).length})
    </button>
    <button
      onclick={() => filter = 'approved'}
      class="px-4 py-2 rounded-md {filter === 'approved' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      Approved ({users.filter(u => u.approved && u.isActive).length})
    </button>
    <button
      onclick={() => filter = 'inactive'}
      class="px-4 py-2 rounded-md {filter === 'inactive' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      Inactive ({users.filter(u => !u.isActive).length})
    </button>
  </div>

  {#if loading && users.length === 0}
    <div class="flex justify-center items-center py-12">
      <div class="text-gray-500">Loading users...</div>
    </div>
  {:else if filteredUsers.length === 0}
    <div class="bg-white rounded-lg shadow p-8 text-center text-gray-500">
      No users found
    </div>
  {:else}
    <!-- Users Table -->
    <div class="bg-white rounded-lg shadow overflow-hidden">
      <table class="min-w-full divide-y divide-gray-200">
        <thead class="bg-gray-50">
          <tr>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Email
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Status
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Devices
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Notes
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Created
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              Actions
            </th>
          </tr>
        </thead>
        <tbody class="bg-white divide-y divide-gray-200">
          {#each filteredUsers as user (user.id)}
            {@const badge = getStatusBadge(user)}
            <tr class="hover:bg-gray-50">
              <td class="px-6 py-4 whitespace-nowrap">
                <div class="text-sm font-medium text-gray-900">{user.email}</div>
              </td>
              <td class="px-6 py-4 whitespace-nowrap">
                <span class="px-2 py-1 inline-flex text-xs leading-5 font-semibold rounded-full {badge.class}">
                  {badge.text}
                </span>
              </td>
              <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                {user.deviceCount}
              </td>
              <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                {user.noteCount}
              </td>
              <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                {new Date(user.createdAt).toLocaleDateString('en-GB')}
              </td>
              <td class="px-6 py-4 whitespace-nowrap text-sm font-medium space-x-2">
                {#if !user.approved}
                  <button
                    onclick={() => requestApprove(user.id, user.email)}
                    class="text-green-600 hover:text-green-900"
                  >
                    Approve
                  </button>
                {/if}
                {#if user.isActive}
                  <button
                    onclick={() => requestDeactivate(user.id, user.email)}
                    class="text-yellow-600 hover:text-yellow-900"
                  >
                    Deactivate
                  </button>
                {:else}
                  <button
                    onclick={() => requestActivate(user.id, user.email)}
                    class="text-green-600 hover:text-green-900"
                  >
                    Activate
                  </button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if confirmAction}
  {@const actionText = confirmAction.type === 'approve' ? 'Approve' : confirmAction.type === 'deactivate' ? 'Deactivate' : 'Activate'}
  {@const message = `Are you sure you want to ${confirmAction.type} user ${confirmAction.email}?`}
  <ConfirmModal
    title="{actionText} User"
    message={message}
    confirmText={actionText}
    onConfirm={executeAction}
    onCancel={() => confirmAction = null}
  />
{/if}
