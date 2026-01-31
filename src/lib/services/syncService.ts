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
  SyncNote,
  SyncRejected,
  SyncAccepted,
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
import { SYNC_PUSH_BATCH_SIZE, SYNC_PULL_BATCH_SIZE, SYNC_AUTO_INTERVAL_MINUTES } from '../constants';
import {
  normalizeEndpoint,
  registerDevice as registerDeviceApi,
  pushToServer,
  pullFromServer,
  getServerStatus,
} from './syncClient';

// Debounce delay for background sync (30 seconds of inactivity)
const BACKGROUND_SYNC_DEBOUNCE_MS = 30000;

class SyncService {
  private isSyncing = false;
  private autoSyncTimer?: number;
  private backgroundSyncTimer?: number;
  private eventSource: EventSource | null = null;
  private sseReconnectAttempts = 0;
  private maxSseReconnectAttempts = 5;
  private sseReconnectDelay = 1000; // Start with 1 second
  private savedAutoSyncInterval?: number; // Saved interval when SSE suspends periodic sync
  private sseFailedOver = false; // True when SSE failed and we're using periodic sync as fallback
  private networkListenersAttached = false;

  /**
   * Check if the browser reports being online
   */
  private isOnline(): boolean {
    return typeof navigator !== 'undefined' ? navigator.onLine : true;
  }

  /**
   * Handle network coming back online
   */
  private handleOnline = (): void => {
    console.log('[SyncService] Network online detected');

    // If SSE had failed over to periodic sync, try to reconnect SSE
    if (this.sseFailedOver) {
      console.log('[SyncService] Attempting to restore SSE connection after network recovery');
      this.sseReconnectAttempts = 0;
      this.sseReconnectDelay = 1000;
      this.sseFailedOver = false;
      this.connectToSyncEvents();
    }

    // Trigger a sync now that we're back online
    this.syncNow();
  };

  /**
   * Handle network going offline
   */
  private handleOffline = (): void => {
    console.log('[SyncService] Network offline detected');
    // No action needed - sync attempts will fail gracefully
  };

  /**
   * Attach network status event listeners
   */
  private attachNetworkListeners(): void {
    if (this.networkListenersAttached || typeof window === 'undefined') return;

    window.addEventListener('online', this.handleOnline);
    window.addEventListener('offline', this.handleOffline);
    this.networkListenersAttached = true;
  }

  /**
   * Detach network status event listeners
   */
  private detachNetworkListeners(): void {
    if (!this.networkListenersAttached || typeof window === 'undefined') return;

    window.removeEventListener('online', this.handleOnline);
    window.removeEventListener('offline', this.handleOffline);
    this.networkListenersAttached = false;
  }

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

    // Check network availability before attempting sync
    if (!this.isOnline()) {
      console.log('[SyncService] Skipping sync - network offline');
      return { success: false, error: 'Network offline' };
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
        // Get current sort order from settings
        let settingsSnapshot: { sortOrder?: string } = {};
        settings.subscribe(s => settingsSnapshot = s)();
        const sortOrder = (settingsSnapshot.sortOrder ?? 'recent') as 'recent' | 'oldest' | 'alpha' | 'created';

        // Set flag to prevent EditorPane from triggering sync during refresh
        isSyncRefreshing.set(true);

        // Get all notes from database (already decrypted and sorted)
        const allNotes = await noteService.getAllNotes(sortOrder);

        // Replace store with properly sorted notes from database
        notes.set(allNotes);

        // Rebuild search index with all notes
        searchService.indexNotes(allNotes);

        console.log('[SyncService] Notes refreshed after pull:', allNotes.length);

        // Clear refresh flag
        isSyncRefreshing.set(false);
      } else if (pushedCount > 0) {
        // Refresh notes store to pick up updated syncedAt timestamps
        let settingsSnapshot: { sortOrder?: string } = {};
        settings.subscribe(s => settingsSnapshot = s)();
        const sortOrder = (settingsSnapshot.sortOrder ?? 'recent') as 'recent' | 'oldest' | 'alpha' | 'created';
        const allNotes = await noteService.getAllNotes(sortOrder);
        notes.set(allNotes);
        console.log('[SyncService] Pushed', pushedCount, 'notes, refreshed store');
      } else {
        console.log('[SyncService] Sync complete (no changes)');
      }

