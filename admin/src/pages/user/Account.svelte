<script lang="ts">
  import { userApi, type UserAccountInfo } from '../../lib/userApi';
  import { _ } from 'svelte-i18n';
  import DevicesSection from '../../components/user/DevicesSection.svelte';

  let accountInfo = $state<UserAccountInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function loadAccountInfo() {
    loading = true;
    error = null;
    try {
      accountInfo = await userApi.getAccountInfo();
    } catch (e) {
      error = $_('userPortal.account.loadError');
      console.error('Failed to load account info:', e);
    } finally {
      loading = false;
    }
  }

  // Load account info on mount
  $effect(() => {
    loadAccountInfo();
  });

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });
  }

  function formatDateTime(dateStr: string | null): string {
    if (!dateStr) return $_('userPortal.account.never');
    return new Date(dateStr).toLocaleString();
  }
</script>

<div class="space-y-6">
  <h2 class="text-2xl font-bold text-gray-900">{$_('userPortal.account.title')}</h2>

  {#if loading}
    <div class="flex items-center justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
    </div>
  {:else if error}
    <div class="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
      {error}
    </div>
  {:else if accountInfo}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
      <!-- Email -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center space-x-3 mb-2">
          <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 12a4 4 0 10-8 0 4 4 0 008 0zm0 0v1.5a2.5 2.5 0 005 0V12a9 9 0 10-9 9m4.5-1.206a8.959 8.959 0 01-4.5 1.207" />
          </svg>
          <h3 class="text-sm font-medium text-gray-500">{$_('userPortal.account.email')}</h3>
        </div>
        <p class="text-lg text-gray-900">{accountInfo.email}</p>
      </div>

      <!-- Member Since -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center space-x-3 mb-2">
          <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
          <h3 class="text-sm font-medium text-gray-500">{$_('userPortal.account.memberSince')}</h3>
        </div>
        <p class="text-lg text-gray-900">{formatDate(accountInfo.createdAt)}</p>
      </div>

      <!-- Note Count -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center space-x-3 mb-2">
          <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <h3 class="text-sm font-medium text-gray-500">{$_('userPortal.account.noteCount')}</h3>
        </div>
        <p class="text-lg text-gray-900">{accountInfo.noteCount}</p>
      </div>

      <!-- Attachment Count -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center space-x-3 mb-2">
          <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
          </svg>
          <h3 class="text-sm font-medium text-gray-500">{$_('userPortal.account.attachmentCount')}</h3>
        </div>
        <p class="text-lg text-gray-900">{accountInfo.attachmentCount}</p>
      </div>

      <!-- Storage Used -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center space-x-3 mb-2">
          <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4" />
          </svg>
          <h3 class="text-sm font-medium text-gray-500">{$_('userPortal.account.storageUsed')}</h3>
        </div>
        <p class="text-lg text-gray-900">
          {formatBytes(accountInfo.storageUsedBytes)} / {accountInfo.storageQuotaMb} MB
        </p>
        <div class="mt-2 w-full bg-gray-200 rounded-full h-2">
          <div
            class="bg-blue-600 h-2 rounded-full"
            style="width: {Math.min(100, (accountInfo.storageUsedBytes / (accountInfo.storageQuotaMb * 1024 * 1024)) * 100)}%"
          ></div>
        </div>
      </div>

      <!-- Last Sync -->
      <div class="bg-white rounded-lg shadow p-6">
        <div class="flex items-center space-x-3 mb-2">
          <svg class="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          <h3 class="text-sm font-medium text-gray-500">{$_('userPortal.account.lastSync')}</h3>
        </div>
        <p class="text-lg text-gray-900">{formatDateTime(accountInfo.lastSyncAt)}</p>
      </div>
    </div>

    <!-- Devices Section -->
    <div class="mt-8 bg-white rounded-lg shadow p-6">
      <DevicesSection />
    </div>
  {/if}
</div>
