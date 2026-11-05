<script lang="ts">
  import { api } from '../lib/api';
  import { describeError } from '../lib/apiErrors';
  import { isProtectedAccount } from '../lib/userActions';
  import { auth } from '../lib/auth.svelte';
  import type { UserListItem } from '../lib/api';
  import { onMount } from 'svelte';
  import { toast } from '../lib/toast.svelte';
  import ConfirmModal from '../components/ConfirmModal.svelte';
  import UserDetailModal from '../components/UserDetailModal.svelte';
  import { _, locale } from 'svelte-i18n';

  let users = $state<UserListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state<'all' | 'pending' | 'approved' | 'inactive'>('all');

  // User detail modal state
  let selectedUserId = $state<string | null>(null);

  // Confirmation modal state
  type ConfirmAction = {
    type: 'approve' | 'deactivate' | 'activate' | 'delete' | 'toggleAdmin';
    userId: string;
    email: string;
    isAdmin?: boolean;
    noteCount?: number;
    deviceCount?: number;
  } | null;
  let confirmAction = $state<ConfirmAction>(null);

  async function loadUsers() {
    try {
      loading = true;
      error = null;
      const response = await api.listUsers();
      users = response.users;
    } catch (err) {
      error = describeError(err, $_, 'users.failedToLoad');
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

  function requestDelete(user: UserListItem) {
    confirmAction = {
      type: 'delete',
      userId: user.id,
      email: user.email,
      noteCount: user.noteCount,
      deviceCount: user.deviceCount,
    };
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
      const action = type === 'toggleAdmin' ? (isAdmin ? 'promote' : 'demote') : type;
      toast.error($_(`users.toast.failed.${action}`, { values: { error: describeError(err, $_) } }));
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

  // Below lg each row stacks into a card, so nothing needs sideways scrolling on a
  // phone. The table needs about 900px, which a laptop window with the sidebar lacks.
  const headerCell = 'px-3 xl:px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider';
  const cell = 'block lg:table-cell py-1 lg:px-3 xl:px-6 lg:py-4 lg:whitespace-nowrap';
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
    <div role="alert" class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
      {error}
    </div>
  {/if}

  <!-- Filters -->
  <div class="flex flex-wrap gap-2">
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
    <div class="bg-white rounded-lg shadow overflow-x-auto">
      <table class="block lg:table min-w-full divide-y divide-gray-200">
        <thead class="hidden lg:table-header-group bg-gray-50">
          <tr>
            <th class={headerCell}>
              {$_('users.table.email')}
            </th>
            <th class={headerCell}>
              {$_('users.table.status')}
            </th>
            <th class={headerCell}>
              {$_('users.table.devices')}
            </th>
            <th class={headerCell}>
              {$_('users.table.notes')}
            </th>
            <th class="{headerCell} hidden xl:table-cell">
              {$_('users.table.created')}
            </th>
            <th class={headerCell}>
              {$_('users.table.actions')}
            </th>
          </tr>
        </thead>
        <tbody class="block lg:table-row-group bg-white divide-y divide-gray-200">
          {#each filteredUsers as user (user.id)}
            {@const badge = getStatusBadge(user)}
            {@const isProtected = isProtectedAccount(user, users, auth.user?.email)}
            <tr class="block lg:table-row p-4 lg:p-0 hover:bg-gray-50">
              <td class={cell}>
                <button
                  onclick={() => selectedUserId = user.id}
                  class="text-sm font-medium text-blue-600 hover:text-blue-800 hover:underline break-all text-left"
                  title={$_('users.detail.viewDetails')}
                >
                  {user.email}
                </button>
              </td>
              <td class={cell}>
                <span class="px-2 py-1 inline-flex text-xs leading-5 font-semibold rounded-full {badge.class}">
                  {badge.text}
                </span>
              </td>
              <td class="{cell} text-sm">
                <span class="lg:hidden text-gray-500">{$_('users.table.devices')}:</span>
                <button
                  onclick={() => selectedUserId = user.id}
                  class="text-blue-600 hover:text-blue-800 hover:underline"
                  title={$_('users.detail.viewDevices')}
                >
                  {user.deviceCount}
                </button>
              </td>
              <td class="{cell} text-sm text-gray-500">
                <span class="lg:hidden">{$_('users.table.notes')}:</span>
                {user.noteCount}
              </td>
              <td class="{cell} lg:hidden xl:table-cell text-sm text-gray-500">
                <span class="lg:hidden">{$_('users.table.created')}:</span>
                {new Date(user.createdAt).toLocaleDateString($locale ?? undefined)}
              </td>
              <td class="{cell} text-sm font-medium">
                <div class="flex flex-wrap gap-x-3 gap-y-2 pt-2 lg:pt-0">
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
                      {#if !isProtected}
                        <button
                          onclick={() => requestToggleAdmin(user.id, user.email, false)}
                          class="text-purple-600 hover:text-purple-900"
                          title={$_('users.actions.demoteFromAdmin')}
                        >
                          {$_('users.actions.removeAdmin')}
                        </button>
                      {/if}
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

                  {#if !user.isActive}
                    <button
                      onclick={() => requestActivate(user.id, user.email)}
                      class="text-green-600 hover:text-green-900"
                    >
                      {$_('users.actions.activate')}
                    </button>
                  {:else if !isProtected}
                    <button
                      onclick={() => requestDeactivate(user.id, user.email)}
                      class="text-yellow-600 hover:text-yellow-900"
                    >
                      {$_('users.actions.deactivate')}
                    </button>
                  {/if}

                  {#if !isProtected}
                    <button
                      onclick={() => requestDelete(user)}
                      class="text-red-600 hover:text-red-900"
                      title={$_('users.actions.deleteUser')}
                    >
                      {$_('users.actions.delete')}
                    </button>
                  {/if}
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
  {@const messageKey = confirmAction.type !== 'toggleAdmin' ? confirmAction.type :
                        confirmAction.isAdmin ? 'promoteToAdmin' : 'demoteFromAdmin'}
  {@const isDelete = confirmAction.type === 'delete'}
  <ConfirmModal
    title={actionText}
    message={$_(`users.confirm.messages.${messageKey}`, { values: { email: confirmAction.email } })}
    details={isDelete
      ? $_('users.confirm.deleteCounts', { values: { notes: confirmAction.noteCount ?? 0, devices: confirmAction.deviceCount ?? 0 } })
      : undefined}
    confirmText={actionText}
    cancelText={$_('common.cancel')}
    typeToConfirm={isDelete ? { value: confirmAction.email, label: $_('users.confirm.typeEmail') } : undefined}
    danger={isDelete || confirmAction.type === 'deactivate' || (confirmAction.type === 'toggleAdmin' && !confirmAction.isAdmin)}
    onConfirm={executeAction}
    onCancel={() => confirmAction = null}
  />
{/if}

{#if selectedUserId}
  <UserDetailModal
    userId={selectedUserId}
    onClose={() => { selectedUserId = null; loadUsers(); }}
  />
{/if}
