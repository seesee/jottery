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
import { isDBAvailable, wasDBTerminated } from './db';

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
    endpoint = this.normalizeEndpoint(endpoint);
    console.log('[SyncService] Registering device:', deviceName, 'at', endpoint);

    const request: AuthRegisterRequest = {
      deviceName,
      deviceType: 'web',
    };

    const response = await fetch(`${endpoint}/api/${API_VERSION}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error('[SyncService] Registration failed:', errorText);
      throw new Error(`Registration failed: ${response.statusText} - ${errorText}`);
    }

    const data: AuthRegisterResponse = await response.json();

    // Encrypt and store API key
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    const encryptedApiKey = await cryptoService.encryptText(data.apiKey, masterKey.key);

    // Update sync metadata
    await syncRepository.updateMetadata({
      apiKey: JSON.stringify(encryptedApiKey),
      clientId: data.clientId,
      syncEnabled: true,
      syncEndpoint: endpoint,
      autoSyncInterval: 5, // Default: 5 minutes
    });

    // Update settings
    await settingsRepository.update({
      syncEnabled: true,
      syncEndpoint: endpoint,
    });

    console.log('[SyncService] Registration complete, clientId:', data.clientId);

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
    endpoint = this.normalizeEndpoint(endpoint);
    console.log('[SyncService] Configuring sync credentials for', endpoint);

    // Verify master key is available
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    // Encrypt API key before storing
    const encryptedApiKey = await cryptoService.encryptText(apiKey, masterKey.key);

    // Save sync metadata
    await syncRepository.updateMetadata({
      apiKey: JSON.stringify(encryptedApiKey),
      clientId: clientId,
      syncEnabled: true,
      syncEndpoint: endpoint,
      autoSyncInterval: 5, // Default: 5 minutes
    });

    // Update settings
    await settingsRepository.update({
      syncEnabled: true,
      syncEndpoint: endpoint,
    });

    console.log('[SyncService] Credentials configured successfully');

    // Create sync recovery note (non-blocking)
    createSyncRecoveryNote().catch(err =>
      console.warn('[SyncService] Failed to create recovery note:', err)
    );
  }

  /**
   * Perform full bidirectional sync
   */
  async syncNow(forceFullSync = false): Promise<{ success: boolean; error?: string }> {
    // Check if database is available before attempting sync
    if (!isDBAvailable()) {
      if (wasDBTerminated()) {
        // Database was terminated - disable auto-sync to stop the error loop
        this.disableAutoSync();
        console.error('[SyncService] Database terminated. Auto-sync disabled. Please refresh the page.');
        toast.error('Database connection lost. Please refresh the page to continue.');
        return { success: false, error: 'Database connection lost. Please refresh the page.' };
      }
      return { success: false, error: 'Database not available' };
    }

    if (this.isSyncing) {
      return { success: false, error: 'Sync already in progress' };
    }

    console.log('[SyncService] Starting sync', forceFullSync ? '(force full)' : '');
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
        console.error('[SyncService] Server status check failed:', error);
        // Continue anyway - server might be slow but still functional
      }

      // 2. Push local changes (force full sync if requested)
      await this.push(metadata.syncEndpoint, apiKey, forceFullSync);

      // 3. Pull remote changes
      await this.pull(metadata.syncEndpoint, apiKey);

      // 4. Update last sync timestamp
      await syncRepository.updateMetadata({
        lastSyncAt: new Date().toISOString(),
      });

      // 5. Reload notes into app state and rebuild search index
      // Use incremental update to preserve scroll position and selected note
      let currentSettings: any;
      settings.subscribe(s => currentSettings = s)();

      // Set flag to prevent EditorPane from triggering sync during refresh
      isSyncRefreshing.set(true);

      // Get all notes from database (already decrypted and sorted)
      const allNotes = await noteService.getAllNotes(currentSettings.sortOrder);

      // Update store incrementally - preserves UI state better than full replace
      notes.update(currentNotes => {
        // Build a map of new notes by ID for quick lookup
        const newNotesMap = new Map(allNotes.map(n => [n.id, n]));

        // Update existing notes and track which ones we've seen
        const seenIds = new Set<string>();
        const updatedNotes = currentNotes.map(note => {
          seenIds.add(note.id);
          const newNote = newNotesMap.get(note.id);
          return newNote || note; // Use new version if available
        }).filter(note => newNotesMap.has(note.id)); // Remove deleted notes

        // Add any new notes that weren't in the current list
        allNotes.forEach(note => {
          if (!seenIds.has(note.id)) {
            updatedNotes.push(note);
          }
        });

        return updatedNotes;
      });

      // Rebuild search index with all notes
      searchService.indexNotes(allNotes);
      console.log('[SyncService] Notes refreshed:', allNotes.length);

      // Clear refresh flag
      isSyncRefreshing.set(false);

      return { success: true };
    } catch (error) {
      console.error('Sync failed:', error);
      // Clear refresh flag immediately on error (no batches loading)
      isSyncRefreshing.set(false);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    } finally {
      this.isSyncing = false;
    }
  }

  /**
   * Push local changes to server
   */
  private async push(endpoint: string, apiKey: string, forceAll = false): Promise<void> {
    endpoint = this.normalizeEndpoint(endpoint);

    let modifiedNotes: Note[];
    if (forceAll) {
      // Force push ALL notes, regardless of timestamps
      modifiedNotes = await noteRepository.getAll();
    } else {
      // Only push notes that need syncing (flagged with needsSync)
      modifiedNotes = await noteRepository.getNotesNeedingSync();
    }

    if (modifiedNotes.length === 0) {
      return; // Nothing to push
    }

    console.log(`[SyncService] Pushing ${modifiedNotes.length} notes${forceAll ? ' (force)' : ''}`);

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

    console.log(`[SyncService] Pulled ${result.notes.length} notes, ${result.attachments.length} attachments, ${result.deletions?.length || 0} deletions`);

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
        await noteRepository.create(noteForStorage);
      } else {
        // Conflict resolution: Last-Write-Wins by modifiedAt
        if (remoteNote.modifiedAt > localNote.modifiedAt) {
          // Server version is newer - update local
          await noteRepository.update(noteForStorage);
        }
        // Local version is newer or equal - keep local (already pushed or will be pushed)
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
      } catch (error) {
        console.error(`[SyncService] Failed to download attachment ${attachment.id}:`, error);
      }
    }

    // Merge versions from server
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
      }
    }

    // Handle deletions
    if (result.deletions) {
      for (const deletion of result.deletions) {
        const localNote = await noteRepository.getById(deletion.id);
        if (localNote && !localNote.deleted) {
          // Server says deleted - soft delete locally
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
    console.log(`[SyncService] Auto-sync enabled (${intervalMinutes}m interval)`);
    this.autoSyncTimer = window.setInterval(
      () => this.syncNow(),
      intervalMinutes * 60 * 1000
    );
  }

  /**
   * Disable automatic sync
   */
  disableAutoSync(): void {
    if (this.autoSyncTimer) {
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

    return versions;
  }
}

export const syncService = new SyncService();
