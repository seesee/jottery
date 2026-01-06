<script lang="ts">
  import type { SyncStatus } from '../../types';

  // Sync configuration
  export let _syncStatus: SyncStatus | null;
  export let _syncEndpoint: string;
  export let _deviceName: string;
  export let _syncing: boolean;
  export let _syncError: string;

  // Registration state
  export let _registrationMode: 'select' | 'newUser' | 'existingUser';
  export let _registrationStep: 'email' | 'pending' | 'device' | 'complete';
  export let _userEmail: string;
  export let _userPassword: string;
  export let _registeringUser: boolean;
  export let _registeringDevice: boolean;
  export let _registeredUserId: string;
  export let _userRegistrationMessage: string;

  // Import credentials
  export let _showImportCredentials: boolean;
  export let _importCredentialsText: string;
  export let _importing: boolean;

  // Copy credentials
  export let _showCopiedMessage: boolean;
  export let _showCredentialsModal: boolean;
  export let _credentialsText: string;

  // Account management
  export let _showAccountManagement: boolean;
  export let _accountEmail: string;
  export let _accountPassword: string;
  export let _loggingIn: boolean;
  export let _userSession: { sessionId: string; email: string; isAdmin: boolean } | null;
  export let _accountInfo: {
    email: string;
    noteCount: number;
    attachmentCount: number;
    storageUsedBytes: number;
    storageQuotaMb: number;
    createdAt: string;
    lastSyncAt: string | null;
  } | null;
  export let _loadingAccountInfo: boolean;
  export let _deletingNotes: boolean;
  export let _showDeleteServerNotesConfirm: boolean;

  // Callbacks
  export let _onRegisterUser: () => void;
  export let _onRegisterDevice: () => void;
  export let _onResetRegistrationFlow: () => void;
  export let _onImportCredentials: () => void;
  export let _onSyncNow: () => void;
  export let _onDisconnectSync: () => void;
  export let _onCopySyncCredentials: () => void;
  export let _onAccountLogin: () => void;
  export let _onAccountLogout: () => void;
  export let _onDeleteServerNotes: () => void;
</script>
