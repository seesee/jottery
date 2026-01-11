/**
 * Sync service for synchronizing notes with remote server
 */

import type {
  SyncStatus,
  SyncPushRequest,
  SyncPushResponse,
  SyncPullRequest,
  SyncPullResponse,
  SyncStatusResponse,
  AuthRegisterRequest,
  AuthRegisterResponse,
  Note,
  SyncNoteVersion,
} from '../types';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { settingsRepository } from './settingsRepository';
import { keyManager } from './keyManager';
import { versionRepository } from './versionRepository';
import { cryptoService } from './crypto';
import { storageToSync, syncToStorage } from './tagConversionService';
import { noteService } from './noteService';
import { storeConflict } from './conflictService';
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';
import { searchService } from './searchService';
import { notes, settings, isSyncRefreshing } from '../stores/appStore';
import { toast } from '../utils/toast.svelte';
import { createSyncRecoveryNote, deleteSyncRecoveryNote } from './syncRecoveryService';

const API_VERSION = 'v1';

class SyncService {
  private isSyncing = false;
  private autoSyncTimer?: number;

  /**
   * Normalize endpoint URL by removing trailing slash
   */
  private normalizeEndpoint(endpoint: string): string {
    return endpoint.endsWith('/') ? endpoint.slice(0, -1) : endpoint;
  }

