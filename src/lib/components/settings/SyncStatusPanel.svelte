<script lang="ts">
  import { _ } from 'svelte-i18n';
  import AccountManagementPanel from './AccountManagementPanel.svelte';
  import type { SyncStatus } from '../../types';

  // Sync status
  export let syncStatus: SyncStatus | null;
  export let syncing: boolean;
  export let syncError: string;

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
  export let onCopySyncCredentials: (useLegacyFormat: boolean) => void;
  export let onAccountLogin: () => void;
  export let onAccountLogout: () => void;
  export let onShowDeleteServerNotesConfirm: () => void;

  // Local state for legacy format toggle
  let showLegacyOption = false;
  let useLegacyFormat = false;
</script>

<!-- Sync Enabled - Show Status & Copy Credentials -->
              <div class="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-3 space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-medium text-green-800 dark:text-green-200">
                    ✓ Sync Enabled
                  </span>
                  {#if syncStatus?.isSyncing}
                    <span class="text-xs text-green-600 dark:text-green-400">Syncing...</span>
                  {:else if syncStatus?.lastSyncAt}
                    <span class="text-xs text-green-600 dark:text-green-400">
                      Last sync: {new Date(syncStatus.lastSyncAt).toLocaleString()}
                    </span>
                  {/if}
                </div>

                <button
                  on:click={onSyncNow}
                  disabled={syncing || syncStatus?.isSyncing}
                  class="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white text-sm font-medium rounded-md transition-colors"
                >
                  {syncing || syncStatus?.isSyncing ? 'Syncing...' : '🔄 Sync Now'}
                </button>

                {#if syncStatus?.pendingNotes && syncStatus.pendingNotes > 0}
                  <p class="text-xs text-gray-600 dark:text-gray-400">
                    {syncStatus.pendingNotes} note{syncStatus.pendingNotes !== 1 ? 's' : ''} pending sync
                  </p>
                {/if}

                <div class="border-t border-green-200 dark:border-green-700 pt-3">
                  <button
                    on:click={() => onCopySyncCredentials(useLegacyFormat)}
                    class="w-full px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium rounded-md transition-colors"
                  >
                    📋 Show Credentials for Other Devices
                  </button>

                  <p class="mt-2 text-xs text-gray-600 dark:text-gray-400">
                    Click to display credentials as text. Use "Use Existing Credentials" on other devices to import.
                  </p>
                  <p class="mt-1 text-xs text-orange-600 dark:text-orange-400 font-medium">
                    ⚠️ All devices must use the SAME password to decrypt notes!
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
                    <div class="mt-2 p-2 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded text-xs">
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

<!-- Disconnect Sync Server -->
