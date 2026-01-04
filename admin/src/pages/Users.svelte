<script lang="ts">
  import { api, ApiError } from '../lib/api';
  import type { UserListItem } from '../lib/api';
  import { onMount } from 'svelte';
  import { toast } from '../lib/toast.svelte';
  import ConfirmModal from '../components/ConfirmModal.svelte';
  import { _ } from 'svelte-i18n';

  let users = $state<UserListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state<'all' | 'pending' | 'approved' | 'inactive'>('all');

  // Confirmation modal state
  type ConfirmAction = { type: 'approve' | 'deactivate' | 'activate' | 'delete' | 'toggleAdmin'; userId: string; email: string; isAdmin?: boolean } | null;
  let confirmAction = $state<ConfirmAction>(null);

  async function loadUsers() {
    try {
      loading = true;
      error = null;
      const response = await api.listUsers();
      users = response.users;
    } catch (err) {
      error = err instanceof Error ? err.message : $_('users.failedToLoad');
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

  function requestDelete(userId: string, email: string) {
    confirmAction = { type: 'delete', userId, email };
  }

  function requestToggleAdmin(userId: string, email: string, isAdmin: boolean) {
    confirmAction = { type: 'toggleAdmin', userId, email, isAdmin };
  }

  async function executeAction() {
    if (!confirmAction) return;

    const { type, userId, email, isAdmin } = confirmAction;
    confirmAction = null;

    try {
      if (type === 'approve') {
        await api.approveUser(userId);
        toast.success($_('users.toast.approved', { values: { email } }));
      } else if (type === 'deactivate') {
        await api.deactivateUser(userId);
        toast.success($_('users.toast.deactivated', { values: { email } }));
      } else if (type === 'activate') {
        await api.activateUser(userId);
        toast.success($_('users.toast.activated', { values: { email } }));
      } else if (type === 'delete') {
        await api.deleteUser(userId);
        toast.success($_('users.toast.deleted', { values: { email } }));
      } else if (type === 'toggleAdmin') {
        await api.toggleAdmin(userId, isAdmin!);
        if (isAdmin) {
          toast.success($_('users.toast.promotedToAdmin', { values: { email } }));
        } else {
          toast.success($_('users.toast.demotedFromAdmin', { values: { email } }));
        }
      }
      await loadUsers();
    } catch (err) {
      if (err instanceof ApiError) {
        toast.error($_('users.toast.failedToAction', { values: { action: type, error: err.message } }));
      } else {
        toast.error($_('users.toast.failedGeneric', { values: { action: type } }));
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
      return { text: $_('users.status.pending'), class: 'bg-yellow-100 text-yellow-800' };
    }
    if (!user.isActive) {
      return { text: $_('users.status.inactive'), class: 'bg-gray-100 text-gray-800' };
    }
    if (user.isAdmin) {
      return { text: $_('users.status.admin'), class: 'bg-purple-100 text-purple-800' };
    }
    return { text: $_('users.status.active'), class: 'bg-green-100 text-green-800' };
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
    <h1 class="text-3xl font-bold text-gray-900">{$_('users.title')}</h1>
    <button
      onclick={loadUsers}
      class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:bg-gray-400"
      disabled={loading}
    >
      {loading ? $_('common.refreshing') : $_('common.refresh')}
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
      {$_('users.filters.all')} ({users.length})
    </button>
    <button
      onclick={() => filter = 'pending'}
      class="px-4 py-2 rounded-md {filter === 'pending' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      {$_('users.filters.pending')} ({users.filter(u => !u.approved).length})
    </button>
    <button
      onclick={() => filter = 'approved'}
      class="px-4 py-2 rounded-md {filter === 'approved' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      {$_('users.filters.approved')} ({users.filter(u => u.approved && u.isActive).length})
    </button>
    <button
      onclick={() => filter = 'inactive'}
      class="px-4 py-2 rounded-md {filter === 'inactive' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}"
    >
      {$_('users.filters.inactive')} ({users.filter(u => !u.isActive).length})
    </button>
  </div>

  {#if loading && users.length === 0}
    <div class="flex justify-center items-center py-12">
      <div class="text-gray-500">{$_('users.loadingUsers')}</div>
    </div>
  {:else if filteredUsers.length === 0}
    <div class="bg-white rounded-lg shadow p-8 text-center text-gray-500">
      {$_('users.noUsersFound')}
    </div>
  {:else}
    <!-- Users Table -->
    <div class="bg-white rounded-lg shadow overflow-hidden">
      <table class="min-w-full divide-y divide-gray-200">
        <thead class="bg-gray-50">
          <tr>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              {$_('users.table.email')}
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              {$_('users.table.status')}
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              {$_('users.table.devices')}
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              {$_('users.table.notes')}
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              {$_('users.table.created')}
            </th>
            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
              {$_('users.table.actions')}
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
              <td class="px-6 py-4 whitespace-nowrap text-sm font-medium">
                <div class="flex flex-wrap gap-2">
                  {#if !user.approved}
                    <button
                      onclick={() => requestApprove(user.id, user.email)}
                      class="text-green-600 hover:text-green-900"
                    >
                      {$_('users.actions.approve')}
                    </button>
                  {/if}

                  {#if user.approved}
                    {#if user.isAdmin}
                      <button
                        onclick={() => requestToggleAdmin(user.id, user.email, false)}
                        class="text-purple-600 hover:text-purple-900"
                        title={$_('users.actions.demoteFromAdmin')}
                      >
                        {$_('users.actions.removeAdmin')}
                      </button>
                    {:else}
                      <button
                        onclick={() => requestToggleAdmin(user.id, user.email, true)}
                        class="text-purple-600 hover:text-purple-900"
                        title={$_('users.actions.promoteToAdmin')}
                      >
                        {$_('users.actions.makeAdmin')}
                      </button>
                    {/if}
                  {/if}

                  {#if user.isActive}
                    <button
                      onclick={() => requestDeactivate(user.id, user.email)}
                      class="text-yellow-600 hover:text-yellow-900"
                    >
                      {$_('users.actions.deactivate')}
                    </button>
                  {:else}
                    <button
                      onclick={() => requestActivate(user.id, user.email)}
                      class="text-green-600 hover:text-green-900"
                    >
                      {$_('users.actions.activate')}
                    </button>
                  {/if}

                  <button
                    onclick={() => requestDelete(user.id, user.email)}
                    class="text-red-600 hover:text-red-900"
                    title={$_('users.actions.deleteUser')}
                  >
                    {$_('users.actions.delete')}
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if confirmAction}
  {@const actionText = confirmAction.type === 'approve' ? $_('users.confirm.approve') :
                        confirmAction.type === 'deactivate' ? $_('users.confirm.deactivate') :
                        confirmAction.type === 'activate' ? $_('users.confirm.activate') :
                        confirmAction.type === 'delete' ? $_('users.confirm.delete') :
                        confirmAction.isAdmin ? $_('users.confirm.promoteToAdmin') : $_('users.confirm.demoteFromAdmin')}
  {@const message = confirmAction.type === 'delete'
    ? $_('users.confirm.messages.delete', { values: { email: confirmAction.email } })
    : confirmAction.type === 'toggleAdmin' && confirmAction.isAdmin
    ? $_('users.confirm.messages.promoteToAdmin', { values: { email: confirmAction.email } })
    : confirmAction.type === 'toggleAdmin' && !confirmAction.isAdmin
    ? $_('users.confirm.messages.demoteFromAdmin', { values: { email: confirmAction.email } })
    : $_('users.confirm.messages.genericAction', { values: { action: confirmAction.type, email: confirmAction.email } })}
  <ConfirmModal
    title={actionText}
    message={message}
    confirmText={actionText}
    cancelText={$_('common.cancel')}
    onConfirm={executeAction}
    onCancel={() => confirmAction = null}
  />
{/if}
