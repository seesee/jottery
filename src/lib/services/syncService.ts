/**
 * Sync service for synchronizing notes with remote server
 */

import type {
  SyncStatus,
  SyncPushRequest,
  SyncPullRequest,
  Note,
  SyncNoteVersion,
  SyncSavedSearch,
} from '../types';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { settingsRepository } from './settingsRepository';
import { keyManager } from './keyManager';
import { versionRepository } from './versionRepository';
import { savedSearchRepository } from './savedSearchRepository';
import { cryptoService } from './crypto';
import { storageToSync, syncToStorage } from './tagConversionService';
import { noteService } from './noteService';
import { storeConflict } from './conflictService';
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';
import { searchService } from './searchService';
import { notes, settings, isSyncRefreshing, isSyncing as isSyncingStore, syncProgress } from '../stores/appStore';
import { toast } from '../utils/toast.svelte';
import { createSyncRecoveryNote, deleteSyncRecoveryNote } from './syncRecoveryService';
import { isDBAvailable, wasDBTerminated } from './db';
import {
  normalizeEndpoint,
  registerDevice as registerDeviceApi,
  pushToServer,
  pullFromServer,
  getServerStatus,
} from './syncClient';

class SyncService {
  private isSyncing = false;
  private autoSyncTimer?: number;

  /**
   * Register a new client with the server
   */
  async register(endpoint: string, deviceName: string): Promise<{ clientId: string; apiKey: string }> {
    endpoint = normalizeEndpoint(endpoint);
    console.log('[SyncService] Registering device:', deviceName, 'at', endpoint);

    const data = await registerDeviceApi(endpoint, deviceName);

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
    endpoint = normalizeEndpoint(endpoint);
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
    // Check if sync is enabled in user settings first
    let currentSettings: { syncEnabled?: boolean } | undefined;
    settings.subscribe(s => currentSettings = s)();
    if (!currentSettings?.syncEnabled) {
      return { success: false, error: 'Sync is disabled' };
    }

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
    isSyncingStore.set(true);
    // Reset progress
    syncProgress.set({ total: 0, completed: 0 });
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
        await getServerStatus(metadata.syncEndpoint || '', apiKey);
      } catch (error) {
        console.error('[SyncService] Server status check failed:', error);
        // Continue anyway - server might be slow but still functional
      }

      // 2. Push local changes (force full sync if requested)
      const pushedCount = await this.push(metadata.syncEndpoint, apiKey, forceFullSync);

      // 3. Pull remote changes
      const pulledCount = await this.pull(metadata.syncEndpoint, apiKey);

      // 4. Update last sync timestamp
      await syncRepository.updateMetadata({
        lastSyncAt: new Date().toISOString(),
      });

      // 5. Only reload notes if we PULLED remote changes
      // When we push, the store is already up-to-date (we saved locally before syncing)
      // Refreshing on push was causing UI lag during active editing
      if (pulledCount > 0) {
        let currentSettings: any;
        settings.subscribe(s => currentSettings = s)();

        // Set flag to prevent EditorPane from triggering sync during refresh
        isSyncRefreshing.set(true);

        // Get all notes from database (already decrypted and sorted)
        const allNotes = await noteService.getAllNotes(currentSettings.sortOrder);

        // Replace store with properly sorted notes from database
        notes.set(allNotes);

        // Rebuild search index with all notes
        searchService.indexNotes(allNotes);

        console.log('[SyncService] Notes refreshed after pull:', allNotes.length);

        // Clear refresh flag
        isSyncRefreshing.set(false);
      } else if (pushedCount > 0) {
        console.log('[SyncService] Pushed', pushedCount, 'notes (no refresh needed)');
      } else {
        console.log('[SyncService] Sync complete (no changes)');
      }