      // Check for conflicts and notify user
      const conflictCount = await syncRepository.getConflictCount();
      if (conflictCount > 0) {
        const message = conflictCount === 1
          ? '1 note has a sync conflict'
          : `${conflictCount} notes have sync conflicts`;
        toast.info(message);
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
   * Trigger background sync with debouncing
   * Safe to call from anywhere - checks all preconditions
   *
   * @param flush - If true, triggers immediate sync (use when navigating away, locking, etc.)
   *                If false (default), debounces for 30 seconds of inactivity
   */
  async triggerBackgroundSync(flush = false): Promise<void> {
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

    // Clear any existing debounce timer
    if (this.backgroundSyncTimer) {
      clearTimeout(this.backgroundSyncTimer);
      this.backgroundSyncTimer = undefined;
    }

    // If flush is requested, sync immediately
    if (flush) {
      await this.executeBackgroundSync();
      return;
    }

    // Otherwise, debounce: wait for 30 seconds of inactivity before syncing
    this.backgroundSyncTimer = window.setTimeout(() => {
      this.backgroundSyncTimer = undefined;
      this.executeBackgroundSync();
    }, BACKGROUND_SYNC_DEBOUNCE_MS);
  }

  /**
   * Cancel any pending background sync
   * Called when disabling sync or during cleanup
   */
  cancelPendingBackgroundSync(): void {
    if (this.backgroundSyncTimer) {
      clearTimeout(this.backgroundSyncTimer);
      this.backgroundSyncTimer = undefined;
    }
  }

  /**
   * Execute the actual background sync
   * Internal method - use triggerBackgroundSync() instead
   */
  private async executeBackgroundSync(): Promise<void> {
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

  // ===========================================================================
  // Push Helper Methods
  // ===========================================================================

  /** Collect attachment blobs for a batch of notes */
  private async collectAttachmentsForBatch(batchNotes: Note[]): Promise<Map<string, string>> {
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
    return attachmentMap;
  }

  /** Collect saved searches that need syncing */
  private async collectSavedSearchesForPush(): Promise<SyncSavedSearch[]> {
    const savedSearches = await savedSearchRepository.getAllNeedingSync();
    return savedSearches.map(search => ({
      id: search.id,
      name: search.name,
      query: search.query,
      order: search.order,
      createdAt: search.createdAt,
      modifiedAt: search.modifiedAt,
      deleted: search.deleted,
      deletedAt: search.deletedAt,
      version: search.version,
    }));
  }

  /** Process notes accepted by server during push */
  private async processAcceptedNotes(accepted: SyncAccepted[]): Promise<void> {
    for (const item of accepted) {
      const syncedAt = item.syncedAt ?? new Date().toISOString();
      const serverVersion = item.serverVersion ?? 1;

      await syncRepository.updateNoteSyncMetadata(item.id, {
        noteId: item.id,
        syncedAt,
        serverVersion,
        lastSyncStatus: 'synced',
      });

      const note = await noteRepository.getById(item.id);
      if (note) {
        note.syncedAt = syncedAt;
        note.needsSync = false;
        await noteRepository.update(note, false, true);
      }
    }
  }

  /** Process notes rejected by server during push (conflicts) */
  private async processRejectedNotes(rejected: SyncRejected[]): Promise<void> {
    for (const item of rejected) {
      console.warn(`[SyncService] Note ${item.id} rejected: ${item.reason}`);
      await storeConflict(item.id, {
        serverContent: item.serverContent,
        serverTags: item.serverTags,
        serverModifiedAt: item.serverModifiedAt,
        serverVersion: item.serverVersion,
        serverAttachments: item.serverAttachments,
        serverPinned: item.serverPinned,
        serverSyntaxLanguage: item.serverSyntaxLanguage,
        serverWordWrap: item.serverWordWrap,
      });

      // Clear needsSync flag to prevent infinite retry loop
      const note = await noteRepository.getById(item.id);
      if (note) {
        note.needsSync = false;
        await noteRepository.update(note, false, true);
      }
    }
  }

  /** Convert a note to sync format */
  private async convertNoteToSyncFormat(note: Note, masterKey: CryptoKey) {
    const syncTags = await storageToSync(note.tags, masterKey);
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
  }

  /** Process a single batch of notes for pushing */
  private async processPushBatch(
    batchNotes: Note[],
    batchIndex: number,
    totalBatches: number,
    endpoint: string,
    apiKey: string,
    masterKey: CryptoKey
  ): Promise<{ accepted: number; rejected: number }> {
    if (totalBatches > 1) {
      console.log(`[SyncService] Pushing batch ${batchIndex + 1}/${totalBatches} (${batchNotes.length} notes)`);
    }

    // Collect attachments and build request
    const attachmentMap = await this.collectAttachmentsForBatch(batchNotes);
    const savedSearchesForPush = batchIndex === 0 ? await this.collectSavedSearchesForPush() : [];
    const pendingDeletions = batchIndex === 0 ? await noteRepository.getPendingDeletions() : [];

    const pushRequest: SyncPushRequest = {
      notes: await Promise.all(batchNotes.map(note => this.convertNoteToSyncFormat(note, masterKey))),
      attachments: Array.from(attachmentMap.entries()).map(([id, data]) => ({ id, data })),
      versions: await this.collectVersionsForPush(batchNotes),
      savedSearches: savedSearchesForPush.length > 0 ? savedSearchesForPush : undefined,
      deletions: pendingDeletions.length > 0 ? pendingDeletions : undefined,
    };

    // Send to server and process results
    const result = await pushToServer(endpoint, apiKey, pushRequest);
    await this.processAcceptedNotes(result.accepted);
    await this.processRejectedNotes(result.rejected);

    // Update progress
    syncProgress.update(p => ({ ...p, completed: p.completed + batchNotes.length }));

    // Mark saved searches and deletions as synced (only in first batch)
    if (batchIndex === 0) {
      if (savedSearchesForPush.length > 0) {
        for (const search of savedSearchesForPush) {
          await savedSearchRepository.markSynced(search.id);
        }
        console.log(`[SyncService] Marked ${savedSearchesForPush.length} saved searches as synced`);
      }
      if (pendingDeletions.length > 0) {
        await noteRepository.markDeletionsSynced(pendingDeletions.map(d => d.id));
        console.log(`[SyncService] Marked ${pendingDeletions.length} deletions as synced`);
      }
    }

    return { accepted: result.accepted.length, rejected: result.rejected.length };
  }

  /**
   * Push local changes to server (in batches to avoid memory limits)
   * @returns Number of notes accepted by server
   */
  private async push(endpoint: string, apiKey: string, forceAll = false): Promise<number> {
    endpoint = normalizeEndpoint(endpoint);

    const modifiedNotes = forceAll
      ? await noteRepository.getAll()
      : await noteRepository.getNotesNeedingSync();

    if (modifiedNotes.length === 0) {
      return 0;
    }

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    syncProgress.update(p => ({ ...p, total: p.total + modifiedNotes.length }));

    const totalBatches = Math.ceil(modifiedNotes.length / SYNC_PUSH_BATCH_SIZE);
    let totalAccepted = 0;
    let totalRejected = 0;

    console.log(`[SyncService] Pushing ${modifiedNotes.length} notes in ${totalBatches} batch${totalBatches > 1 ? 'es' : ''}${forceAll ? ' (force)' : ''}`);

    for (let batchIndex = 0; batchIndex < totalBatches; batchIndex++) {
      const start = batchIndex * SYNC_PUSH_BATCH_SIZE;
      const batchNotes = modifiedNotes.slice(start, start + SYNC_PUSH_BATCH_SIZE);

      const { accepted, rejected } = await this.processPushBatch(
        batchNotes, batchIndex, totalBatches, endpoint, apiKey, masterKey.key
      );
      totalAccepted += accepted;
      totalRejected += rejected;
    }

    console.log(`[SyncService] Push complete: ${totalAccepted} accepted, ${totalRejected} rejected`);
    await syncRepository.updateMetadata({ lastPushAt: new Date().toISOString() });

    return totalAccepted;
  }

  // ===========================================================================
  // Pull Helper Methods
  // ===========================================================================

  /** Collect all known attachment IDs from local notes */
  private getKnownAttachmentIds(allNotes: Note[]): string[] {
    const ids: string[] = [];
    for (const note of allNotes) {
      for (const attachment of note.attachments) {
        if (!ids.includes(attachment.id)) {
          ids.push(attachment.id);
        }
      }
    }
    return ids;
  }

  /** Process a single remote note during pull (handles conflicts) */
  private async processRemoteNote(
    remoteNote: SyncNote,
    masterKey: CryptoKey,
    syncedAt: string
  ): Promise<boolean> {
    const tagsForStorage = await syncToStorage(remoteNote.tags, masterKey);
    const noteForStorage: Note = {
      id: remoteNote.id,
      createdAt: remoteNote.createdAt,
      modifiedAt: remoteNote.modifiedAt,
      content: remoteNote.content,
      tags: tagsForStorage,
      attachments: remoteNote.attachments,
      pinned: remoteNote.pinned,
      archived: remoteNote.archived,
      archivedAt: remoteNote.archivedAt,
      locked: remoteNote.locked ?? false,
      lockedAt: remoteNote.lockedAt,
      deleted: remoteNote.deleted,
      deletedAt: remoteNote.deletedAt,
      version: remoteNote.version,
      wordWrap: remoteNote.wordWrap,
      syntaxLanguage: remoteNote.syntaxLanguage,
      showPreview: remoteNote.showPreview,
      color: remoteNote.color,
      syncedAt,
      needsSync: false,
    };

    const localNote = await noteRepository.getById(remoteNote.id);

    if (!localNote) {
      // New note from server
      await noteRepository.create(noteForStorage);
      await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
        noteId: remoteNote.id,
        syncedAt,
        serverVersion: remoteNote.version,
        lastSyncStatus: 'synced',
      });
      return true;
    }

    // Check for conflict
    if (remoteNote.modifiedAt > localNote.modifiedAt && localNote.needsSync) {
      console.warn(`[SyncService] Conflict detected during pull for note ${remoteNote.id}`);
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
      return false;
    }

    if (remoteNote.modifiedAt > localNote.modifiedAt) {
      // Server is newer, safe to update
      await noteRepository.update(noteForStorage, false, true);
      await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
        noteId: remoteNote.id,
        syncedAt,
        serverVersion: remoteNote.version,
        lastSyncStatus: 'synced',
      });
      return true;
    }

    return false; // Local is newer or equal
  }

  /** Download attachments from server */
  private async downloadAttachments(attachments: Array<{ id: string; data: string }>): Promise<void> {
    for (const attachment of attachments) {
      try {
        const blob = base64ToArrayBuffer(attachment.data);
        await attachmentRepository.storeBlob(attachment.id, blob);
      } catch (error) {
        console.error(`[SyncService] Failed to download attachment ${attachment.id}:`, error);
      }
    }
  }

  /** Merge note versions from server */
  private async mergeVersionsFromServer(versions: SyncNoteVersion[]): Promise<void> {
    for (const serverVersion of versions) {
      const existingVersion = await versionRepository.getVersion(
        serverVersion.noteId,
        serverVersion.version
      );

      if (!existingVersion) {
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
  }

  /** Process saved searches from server */
  private async processSavedSearchesFromServer(
    savedSearches: SyncSavedSearch[],
    syncedAt: string
  ): Promise<void> {
    if (savedSearches.length === 0) return;

    const { getDB, STORES } = await import('./db');
    const db = getDB();

    for (const remoteSearch of savedSearches) {
      const localSearch = await savedSearchRepository.getById(remoteSearch.id);

      if (!localSearch || remoteSearch.modifiedAt > localSearch.modifiedAt) {
        await db.put(STORES.SAVED_SEARCHES, {
          id: remoteSearch.id,
          name: remoteSearch.name,
          query: remoteSearch.query,
          order: remoteSearch.order,
          createdAt: remoteSearch.createdAt,
          modifiedAt: remoteSearch.modifiedAt,
          syncedAt,
          deleted: remoteSearch.deleted,
          deletedAt: remoteSearch.deletedAt,
          version: remoteSearch.version,
          needsSync: false,
        });
      }
    }
    console.log(`[SyncService] Processed ${savedSearches.length} saved searches from server`);
  }

  /** Process hard deletions from server */
  private async processRemoteDeletions(deletions: Array<{ id: string }>): Promise<void> {
    for (const deletion of deletions) {
      const wasDeleted = await noteRepository.applyRemoteDeletion(deletion.id);
      if (wasDeleted) {
        console.log(`[SyncService] Hard deleted note ${deletion.id} (from server)`);
      }
    }
  }

  /**
   * Pull remote changes from server (with pagination for large datasets)
   * @returns Number of notes received from server
   */
  private async pull(endpoint: string, apiKey: string): Promise<number> {
    endpoint = normalizeEndpoint(endpoint);
    const metadata = await syncRepository.getMetadata();
    const allNotes = await noteRepository.getAll();

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    let offset = 0;
    let hasMore = true;
    let totalNotes = 0;
    let totalAttachments = 0;
    let totalDeletions = 0;
    let totalCount = 0;

    while (hasMore) {
      const pullRequest: SyncPullRequest = {
        lastSyncAt: metadata?.lastSyncAt,
        knownNoteIds: allNotes.map(n => n.id),
        knownAttachmentIds: this.getKnownAttachmentIds(allNotes),
        limit: SYNC_PULL_BATCH_SIZE,
        offset,
      };

      const result = await pullFromServer(endpoint, apiKey, pullRequest);
      const syncedAt = result.syncedAt ?? new Date().toISOString();

      // Update pagination state
      hasMore = result.hasMore ?? false;
      const previousTotalCount = totalCount;
      totalCount = result.totalCount ?? result.notes.length;
      offset += result.notes.length;

      if (previousTotalCount === 0 && totalCount > 0) {
        syncProgress.update(p => ({ ...p, total: p.total + totalCount }));
      }

      totalAttachments += result.attachments.length;
      totalDeletions += result.deletions?.length || 0;

      if (totalCount > SYNC_PULL_BATCH_SIZE) {
        const page = Math.floor((offset - result.notes.length) / SYNC_PULL_BATCH_SIZE) + 1;
        console.log(`[SyncService] Pulling page ${page}/${Math.ceil(totalCount / SYNC_PULL_BATCH_SIZE)} (${result.notes.length} notes)`);
      }

      // Process notes with incremental progress updates
      let processedInBatch = 0;
      for (const remoteNote of result.notes) {
        const wasChanged = await this.processRemoteNote(remoteNote, masterKey.key, syncedAt);
        if (wasChanged) totalNotes++;
        processedInBatch++;

        // Update progress every 10 notes to avoid too many store updates
        if (processedInBatch % 10 === 0) {
          syncProgress.update(p => ({ ...p, completed: p.completed + 10 }));
        }
      }

      // Update remaining progress (notes not covered by the modulo 10 updates)
      const remaining = processedInBatch % 10;
      if (remaining > 0) {
        syncProgress.update(p => ({ ...p, completed: p.completed + remaining }));
      }

      // Process other data
      await this.downloadAttachments(result.attachments);
      await this.mergeVersionsFromServer(result.versions);
      if (result.savedSearches) {
        await this.processSavedSearchesFromServer(result.savedSearches, syncedAt);
      }
      if (result.deletions) {
        await this.processRemoteDeletions(result.deletions);
      }
    }

    console.log(`[SyncService] Pulled ${totalNotes} notes, ${totalAttachments} attachments, ${totalDeletions} deletions`);
    await syncRepository.updateMetadata({ lastPullAt: new Date().toISOString() });

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
  enableAutoSync(intervalMinutes: number = SYNC_AUTO_INTERVAL_MINUTES): void {
    this.disableAutoSync(); // Clear any existing timer
    this.savedAutoSyncInterval = intervalMinutes; // Save for later restoration
    console.log(`[SyncService] Auto-sync enabled (${intervalMinutes}m interval)`);
    this.autoSyncTimer = window.setInterval(
      () => this.syncNow(),
      intervalMinutes * 60 * 1000
    );
    // Attach network listeners to handle online/offline transitions
    this.attachNetworkListeners();
  }

  /**
   * Suspend periodic sync (when SSE is active)
   * Unlike disableAutoSync, this preserves the interval for later restoration
   */
  private suspendPeriodicSync(): void {
    if (this.autoSyncTimer) {
      console.log('[SyncService] Suspending periodic sync (SSE active)');
      clearInterval(this.autoSyncTimer);
      this.autoSyncTimer = undefined;
    }
  }

  /**
   * Restore periodic sync (when SSE fails)
   */
  private restorePeriodicSync(): void {
    if (this.savedAutoSyncInterval && !this.autoSyncTimer) {
      console.log(`[SyncService] Restoring periodic sync (${this.savedAutoSyncInterval}m interval)`);
      this.autoSyncTimer = window.setInterval(
        () => this.syncNow(),
        this.savedAutoSyncInterval * 60 * 1000
      );
    }
  }

  /**
   * Disable automatic sync
   */
  disableAutoSync(): void {
    if (this.autoSyncTimer) {
      clearInterval(this.autoSyncTimer);
      this.autoSyncTimer = undefined;
    }
    this.savedAutoSyncInterval = undefined;
    this.sseFailedOver = false;
    // Also cancel any pending debounced background sync
    this.cancelPendingBackgroundSync();
    // Detach network listeners
    this.detachNetworkListeners();
  }

  /**
   * Check if auto-sync is enabled
   */
  isAutoSyncEnabled(): boolean {
    return this.autoSyncTimer !== undefined;
  }

  /**
   * Connect to SSE endpoint for real-time sync notifications
   * When another device syncs, the server sends a notification to trigger an immediate pull
   */
  async connectToSyncEvents(): Promise<void> {
    // Don't reconnect if already connected
    if (this.eventSource) {
      return;
    }

    // Check network availability first
    if (!this.isOnline()) {
      console.log('[SyncService] Skipping SSE connection - network offline');
      // Fall back to periodic sync
      this.sseFailedOver = true;
      this.restorePeriodicSync();
      return;
    }

    try {
      const metadata = await syncRepository.getMetadata();
      if (!metadata?.syncEnabled || !metadata?.apiKey || !metadata?.syncEndpoint) {
        return;
      }

      // Decrypt API key
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        return; // App is locked
      }

      const apiKeyEncrypted = JSON.parse(metadata.apiKey);
      const apiKey = await cryptoService.decryptText(apiKeyEncrypted, masterKey.key);

      // Build SSE URL with API key as query parameter
      // EventSource doesn't support custom headers, so we use query params
      const endpoint = normalizeEndpoint(metadata.syncEndpoint);
      const sseUrl = `${endpoint}/api/v1/sync/events?api_key=${encodeURIComponent(apiKey)}`;

      console.log('[SyncService] Connecting to SSE for real-time sync notifications');
      this.eventSource = new EventSource(sseUrl);

      // Reset reconnect state on successful connection
      this.eventSource.onopen = () => {
        console.log('[SyncService] SSE connection established');
        this.sseReconnectAttempts = 0;
        this.sseReconnectDelay = 1000;
        this.sseFailedOver = false;
        // Suspend periodic sync while SSE is active
        this.suspendPeriodicSync();
      };

      // Handle sync notification from server
      this.eventSource.addEventListener('sync', () => {
        console.log('[SyncService] Received sync notification from server, triggering sync');
        // Trigger immediate sync to pull changes from other devices
        this.syncNow();
      });

      // Handle connection errors
      this.eventSource.onerror = (event) => {
        console.warn('[SyncService] SSE connection error:', event);

        // EventSource automatically reconnects, but we want to limit attempts
        // and use exponential backoff
        if (this.eventSource?.readyState === EventSource.CLOSED) {
          this.sseReconnectAttempts++;

          if (this.sseReconnectAttempts <= this.maxSseReconnectAttempts) {
            console.log(`[SyncService] SSE reconnect attempt ${this.sseReconnectAttempts}/${this.maxSseReconnectAttempts} in ${this.sseReconnectDelay}ms`);

            // Close the failed connection and try again with backoff
            this.eventSource?.close();
            this.eventSource = null;

            setTimeout(() => {
              this.connectToSyncEvents();
            }, this.sseReconnectDelay);

            // Exponential backoff with max of 30 seconds
            this.sseReconnectDelay = Math.min(this.sseReconnectDelay * 2, 30000);
          } else {
            console.warn('[SyncService] SSE max reconnect attempts reached, falling back to periodic sync');
            this.eventSource?.close();
            this.eventSource = null;
            // Mark as failed over and restore periodic sync
            this.sseFailedOver = true;
            this.restorePeriodicSync();
          }
        }
      };
    } catch (error) {
      console.error('[SyncService] Failed to connect to SSE:', error);
      // Fall back to periodic sync on any error
      this.sseFailedOver = true;
      this.restorePeriodicSync();
    }
  }

  /**
   * Disconnect from SSE endpoint
   * Called when locking the app or disabling sync
   */
  disconnectFromSyncEvents(): void {
    if (this.eventSource) {
      console.log('[SyncService] Disconnecting from SSE');
      this.eventSource.close();
      this.eventSource = null;
    }
    // Reset reconnect state
    this.sseReconnectAttempts = 0;
    this.sseReconnectDelay = 1000;
  }

  /**
   * Check if SSE is connected
   */
  isSseConnected(): boolean {
    return this.eventSource !== null && this.eventSource.readyState === EventSource.OPEN;
  }

  /**
   * Disconnect from sync server
   * Clears all sync credentials and metadata, but preserves local notes
   */
  async disconnect(): Promise<void> {
    // Disable auto-sync and SSE first
    this.disableAutoSync();
    this.disconnectFromSyncEvents();

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
