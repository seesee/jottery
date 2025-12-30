<script lang="ts">
  import SyncSetupForm from './SyncSetupForm.svelte';
  import SyncStatusPanel from './SyncStatusPanel.svelte';
  import type { SyncStatus } from '../../types';

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
  export let registeringUser: boolean;
  export let registeringDevice: boolean;
  export let registeredUserId: string;
  export let userRegistrationMessage: string;

  // Import credentials
  export let showImportCredentials: boolean;
  export let importCredentialsText: string;
  export let importing: boolean;

  // Copy credentials
  export let showCopiedMessage: boolean;
  export let showCredentialsModal: boolean;
  export let credentialsText: string;

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
  export let deletingNotes: boolean;

  // Callbacks
  export let onRegisterUser: () => void;
  export let onRegisterDevice: () => void;
  export let onResetRegistrationFlow: () => void;
  export let onImportCredentials: () => void;
  export let onSyncNow: () => void;
  export let onDisconnectSync: () => void;
  export let onCopySyncCredentials: () => void;
  export let onAccountLogin: () => void;
  export let onAccountLogout: () => void;
  export let onShowDeleteServerNotesConfirm: () => void;
</script>

<div class="space-y-4">
  {#if !syncStatus?.isEnabled}
    <!-- Sync Setup Form -->
    <SyncSetupForm
      bind:syncEndpoint
      bind:deviceName
      bind:registrationMode
      bind:registrationStep
      bind:userEmail
      bind:userPassword
      bind:registeringUser
      bind:registeringDevice
      bind:registeredUserId
      bind:userRegistrationMessage
      bind:showImportCredentials
      bind:importCredentialsText
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
      bind:showCopiedMessage
      bind:showCredentialsModal
      bind:credentialsText
      bind:showAccountManagement
      bind:accountEmail
      bind:accountPassword
      bind:loggingIn
      bind:userSession
      bind:accountInfo
      bind:loadingAccountInfo
      bind:deletingNotes
      {onSyncNow}
      {onDisconnectSync}
      {onCopySyncCredentials}
      {onAccountLogin}
      {onAccountLogout}
      {onShowDeleteServerNotesConfirm}
    />
  {/if}
</div>