      // Check for conflicts and notify user
      const conflictCount = await syncRepository.getConflictCount();
      if (conflictCount > 0) {
        const { _ } = await import('svelte-i18n');
        let getMessage: (key: string, options?: { values?: Record<string, unknown> }) => string;
        _.subscribe(t => getMessage = t)();
        const message = getMessage('conflict.syncNotification', { values: { count: conflictCount } });
        toast.warning(message);
      }

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
      isSyncingStore.set(false);
      // Reset progress after sync completes
      syncProgress.set({ total: 0, completed: 0 });
    }
  }

  /**
   * Trigger background sync without blocking
   * Safe to call from anywhere - checks all preconditions
   */
  async triggerBackgroundSync(): Promise<void> {
    // Skip if sync is disabled in settings
    let currentSettings: { syncEnabled?: boolean } | undefined;
    settings.subscribe(s => currentSettings = s)();
    if (!currentSettings?.syncEnabled) {
      return;
    }

    // Skip if sync is currently refreshing notes (prevents infinite loop)
    let isRefreshing = false;
    isSyncRefreshing.subscribe(v => isRefreshing = v)();
    if (isRefreshing) {
      return;
    }

    try {
      const metadata = await syncRepository.getMetadata();
      if (metadata?.apiKey) {
        // Don't await - let it run in background
        this.syncNow().then(result => {
          if (!result.success && result.error !== 'Sync already in progress') {
            console.warn('[SyncService] Background sync failed:', result.error);
          }
        });
      }
    } catch (error) {
      console.error('[SyncService] Failed to trigger background sync:', error);
    }
  }

  /**
   * Push local changes to server (in batches to avoid memory limits)
   * @returns Number of notes accepted by server
   */
  private async push(endpoint: string, apiKey: string, forceAll = false): Promise<number> {
    endpoint = normalizeEndpoint(endpoint);

    let modifiedNotes: Note[];
    if (forceAll) {
      // Force push ALL notes, regardless of timestamps
      modifiedNotes = await noteRepository.getAll();
    } else {
      // Only push notes that need syncing (flagged with needsSync)
      modifiedNotes = await noteRepository.getNotesNeedingSync();
    }

    if (modifiedNotes.length === 0) {
      return 0; // Nothing to push
    }

    // Get master key for tag encryption conversion
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    // Update progress with push count (pull will add to this later)
    syncProgress.update(p => ({ ...p, total: p.total + modifiedNotes.length }));

    // Push in batches to avoid JSON.stringify memory limits
    const BATCH_SIZE = 100;
    const totalBatches = Math.ceil(modifiedNotes.length / BATCH_SIZE);
    let totalAccepted = 0;
    let totalRejected = 0;

    console.log(`[SyncService] Pushing ${modifiedNotes.length} notes in ${totalBatches} batch${totalBatches > 1 ? 'es' : ''}${forceAll ? ' (force)' : ''}`);

    for (let batchIndex = 0; batchIndex < totalBatches; batchIndex++) {
      const start = batchIndex * BATCH_SIZE;
      const end = Math.min(start + BATCH_SIZE, modifiedNotes.length);
      const batchNotes = modifiedNotes.slice(start, end);

      if (totalBatches > 1) {
        console.log(`[SyncService] Pushing batch ${batchIndex + 1}/${totalBatches} (${batchNotes.length} notes)`);
      }

      // Collect attachments for this batch only
      const attachmentMap = new Map<string, string>();
      for (const note of batchNotes) {
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

      // Collect saved searches for push (only in first batch to avoid duplication)
      const savedSearchesForPush: SyncSavedSearch[] = [];
      if (batchIndex === 0) {
        const savedSearches = await savedSearchRepository.getAllNeedingSync();
        for (const search of savedSearches) {
          savedSearchesForPush.push({
            id: search.id,
            name: search.name,
            query: search.query,
            order: search.order,
            createdAt: search.createdAt,
            modifiedAt: search.modifiedAt,
            deleted: search.deleted,
            deletedAt: search.deletedAt,
            version: search.version,
          });
        }
      }

      // Collect pending deletions (from emptying trash) - only in first batch
      const pendingDeletions = batchIndex === 0 ? await noteRepository.getPendingDeletions() : [];

      // Build push request for this batch
      const pushRequest: SyncPushRequest = {
        notes: await Promise.all(batchNotes.map(async note => {
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
            archived: note.archived,
            archivedAt: note.archivedAt,
            deleted: note.deleted,
            deletedAt: note.deletedAt,
            version: note.version,
            wordWrap: note.wordWrap,
            syntaxLanguage: note.syntaxLanguage,
            showPreview: note.showPreview,
            color: note.color,
          };
        })),
        attachments: Array.from(attachmentMap.entries()).map(([id, data]) => ({ id, data })),
        versions: await this.collectVersionsForPush(batchNotes),
        savedSearches: savedSearchesForPush.length > 0 ? savedSearchesForPush : undefined,
        deletions: pendingDeletions.length > 0 ? pendingDeletions : undefined,
      };

      // Send batch to server
      const result = await pushToServer(endpoint, apiKey, pushRequest);
      totalAccepted += result.accepted.length;
      totalRejected += result.rejected.length;

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

      // Update progress after each batch
      syncProgress.update(p => ({ ...p, completed: p.completed + batchNotes.length }));

      // Mark saved searches as synced (only in first batch where they were sent)
      if (batchIndex === 0 && savedSearchesForPush.length > 0) {
        for (const search of savedSearchesForPush) {
          await savedSearchRepository.markSynced(search.id);
        }
        console.log(`[SyncService] Marked ${savedSearchesForPush.length} saved searches as synced`);
      }

      // Mark deletions as synced (only in first batch where they were sent)
      if (batchIndex === 0 && pendingDeletions.length > 0) {
        const deletionIds = pendingDeletions.map(d => d.id);
        await noteRepository.markDeletionsSynced(deletionIds);
        console.log(`[SyncService] Marked ${deletionIds.length} deletions as synced`);
      }
    }

    console.log(`[SyncService] Push complete: ${totalAccepted} accepted, ${totalRejected} rejected`);

    await syncRepository.updateMetadata({
      lastPushAt: new Date().toISOString(),
    });

    return totalAccepted;
  }

  /**
   * Pull remote changes from server (with pagination for large datasets)
   * @returns Number of notes received from server
   */
  private async pull(endpoint: string, apiKey: string): Promise<number> {
    endpoint = normalizeEndpoint(endpoint);
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

    // Get master key for tag conversion (needed for processing)
    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    // Pull in pages to avoid memory limits with large datasets
    const PULL_BATCH_SIZE = 100;
    let offset = 0;
    let hasMore = true;
    let totalNotes = 0;
    let totalAttachments = 0;
    let totalDeletions = 0;
    let totalCount = 0;

    while (hasMore) {
      const pullRequest: SyncPullRequest = {
        lastSyncAt,
        knownNoteIds: knownIds,
        knownAttachmentIds,
        limit: PULL_BATCH_SIZE,
        offset,
      };

      const result = await pullFromServer(endpoint, apiKey, pullRequest);

      // Update pagination state
      hasMore = result.hasMore ?? false;
      const previousTotalCount = totalCount;
      totalCount = result.totalCount ?? result.notes.length;
      offset += result.notes.length;

      // On first response, add pull count to progress total
      if (previousTotalCount === 0 && totalCount > 0) {
        syncProgress.update(p => ({ ...p, total: p.total + totalCount }));
      }

      // Track totals (totalNotes counts only actual changes - new notes or remote-newer updates)
      totalAttachments += result.attachments.length;
      totalDeletions += result.deletions?.length || 0;

      if (totalCount > PULL_BATCH_SIZE) {
        const page = Math.floor((offset - result.notes.length) / PULL_BATCH_SIZE) + 1;
        const totalPages = Math.ceil(totalCount / PULL_BATCH_SIZE);
        console.log(`[SyncService] Pulling page ${page}/${totalPages} (${result.notes.length} notes)`);
      }

      // Apply remote changes with Last-Write-Wins conflict resolution
      for (const remoteNote of result.notes) {
        // Convert tags from sync format to storage format
        const tagsForStorage = await syncToStorage(remoteNote.tags, masterKey.key);

        const noteForStorage = {
          ...remoteNote,
          tags: tagsForStorage,
          syncedAt: result.syncedAt,
          needsSync: false, // Pulled from server - already synced
        };

        const localNote = await noteRepository.getById(remoteNote.id);

        if (!localNote) {
          // New note from server - create locally
          await noteRepository.create(noteForStorage);
          totalNotes++; // Count as actual change

          // Update sync metadata for new note
          await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
            noteId: remoteNote.id,
            syncedAt: result.syncedAt,
            serverVersion: remoteNote.version,
            lastSyncStatus: 'synced',
          });
        } else {
          // Check for conflict: server is newer AND local has unsaved changes
          if (remoteNote.modifiedAt > localNote.modifiedAt && localNote.needsSync) {
            // Conflict detected - local note has unsaved changes and server has a newer version
            console.warn(`[SyncService] Conflict detected during pull for note ${remoteNote.id}: local has unsaved changes, server is newer`);

            // Store conflict data
            await storeConflict(remoteNote.id, {
              serverContent: remoteNote.content,
              serverTags: remoteNote.tags,
              serverModifiedAt: remoteNote.modifiedAt,
              serverVersion: remoteNote.version,
              serverAttachments: remoteNote.attachments,
              serverPinned: remoteNote.pinned,
              serverSyntaxLanguage: remoteNote.syntaxLanguage,
              serverWordWrap: remoteNote.wordWrap,
            });
            // Don't update the local note - keep it for conflict resolution
          } else if (remoteNote.modifiedAt > localNote.modifiedAt) {
            // Server version is newer and local has no unsaved changes - safe to update
            // Preserve server's modifiedAt (false), skip needsSync flag (true) since this came from server
            await noteRepository.update(noteForStorage, false, true);
            totalNotes++; // Count as actual change

            // Update sync metadata
            await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
              noteId: remoteNote.id,
              syncedAt: result.syncedAt,
              serverVersion: remoteNote.version,
              lastSyncStatus: 'synced',
            });
          }
          // Local version is newer or equal - keep local (already pushed or will be pushed)
          // NOT counted as a change - no need to refresh the store
        }
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
              showPreview: serverVersion.showPreview,
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

      // Process saved searches from server (Last-Write-Wins)
      if (result.savedSearches && result.savedSearches.length > 0) {
        const { getDB, STORES } = await import('./db');
        const db = getDB();

        for (const remoteSearch of result.savedSearches) {
          const localSearch = await savedSearchRepository.getById(remoteSearch.id);

          if (!localSearch || remoteSearch.modifiedAt > localSearch.modifiedAt) {
            // New search from server OR server version is newer - store/update locally
            const searchForStorage = {
              id: remoteSearch.id,
              name: remoteSearch.name,
              query: remoteSearch.query,
              order: remoteSearch.order,
              createdAt: remoteSearch.createdAt,
              modifiedAt: remoteSearch.modifiedAt,
              syncedAt: result.syncedAt,
              deleted: remoteSearch.deleted,
              deletedAt: remoteSearch.deletedAt,
              version: remoteSearch.version,
              needsSync: false, // Came from server - already synced
            };

            await db.put(STORES.SAVED_SEARCHES, searchForStorage);
          }
          // Local version is newer or equal - keep local
        }
        console.log(`[SyncService] Processed ${result.savedSearches.length} saved searches from server`);
      }

      // Handle hard deletions from server
      // These are notes that were permanently deleted (trash emptied) on another device
      if (result.deletions) {
        for (const deletion of result.deletions) {
          // Apply remote deletion (hard delete without re-syncing back)
          const wasDeleted = await noteRepository.applyRemoteDeletion(deletion.id);
          if (wasDeleted) {
            console.log(`[SyncService] Hard deleted note ${deletion.id} (from server)`);
          }
        }
      }

      // Update progress after each batch
      syncProgress.update(p => ({ ...p, completed: p.completed + result.notes.length }));
    }

    console.log(`[SyncService] Pulled ${totalNotes} notes, ${totalAttachments} attachments, ${totalDeletions} deletions`);

    await syncRepository.updateMetadata({
      lastPullAt: new Date().toISOString(),
    });

    return totalNotes;
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
          showPreview: version.showPreview,
          reason: version.reason,
        });
      }
    }

    return versions;
  }
}

export const syncService = new SyncService();
