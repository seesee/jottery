<script lang="ts">
  import { userApi, type UserAccountInfo } from '../../lib/userApi';
  import { _ } from 'svelte-i18n';
  import DevicesSection from '../../components/user/DevicesSection.svelte';

  let accountInfo = $state<UserAccountInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Inbox token state
  let inboxTokenState = $state<'none' | 'active' | 'generated'>('none');
  let generatedToken = $state<string | null>(null);
  let inboxTokenLoading = $state(false);
  let showRevokeConfirm = $state(false);
  let showQuickStart = $state(false);

  async function loadAccountInfo() {
    loading = true;
    error = null;
    try {
      accountInfo = await userApi.getAccountInfo();
      inboxTokenState = accountInfo.inbox.hasToken ? 'active' : 'none';
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

  async function handleGenerateToken() {
    inboxTokenLoading = true;
    try {
      const response = await userApi.generateInboxToken();
      generatedToken = response.token;
      inboxTokenState = 'generated';
      // Refresh account info to get updated inbox stats
      if (accountInfo) {
        accountInfo.inbox.hasToken = true;
      }
    } catch (e) {
      console.error('Failed to generate inbox token:', e);
    } finally {
      inboxTokenLoading = false;
    }
  }

  async function handleRevokeToken() {
    inboxTokenLoading = true;
    showRevokeConfirm = false;
    try {
      await userApi.revokeInboxToken();
      inboxTokenState = 'none';
      generatedToken = null;
      if (accountInfo) {
        accountInfo.inbox.hasToken = false;
      }
    } catch (e) {
      console.error('Failed to revoke inbox token:', e);
    } finally {
      inboxTokenLoading = false;
    }
  }

  async function handleCopyToken() {
    if (generatedToken) {
      await navigator.clipboard.writeText(generatedToken);
    }
  }

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

  const serverBase = import.meta.env.VITE_API_BASE_URL || window.location.origin;

  let curlExample = $derived(
    `curl -X POST ${serverBase}/api/v1/inbox \\
  -H "Authorization: Bearer ${generatedToken || '<your-token>'}" \\
  -H "Content-Type: application/json" \\
  -d '{"content":"My note","tags":["inbox"],"source":"curl"}'`
  );
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

    <!-- Inbox API Section -->
    <div class="mt-8 bg-white rounded-lg shadow p-6">
      <h3 class="text-lg font-semibold text-gray-900 mb-4">{$_('userPortal.account.inbox.title')}</h3>
      <p class="text-sm text-gray-600 mb-4">{$_('userPortal.account.inbox.description')}</p>

      <!-- Inbox Usage -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-6">
        <div class="bg-gray-50 rounded-lg p-4">
          <div class="text-sm text-gray-500">{$_('userPortal.account.inbox.itemsLabel')}</div>
          <div class="text-lg font-medium text-gray-900">
            {accountInfo.inbox.itemCount} / {accountInfo.inbox.maxItems}
          </div>
          <div class="mt-1 w-full bg-gray-200 rounded-full h-1.5">
            <div
              class="bg-amber-500 h-1.5 rounded-full"
              style="width: {Math.min(100, (accountInfo.inbox.itemCount / accountInfo.inbox.maxItems) * 100)}%"
            ></div>
          </div>
        </div>
        <div class="bg-gray-50 rounded-lg p-4">
          <div class="text-sm text-gray-500">{$_('userPortal.account.inbox.sizeLabel')}</div>
          <div class="text-lg font-medium text-gray-900">
            {formatBytes(accountInfo.inbox.totalSizeBytes)} / {accountInfo.inbox.maxSizeMb} MB
          </div>
          <div class="mt-1 w-full bg-gray-200 rounded-full h-1.5">
            <div
              class="bg-amber-500 h-1.5 rounded-full"
              style="width: {Math.min(100, (accountInfo.inbox.totalSizeBytes / (accountInfo.inbox.maxSizeMb * 1024 * 1024)) * 100)}%"
            ></div>
          </div>
        </div>
      </div>

      <!-- Token Management -->
      {#if inboxTokenState === 'none'}
        <div class="border border-gray-200 rounded-lg p-4">
          <p class="text-sm text-gray-500 mb-3">{$_('userPortal.account.inbox.noToken')}</p>
          <button
            onclick={handleGenerateToken}
            disabled={inboxTokenLoading}
            class="px-4 py-2 bg-amber-600 text-white text-sm font-medium rounded-lg hover:bg-amber-700 disabled:opacity-50"
          >
            {#if inboxTokenLoading}
              <span class="inline-block animate-spin mr-2">&#8635;</span>
            {/if}
            {$_('userPortal.account.inbox.generate')}
          </button>
        </div>
      {:else if inboxTokenState === 'generated'}
        <div class="border border-amber-200 bg-amber-50 rounded-lg p-4 space-y-4">
          <div>
            <p class="text-sm font-medium text-amber-800 mb-2">{$_('userPortal.account.inbox.generated')}</p>
            <div class="flex items-center gap-2">
              <code class="flex-1 bg-white border border-amber-300 rounded px-3 py-2 text-sm font-mono text-gray-900 break-all select-all">
                {generatedToken}
              </code>
              <button
                onclick={handleCopyToken}
                class="px-3 py-2 bg-white border border-gray-300 rounded-lg text-sm hover:bg-gray-50"
                title={$_('userPortal.account.inbox.copyToken')}
              >
                {$_('userPortal.account.inbox.copyToken')}
              </button>
            </div>
          </div>

          <!-- Quick Start -->
          <div>
            <button
              onclick={() => showQuickStart = !showQuickStart}
              class="text-sm font-medium text-amber-700 hover:text-amber-800 flex items-center gap-1"
            >
              <span class="transition-transform" class:rotate-90={showQuickStart}>&#9654;</span>
              {$_('userPortal.account.inbox.quickStart')}
            </button>
            {#if showQuickStart}
              <div class="mt-2 bg-gray-900 text-gray-100 rounded-lg p-4 overflow-x-auto">
                <pre class="text-xs whitespace-pre-wrap">{curlExample}</pre>
              </div>
            {/if}
          </div>

          <!-- Revoke -->
          <div class="pt-2 border-t border-amber-200">
            <button
              onclick={() => showRevokeConfirm = true}
              disabled={inboxTokenLoading}
              class="text-sm text-red-600 hover:text-red-700 font-medium"
            >
              {$_('userPortal.account.inbox.revoke')}
            </button>
          </div>
        </div>
      {:else}
        <!-- Active token state -->
        <div class="border border-gray-200 rounded-lg p-4 space-y-4">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="inline-block w-2 h-2 bg-green-500 rounded-full"></span>
              <span class="text-sm font-medium text-gray-700">{$_('userPortal.account.inbox.active')}</span>
            </div>
          </div>

          <!-- Quick Start -->
          <div>
            <button
              onclick={() => showQuickStart = !showQuickStart}
              class="text-sm font-medium text-gray-600 hover:text-gray-800 flex items-center gap-1"
            >
              <span class="transition-transform" class:rotate-90={showQuickStart}>&#9654;</span>
              {$_('userPortal.account.inbox.quickStart')}
            </button>
            {#if showQuickStart}
              <div class="mt-2 bg-gray-900 text-gray-100 rounded-lg p-4 overflow-x-auto">
                <pre class="text-xs whitespace-pre-wrap">{curlExample}</pre>
              </div>
            {/if}
          </div>

          <!-- Revoke -->
          <div class="pt-2 border-t border-gray-200">
            <button
              onclick={() => showRevokeConfirm = true}
              disabled={inboxTokenLoading}
              class="text-sm text-red-600 hover:text-red-700 font-medium"
            >
              {$_('userPortal.account.inbox.revoke')}
            </button>
          </div>
        </div>
      {/if}

      <!-- Revoke Confirmation Modal -->
      {#if showRevokeConfirm}
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div class="bg-white rounded-lg shadow-xl p-6 max-w-sm mx-4">
            <h4 class="text-lg font-semibold text-gray-900 mb-2">{$_('userPortal.account.inbox.revokeConfirmTitle')}</h4>
            <p class="text-sm text-gray-600 mb-4">{$_('userPortal.account.inbox.revokeConfirmMessage')}</p>
            <div class="flex justify-end gap-3">
              <button
                onclick={() => showRevokeConfirm = false}
                class="px-4 py-2 text-sm text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200"
              >
                {$_('userPortal.account.inbox.cancel')}
              </button>
              <button
                onclick={handleRevokeToken}
                class="px-4 py-2 text-sm text-white bg-red-600 rounded-lg hover:bg-red-700"
              >
                {$_('userPortal.account.inbox.revoke')}
              </button>
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>
