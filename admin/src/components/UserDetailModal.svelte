<script lang="ts">
  import { api, ApiError } from '../lib/api';
  import type { UserDetail, Device } from '../lib/api';
  import { toast } from '../lib/toast.svelte';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from './ConfirmModal.svelte';

  interface Props {
    userId: string;
    onClose: () => void;
  }

  let { userId, onClose }: Props = $props();

  let user = $state<UserDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Device actions
  let confirmAction = $state<{ type: 'revoke'; device: Device } | null>(null);
  let editingDevice = $state<{ id: string; name: string } | null>(null);

  // Settings editing
  let editingSettings = $state(false);
  let settingsForm = $state({ storageQuotaMb: 0, maxUploadSizeMb: 0 });
  let savingSettings = $state(false);

  // Svelte action to focus element on mount (avoids a11y autofocus warning)
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }

  async function loadUser() {
    try {
      loading = true;
      error = null;
      user = await api.getUser(userId);
    } catch (err) {
      error = err instanceof Error ? err.message : $_('users.failedToLoad');
    } finally {
      loading = false;
    }
  }

  async function handleRevokeDevice() {
    if (!confirmAction || confirmAction.type !== 'revoke') return;

    const device = confirmAction.device;
    confirmAction = null;

    try {
      await api.revokeDevice(device.id);
      toast.success($_('devices.toast.revoked', { values: { name: device.name } }));
      await loadUser();
    } catch (err) {
      if (err instanceof ApiError) {
        toast.error($_('devices.toast.revokeFailed', { values: { error: err.message } }));
      } else {
        toast.error($_('devices.toast.revokeFailedGeneric'));
      }
    }
  }

  async function handleRenameDevice() {
    if (!editingDevice) return;

    const { id, name } = editingDevice;
    if (!name.trim()) {
      toast.error($_('devices.nameRequired'));
      return;
    }

    editingDevice = null;

    try {
      await api.renameDevice(id, name.trim());
      toast.success($_('devices.toast.renamed'));
      await loadUser();
    } catch (err) {
      if (err instanceof ApiError) {
        toast.error($_('devices.toast.renameFailed', { values: { error: err.message } }));
      } else {
        toast.error($_('devices.toast.renameFailedGeneric'));
      }
    }
  }

  function startEditingSettings() {
    if (!user) return;
    settingsForm = {
      storageQuotaMb: user.storageQuotaMb,
      maxUploadSizeMb: user.maxUploadSizeMb,
    };
    editingSettings = true;
  }

  async function handleSaveSettings() {
    if (!user) return;

    savingSettings = true;
    try {
      await api.updateUserSettings(user.id, {
        storageQuotaMb: settingsForm.storageQuotaMb,
        maxUploadSizeMb: settingsForm.maxUploadSizeMb,
      });
      toast.success($_('users.detail.settingsSaved'));
      editingSettings = false;
      await loadUser();
    } catch (err) {
      if (err instanceof ApiError) {
        toast.error($_('users.detail.settingsSaveFailed', { values: { error: err.message } }));
      } else {
        toast.error($_('users.detail.settingsSaveFailedGeneric'));
      }
    } finally {
      savingSettings = false;
    }
  }

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleDateString('en-GB', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  function formatDateTime(dateStr: string | null): string {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleString('en-GB', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function getDeviceTypeIcon(type: string): string {
    switch (type.toLowerCase()) {
      case 'web': return '🌐';
      case 'tui': return '💻';
      default: return '📱';
    }
  }

  // Load user on mount
  $effect(() => {
    loadUser();
  });
</script>

<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
  <div class="bg-white rounded-lg shadow-xl max-w-4xl w-full max-h-[90vh] overflow-hidden flex flex-col">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-gray-200 flex justify-between items-center">
      <h2 class="text-xl font-semibold text-gray-900">{$_('users.detail.title')}</h2>
      <button
        onclick={onClose}
        class="text-gray-400 hover:text-gray-600 transition-colors"
        aria-label={$_('common.close')}
      >
        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-6">
      {#if loading}
        <div class="flex items-center justify-center py-12">
          <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
        </div>
      {:else if error}
        <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {error}
        </div>
      {:else if user}
        <!-- User Info Section -->
        <div class="mb-8">
          <h3 class="text-lg font-medium text-gray-900 mb-4">{$_('users.detail.userInfo')}</h3>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <span class="text-sm text-gray-500">{$_('users.table.email')}</span>
              <p class="font-medium">{user.email}</p>
            </div>
            <div>
              <span class="text-sm text-gray-500">{$_('users.table.status')}</span>
              <p>
                {#if !user.approved}
                  <span class="px-2 py-1 text-xs font-semibold rounded-full bg-yellow-100 text-yellow-800">
                    {$_('users.status.pending')}
                  </span>
                {:else if !user.isActive}
                  <span class="px-2 py-1 text-xs font-semibold rounded-full bg-gray-100 text-gray-800">
                    {$_('users.status.inactive')}
                  </span>
                {:else if user.isAdmin}
                  <span class="px-2 py-1 text-xs font-semibold rounded-full bg-purple-100 text-purple-800">
                    {$_('users.status.admin')}
                  </span>
                {:else}
                  <span class="px-2 py-1 text-xs font-semibold rounded-full bg-green-100 text-green-800">
                    {$_('users.status.active')}
                  </span>
                {/if}
              </p>
            </div>
            <div>
              <span class="text-sm text-gray-500">{$_('users.table.created')}</span>
              <p class="font-medium">{formatDate(user.createdAt)}</p>
            </div>
            <div>
              <span class="text-sm text-gray-500">{$_('users.detail.approvedAt')}</span>
              <p class="font-medium">{formatDate(user.approvedAt)}</p>
            </div>
            <div>
              <span class="text-sm text-gray-500">{$_('users.detail.lastLogin')}</span>
              <p class="font-medium">{formatDateTime(user.lastLoginAt)}</p>
            </div>
            <div>
              <span class="text-sm text-gray-500">{$_('users.detail.lastSync')}</span>
              <p class="font-medium">{formatDateTime(user.stats.lastSyncAt)}</p>
            </div>
          </div>
        </div>

        <!-- Stats Section -->
        <div class="mb-8">
          <h3 class="text-lg font-medium text-gray-900 mb-4">{$_('users.detail.statistics')}</h3>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div class="bg-gray-50 rounded-lg p-4">
              <span class="text-sm text-gray-500">{$_('users.table.notes')}</span>
              <p class="text-2xl font-semibold text-gray-900">{user.stats.notes.count}</p>
            </div>
            <div class="bg-gray-50 rounded-lg p-4">
              <span class="text-sm text-gray-500">{$_('users.detail.attachments')}</span>
              <p class="text-2xl font-semibold text-gray-900">{user.stats.attachments.count}</p>
            </div>
            <div class="bg-gray-50 rounded-lg p-4">
              <span class="text-sm text-gray-500">{$_('users.detail.storage')}</span>
              <p class="text-2xl font-semibold text-gray-900">{formatBytes(user.stats.attachments.totalBytes)}</p>
            </div>
          </div>
        </div>

        <!-- User Settings Section -->
        <div class="mb-8">
          <div class="flex justify-between items-center mb-4">
            <h3 class="text-lg font-medium text-gray-900">{$_('users.detail.settings')}</h3>
            {#if !editingSettings}
              <button
                onclick={startEditingSettings}
                class="text-sm text-blue-600 hover:text-blue-800"
              >
                {$_('common.edit')}
              </button>
            {/if}
          </div>

          {#if editingSettings}
            <div class="bg-gray-50 rounded-lg p-4 space-y-4">
              <div>
                <label for="storageQuota" class="block text-sm font-medium text-gray-700 mb-1">
                  {$_('users.detail.quota')}
                </label>
                <div class="flex items-center space-x-2">
                  <input
                    id="storageQuota"
                    type="number"
                    bind:value={settingsForm.storageQuotaMb}
                    min="1"
                    max="100000"
                    class="w-32 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                  <span class="text-sm text-gray-500">MB</span>
                </div>
              </div>
              <div>
                <label for="maxUpload" class="block text-sm font-medium text-gray-700 mb-1">
                  {$_('users.detail.maxUploadSize')}
                </label>
                <div class="flex items-center space-x-2">
                  <input
                    id="maxUpload"
                    type="number"
                    bind:value={settingsForm.maxUploadSizeMb}
                    min="1"
                    max="1000"
                    class="w-32 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                  <span class="text-sm text-gray-500">MB</span>
                </div>
                <p class="mt-1 text-xs text-gray-500">{$_('users.detail.maxUploadSizeHelp')}</p>
              </div>
              <div class="flex space-x-2 pt-2">
                <button
                  onclick={handleSaveSettings}
                  disabled={savingSettings}
                  class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {savingSettings ? $_('common.saving') : $_('common.save')}
                </button>
                <button
                  onclick={() => editingSettings = false}
                  disabled={savingSettings}
                  class="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300"
                >
                  {$_('common.cancel')}
                </button>
              </div>
            </div>
          {:else}
            <div class="grid grid-cols-2 gap-4">
              <div class="bg-gray-50 rounded-lg p-4">
                <span class="text-sm text-gray-500">{$_('users.detail.quota')}</span>
                <p class="text-2xl font-semibold text-gray-900">{user.storageQuotaMb} MB</p>
              </div>
              <div class="bg-gray-50 rounded-lg p-4">
                <span class="text-sm text-gray-500">{$_('users.detail.maxUploadSize')}</span>
                <p class="text-2xl font-semibold text-gray-900">{user.maxUploadSizeMb} MB</p>
              </div>
            </div>
          {/if}
        </div>

        <!-- Devices Section -->
        <div>
          <h3 class="text-lg font-medium text-gray-900 mb-4">
            {$_('devices.title')}
            <span class="text-sm font-normal text-gray-500 ml-2">
              ({user.stats.devices.active} {$_('devices.active')} / {user.stats.devices.total} {$_('devices.total')})
            </span>
          </h3>

          {#if user.devices.length === 0}
            <div class="bg-gray-50 rounded-lg p-8 text-center text-gray-500">
              {$_('devices.noDevices')}
            </div>
          {:else}
            <div class="overflow-x-auto">
              <table class="min-w-full divide-y divide-gray-200">
                <thead class="bg-gray-50">
                  <tr>
                    <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">{$_('devices.name')}</th>
                    <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">{$_('devices.type')}</th>
                    <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">{$_('devices.createdAt')}</th>
                    <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">{$_('devices.lastSeen')}</th>
                    <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">{$_('devices.status')}</th>
                    <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">{$_('users.table.actions')}</th>
                  </tr>
                </thead>
                <tbody class="bg-white divide-y divide-gray-200">
                  {#each user.devices as device (device.id)}
                    <tr class="hover:bg-gray-50" class:opacity-50={!device.isActive}>
                      <td class="px-4 py-3 whitespace-nowrap">
                        {#if editingDevice?.id === device.id}
                          <input
                            type="text"
                            bind:value={editingDevice.name}
                            onkeydown={(e) => {
                              if (e.key === 'Enter') handleRenameDevice();
                              if (e.key === 'Escape') editingDevice = null;
                            }}
                            class="px-2 py-1 border border-gray-300 rounded text-sm w-full"
                            use:focusOnMount
                          />
                        {:else}
                          <span class="text-sm font-medium text-gray-900">{device.name}</span>
                        {/if}
                      </td>
                      <td class="px-4 py-3 whitespace-nowrap">
                        <span class="text-sm text-gray-600">
                          {getDeviceTypeIcon(device.type)} {device.type}
                        </span>
                      </td>
                      <td class="px-4 py-3 whitespace-nowrap text-sm text-gray-500">
                        {formatDate(device.createdAt)}
                      </td>
                      <td class="px-4 py-3 whitespace-nowrap text-sm text-gray-500">
                        {formatDateTime(device.lastSeenAt)}
                      </td>
                      <td class="px-4 py-3 whitespace-nowrap">
                        {#if device.isActive}
                          <span class="px-2 py-1 text-xs font-semibold rounded-full bg-green-100 text-green-800">
                            {$_('devices.active')}
                          </span>
                        {:else}
                          <span class="px-2 py-1 text-xs font-semibold rounded-full bg-gray-100 text-gray-800">
                            {$_('devices.inactive')}
                          </span>
                        {/if}
                      </td>
                      <td class="px-4 py-3 whitespace-nowrap text-sm">
                        <div class="flex space-x-2">
                          {#if editingDevice?.id === device.id}
                            <button
                              onclick={handleRenameDevice}
                              class="text-green-600 hover:text-green-800"
                            >
                              {$_('common.save')}
                            </button>
                            <button
                              onclick={() => editingDevice = null}
                              class="text-gray-600 hover:text-gray-800"
                            >
                              {$_('common.cancel')}
                            </button>
                          {:else}
                            <button
                              onclick={() => editingDevice = { id: device.id, name: device.name }}
                              class="text-blue-600 hover:text-blue-800"
                            >
                              {$_('devices.rename')}
                            </button>
                            {#if device.isActive}
                              <button
                                onclick={() => confirmAction = { type: 'revoke', device }}
                                class="text-red-600 hover:text-red-800"
                              >
                                {$_('devices.revoke')}
                              </button>
                            {/if}
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
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-gray-200 bg-gray-50">
      <button
        onclick={onClose}
        class="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 transition-colors"
      >
        {$_('common.close')}
      </button>
    </div>
  </div>
</div>

{#if confirmAction?.type === 'revoke'}
  <ConfirmModal
    title={$_('devices.revokeConfirmTitle')}
    message={$_('devices.revokeConfirm', { values: { name: confirmAction.device.name } })}
    confirmText={$_('devices.revoke')}
    cancelText={$_('common.cancel')}
    onConfirm={handleRevokeDevice}
    onCancel={() => confirmAction = null}
    danger={true}
  />
{/if}
