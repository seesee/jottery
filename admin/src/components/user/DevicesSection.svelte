<script lang="ts">
  import { userApi, ApiError } from '../../lib/userApi';
  import type { Device } from '../../lib/userApi';
  import { toast } from '../../lib/toast.svelte';
  import { _ } from 'svelte-i18n';
  import ConfirmModal from '../ConfirmModal.svelte';

  let devices = $state<Device[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Revoke confirmation
  let deviceToRevoke = $state<Device | null>(null);

  async function loadDevices() {
    try {
      loading = true;
      error = null;
      devices = await userApi.getDevices();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('userPortal.devices.loadError');
    } finally {
      loading = false;
    }
  }

  async function handleRevokeDevice() {
    if (!deviceToRevoke) return;

    const device = deviceToRevoke;
    deviceToRevoke = null;

    try {
      await userApi.revokeDevice(device.id);
      toast.success($_('userPortal.devices.revoked', { values: { name: device.name } }));
      await loadDevices();
    } catch (err) {
      if (err instanceof ApiError) {
        toast.error($_('userPortal.devices.revokeFailed', { values: { error: err.message } }));
      } else {
        toast.error($_('userPortal.devices.revokeFailedGeneric'));
      }
    }
  }

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  function formatDateTime(dateStr: string | null): string {
    if (!dateStr) return $_('userPortal.devices.never');
    return new Date(dateStr).toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function getDeviceTypeIcon(type: string): string {
    switch (type.toLowerCase()) {
      case 'web': return '🌐';
      case 'tui': return '💻';
      default: return '📱';
    }
  }

  // Load devices on mount
  $effect(() => {
    loadDevices();
  });
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h3 class="text-lg font-semibold text-gray-900">{$_('userPortal.devices.title')}</h3>
    <button
      onclick={loadDevices}
      class="text-sm text-blue-600 hover:text-blue-800"
      disabled={loading}
    >
      {loading ? $_('common.refreshing') : $_('common.refresh')}
    </button>
  </div>

  {#if loading && devices.length === 0}
    <div class="flex items-center justify-center py-8">
      <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600"></div>
    </div>
  {:else if error}
    <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
      {error}
    </div>
  {:else if devices.length === 0}
    <div class="bg-gray-50 rounded-lg p-6 text-center text-gray-500">
      {$_('userPortal.devices.noDevices')}
    </div>
  {:else}
    <div class="space-y-3">
      {#each devices as device (device.id)}
        <div
          class="bg-white border rounded-lg p-4 flex items-center justify-between"
          class:border-gray-200={device.isActive}
          class:border-gray-100={!device.isActive}
          class:opacity-60={!device.isActive}
        >
          <div class="flex items-center space-x-4">
            <div class="text-2xl">
              {getDeviceTypeIcon(device.type)}
            </div>
            <div>
              <div class="flex items-center space-x-2">
                <span class="font-medium text-gray-900">{device.name}</span>
                {#if device.isActive}
                  <span class="px-2 py-0.5 text-xs font-medium rounded-full bg-green-100 text-green-800">
                    {$_('devices.active')}
                  </span>
                {:else}
                  <span class="px-2 py-0.5 text-xs font-medium rounded-full bg-gray-100 text-gray-600">
                    {$_('devices.inactive')}
                  </span>
                {/if}
              </div>
              <div class="text-xs text-gray-400 font-mono mt-0.5">{device.id}</div>
              <div class="text-sm text-gray-500 mt-1">
                <span>{device.type}</span>
                <span class="mx-2">•</span>
                <span>{$_('devices.createdAt')}: {formatDate(device.createdAt)}</span>
                <span class="mx-2">•</span>
                <span>{$_('devices.lastSeen')}: {formatDateTime(device.lastSeenAt)}</span>
              </div>
            </div>
          </div>

          {#if device.isActive}
            <button
              onclick={() => deviceToRevoke = device}
              class="px-3 py-1.5 text-sm text-red-600 hover:text-red-800 hover:bg-red-50 rounded transition-colors"
            >
              {$_('devices.revoke')}
            </button>
          {/if}
        </div>
      {/each}
    </div>

    <p class="text-sm text-gray-500 mt-4">
      {$_('userPortal.devices.revokeNote')}
    </p>
  {/if}
</div>

{#if deviceToRevoke}
  <ConfirmModal
    title={$_('userPortal.devices.revokeTitle')}
    message={$_('userPortal.devices.revokeConfirm', { values: { name: deviceToRevoke.name } })}
    confirmText={$_('devices.revoke')}
    cancelText={$_('common.cancel')}
    onConfirm={handleRevokeDevice}
    onCancel={() => deviceToRevoke = null}
    danger={true}
  />
{/if}
