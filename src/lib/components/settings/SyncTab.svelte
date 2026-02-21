<script lang="ts">
  import { _ } from 'svelte-i18n';
  import SyncSetupForm from './SyncSetupForm.svelte';
  import SyncStatusPanel from './SyncStatusPanel.svelte';
  import type { SyncStatus } from '../../types';

  // Master sync enable/disable toggle
  export let syncFeatureEnabled: boolean;
  export let onToggleSyncFeature: (enabled: boolean) => void;

  // Sync configuration
  export let syncStatus: SyncStatus | null;
  export let syncEndpoint: string;
  export let deviceName: string;
  export let syncing: boolean;
  export let syncError: string;

  // Registration state
  export let registrationMode: 'select' | 'newUser' | 'existingUser';
  export let registrationStep: 'email' | 'pending' | 'device' | 'complete';
  export let userEmail: string;
  export let userPassword: string;
  export let userPasswordConfirm: string = '';
  export let registeringUser: boolean;
  export let registeringDevice: boolean;
  export let userRegistrationMessage: string;

  // Import credentials
  export let importCredentialsText: string;
  export let importDeviceName: string;
  export let importing: boolean;

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
  export let onRegisterUser: () => void;
  export let onRegisterDevice: () => void;
  export let onResetRegistrationFlow: () => void;
  export let onImportCredentials: () => void;
  export let onSyncNow: () => void;
  export let onFullSync: () => void;
  export let onCopySyncCredentials: () => void;
  export let onAccountLogin: () => void;
  export let onAccountLogout: () => void;
  export let onShowDeleteServerNotesConfirm: () => void;
  export let onShowDisconnectConfirm: () => void;
</script>

<div class="space-y-4">
  <!-- Master Sync Toggle -->
  <div class="border-b border-gray-200 dark:border-gray-700 pb-4">
    <div class="flex items-center justify-between">
      <div class="flex-1">
        <h3 class="text-sm font-semibold text-gray-900 dark:text-white mb-1">
          {$_('settings.syncTab.enableSync')}
        </h3>
        <p class="text-xs text-gray-600 dark:text-gray-400">
          {$_('settings.syncTab.syncDescription')}
        </p>
      </div>
      <label class="relative inline-flex items-center cursor-pointer">
        <input
          type="checkbox"
          checked={syncFeatureEnabled}
          on:change={(e) => onToggleSyncFeature(e.currentTarget.checked)}
          class="sr-only peer"
        />
        <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
      </label>
    </div>
  </div>

  {#if !syncFeatureEnabled}
    <!-- Sync Disabled Message -->
    <div class="bg-gray-50 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg p-4">
      <div class="flex items-start gap-3">
        <span class="text-2xl">🔒</span>
        <div class="flex-1">
          <h4 class="font-semibold text-sm text-gray-900 dark:text-white mb-2">
            {$_('settings.syncTab.syncDisabled')}
          </h4>
          <div class="text-xs text-gray-600 dark:text-gray-400 space-y-2">
            <p>
              {@html $_('settings.syncTab.syncDisabledDescription')}
            </p>
            <p>
              {$_('settings.syncTab.syncDisabledDetails')}
            </p>
            <p>
              {@html $_('settings.syncTab.toEnableSync')}
            </p>
          </div>

          <div class="mt-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700 rounded p-2">
            <p class="text-xs text-blue-800 dark:text-blue-200">
              {@html $_('settings.syncTab.privacyNote')}
            </p>
          </div>
        </div>
      </div>
    </div>
  {:else}
    <!-- Sync Enabled: Show Configuration UI -->
    {#if !syncStatus?.clientId}
      <!-- Sync Setup Form -->
      <SyncSetupForm
        bind:syncEndpoint
        bind:deviceName
        bind:registrationMode
        bind:registrationStep
        bind:userEmail
        bind:userPassword
        bind:userPasswordConfirm
        bind:registeringUser
        bind:registeringDevice
        bind:userRegistrationMessage
        bind:importCredentialsText
        bind:importDeviceName
        bind:importing
        bind:syncError
        {onRegisterUser}
        {onRegisterDevice}
        {onResetRegistrationFlow}
        {onImportCredentials}
      />
    {:else}
      <!-- Sync Status Panel -->
      <SyncStatusPanel
        bind:syncStatus
        bind:syncing
        bind:syncError
        bind:showAccountManagement
        bind:accountEmail
        bind:accountPassword
        bind:loggingIn
        bind:userSession
        bind:accountInfo
        bind:loadingAccountInfo
        {syncEndpoint}
        {deviceName}
        {onSyncNow}
        {onFullSync}
        {onCopySyncCredentials}
        {onAccountLogin}
        {onAccountLogout}
        {onShowDeleteServerNotesConfirm}
        {onShowDisconnectConfirm}
      />
    {/if}
  {/if}
</div>