  /**
   * Register a new client with the server
   */
  async register(endpoint: string, deviceName: string): Promise<AuthRegisterResponse> {
    console.log('[SyncService] Starting registration...', { endpoint, deviceName });
    endpoint = this.normalizeEndpoint(endpoint);
    console.log('[SyncService] Normalized endpoint:', endpoint);

    const request: AuthRegisterRequest = {
      deviceName,
      deviceType: 'web',
    };

    console.log('[SyncService] Sending registration request...');
    const response = await fetch(`${endpoint}/api/${API_VERSION}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });

    console.log('[SyncService] Registration response:', response.status, response.statusText);

    if (!response.ok) {
      const errorText = await response.text();
      console.error('[SyncService] Registration failed:', errorText);
      throw new Error(`Registration failed: ${response.statusText} - ${errorText}`);
    }

    const data: AuthRegisterResponse = await response.json();
    console.log('[SyncService] Registration successful:', { clientId: data.clientId });

    // Encrypt and store API key
    console.log('[SyncService] Encrypting API key...');
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    const encryptedApiKey = await cryptoService.encryptText(data.apiKey, masterKey.key);
    console.log('[SyncService] API key encrypted');

    // Update sync metadata
    console.log('[SyncService] Saving sync metadata...');
    await syncRepository.updateMetadata({
      apiKey: JSON.stringify(encryptedApiKey),
      clientId: data.clientId,
      syncEnabled: true,
      syncEndpoint: endpoint,
      autoSyncInterval: 5, // Default: 5 minutes
    });

    // Update settings
    console.log('[SyncService] Updating settings...');
    await settingsRepository.update({
      syncEnabled: true,
      syncEndpoint: endpoint,
    });

    console.log('[SyncService] Registration complete!');

    // Create sync recovery note (non-blocking)
    createSyncRecoveryNote().catch(err =>
      console.warn('[SyncService] Failed to create recovery note:', err)
    );

    return data;
  }

  /**
   * Configure sync manually with existing credentials
   */
  async configureCredentials(endpoint: string, clientId: string, apiKey: string): Promise<void> {
    console.log('[SyncService] Configuring sync with manual credentials...');
    endpoint = this.normalizeEndpoint(endpoint);

    // Verify master key is available
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    // Encrypt API key before storing
    console.log('[SyncService] Encrypting API key...');
    const encryptedApiKey = await cryptoService.encryptText(apiKey, masterKey.key);

    // Save sync metadata
    console.log('[SyncService] Saving sync metadata...');
    await syncRepository.updateMetadata({
      apiKey: JSON.stringify(encryptedApiKey),
      clientId: clientId,
      syncEnabled: true,
      syncEndpoint: endpoint,
      autoSyncInterval: 5, // Default: 5 minutes
    });

    // Update settings
    console.log('[SyncService] Updating settings...');
    await settingsRepository.update({
      syncEnabled: true,
      syncEndpoint: endpoint,
    });

    console.log('[SyncService] Manual configuration complete!');

    // Create sync recovery note (non-blocking)
    createSyncRecoveryNote().catch(err =>
      console.warn('[SyncService] Failed to create recovery note:', err)
    );
  }

  /**
   * Perform full bidirectional sync
   */
  async syncNow(forceFullSync = false): Promise<{ success: boolean; error?: string }> {
    console.log('[SyncService] ========================================');
    console.log('[SyncService] syncNow() called with forceFullSync =', forceFullSync);
    console.log('[SyncService] ========================================');

    if (this.isSyncing) {
      return { success: false, error: 'Sync already in progress' };
    }

    this.isSyncing = true;
    try {
      const metadata = await syncRepository.getMetadata();
      if (!metadata || !metadata.syncEnabled || !metadata.apiKey) {
        throw new Error('Sync not configured');
      }

      // Decrypt API key
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        throw new Error('Application is locked');
      }

      const apiKeyEncrypted = JSON.parse(metadata.apiKey);
      const apiKey = await cryptoService.decryptText(apiKeyEncrypted, masterKey.key);

      // 1. Check server status (optional, but good for error detection)
      try {
        await this.getServerStatus(metadata.syncEndpoint, apiKey);
      } catch (error) {
        console.error('Server status check failed:', error);
        // Continue anyway - server might be slow but still functional
      }

      // 2. Push local changes (force full sync if requested)
      console.log('[SyncService] About to call push() with forceFullSync =', forceFullSync);
      await this.push(metadata.syncEndpoint, apiKey, forceFullSync);

      // 3. Pull remote changes
      await this.pull(metadata.syncEndpoint, apiKey);

      // 4. Update last sync timestamp
      await syncRepository.updateMetadata({
        lastSyncAt: new Date().toISOString(),
      });

      // 5. Reload notes into app state and rebuild search index (batched for performance)
      console.log('[SyncService] Reloading notes and rebuilding search index...');
      let currentSettings: any;
      settings.subscribe(s => currentSettings = s)();

      // Set flag to prevent EditorPane from triggering sync during refresh
      // (selectedNote may briefly become null if selected note isn't in first batch)
      isSyncRefreshing.set(true);

      // Use batched loading to show first notes immediately, load rest in background
      const firstBatch = await noteService.getAllNotesBatched(
        currentSettings.sortOrder,
        50,  // Show first 50 notes immediately
        (batchedNotes) => {
          // Background batches update the store incrementally
          notes.update(currentNotes => {
            const newNotes = [...currentNotes];
            batchedNotes.forEach(note => {
              const index = newNotes.findIndex(n => n.id === note.id);
              if (index !== -1) {
                newNotes[index] = note;
              } else {
                newNotes.push(note);
              }
            });
            return newNotes;
          });
          // Update search index incrementally for background batches
          batchedNotes.forEach(note => searchService.updateNote(note));
        }
      );

      // Set first batch immediately (non-blocking UI)
      notes.set(firstBatch);
      searchService.indexNotes(firstBatch);
      console.log('[SyncService] UI refreshed with first batch, loading remaining notes in background');

      // Clear flag after a short delay to allow background batches to load
      // This prevents EditorPane from triggering sync while notes are being refreshed
      setTimeout(() => isSyncRefreshing.set(false), 500);

      return { success: true };
    } catch (error) {
      console.error('Sync failed:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    } finally {
      this.isSyncing = false;
      // Ensure refresh flag is cleared on error (in case we failed before the timeout was set)
      isSyncRefreshing.set(false);
    }
  }

  /**
   * Push local changes to server
   */
  private async push(endpoint: string, apiKey: string, forceAll = false): Promise<void> {
    console.log('[SyncService] ========== PUSH START ==========');
    console.log('[SyncService] Push called with forceAll =', forceAll);
    endpoint = this.normalizeEndpoint(endpoint);
    const metadata = await syncRepository.getMetadata();
    console.log('[SyncService] Last sync timestamp:', metadata?.lastSyncAt || 'NEVER');

    let modifiedNotes: Note[];
    if (forceAll) {
      // Force push ALL notes, regardless of timestamps
      console.log('[SyncService] Force sync enabled - pushing all notes');
      modifiedNotes = await noteRepository.getAll();
      console.log('[SyncService] Retrieved ALL notes from repository:', modifiedNotes.length);
      console.log('[SyncService] Note IDs:', modifiedNotes.map(n => n.id));
    } else {
      // Only push notes that need syncing (flagged with needsSync)
      console.log('[SyncService] Getting notes that need syncing');
      modifiedNotes = await noteRepository.getNotesNeedingSync();
      console.log('[SyncService] Retrieved notes needing sync:', modifiedNotes.length);
    }

    if (modifiedNotes.length === 0) {
      console.log('[SyncService] No notes to push - exiting');
      return; // Nothing to push
    }

    console.log(`[SyncService] Pushing ${modifiedNotes.length} notes to server`);

    // Collect attachments for all modified notes
    const attachmentMap = new Map<string, string>();
    for (const note of modifiedNotes) {
      for (const attachment of note.attachments) {
        if (!attachmentMap.has(attachment.id)) {
          const blob = await attachmentRepository.getBlob(attachment.data);
          if (blob) {
            const base64 = arrayBufferToBase64(blob);
            attachmentMap.set(attachment.id, base64);
          }
        }
      }
    }

    // Get master key for tag encryption conversion
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    // Build push request - convert tags from storage format to sync format
    const pushRequest: SyncPushRequest = {
      notes: await Promise.all(modifiedNotes.map(async note => {
        // Convert tags from storage format to sync format
        const syncTags = await storageToSync(note.tags, masterKey.key);

        return {
          id: note.id,
          createdAt: note.createdAt,
          modifiedAt: note.modifiedAt,
          content: note.content,
          tags: syncTags,
          attachments: note.attachments.map(a => ({
            id: a.id,
            filename: a.filename,
            mimeType: a.mimeType,
            size: a.size,
            data: a.data,
          })),
          pinned: note.pinned,
          deleted: note.deleted,
          deletedAt: note.deletedAt,
          version: note.version,
          wordWrap: note.wordWrap,
          syntaxLanguage: note.syntaxLanguage,
        };
      })),
      attachments: Array.from(attachmentMap.entries()).map(([id, data]) => ({ id, data })),
      versions: await this.collectVersionsForPush(modifiedNotes),
    };

    // Debug log tag formats being sent
    pushRequest.notes.forEach((note, idx) => {
      console.log(`[SyncService] Push - Note[${idx}] ${note.id}: ${note.tags.length} tags`);
      if (note.tags.length > 0) {
        console.log(`[SyncService] Push - Note[${idx}] first tag format:`, note.tags[0].substring(0, 100));
      }
    });

    // Send to server
    const response = await fetch(`${endpoint}/api/${API_VERSION}/sync/push`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${apiKey}`,
      },
      body: JSON.stringify(pushRequest),
    });

    if (!response.ok) {
      const errorText = await response.text();

      // Handle specific HTTP status codes
      if (response.status === 413) {
        const errorMessage = 'Upload too large. Please reduce attachment sizes or sync fewer notes at once. Maximum size: 5MB';
        toast.error(errorMessage);
        throw new Error(errorMessage);
      }

      throw new Error(`Push failed: ${response.statusText} - ${errorText}`);
    }

    const result: SyncPushResponse = await response.json();

    console.log(`[SyncService] Push complete: ${result.accepted.length} accepted, ${result.rejected.length} rejected`);

    // Update sync metadata for accepted notes
    for (const accepted of result.accepted) {
      await syncRepository.updateNoteSyncMetadata(accepted.id, {
        noteId: accepted.id,
        syncedAt: accepted.syncedAt,
        serverVersion: accepted.serverVersion,
        lastSyncStatus: 'synced',
      });

      // Clear needsSync flag for successfully synced notes
      const note = await noteRepository.getById(accepted.id);
      if (note) {
        note.needsSync = false;
        await noteRepository.update(note, false, true); // Don't update modifiedAt, don't re-set needsSync
      }

      // Version creation is now handled separately by EditorPane idle timer
      // and on note navigation, not during sync
    }

    // Handle rejected notes (conflicts) - store server data for resolution
    for (const rejected of result.rejected) {
      console.warn(`[SyncService] Note ${rejected.id} rejected: ${rejected.reason}`);
      await storeConflict(rejected.id, {
        serverContent: rejected.serverContent,
        serverTags: rejected.serverTags,
        serverModifiedAt: rejected.serverModifiedAt,
        serverVersion: rejected.serverVersion,
        serverAttachments: rejected.serverAttachments,
        serverPinned: rejected.serverPinned,
        serverSyntaxLanguage: rejected.serverSyntaxLanguage,
        serverWordWrap: rejected.serverWordWrap,
      });

      // Clear needsSync flag to prevent infinite retry loop
      // User must resolve conflict before note can sync again
      const note = await noteRepository.getById(rejected.id);
      if (note) {
        note.needsSync = false;
        await noteRepository.update(note, false, true); // Don't update modifiedAt, don't re-set needsSync
      }
    }

    await syncRepository.updateMetadata({
      lastPushAt: new Date().toISOString(),
    });
  }

  /**
   * Pull remote changes from server
   */
  private async pull(endpoint: string, apiKey: string): Promise<void> {
    endpoint = this.normalizeEndpoint(endpoint);
    const metadata = await syncRepository.getMetadata();
    const lastSyncAt = metadata?.lastSyncAt;

    // Get all known note IDs and attachment IDs
    const allNotes = await noteRepository.getAll();
    const knownIds = allNotes.map(n => n.id);

    // Collect all known attachment IDs from local notes
    const knownAttachmentIds: string[] = [];
    for (const note of allNotes) {
      for (const attachment of note.attachments) {
        if (!knownAttachmentIds.includes(attachment.id)) {
          knownAttachmentIds.push(attachment.id);
        }
      }
    }

    const pullRequest: SyncPullRequest = {
      lastSyncAt,
      knownNoteIds: knownIds,
      knownAttachmentIds,
    };

    const response = await fetch(`${endpoint}/api/${API_VERSION}/sync/pull`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${apiKey}`,
      },
      body: JSON.stringify(pullRequest),
    });

    if (!response.ok) {
      const errorText = await response.text();

      // Handle specific HTTP status codes
      if (response.status === 413) {
        const errorMessage = 'Server response too large. Please contact your administrator.';
        toast.error(errorMessage);
        throw new Error(errorMessage);
      }

      throw new Error(`Pull failed: ${response.statusText} - ${errorText}`);
    }

    const result: SyncPullResponse = await response.json();

    console.log(`[SyncService] Pull complete: ${result.notes.length} notes, ${result.attachments.length} attachments, ${result.deletions?.length || 0} deletions`);

    // Debug log tag formats being received
    result.notes.forEach((note, idx) => {
      console.log(`[SyncService] Pull - Note[${idx}] ${note.id}: ${note.tags?.length || 0} tags`);
      if (note.tags && note.tags.length > 0) {
        console.log(`[SyncService] Pull - Note[${idx}] tags type:`, Array.isArray(note.tags) ? 'array' : typeof note.tags);
        console.log(`[SyncService] Pull - Note[${idx}] first tag:`, JSON.stringify(note.tags[0]).substring(0, 100));
      }
    });

    // Get master key for tag conversion
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    // Apply remote changes with Last-Write-Wins conflict resolution
    for (const remoteNote of result.notes) {
      // Convert tags from sync format to storage format
      const tagsForStorage = await syncToStorage(remoteNote.tags, masterKey.key);

      const noteForStorage = {
        ...remoteNote,
        tags: tagsForStorage,
        syncedAt: result.syncedAt,
      };

      const localNote = await noteRepository.getById(remoteNote.id);

      if (!localNote) {
        // New note from server - create locally
        console.log(`[SyncService] Creating new note from server: ${remoteNote.id}`);
        await noteRepository.create(noteForStorage);
      } else {
        // Conflict resolution: Last-Write-Wins by modifiedAt
        if (remoteNote.modifiedAt > localNote.modifiedAt) {
          // Server version is newer - update local
          console.log(`[SyncService] Updating note with server version (newer): ${remoteNote.id}`);
          await noteRepository.update(noteForStorage);
        } else {
          // Local version is newer or equal - keep local (already pushed or will be pushed)
          console.log(`[SyncService] Keeping local version (newer or equal): ${remoteNote.id}`);
        }
      }

      // Update sync metadata
      await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
        noteId: remoteNote.id,
        syncedAt: result.syncedAt,
        serverVersion: remoteNote.version,
        lastSyncStatus: 'synced',
      });
    }

    // Download attachments
    for (const attachment of result.attachments) {
      try {
        const blob = base64ToArrayBuffer(attachment.data);
        await attachmentRepository.storeBlob(attachment.id, blob);
        console.log(`[SyncService] Downloaded attachment: ${attachment.id}`);
      } catch (error) {
        console.error(`Failed to download attachment ${attachment.id}:`, error);
      }
    }

    // Merge versions from server
    console.log(`[SyncService] Pull - Received ${result.versions.length} versions from server`);
    for (const serverVersion of result.versions) {
      // Check if we already have this version locally
      const existingVersion = await versionRepository.getVersion(
        serverVersion.noteId,
        serverVersion.version
      );

      if (!existingVersion) {
        // New version from server - store it locally
        await versionRepository.createVersion(
          {
            id: serverVersion.noteId,
            content: serverVersion.content,
            tags: serverVersion.tags,
            attachments: serverVersion.attachments,
            version: serverVersion.version,
            syntaxLanguage: serverVersion.syntaxLanguage,
            wordWrap: serverVersion.wordWrap,
            // These fields won't be used since we're directly storing the version
            createdAt: serverVersion.createdAt,
            modifiedAt: serverVersion.createdAt,
            pinned: false,
            deleted: false,
            syncedAt: serverVersion.syncedAt,
          } as Note,
          {
            syncedAt: serverVersion.syncedAt,
            reason: serverVersion.reason as 'sync' | 'manual-sync',
          }
        );

        console.log(`[SyncService] Pull - Stored new version from server: ${serverVersion.versionKey}`);
      }
    }

    // Handle deletions
    if (result.deletions) {
      for (const deletion of result.deletions) {
        const localNote = await noteRepository.getById(deletion.id);
        if (localNote && !localNote.deleted) {
          // Server says deleted - soft delete locally
          console.log(`[SyncService] Soft deleting note from server: ${deletion.id}`);
          await noteRepository.softDelete(deletion.id);
        }
      }
    }

    await syncRepository.updateMetadata({
      lastPullAt: new Date().toISOString(),
    });
  }

  /**
   * Get server status
   */
  private async getServerStatus(
    endpoint: string,
    apiKey: string
  ): Promise<SyncStatusResponse> {
    endpoint = this.normalizeEndpoint(endpoint);
    const response = await fetch(`${endpoint}/api/${API_VERSION}/sync/status`, {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${apiKey}`,
      },
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Status check failed: ${response.statusText} - ${errorText}`);
    }

    return await response.json();
  }

  /**
   * Get current sync status for UI
   */
  async getSyncStatus(): Promise<SyncStatus> {
    const metadata = await syncRepository.getMetadata();
    const pendingNotes = await syncRepository.getPendingNotes();
    const conflictCount = await syncRepository.getConflictCount();

    return {
      isEnabled: metadata?.syncEnabled || false,
      isSyncing: this.isSyncing,
      lastSyncAt: metadata?.lastSyncAt,
      pendingNotes: pendingNotes.length,
      conflictCount,
      clientId: metadata?.clientId,
      syncEndpoint: metadata?.syncEndpoint,
    };
  }

  /**
   * Enable automatic periodic sync
   */
  enableAutoSync(intervalMinutes: number = 5): void {
    this.disableAutoSync(); // Clear any existing timer
    console.log(`[SyncService] Enabling auto-sync every ${intervalMinutes} minutes`);
    this.autoSyncTimer = window.setInterval(
      () => {
        console.log('[SyncService] Auto-sync triggered');
        this.syncNow();
      },
      intervalMinutes * 60 * 1000
    );
  }

  /**
   * Disable automatic sync
   */
  disableAutoSync(): void {
    if (this.autoSyncTimer) {
      console.log('[SyncService] Disabling auto-sync');
      clearInterval(this.autoSyncTimer);
      this.autoSyncTimer = undefined;
    }
  }

  /**
   * Check if auto-sync is enabled
   */
  isAutoSyncEnabled(): boolean {
    return this.autoSyncTimer !== undefined;
  }

  /**
   * Disconnect from sync server
   * Clears all sync credentials and metadata, but preserves local notes
   */
  async disconnect(): Promise<void> {
    console.log('[SyncService] Disconnecting from sync server...');

    // Disable auto-sync first
    this.disableAutoSync();

    // Delete recovery note (non-blocking)
    deleteSyncRecoveryNote().catch(err =>
      console.warn('[SyncService] Failed to delete recovery note:', err)
    );

    // Clear all sync metadata (credentials, client ID, etc.)
    await syncRepository.clearAll();

    // Update settings to disable sync
    await settingsRepository.update({
      syncEnabled: false,
      syncEndpoint: undefined,
    });

    console.log('[SyncService] Disconnected from sync server');
  }

  /**
   * Collect all versions for notes being pushed
   */
  private async collectVersionsForPush(notes: Note[]): Promise<SyncNoteVersion[]> {
    const versions: SyncNoteVersion[] = [];

    for (const note of notes) {
      // Get all versions for this note
      const noteVersions = await versionRepository.getVersionsForNote(note.id);

      // Convert to SyncNoteVersion format (versions are already encrypted in storage)
      for (const version of noteVersions) {
        versions.push({
          versionKey: version.versionKey,
          noteId: version.noteId,
          version: version.version,
          createdAt: version.createdAt,
          syncedAt: version.syncedAt,
          content: version.content,
          tags: version.tags,
          attachments: version.attachments,
          syntaxLanguage: version.syntaxLanguage,
          wordWrap: version.wordWrap,
          reason: version.reason,
        });
      }
    }

    console.log(`[SyncService] Push - Sending ${versions.length} versions`);
    return versions;
  }
}

export const syncService = new SyncService();
