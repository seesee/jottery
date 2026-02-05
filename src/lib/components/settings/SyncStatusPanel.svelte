<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import AccountManagementPanel from './AccountManagementPanel.svelte';
  import ConfirmModal from '../ConfirmModal.svelte';
  import type { SyncStatus } from '../../types';
  import { toast } from '../../utils/toast.svelte';

  // Sync status
  export let syncStatus: SyncStatus | null;
  export let syncing: boolean;
  export let syncError: string;

  // Connection details
  export let syncEndpoint: string = '';
  export let deviceName: string = '';

  // Account management
  export let showAccountManagement: boolean;
  export let accountEmail: string;
  export let accountPassword: string;
  export let loggingIn: boolean;
  export let userSession: { sessionId: string; email: string; isAdmin: boolean } | null;
  export let accountInfo: {
    email: string;
    noteCount: number;
    attachmentCount: number;
    storageUsedBytes: number;
    storageQuotaMb: number;
    createdAt: string;
    lastSyncAt: string | null;
  } | null;
  export let loadingAccountInfo: boolean;

  // Callbacks
  export let onSyncNow: () => void;
  export let onFullSync: () => void;
  export let onCopySyncCredentials: (useLegacyFormat: boolean) => void;
  export let onAccountLogin: () => void;
  export let onAccountLogout: () => void;
  export let onShowDeleteServerNotesConfirm: () => void;
  export let onShowDisconnectConfirm: () => void;

  // Local state for legacy format toggle
  let showLegacyOption = false;
  let useLegacyFormat = false;
  let showDangerZone = false;
  let showConnectionDetails = false;

  // Inbox token state
  let inboxTokenState: 'none' | 'active' | 'generated' = 'none';
  let generatedToken: string | null = null;
  let showInboxSection = false;
  let showRevokeConfirm = false;
  let showQuickStart = false;
  let inboxTokenLoading = false;

  // Derive user portal URL from sync endpoint
  $: userPortalUrl = syncEndpoint ? `${syncEndpoint}/user` : '';

  // Check inbox token status when user session is available
  $: if (userSession && syncEndpoint) {
    checkInboxTokenStatus();
  }

  async function checkInboxTokenStatus() {
    if (!userSession || !syncEndpoint) return;
    try {
      const response = await fetch(`${syncEndpoint}/api/v1/user/inbox-token/status`, {
        headers: { 'Authorization': `Bearer ${userSession.sessionId}` },
      });
      if (response.ok) {
        const data = await response.json();
        inboxTokenState = data.hasToken ? 'active' : 'none';
      }
    } catch (e) {
      console.error('Failed to check inbox token status:', e);
    }
  }

  async function handleGenerateToken() {
    if (!userSession || !syncEndpoint) return;
    inboxTokenLoading = true;
    try {
      const response = await fetch(`${syncEndpoint}/api/v1/user/inbox-token`, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${userSession.sessionId}` },
      });
      if (response.ok) {
        const data = await response.json();
        generatedToken = data.token;
        inboxTokenState = 'generated';
        toast.success($_('inbox.token.generated'));
      } else {
        toast.error($_('inbox.token.error.generate'));
      }
    } catch (e) {
      console.error('Failed to generate inbox token:', e);
      toast.error($_('inbox.token.error.generate'));
    } finally {
      inboxTokenLoading = false;
    }
  }

  async function handleRevokeToken() {
    if (!userSession || !syncEndpoint) return;
    showRevokeConfirm = false;
    inboxTokenLoading = true;
    try {
      const response = await fetch(`${syncEndpoint}/api/v1/user/inbox-token`, {
        method: 'DELETE',
        headers: { 'Authorization': `Bearer ${userSession.sessionId}` },
      });
      if (response.ok) {
        inboxTokenState = 'none';
        generatedToken = null;
        toast.success($_('inbox.token.revoked'));
      } else {
        toast.error($_('inbox.token.error.revoke'));
      }
    } catch (e) {
      console.error('Failed to revoke inbox token:', e);
      toast.error($_('inbox.token.error.revoke'));
    } finally {
      inboxTokenLoading = false;
    }
  }

  function handleCopyToken() {
    if (!generatedToken) return;
    navigator.clipboard.writeText(generatedToken).then(() => {
      toast.success($_('inbox.token.copied'));
    });
  }

  $: curlExample = syncEndpoint && generatedToken
    ? `curl -X POST ${syncEndpoint}/api/v1/inbox \\
  -H "Authorization: Bearer ${generatedToken}" \\
  -H "Content-Type: application/json" \\
  -d '{"content":"My note","tags":["inbox"]}'`
    : syncEndpoint
    ? `curl -X POST ${syncEndpoint}/api/v1/inbox \\
  -H "Authorization: Bearer <your-token>" \\
  -H "Content-Type: application/json" \\
  -d '{"content":"My note","tags":["inbox"]}'`
    : '';
</script>

<!-- Sync Enabled - Show Status & Copy Credentials -->
              <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-3 space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-medium text-green-800 dark:text-green-200">
                    ✓ {$_('settings.syncTab.syncEnabled')}
                  </span>
                  {#if syncStatus?.isSyncing}
                    <span class="text-xs text-green-600 dark:text-green-400">{$_('settings.syncTab.syncing')}</span>
                  {:else if syncStatus?.lastSyncAt}
                    <span class="text-xs text-green-600 dark:text-green-400">
                      {$_('settings.syncTab.lastSync')} {new Date(syncStatus.lastSyncAt).toLocaleString()}
                    </span>
                  {/if}
                </div>

                <button
                  on:click={onSyncNow}
                  disabled={syncing || syncStatus?.isSyncing}
                  class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                >
                  {syncing || syncStatus?.isSyncing ? $_('settings.syncTab.syncing') : '🔄 ' + $_('settings.syncTab.syncNow')}
                </button>

                {#if syncStatus?.pendingNotes && syncStatus.pendingNotes > 0}
                  <p class="text-xs text-gray-600 dark:text-gray-400">
                    {syncStatus.pendingNotes === 1 ? $_('settings.syncTab.notesPendingSingle', { values: { count: syncStatus.pendingNotes }}) : $_('settings.syncTab.notesPendingPlural', { values: { count: syncStatus.pendingNotes }})}
                  </p>
                {/if}

                <div class="border-t border-green-200 dark:border-green-700 pt-3">
                  <button
                    on:click={() => onCopySyncCredentials(useLegacyFormat)}
                    class="w-full px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium rounded-md transition-colors"
                  >
                    📋 {$_('settings.syncTab.showCredentials')}
                  </button>

                  <p class="mt-2 text-xs text-gray-600 dark:text-gray-400">
                    {$_('settings.syncTab.credentialsHelp')}
                  </p>
                  <p class="mt-1 text-xs text-orange-600 dark:text-orange-400 font-medium">
                    ⚠️ {$_('settings.syncTab.samePasswordWarning')}
                  </p>

                  <!-- Advanced: Legacy format option -->
                  <button
                    type="button"
                    on:click={() => showLegacyOption = !showLegacyOption}
                    class="mt-2 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 underline"
                  >
                    {showLegacyOption ? '▼' : '▶'} {$_('settings.syncCredentials.advancedOptions')}
                  </button>

                  {#if showLegacyOption}
                    <div class="mt-2 space-y-3">
                      <!-- Full Sync Option -->
                      <div class="p-2 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700 rounded">
                        <p class="text-xs text-blue-800 dark:text-blue-200 mb-2">
                          <strong>{$_('settings.syncTab.fullSync.title')}</strong><br/>
                          <span class="text-blue-600 dark:text-blue-400">
                            {$_('settings.syncTab.fullSync.description')}
                          </span>
                        </p>
                        <button
                          on:click={onFullSync}
                          disabled={syncing || syncStatus?.isSyncing}
                          class="w-full px-3 py-1.5 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-xs font-medium rounded transition-colors"
                        >
                          {syncing || syncStatus?.isSyncing ? $_('settings.syncTab.syncing') : '🔄 ' + $_('settings.syncTab.fullSync.button')}
                        </button>
                      </div>

                      <!-- Legacy Format Option -->
                      <div class="p-2 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded text-xs">
                        <label class="flex items-start gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            bind:checked={useLegacyFormat}
                            class="mt-0.5 rounded border-amber-300 text-amber-600 focus:ring-amber-500"
                          />
                          <span class="text-amber-800 dark:text-amber-200">
                            <strong>{$_('settings.syncCredentials.legacyFormat.label')}</strong><br/>
                            <span class="text-amber-600 dark:text-amber-400">
                              {$_('settings.syncCredentials.legacyFormat.warning')}
                            </span>
                          </span>
                        </label>
                      </div>
                    </div>
                  {/if}
                </div>
              </div>

<!-- Account Management -->
<AccountManagementPanel
  bind:showAccountManagement
  bind:accountEmail
  bind:accountPassword
  bind:loggingIn
  bind:userSession
  bind:accountInfo
  bind:loadingAccountInfo
  {onAccountLogin}
  {onAccountLogout}
  {onShowDeleteServerNotesConfirm}
/>

<!-- Error Display -->
            {#if syncError}
              <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3">
                <p class="text-sm text-red-700 dark:text-red-300">
                  {syncError}
                </p>
              </div>
            {/if}

<!-- Connection Details -->
<div class="border border-gray-200 dark:border-gray-700 rounded-lg">
  <button
    type="button"
    on:click={() => showConnectionDetails = !showConnectionDetails}
    class="w-full flex items-center justify-between p-3 text-left"
  >
    <span class="text-sm font-medium text-gray-900 dark:text-white">
      {$_('settings.syncTab.connectionDetails.title')}
    </span>
    <span class="text-gray-500 dark:text-gray-400">
      {showConnectionDetails ? '▼' : '▶'}
    </span>
  </button>

  {#if showConnectionDetails}
    <div class="border-t border-gray-200 dark:border-gray-700 p-3 space-y-2">
      <div>
        <span class="text-xs text-gray-500 dark:text-gray-400">{$_('settings.syncTab.connectionDetails.server')}</span>
        <p class="text-sm text-gray-900 dark:text-white font-mono break-all">{syncEndpoint || '-'}</p>
      </div>
      {#if deviceName}
        <div>
          <span class="text-xs text-gray-500 dark:text-gray-400">{$_('settings.syncTab.connectionDetails.deviceName')}</span>
          <p class="text-sm text-gray-900 dark:text-white">{deviceName}</p>
        </div>
      {/if}
      {#if syncStatus?.clientId}
        <div>
          <span class="text-xs text-gray-500 dark:text-gray-400">{$_('settings.syncTab.connectionDetails.deviceId')}</span>
          <p class="text-sm text-gray-900 dark:text-white font-mono break-all">{syncStatus.clientId}</p>
        </div>
      {/if}
    </div>
  {/if}
</div>

<!-- User Portal Link -->
{#if userPortalUrl}
  <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-3">
    <div class="flex items-center justify-between">
      <div>
        <h4 class="text-sm font-medium text-gray-900 dark:text-white">
          {$_('settings.syncTab.userPortal.title')}
        </h4>
        <p class="text-xs text-gray-600 dark:text-gray-400 mt-1">
          {$_('settings.syncTab.userPortal.description')}
        </p>
      </div>
      <a
        href={userPortalUrl}
        target="_blank"
        rel="noopener noreferrer"
        class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-md transition-colors"
      >
        {$_('settings.syncTab.userPortal.openButton')} →
      </a>
    </div>
  </div>
{/if}

<!-- Inbox API -->
{#if userSession}
<div class="border border-gray-200 dark:border-gray-700 rounded-lg">
  <button
    type="button"
    on:click={() => showInboxSection = !showInboxSection}
    class="w-full flex items-center justify-between p-3 text-left"
  >
    <span class="text-sm font-medium text-gray-900 dark:text-white">
      {$_('inbox.token.title')}
    </span>
    <div class="flex items-center gap-2">
      {#if inboxTokenState === 'active' || inboxTokenState === 'generated'}
        <span class="text-xs text-green-600 dark:text-green-400">{$_('inbox.token.active')}</span>
      {/if}
      <span class="text-gray-500 dark:text-gray-400">
        {showInboxSection ? '▼' : '▶'}
      </span>
    </div>
  </button>

  {#if showInboxSection}
    <div class="border-t border-gray-200 dark:border-gray-700 p-3 space-y-3">
      <p class="text-xs text-gray-600 dark:text-gray-400">
        {$_('inbox.token.description')}
      </p>

      {#if inboxTokenState === 'none'}
        <!-- No token — show generate button -->
        <button
          on:click={handleGenerateToken}
          disabled={inboxTokenLoading}
          class="w-full px-4 py-2 bg-amber-600 hover:bg-amber-700 disabled:bg-amber-400 text-white text-sm font-medium rounded-md transition-colors"
        >
          {inboxTokenLoading ? '...' : $_('inbox.token.generate')}
        </button>

      {:else if inboxTokenState === 'generated' && generatedToken}
        <!-- Token just generated — show it once -->
        <div class="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg p-3 space-y-2">
          <p class="text-xs font-medium text-amber-800 dark:text-amber-200">
            {$_('inbox.token.generated')}
          </p>
          <div class="flex items-center gap-2">
            <code class="flex-1 text-xs bg-gray-100 dark:bg-gray-700 px-2 py-1.5 rounded font-mono break-all select-all">
              {generatedToken}
            </code>
            <button
              on:click={handleCopyToken}
              class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-xs font-medium rounded transition-colors whitespace-nowrap"
            >
              {$_('inbox.token.copyToken')}
            </button>
          </div>
        </div>

        <!-- Quick Start -->
        <button
          type="button"
          on:click={() => showQuickStart = !showQuickStart}
          class="text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 underline"
        >
          {showQuickStart ? '▼' : '▶'} {$_('inbox.token.quickStart')}
        </button>

        {#if showQuickStart}
          <div class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-3">
            <p class="text-xs text-gray-600 dark:text-gray-400 mb-2">{$_('inbox.token.curlExample')}</p>
            <pre class="text-xs bg-gray-100 dark:bg-gray-700 p-2 rounded font-mono overflow-x-auto whitespace-pre-wrap break-all">{curlExample}</pre>
          </div>
        {/if}

        <button
          on:click={() => showRevokeConfirm = true}
          disabled={inboxTokenLoading}
          class="w-full px-4 py-2 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 text-sm font-medium rounded-md transition-colors border border-red-200 dark:border-red-700"
        >
          {$_('inbox.token.revoke')}
        </button>

      {:else}
        <!-- Token active (existing, not just generated) -->
        <div class="flex items-center gap-2">
          <span class="text-sm text-green-700 dark:text-green-400">✓ {$_('inbox.token.active')}</span>
        </div>

        <!-- Quick Start (always available when token exists) -->
        <button
          type="button"
          on:click={() => showQuickStart = !showQuickStart}
          class="text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 underline"
        >
          {showQuickStart ? '▼' : '▶'} {$_('inbox.token.quickStart')}
        </button>

        {#if showQuickStart}
          <div class="bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-3">
            <p class="text-xs text-gray-600 dark:text-gray-400 mb-2">{$_('inbox.token.curlExample')}</p>
            <pre class="text-xs bg-gray-100 dark:bg-gray-700 p-2 rounded font-mono overflow-x-auto whitespace-pre-wrap break-all">{curlExample}</pre>
          </div>
        {/if}

        <button
          on:click={() => showRevokeConfirm = true}
          disabled={inboxTokenLoading}
          class="w-full px-4 py-2 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 text-sm font-medium rounded-md transition-colors border border-red-200 dark:border-red-700"
        >
          {$_('inbox.token.revoke')}
        </button>
      {/if}
    </div>
  {/if}
</div>
{/if}

<!-- Revoke Inbox Token Confirmation -->
<ConfirmModal
  show={showRevokeConfirm}
  title={$_('inbox.token.confirm.revoke.title')}
  message={$_('inbox.token.confirm.revoke.message')}
  confirmText={$_('inbox.token.confirm.revoke.confirmButton')}
  cancelText={$_('common.cancel')}
  confirmClass="bg-red-600 hover:bg-red-700"
  onConfirm={handleRevokeToken}
  onCancel={() => showRevokeConfirm = false}
/>

<!-- Danger Zone -->
<div class="border border-gray-200 dark:border-gray-700 rounded-lg">
  <button
    type="button"
    on:click={() => showDangerZone = !showDangerZone}
    class="w-full flex items-center justify-between p-3 text-left"
  >
    <span class="text-sm font-medium text-gray-900 dark:text-white">
      {$_('settings.syncTab.dangerZone.title')}
    </span>
    <span class="text-gray-500 dark:text-gray-400">
      {showDangerZone ? '▼' : '▶'}
    </span>
  </button>

  {#if showDangerZone}
    <div class="border-t border-gray-200 dark:border-gray-700 p-3 space-y-3">
      <div class="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3">
        <h4 class="text-sm font-semibold text-red-900 dark:text-red-100 mb-2">
          {$_('settings.syncTab.dangerZone.disconnectTitle')}
        </h4>
        <p class="text-xs text-red-800 dark:text-red-200 mb-3">
          {$_('settings.syncTab.dangerZone.disconnectDescription')}
        </p>
        <button
          on:click={onShowDisconnectConfirm}
          class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-medium rounded-md transition-colors"
        >
          {$_('settings.syncTab.dangerZone.disconnectButton')}
        </button>
      </div>
    </div>
  {/if}
</div>
