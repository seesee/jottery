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
  SyncDeletion,
} from '../types';
import type { Attachment, AttachmentWarning, InboxItem, SyncPushResponse } from '../types';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { settingsRepository } from './settingsRepository';
import { keyManager } from './keyManager';
import { versionRepository } from './versionRepository';
import { savedSearchRepository } from './savedSearchRepository';
import { cryptoService, findCommonAncestor } from './crypto';
import { storageToSync, syncToStorage } from './tagConversionService';
import { noteService } from './noteService';
import { storeConflict } from './conflictService';
import { arrayBufferToBase64, base64ToArrayBuffer } from '../utils/base64';
import { searchService } from './searchService';
import { notes, remoteNoteUpdate, settings, isSyncRefreshing, isSyncing as isSyncingStore, syncProgress, inboxItems } from '../stores/appStore';
import { toast } from '../utils/toast.svelte';
import { recordRejectedRemoteRecord, reportServerMissingAttachments } from './vaultHealthService';
import {
  isTimestamp,
  isUuid,
  validateConflict,
  validateInboxItem,
  validatePullResponse,
  validatePushResponse,
  validateRemoteNote,
  validateRemoteVersion,
  validateSavedSearch,
} from './syncValidation';
import { isDBAvailable, wasDBTerminated } from './db';
import { SYNC_PUSH_BATCH_SIZE, SYNC_PUSH_MAX_BYTES, SYNC_PULL_BATCH_SIZE, SYNC_AUTO_INTERVAL_MINUTES } from '../constants';
import {
  normalizeEndpoint,
  registerDevice as registerDeviceApi,
  pushToServer,
  pullFromServer,
  getServerStatus,
  revokeDevice,
  deleteInboxItem,
  SyncApiError,
} from './syncClient';
import { get } from 'svelte/store';
import { _ } from 'svelte-i18n';

// Debounce delay for background sync (30 seconds of inactivity)
const BACKGROUND_SYNC_DEBOUNCE_MS = 30000;

// How long a sync can go without finishing a page or batch before we consider it stuck
const SYNC_STUCK_TIMEOUT_MS = 120000;

// How far before the last sync an older server is asked to look. It stamps a push
// before committing it, so a slow one can land behind a timestamp. Anything sent
// twice is harmless, because applying a pulled note is idempotent.
const PULL_OVERLAP_MS = 2 * 60 * 1000;

// The server refuses these for what the request holds, so a smaller one can succeed
const REFUSED_PUSH_STATUSES = new Set([400, 413]);

// The request took too long. Sending the same push twice is safe, and less of it
// is likelier to finish, but a single note that times out hasn't been refused.
const TIMED_OUT_STATUS = 408;

// How long to wait when a refusal doesn't say
const DEFAULT_RETRY_SECONDS = 60;
const TIMED_OUT_RETRY_SECONDS = 15;

// A timer can fire a moment before the time it was set for
const RETRY_MARGIN_MS = 100;

// A pull that deletes more live notes than this is held for the user to look at.
// Nothing authenticates the deletions list, so without this a compromised server
// could wipe every device. Emptying a bin elsewhere doesn't count: those notes are in
// the bin here too.
const MASS_DELETION_MINIMUM = 10;
const MASS_DELETION_SHARE = 0.25;

// How often to repeat that the account is full: this comes back on every sync
const QUOTA_NOTICE_INTERVAL_MS = 60 * 60 * 1000;

// The server skips anything dated further ahead of its clock than this
const CLOCK_TOLERANCE_MS = 24 * 60 * 60 * 1000;

/** Live notes deleted so far in one pull, and how many that may reach */
interface DeletionBudget {
  limit: number | null;
  used: number;
}

interface PushOutcome {
  accepted: number;
  rejected: number;
  warnings: AttachmentWarning[];
  refused: number;
}

/** Group items so each group stays within a size and a count. An oversized item goes alone. */
function chunkBySize<T>(items: T[], sizeOf: (item: T) => number, maxBytes: number, maxItems = Infinity): T[][] {
  const chunks: T[][] = [];
  let current: T[] = [];
  let bytes = 0;

  for (const item of items) {
    const size = sizeOf(item);
    if (current.length > 0 && (current.length >= maxItems || bytes + size > maxBytes)) {
      chunks.push(current);
      current = [];
      bytes = 0;
    }
    current.push(item);
    bytes += size;
  }

  if (current.length > 0) {
    chunks.push(current);
  }
  return chunks;
}

function withOverlap(lastSyncAt: string | undefined): string | undefined {
  if (!lastSyncAt) return undefined;
  const time = Date.parse(lastSyncAt);
  return Number.isNaN(time) ? lastSyncAt : new Date(time - PULL_OVERLAP_MS).toISOString();
}

class SyncService {
  private isSyncing = false;
  private syncStartTime = 0; // Timestamp when sync started
  private autoSyncTimer?: number;
  private backgroundSyncTimer?: number;
  private eventSource: EventSource | null = null;
  /** Notes the last pull changed, for telling the open editor */
  private pulledNoteIds: string[] = [];
  private sseReconnectAttempts = 0;
  private maxSseReconnectAttempts = 5;
  private sseReconnectDelay = 1000; // Start with 1 second
  private sseConnecting = false; // Guard against concurrent connection attempts
  private savedAutoSyncInterval?: number; // Saved interval when SSE suspends periodic sync
  private sseFailedOver = false; // True when SSE failed and we're using periodic sync as fallback
  private networkListenersAttached = false;
  private restoreInProgress = false; // True during backup restore to prevent sync
  private retryNotBefore = 0; // Set when the server asks for a wait: no sync before this time
  private retryTimer?: number;
  private quotaNoticeAt = 0;
  private clockNoticeAt = 0;
  /** Notes the last push found were deleted elsewhere, as they were sent */
  private skippedDeletedNotes = new Map<string, Note>();
  /** Times on whatever the last push sent and heard nothing back about */
  private unansweredTimes: string[] = [];

  /**
   * Set restore-in-progress flag to disable sync during backup restore
   */
  setRestoreInProgress(inProgress: boolean): void {
    this.restoreInProgress = inProgress;
    if (inProgress) {
      console.log('[SyncService] Restore in progress - sync disabled');
      // Cancel any pending background sync
      this.cancelPendingBackgroundSync();
    } else {
      console.log('[SyncService] Restore complete - sync re-enabled');
    }
  }

  /**
   * Check if restore is in progress
   */
  isRestoreInProgress(): boolean {
    return this.restoreInProgress;
  }

  /** A sync is only stuck if it stops doing anything. A big vault can take minutes. */
  private noteSyncActivity(): void {
    this.syncStartTime = Date.now();
  }

  /**
   * Check if a sync appears to be stuck (nothing done for too long)
   * and reset the state if needed
   */
  private checkAndResetStuckSync(): boolean {
    if (this.isSyncing && this.syncStartTime > 0) {
      const elapsed = Date.now() - this.syncStartTime;
      if (elapsed > SYNC_STUCK_TIMEOUT_MS) {
        console.warn(`[SyncService] Sync appears stuck (${Math.round(elapsed / 1000)}s), resetting state`);
        this.isSyncing = false;
        this.syncStartTime = 0;
        isSyncingStore.set(false);
        syncProgress.set({ total: 0, completed: 0 });
        return true;
      }
    }
    return false;
  }

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

    // Skip sync during backup restore
    if (this.restoreInProgress) {
      console.log('[SyncService] Skipping sync - restore in progress');
      return { success: false, error: 'Restore in progress' };
    }

    if (Date.now() < this.retryNotBefore) {
      console.log('[SyncService] Skipping sync - rate limited');
      return { success: false, error: 'Rate limited' };
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
        toast.error(get(_)('sync.databaseLost'));
        return { success: false, error: 'Database connection lost. Please refresh the page.' };
      }
      return { success: false, error: 'Database not available' };
    }

    // Check if a previous sync got stuck and reset if needed
    this.checkAndResetStuckSync();

    if (this.isSyncing) {
      return { success: false, error: 'Sync already in progress' };
    }

    console.log('[SyncService] Starting sync', forceFullSync ? '(force full)' : '');
    const startedAt = new Date().toISOString();
    this.isSyncing = true;
    this.syncStartTime = Date.now();
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
        console.warn('[SyncService] Server status check failed:', error);
        // Continue anyway - server might be slow but still functional
      }

      // 2. Push local changes (force full sync if requested). A push that fails must
      // not stop this device hearing about changes made elsewhere.
      this.skippedDeletedNotes.clear();
      this.unansweredTimes = [];
      let pushedCount = 0;
      let pushError: unknown;
      try {
        pushedCount = await this.push(metadata.syncEndpoint, apiKey, forceFullSync);
      } catch (error) {
        console.warn('[SyncService] Push failed, pulling anyway:', error);
        pushError = error;
      }

      // 3. Pull remote changes
      await this.finishAcceptedInboxItems(metadata.syncEndpoint, apiKey);
      const { pulledCount, serverTime } = await this.pull(metadata.syncEndpoint, apiKey);

      // 4. Update last sync timestamp. The server filters on its own clock, and on
      // when the pull began: a push landing during a long pull must not be skipped.
      await syncRepository.updateMetadata({
        lastSyncAt: serverTime ?? startedAt,
      });
      this.warnIfClockLooksWrong(serverTime ?? startedAt);

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
        const allNotes = await noteService.getNotesForList(sortOrder);

        // Replace store with properly sorted notes from database
        notes.set(allNotes);

        // Rebuild search index with all notes
        searchService.indexNotes(allNotes);

        // After the store, so the editor reloads from the new copy
        remoteNoteUpdate.set({ ids: this.pulledNoteIds });

        console.log('[SyncService] Notes refreshed after pull:', allNotes.length);

        // Clear refresh flag
        isSyncRefreshing.set(false);
      } else if (pushedCount > 0) {
        // Refresh notes store to pick up updated syncedAt timestamps
        let settingsSnapshot: { sortOrder?: string } = {};
        settings.subscribe(s => settingsSnapshot = s)();
        const sortOrder = (settingsSnapshot.sortOrder ?? 'recent') as 'recent' | 'oldest' | 'alpha' | 'created';
        const allNotes = await noteService.getNotesForList(sortOrder);
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

      if (pushError !== undefined) {
        throw pushError;
      }

      return { success: true };
    } catch (error) {
      console.error('[SyncService] Sync failed:', error);
      // Clear refresh flag immediately on error (no batches loading)
      isSyncRefreshing.set(false);
      if (error instanceof SyncApiError) {
        this.handleServerRefusal(error);
      }
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    } finally {
      this.isSyncing = false;
      this.syncStartTime = 0;
      isSyncingStore.set(false);
      // Reset progress after sync completes
      syncProgress.set({ total: 0, completed: 0 });
    }
  }

  /**
   * The server skips anything dated over a day ahead of it, and says so only in
   * `errors`, which aren't for parsing. So this looks for what it heard nothing back
   * about and compares its times with the server's clock, as the device's own clock
   * is the one in doubt.
   */
  private warnIfClockLooksWrong(serverTime: string): void {
    const limit = Date.parse(serverTime) + CLOCK_TOLERANCE_MS;
    const tooFarAhead = this.unansweredTimes.some(time => Date.parse(time) > limit);

    if (tooFarAhead && Date.now() - this.clockNoticeAt >= QUOTA_NOTICE_INTERVAL_MS) {
      this.clockNoticeAt = Date.now();
      toast.error(get(_)('sync.clockAhead'));
    }
  }

  /** Wait as asked, then sync. With live updates on, nothing else would start one. */
  private retryIn(seconds: number): void {
    this.retryNotBefore = Date.now() + seconds * 1000;
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
    }
    this.retryTimer = window.setTimeout(() => {
      this.retryTimer = undefined;
      void this.executeBackgroundSync();
    }, seconds * 1000 + RETRY_MARGIN_MS);
  }

  /**
   * A 403 means the account is deactivated or not approved, not that the device was
   * removed. The key is still valid and reactivating the account restores it, so the
   * credentials stay. Background sync stops until the app next starts, or a manual
   * sync succeeds.
   *
   * A 401 means the device was revoked, or its key is unknown. Nothing will work until
   * it's connected again, so this stops too.
   *
   * A 507 means the account is full. Nothing was written, and pulling and deleting
   * still work, so sync carries on and the user is told to make room.
   */
  private handleServerRefusal(error: SyncApiError): void {
    switch (error.status) {
      case 401:
        this.disableAutoSync();
        this.disconnectFromSyncEvents();
        toast.error(get(_)('sync.deviceRevoked'));
        break;
      case 403:
        this.disableAutoSync();
        this.disconnectFromSyncEvents();
        toast.error(get(_)('sync.accountDisabled'));
        break;
      case TIMED_OUT_STATUS:
        this.retryIn(error.retryAfterSeconds ?? TIMED_OUT_RETRY_SECONDS);
        break;
      case 429:
      case 503:
        this.retryIn(error.retryAfterSeconds ?? DEFAULT_RETRY_SECONDS);
        break;
      case 507:
        if (Date.now() - this.quotaNoticeAt >= QUOTA_NOTICE_INTERVAL_MS) {
          this.quotaNoticeAt = Date.now();
          toast.error(get(_)('sync.storageQuotaExceeded'));
        }
        break;
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

    // Skip sync during backup restore
    if (this.restoreInProgress) {
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
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
      this.retryTimer = undefined;
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

  /** Attachments of a note the server already has. A forced push sends everything again. */
  private async attachmentsOnServer(note: Note, forceAll: boolean): Promise<Set<string>> {
    if (forceAll || note.attachments.length === 0) {
      return new Set();
    }
    const metadata = await syncRepository.getNoteSyncMetadata(note.id);
    return new Set(metadata?.uploadedAttachmentIds ?? []);
  }

  /**
   * Collect the blobs a batch of notes needs to send. Most edits don't touch an
   * attachment, and re-sending them on every save could by itself exceed the upload
   * limit. If the server has lost one, it says so in attachmentWarnings.
   */
  private async collectAttachmentsForBatch(batchNotes: Note[], forceAll: boolean): Promise<Map<string, string>> {
    const attachmentMap = new Map<string, string>();
    for (const note of batchNotes) {
      const onServer = await this.attachmentsOnServer(note, forceAll);
      for (const attachment of note.attachments) {
        if (!attachmentMap.has(attachment.id) && !onServer.has(attachment.id)) {
          const blob = await attachmentRepository.getBlob(attachment.data);
          if (blob) {
            const base64 = arrayBufferToBase64(blob);
            attachmentMap.set(attachment.id, base64);
          } else {
            // Never skip silently: the note still pushes without the blob and
            // the server warns back if it has no copy either
            console.warn(`[SyncService] Attachment ${attachment.id} (note ${note.id}) has no local blob — pushing note without it`);
          }
        }
      }
    }
    return attachmentMap;
  }

  /** About what a note adds to a push, counted the way the server counts it */
  private async estimatePushBytes(note: Note, forceAll: boolean): Promise<number> {
    const onServer = await this.attachmentsOnServer(note, forceAll);
    return note.attachments
      .filter(attachment => !onServer.has(attachment.id))
      .reduce((bytes, attachment) => bytes + Math.ceil(attachment.size / 3) * 4, note.content.length);
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

  /**
   * Whether a note is exactly as it was pushed. An edit saved while the request was
   * in flight isn't on the server, so it must stay marked for the next sync.
   */
  private isUnchangedSincePush(note: Note, pushed: Note | undefined): boolean {
    return pushed !== undefined
      && note.modifiedAt === pushed.modifiedAt
      && note.contentHash === pushed.contentHash;
  }

  /** Process notes accepted by server during push */
  private async processAcceptedNotes(
    accepted: SyncAccepted[],
    pushedNotes: Map<string, Note>,
    sentBlobs: Set<string>
  ): Promise<void> {
    for (const item of accepted) {
      if (!isUuid(item?.id)) continue;
      const syncedAt = isTimestamp(item.syncedAt) ? item.syncedAt : new Date().toISOString();
      const serverVersion = typeof item.serverVersion === 'number' ? item.serverVersion : 1;

      // A blob only counts once its note is accepted: the server ignores one with no
      // reference, which is what happens to a note refused as a conflict
      const previous = await syncRepository.getNoteSyncMetadata(item.id);
      const alreadyThere = new Set(previous?.uploadedAttachmentIds ?? []);
      const uploadedAttachmentIds = (pushedNotes.get(item.id)?.attachments ?? [])
        .map(attachment => attachment.id)
        .filter(id => sentBlobs.has(id) || alreadyThere.has(id));

      await syncRepository.updateNoteSyncMetadata(item.id, {
        noteId: item.id,
        syncedAt,
        serverVersion,
        lastSyncStatus: 'synced',
        errorMessage: undefined,
        uploadedAttachmentIds,
      });

      const note = await noteRepository.getById(item.id);
      if (note) {
        note.syncedAt = syncedAt;
        if (this.isUnchangedSincePush(note, pushedNotes.get(item.id))) {
          note.needsSync = false;
        }
        await noteRepository.update(note, false, true);
      }
    }
  }

  /** Process notes rejected by server during push (conflicts) */
  private async processRejectedNotes(rejected: SyncRejected[], pushedNotes: Map<string, Note>): Promise<void> {
    for (const candidate of rejected) {
      const validated = validateConflict(candidate);
      if (!validated.ok) {
        // needsSync stays set, so the note is offered again on the next sync
        recordRejectedRemoteRecord('conflict', candidate?.id, validated.reason);
        continue;
      }

      const item = validated.value;
      console.warn(`[SyncService] Note ${item.id} rejected by the server`);
      await storeConflict(item.id, {
        serverContent: item.serverContent,
        serverTags: item.serverTags,
        serverModifiedAt: item.serverModifiedAt,
        serverVersion: item.serverVersion,
        serverAttachments: item.serverAttachments,
        serverPinned: item.serverPinned,
        serverSyntaxLanguage: item.serverSyntaxLanguage,
        serverWordWrap: item.serverWordWrap,
        // Hash chain fields for git-like conflict detection
        serverContentHash: item.serverContentHash,
        serverParentHash: item.serverParentHash,
        serverHashChain: item.serverHashChain,
        // Ancestor data for 3-way merge (if available)
        ancestorHash: item.ancestorHash,
        ancestorContent: item.ancestorContent,
        ancestorTags: item.ancestorTags,
      });

      // Clear needsSync flag to prevent infinite retry loop
      const note = await noteRepository.getById(item.id);
      if (note && this.isUnchangedSincePush(note, pushedNotes.get(item.id))) {
        note.needsSync = false;
        await noteRepository.update(note, false, true);
      }
    }
  }

  /**
   * Note what the server skipped. Only what was sent counts: it can't name other notes
   * here. Anything it names is dealt with after the pull, whose deletions it may repeat.
   */
  private recordSkippedNotes(result: SyncPushResponse, pushedNotes: Map<string, Note>): void {
    const answered = new Set([...result.accepted, ...result.rejected].map(item => item?.id));
    const deletedElsewhere = new Set(result.skippedDeleted ?? []);

    for (const [id, note] of pushedNotes) {
      if (deletedElsewhere.has(id)) {
        this.skippedDeletedNotes.set(id, note);
      } else if (!answered.has(id)) {
        const { createdAt, modifiedAt, deletedAt, archivedAt, lockedAt } = note;
        this.unansweredTimes.push(...[createdAt, modifiedAt, deletedAt, archivedAt, lockedAt].filter(time => !!time) as string[]);
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
      // The server defaults a missing lock to false, which would clear one set elsewhere
      locked: note.locked ?? false,
      lockedAt: note.lockedAt,
      deleted: note.deleted,
      deletedAt: note.deletedAt,
      version: note.version,
      wordWrap: note.wordWrap,
      syntaxLanguage: note.syntaxLanguage,
      showPreview: note.showPreview,
      color: note.color,
      // Hash chain fields for git-like conflict detection
      contentHash: note.contentHash,
      parentHash: note.parentHash,
      hashChain: note.hashChain,
    };
  }

  /**
   * Whether this device can vouch for every attachment list in a push. A note with no
   * hash can be accepted on its timestamp alone, even if another device has since
   * added an attachment, and the flag would then remove that attachment.
   */
  private canVouchForAttachments(batchNotes: Note[]): boolean {
    return batchNotes.length > 0 && batchNotes.every(note => !!note.contentHash);
  }

  /** Process a single batch of notes for pushing */
  private async processPushBatch(
    batchNotes: Note[],
    endpoint: string,
    apiKey: string,
    masterKey: CryptoKey,
    forceAll: boolean
  ): Promise<PushOutcome> {
    this.noteSyncActivity();

    // A note that can't be converted is set aside. Sending it without its tags would
    // erase them on the server, and it mustn't hold back the rest of the batch.
    const converted: SyncPushRequest['notes'] = [];
    const sendable: Note[] = [];
    for (const note of batchNotes) {
      try {
        converted.push(await this.convertNoteToSyncFormat(note, masterKey));
        sendable.push(note);
      } catch (error) {
        console.error(`[SyncService] Could not prepare note ${note.id} for sync:`, error);
        await syncRepository.updateNoteSyncMetadata(note.id, {
          noteId: note.id,
          lastSyncStatus: 'error',
          errorMessage: 'Tags could not be read',
        });
      }
    }

    if (sendable.length === 0) {
      return { accepted: 0, rejected: 0, warnings: [], refused: 0 };
    }

    // Collect attachments and build request
    const attachmentMap = await this.collectAttachmentsForBatch(sendable, forceAll);

    const pushRequest: SyncPushRequest = {
      notes: converted,
      attachments: Array.from(attachmentMap.entries()).map(([id, data]) => ({ id, data })),
      versions: await this.collectVersionsForPush(sendable, masterKey),
      attachmentsAuthoritative: this.canVouchForAttachments(sendable) ? true : undefined,
    };

    // Send to server and process results
    const result = validatePushResponse(await pushToServer(endpoint, apiKey, pushRequest));
    const pushedNotes = new Map(sendable.map(note => [note.id, note]));
    await this.processAcceptedNotes(result.accepted, pushedNotes, new Set(attachmentMap.keys()));
    await this.processRejectedNotes(result.rejected, pushedNotes);
    this.recordSkippedNotes(result, pushedNotes);
    const warnings = result.attachmentWarnings ?? [];

    // Update progress
    syncProgress.update(p => ({ ...p, completed: p.completed + batchNotes.length }));

    return { accepted: result.accepted.length, rejected: result.rejected.length, warnings, refused: 0 };
  }

  /**
   * Push a batch, splitting it if the server refuses what it holds. A push over the
   * limit fails entirely, so one oversized note would otherwise hold back every note
   * pushed with it, on every sync, for good. A single note that is still refused is
   * set aside and stays marked for the next sync.
   */
  private async pushNotes(
    batchNotes: Note[],
    endpoint: string,
    apiKey: string,
    masterKey: CryptoKey,
    forceAll: boolean
  ): Promise<PushOutcome> {
    try {
      return await this.processPushBatch(batchNotes, endpoint, apiKey, masterKey, forceAll);
    } catch (error) {
      if (!(error instanceof SyncApiError)) {
        throw error;
      }

      const refused = REFUSED_PUSH_STATUSES.has(error.status);
      const canSplit = batchNotes.length > 1 && (refused || error.status === TIMED_OUT_STATUS);
      if (!refused && !canSplit) {
        throw error;
      }

      if (batchNotes.length === 1) {
        console.warn(`[SyncService] Server refused note ${batchNotes[0].id}:`, error.message);
        await syncRepository.updateNoteSyncMetadata(batchNotes[0].id, {
          noteId: batchNotes[0].id,
          lastSyncStatus: 'error',
          errorMessage: error.message,
        });
        return { accepted: 0, rejected: 0, warnings: [], refused: 1 };
      }

      const middle = Math.ceil(batchNotes.length / 2);
      const first = await this.pushNotes(batchNotes.slice(0, middle), endpoint, apiKey, masterKey, forceAll);
      const second = await this.pushNotes(batchNotes.slice(middle), endpoint, apiKey, masterKey, forceAll);
      return {
        accepted: first.accepted + second.accepted,
        rejected: first.rejected + second.rejected,
        warnings: [...first.warnings, ...second.warnings],
        refused: first.refused + second.refused,
      };
    }
  }

  /**
   * Send permanent deletions. They go in a request of their own, so they don't wait
   * for a note to change, and nothing else can hold them back: after the server's
   * 30-day cleanup, a deletion that never left this device would leave that note
   * alive on every other one. An account at its quota can still delete, and has to
   * be able to, so nothing that adds data rides along.
   */
  private async pushDeletions(endpoint: string, apiKey: string, deletions: SyncDeletion[]): Promise<void> {
    const result = validatePushResponse(await pushToServer(endpoint, apiKey, {
      notes: [],
      attachments: [],
      versions: [],
      deletions,
    }));

    // Refused means the note has a newer edit. Sending it again would only make the
    // server re-announce the note to every device, on every push.
    const refused = new Set(result.rejectedDeletions ?? []);
    const sent = deletions.map(deletion => deletion.id);
    await noteRepository.markDeletionsSynced(sent.filter(id => !refused.has(id)));
    await noteRepository.forgetDeletions(sent.filter(id => refused.has(id)));
    await noteRepository.cleanupOldDeletions();

    console.log(`[SyncService] Pushed ${deletions.length} deletions (${refused.size} refused)`);
  }

  /** Send saved searches, which also shouldn't have to wait for a note to change */
  private async pushSavedSearches(endpoint: string, apiKey: string, savedSearches: SyncSavedSearch[]): Promise<void> {
    const result = validatePushResponse(await pushToServer(endpoint, apiKey, { notes: [], attachments: [], versions: [], savedSearches }));

    // A search is all this request holds, so an error means one was skipped. Nothing
    // says which, and sending them all again is harmless.
    if (result.errors?.length) {
      console.warn(`[SyncService] The server skipped saved searches: ${result.errors.length}`);
      this.unansweredTimes.push(...savedSearches.map(search => search.modifiedAt));
      return;
    }

    for (const search of savedSearches) {
      await savedSearchRepository.markSynced(search.id, search.version);
    }
    console.log(`[SyncService] Marked ${savedSearches.length} saved searches as synced`);
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
    const savedSearches = await this.collectSavedSearchesForPush();
    const deletions = await noteRepository.getPendingDeletions();

    if (modifiedNotes.length === 0 && savedSearches.length === 0 && deletions.length === 0) {
      return 0;
    }

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    if (deletions.length > 0) {
      await this.pushDeletions(endpoint, apiKey, deletions);
    }
    if (savedSearches.length > 0) {
      await this.pushSavedSearches(endpoint, apiKey, savedSearches);
    }

    syncProgress.update(p => ({ ...p, total: p.total + modifiedNotes.length }));

    const pushSizes = new Map<string, number>();
    for (const note of modifiedNotes) {
      pushSizes.set(note.id, await this.estimatePushBytes(note, forceAll));
    }
    const batches = chunkBySize(modifiedNotes, note => pushSizes.get(note.id) ?? 0, SYNC_PUSH_MAX_BYTES, SYNC_PUSH_BATCH_SIZE);

    let totalAccepted = 0;
    let totalRejected = 0;
    let totalRefused = 0;
    const allWarnings: AttachmentWarning[] = [];

    console.log(`[SyncService] Pushing ${modifiedNotes.length} notes in ${batches.length} batch${batches.length > 1 ? 'es' : ''}${forceAll ? ' (force)' : ''}`);

    for (const batchNotes of batches) {
      const { accepted, rejected, warnings, refused } = await this.pushNotes(
        batchNotes, endpoint, apiKey, masterKey.key, forceAll
      );
      totalAccepted += accepted;
      totalRejected += rejected;
      totalRefused += refused;
      allWarnings.push(...warnings);
    }

    if (totalRefused > 0) {
      toast.error(get(_)('sync.notesRefused'));
    }

    if (allWarnings.length > 0) {
      await this.handleAttachmentWarnings(endpoint, apiKey, allWarnings);
    }

    console.log(`[SyncService] Push complete: ${totalAccepted} accepted, ${totalRejected} rejected`);
    await syncRepository.updateMetadata({ lastPushAt: new Date().toISOString() });

    return totalAccepted;
  }

  /**
   * React to server attachment warnings: re-push blobs this device holds
   * (once per sync run, no in-run retry — the next regular sync retries
   * naturally), and surface the rest in vault health. The repair response is
   * deliberately not fed back into this handler, preventing loops.
   */
  private async handleAttachmentWarnings(
    endpoint: string,
    apiKey: string,
    warnings: AttachmentWarning[]
  ): Promise<void> {
    const repairAttachments: { id: string; data: string }[] = [];
    const unhealable: AttachmentWarning[] = [];

    for (const warning of warnings) {
      const missingHere: string[] = [];
      for (const attachmentId of warning.attachmentIds.filter(isUuid)) {
        const blob = await attachmentRepository.getBlob(attachmentId);
        if (blob) {
          repairAttachments.push({ id: attachmentId, data: arrayBufferToBase64(blob) });
          console.warn(`[SyncService] Server is missing attachment ${attachmentId} (note ${warning.noteId}) — re-uploading`);
        } else {
          missingHere.push(attachmentId);
          console.warn(`[SyncService] Server is missing attachment ${attachmentId} (note ${warning.noteId}) and it is not on this device`);
        }
      }
      if (missingHere.length > 0 && isUuid(warning.noteId)) {
        unhealable.push({ noteId: warning.noteId, attachmentIds: missingHere });
      }
    }

    for (const attachments of chunkBySize(repairAttachments, item => item.data.length, SYNC_PUSH_MAX_BYTES)) {
      try {
        await pushToServer(endpoint, apiKey, { notes: [], attachments, versions: [] });
        console.log(`[SyncService] Re-uploaded ${attachments.length} attachment(s) the server was missing`);
      } catch (error) {
        console.error('[SyncService] Attachment repair push failed:', error);
      }
    }

    if (unhealable.length > 0) {
      reportServerMissingAttachments(unhealable);
    }
  }

  // ===========================================================================
  // Pull Helper Methods
  // ===========================================================================

  /**
   * Attachment blobs actually present locally. The server re-sends any
   * attachment of a pulled note that is not in this list, so sourcing it from
   * stored blobs (rather than note references) lets locally missing
   * attachments self-heal when their note is next pulled.
   */
  private async getKnownAttachmentIds(): Promise<string[]> {
    return attachmentRepository.listAllIds();
  }

  /** Process a single remote note during pull (handles conflicts) */
  private async processRemoteNote(
    candidate: SyncNote,
    masterKey: CryptoKey,
    syncedAt: string
  ): Promise<boolean> {
    const validated = validateRemoteNote(candidate);
    if (!validated.ok) {
      recordRejectedRemoteRecord('note', candidate?.id, validated.reason);
      return false;
    }

    const remoteNote = validated.value;
    const localNote = await noteRepository.getById(remoteNote.id);
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
      // Whether a note opens in preview stays with this device. The field is
      // plaintext, so taking it from the server would let the server choose to
      // render a note the moment it is selected.
      showPreview: localNote?.showPreview ?? false,
      color: remoteNote.color,
      syncedAt,
      needsSync: false,
      // Hash chain fields
      contentHash: remoteNote.contentHash,
      parentHash: remoteNote.parentHash,
      hashChain: remoteNote.hashChain,
    };

    if (!localNote) {
      // New note from server
      await noteRepository.create(noteForStorage);
      await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
        noteId: remoteNote.id,
        syncedAt,
        serverVersion: remoteNote.version,
        lastSyncStatus: 'synced',
        uploadedAttachmentIds: remoteNote.attachments.map(attachment => attachment.id),
      });
      return true;
    }

    // What a re-delivered note means comes from the hashes, as set out under "Applying
    // a pulled note" in docs/sync-protocol.md. A refused push clears needsSync, so a
    // conflict waiting to be resolved counts as an edit: the note here is the only
    // copy of that side of it.
    const syncState = await syncRepository.getNoteSyncMetadata(remoteNote.id);
    const hasUnsentEdit = !!localNote.needsSync || syncState?.lastSyncStatus === 'conflict';

    let isConflict = false;
    let isFastForward = false;

    if (remoteNote.contentHash && localNote.contentHash) {
      if (localNote.contentHash === remoteNote.contentHash) {
        // The text is the same, but state can still differ: deleted isn't part of the
        // hash, and not every client recomputes it for a pin or an attachment. A note
        // with unsent changes pushes its own state instead.
        if (hasUnsentEdit || !this.hasRemoteStateChange(localNote, remoteNote)) {
          return false;
        }
        noteForStorage.attachments = this.mergedAttachments(localNote, remoteNote);
        isFastForward = true;
      } else if (!hasUnsentEdit) {
        // Nothing here is unsent, so the server's copy is the current one. That holds
        // even if it matches an older hash: a restore or an undo can return to one.
        isFastForward = true;
      } else if (remoteNote.contentHash === localNote.parentHash || localNote.hashChain?.includes(remoteNote.contentHash)) {
        return false; // Already seen, and older than the edit, which will be pushed
      } else if (remoteNote.parentHash === localNote.contentHash || remoteNote.hashChain?.includes(localNote.contentHash)) {
        isFastForward = true; // Newer, and it already includes the edit
      } else {
        // Sharing a parent proves nothing here: every chain starts with its own hash,
        // so two sides of a divergence both hold their common ancestor.
        isConflict = true;
      }
    } else {
      // Fallback to timestamp-based detection for legacy notes
      if (remoteNote.modifiedAt > localNote.modifiedAt && hasUnsentEdit) {
        isConflict = true;
      } else if (remoteNote.modifiedAt > localNote.modifiedAt) {
        isFastForward = true;
      }
    }

    if (isConflict) {
      // A refused push already stored this server copy, with the ancestor for a
      // three-way merge, which a pull can't supply
      if (remoteNote.contentHash && syncState?.conflictData?.serverContentHash === remoteNote.contentHash) {
        return false;
      }

      console.warn(`[SyncService] Conflict detected during pull for note ${remoteNote.id}`);

      // Find common ancestor for 3-way merge
      let ancestorHash: string | null = null;
      if (localNote.hashChain && remoteNote.hashChain) {
        ancestorHash = findCommonAncestor(localNote.hashChain, remoteNote.hashChain);
      }

      await storeConflict(remoteNote.id, {
        serverContent: remoteNote.content,
        serverTags: remoteNote.tags,
        serverModifiedAt: remoteNote.modifiedAt,
        serverVersion: remoteNote.version,
        serverAttachments: remoteNote.attachments,
        serverPinned: remoteNote.pinned,
        serverSyntaxLanguage: remoteNote.syntaxLanguage,
        serverWordWrap: remoteNote.wordWrap,
        // Hash chain fields
        serverContentHash: remoteNote.contentHash,
        serverParentHash: remoteNote.parentHash,
        serverHashChain: remoteNote.hashChain,
        // Ancestor hash (content would need to be fetched from versions)
        ancestorHash: ancestorHash ?? undefined,
      });
      return false;
    }

    if (isFastForward) {
      // Server is newer (fast-forward), safe to update
      await noteRepository.update(noteForStorage, false, true);
      await syncRepository.updateNoteSyncMetadata(remoteNote.id, {
        noteId: remoteNote.id,
        syncedAt,
        serverVersion: remoteNote.version,
        lastSyncStatus: 'synced',
        uploadedAttachmentIds: remoteNote.attachments.map(attachment => attachment.id),
      });
      return true;
    }

    return false; // Local is newer or equal
  }

  /**
   * Whether the remote copy differs in anything apart from the text. showPreview is
   * left out, because it stays with each device.
   */
  private hasRemoteStateChange(local: Note, remote: SyncNote): boolean {
    const localAttachments = new Set(local.attachments.map(attachment => attachment.id));

    return local.pinned !== remote.pinned
      || !!local.archived !== !!remote.archived
      || !!local.locked !== !!remote.locked
      || !!local.deleted !== !!remote.deleted
      || (local.color ?? null) !== (remote.color ?? null)
      || (local.wordWrap ?? true) !== (remote.wordWrap ?? true)
      || (local.syntaxLanguage ?? null) !== (remote.syntaxLanguage ?? null)
      || remote.attachments.some(attachment => !localAttachments.has(attachment.id));
  }

  /**
   * Local attachments plus any new remote ones. A shorter remote list isn't a removal:
   * some clients send empty lists by mistake, and the server can return a note
   * without an attachment it holds. When the text changes, the remote list is taken
   * as it is.
   */
  private mergedAttachments(local: Note, remote: SyncNote): Note['attachments'] {
    const localAttachments = new Set(local.attachments.map(attachment => attachment.id));
    return [...local.attachments, ...remote.attachments.filter(attachment => !localAttachments.has(attachment.id))];
  }

  /** Download attachments from server */
  private async downloadAttachments(attachments: Array<{ id: string; data: string }>): Promise<void> {
    for (const attachment of attachments) {
      if (!isUuid(attachment?.id) || typeof attachment.data !== 'string') {
        recordRejectedRemoteRecord('attachment', attachment?.id, 'invalid attachment');
        continue;
      }

      try {
        const blob = base64ToArrayBuffer(attachment.data);
        await attachmentRepository.storeBlob(attachment.id, blob);
      } catch (error) {
        // A pull that moves on won't send this again, so vault health has to know
        console.error(`[SyncService] Failed to download attachment ${attachment.id}:`, error);
        recordRejectedRemoteRecord('attachment', attachment.id, 'could not be stored');
      }
    }
  }

  /** Merge note versions from server */
  private async mergeVersionsFromServer(versions: SyncNoteVersion[], masterKey: CryptoKey): Promise<void> {
    for (const candidate of versions) {
      const validated = validateRemoteVersion(candidate);
      if (!validated.ok) {
        recordRejectedRemoteRecord('version', candidate?.noteId, validated.reason);
        continue;
      }

      const serverVersion = validated.value;
      const existingVersion = await versionRepository.getVersion(
        serverVersion.noteId,
        serverVersion.version
      );

      if (!existingVersion) {
        await versionRepository.storeRemoteVersion({
          ...serverVersion,
          // Older rows on the server are in either format, depending on the client
          tags: await syncToStorage(serverVersion.tags, masterKey),
          attachments: serverVersion.attachments as Attachment[],
          // Per device, like a note's. A server mustn't be able to turn a preview on.
          showPreview: undefined,
        });
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

    for (const candidate of savedSearches) {
      const validated = validateSavedSearch(candidate);
      if (!validated.ok) {
        recordRejectedRemoteRecord('savedSearch', candidate?.id, validated.reason);
        continue;
      }

      const remoteSearch = validated.value;
      const localSearch = await savedSearchRepository.getStoredById(remoteSearch.id);

      // A live row has to decrypt. A deleted one is never shown, and older clients
      // pushed some as plaintext, so a deletion is applied whatever the row holds.
      if (!remoteSearch.deleted && !(await savedSearchRepository.isReadable(remoteSearch))) {
        recordRejectedRemoteRecord('savedSearch', remoteSearch.id, 'unreadable');
        continue;
      }

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

  /**
   * Finish deleting inbox items accepted while the server couldn't be told. One that
   * still can't be deleted waits for the next sync, and holds up nothing else.
   */
  private async finishAcceptedInboxItems(endpoint: string, apiKey: string): Promise<void> {
    const accepted = (await syncRepository.getMetadata())?.acceptedInboxIds ?? [];
    const deleted: string[] = [];

    for (const id of accepted) {
      try {
        await deleteInboxItem(normalizeEndpoint(endpoint), apiKey, id);
        deleted.push(id);
      } catch (error) {
        console.warn('[SyncService] Could not delete an accepted inbox item:', error);
      }
    }

    if (deleted.length > 0) {
      await syncRepository.removeAcceptedInboxIds(deleted);
    }
  }

  /** Inbox items that are safe to show, reporting any that are not */
  private validInboxItems(items: InboxItem[]): InboxItem[] {
    const valid: InboxItem[] = [];
    for (const item of items) {
      const checked = validateInboxItem(item);
      if (checked) {
        valid.push(checked);
      } else {
        recordRejectedRemoteRecord('inboxItem', item?.id, 'invalid inbox item');
      }
    }
    return valid;
  }

  /**
   * Process hard deletions from server. Past the limit, a note goes to the recycle bin
   * instead, so that a pull like that can be undone.
   *
   * @returns How many notes went from here, and how many of those are held in the bin
   */
  private async processRemoteDeletions(
    deletions: Array<{ id: string }>,
    budget: DeletionBudget
  ): Promise<{ removed: number; held: number }> {
    const valid = deletions.filter(deletion => {
      if (isUuid(deletion?.id)) return true;
      recordRejectedRemoteRecord('deletion', deletion?.id, 'invalid id');
      return false;
    });

    const live: Note[] = [];
    const alreadyHeld = new Set<string>();
    for (const deletion of valid) {
      const note = await noteRepository.getById(deletion.id);
      if (note?.deletionHeld) {
        alreadyHeld.add(deletion.id);
      } else if (note && !note.deleted) {
        live.push(note);
      }
    }

    if (budget.limit === null && live.length > 0) {
      const notes = await noteRepository.getAllNonDeleted();
      budget.limit = Math.max(MASS_DELETION_MINIMUM, Math.floor(notes.length * MASS_DELETION_SHARE));
    }

    // Judged by the page, so one that is suspicious as a whole is held as a whole
    budget.used += live.length;
    const held = budget.limit !== null && budget.used > budget.limit ? new Set(live.map(note => note.id)) : new Set<string>();

    for (const note of live) {
      if (held.has(note.id)) {
        const now = new Date().toISOString();
        await noteRepository.update(
          { ...note, deleted: true, deletedAt: now, deletionHeld: true, needsSync: false, syncedAt: note.syncedAt ?? now },
          false,
          true
        );
      }
    }

    let removed = held.size;
    // A held note waits for the person to decide. Asking again doesn't change that, or
    // a second pull would delete from the bin what the first couldn't delete outright.
    for (const deletion of valid) {
      if (held.has(deletion.id) || alreadyHeld.has(deletion.id)) continue;
      if (await noteService.applyRemoteDeletion(deletion.id)) {
        console.log(`[SyncService] Hard deleted note ${deletion.id} (from server)`);
        removed++;
      }
    }

    return { removed, held: held.size };
  }

  /**
   * Apply what the push learned from `skippedDeleted`, counted with the pull's own
   * deletions so that it can't be used to get round their limit. This matters for a
   * device past the deletion in the pull's order, which won't be sent it again.
   *
   * A note edited since it was sent stays. That edit is later than the deletion, so
   * the server will take it.
   */
  private async dropNotesDeletedElsewhere(budget: DeletionBudget): Promise<{ removed: number; held: number }> {
    const deletions: Array<{ id: string }> = [];
    for (const [id, pushed] of this.skippedDeletedNotes) {
      const note = await noteRepository.getById(id);
      if (note && this.isUnchangedSincePush(note, pushed)) {
        deletions.push({ id });
      }
    }
    this.skippedDeletedNotes.clear();

    return this.processRemoteDeletions(deletions, budget);
  }

  /**
   * Pull remote changes, following the loop in docs/sync-protocol.md. It works
   * against both kinds of server: an old one ignores `cursor` and a new one ignores
   * `offset`, so every response is checked rather than remembering what the last
   * one said. The cursor is saved only after its page is applied. A page repeated
   * after a crash is harmless, whereas saving first could lose one.
   *
   * @returns How many notes changed, and the server's time when the pull began
   */
  private async pull(endpoint: string, apiKey: string): Promise<{ pulledCount: number; serverTime?: string }> {
    endpoint = normalizeEndpoint(endpoint);
    const metadata = await syncRepository.getMetadata();

    const masterKey = keyManager.getMasterKey();
    if (!masterKey) {
      throw new Error('Application is locked');
    }

    this.pulledNoteIds = [];

    const alreadyAccepted = new Set(metadata?.acceptedInboxIds);
    let hasMore = true;
    let serverTime: string | undefined;
    let progressStarted = false;
    let totalNotes = 0;
    let totalAttachments = 0;
    let totalDeletions = 0;
    let heldDeletions = 0;
    const deletionBudget: DeletionBudget = { limit: null, used: 0 };
    let fullPullDone = metadata?.cursorFullPullDone ?? false;
    let storedCursor = metadata?.pullCursor;

    const request: SyncPullRequest = {
      cursor: metadata?.pullCursor ?? '',
      lastSyncAt: withOverlap(metadata?.lastSyncAt),
      // Older servers require this field, and no server has ever used its contents
      knownNoteIds: [],
      knownAttachmentIds: await this.getKnownAttachmentIds(),
      limit: SYNC_PULL_BATCH_SIZE,
      offset: 0,
    };

    while (hasMore) {
      const result = validatePullResponse(await pullFromServer(endpoint, apiKey, request));
      this.noteSyncActivity();
      const syncedAt = result.syncedAt ?? new Date().toISOString();
      serverTime ??= result.syncedAt;
      hasMore = result.hasMore ?? false;

      const totalCount = result.totalCount ?? result.notes.length;
      if (!progressStarted && totalCount > 0) {
        progressStarted = true;
        syncProgress.update(p => ({ ...p, total: p.total + totalCount }));
      }

      totalAttachments += result.attachments.length;
      totalDeletions += result.deletions?.length || 0;

      // Process notes with incremental progress updates
      let processedInBatch = 0;
      for (const remoteNote of result.notes) {
        const wasChanged = await this.processRemoteNote(remoteNote, masterKey.key, syncedAt);
        if (wasChanged) {
          totalNotes++;
          this.pulledNoteIds.push(remoteNote.id);
        }
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
      await this.mergeVersionsFromServer(result.versions, masterKey.key);
      if (result.savedSearches) {
        await this.processSavedSearchesFromServer(result.savedSearches, syncedAt);
      }
      if (result.deletions) {
        const { removed, held } = await this.processRemoteDeletions(result.deletions, deletionBudget);
        heldDeletions += held;
        totalNotes += removed; // So the list refreshes
      }

      // The server sends the whole inbox on every page, or nothing if it's empty
      inboxItems.set(this.validInboxItems(result.inboxItems ?? []).filter(item => !alreadyAccepted.has(item.id)));

      if (result.nextCursor !== undefined) {
        // Timestamp mode can have silently skipped changes, and nothing can detect
        // that afterwards, so a device moving to the cursor reads everything once.
        // What this page held was applied, and arrives again, which is harmless.
        if (!fullPullDone && !request.cursor && request.lastSyncAt) {
          console.log('[SyncService] Server supports the pull cursor, doing one full pull');
          request.lastSyncAt = undefined;
          progressStarted = false;
          hasMore = true;
          continue;
        }

        fullPullDone = true;
        storedCursor = result.nextCursor;
        await syncRepository.updateMetadata({ pullCursor: result.nextCursor, cursorFullPullDone: true });

        if (hasMore && result.nextCursor === request.cursor) {
          console.warn('[SyncService] Server reported more to pull without moving on, stopping');
          break;
        }
        request.cursor = result.nextCursor;
      } else {
        // An older server, or one that has lost the cursor
        if (storedCursor !== undefined) {
          storedCursor = undefined;
          await syncRepository.updateMetadata({ pullCursor: undefined });
        }

        if (hasMore && result.notes.length === 0) {
          console.warn('[SyncService] Server reported more to pull without sending any, stopping');
          break;
        }
        request.offset = (request.offset ?? 0) + result.notes.length;
      }
    }

    const dropped = await this.dropNotesDeletedElsewhere(deletionBudget);
    heldDeletions += dropped.held;
    totalNotes += dropped.removed;

    if (heldDeletions > 0) {
      toast.error(get(_)('sync.massDeletionHeld', { values: { count: String(heldDeletions) } }));
    }

    console.log(`[SyncService] Pulled ${totalNotes} notes, ${totalAttachments} attachments, ${totalDeletions} deletions`);
    await syncRepository.updateMetadata({ lastPullAt: new Date().toISOString() });

    return { pulledCount: totalNotes, serverTime };
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
   * Get a short-lived SSE token from the server
   * This exchanges the API key for a temporary token that can be safely used in URLs
   */
  private async getSseToken(endpoint: string, apiKey: string): Promise<string> {
    const response = await fetch(`${endpoint}/api/v1/sync/events/token`, {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${apiKey}`,
      },
    });

    if (!response.ok) {
      throw new SyncApiError(`Failed to get SSE token: ${response.status}`, response.status, await response.text());
    }

    const data = await response.json();
    return data.token;
  }

  /**
   * Connect to SSE endpoint for real-time sync notifications
   * When another device syncs, the server sends a notification to trigger an immediate pull
   *
   * Security: Uses short-lived tokens instead of API keys in the URL.
   * The API key is exchanged for a token via an authenticated endpoint.
   *
   * A browser doesn't show a stream's status, so a 401 looks like any other refusal.
   * Every reconnection therefore asks for a new token, and that request says why: a
   * 401 or a 403 there means the device or account, not the token.
   */
  async connectToSyncEvents(): Promise<void> {
    // Don't reconnect if already connected or connecting
    if (this.eventSource || this.sseConnecting) {
      return;
    }

    // Set connecting flag immediately to prevent race conditions
    this.sseConnecting = true;

    // Check user settings first - this is the primary source of truth
    let currentSettings: { syncEnabled?: boolean } | undefined;
    settings.subscribe(s => currentSettings = s)();
    if (!currentSettings?.syncEnabled) {
      this.sseConnecting = false;
      return;
    }

    // Check network availability first
    if (!this.isOnline()) {
      console.log('[SyncService] Skipping SSE connection - network offline');
      // Fall back to periodic sync
      this.sseFailedOver = true;
      this.sseConnecting = false;
      this.restorePeriodicSync();
      return;
    }

    try {
      const metadata = await syncRepository.getMetadata();
      if (!metadata?.syncEnabled || !metadata?.apiKey || !metadata?.syncEndpoint) {
        this.sseConnecting = false;
        return;
      }

      // Decrypt API key
      const masterKey = keyManager.getMasterKey();
      if (!masterKey) {
        this.sseConnecting = false;
        return; // App is locked
      }

      const apiKeyEncrypted = JSON.parse(metadata.apiKey);
      const apiKey = await cryptoService.decryptText(apiKeyEncrypted, masterKey.key);

      const endpoint = normalizeEndpoint(metadata.syncEndpoint);

      // Exchange API key for a short-lived SSE token
      // This prevents the API key from appearing in URLs/browser history/logs
      const sseToken = await this.getSseToken(endpoint, apiKey);

      // Build SSE URL with the short-lived token (not the API key)
      const sseUrl = `${endpoint}/api/v1/sync/events?token=${encodeURIComponent(sseToken)}`;

      console.log('[SyncService] Connecting to SSE for real-time sync notifications');
      this.eventSource = new EventSource(sseUrl);
      this.sseConnecting = false; // EventSource created, clear connecting flag

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
      this.sseConnecting = false;
      if (error instanceof SyncApiError && (error.status === 401 || error.status === 403)) {
        this.handleServerRefusal(error);
        return;
      }
      // Fall back to periodic sync on any other error
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
    // Reset connection state
    this.sseConnecting = false;
    this.sseReconnectAttempts = 0;
    this.sseReconnectDelay = 1000;
  }

  /**
   * Check if SSE is connected
   */
  isSseConnected(): boolean {
    return this.eventSource !== null && this.eventSource.readyState === EventSource.OPEN;
  }

  /** Revoke this device on the server. A device that can't be revoked keeps a working key. */
  private async revokeThisDevice(): Promise<boolean> {
    try {
      const metadata = await syncRepository.getMetadata();
      const masterKey = keyManager.getMasterKey();
      if (!metadata?.apiKey || !metadata.syncEndpoint || !masterKey) {
        return false;
      }

      const apiKey = await cryptoService.decryptText(JSON.parse(metadata.apiKey), masterKey.key);
      await revokeDevice(metadata.syncEndpoint, apiKey);
      return true;
    } catch (error) {
      console.warn('[SyncService] Could not revoke this device:', error);
      return false;
    }
  }

  /**
   * Disconnect from sync server
   * Clears all sync credentials and metadata, but preserves local notes
   *
   * @returns Whether the server revoked this device. Disconnecting goes ahead either
   * way, but if it didn't, the device stays registered and its key stays valid.
   */
  async disconnect(): Promise<{ revoked: boolean }> {
    // Disable auto-sync and SSE first
    this.disableAutoSync();
    this.disconnectFromSyncEvents();

    // Before the credentials go, because this needs them
    const revoked = await this.revokeThisDevice();

    // Clear all sync metadata (credentials, client ID, etc.)
    await syncRepository.clearAll();
    inboxItems.set([]);

    // Update settings to disable sync
    await settingsRepository.update({
      syncEnabled: false,
      syncEndpoint: undefined,
    });

    console.log('[SyncService] Disconnected from sync server');
    return { revoked };
  }

  /**
   * Collect all versions for notes being pushed
   */
  private async collectVersionsForPush(notes: Note[], masterKey: CryptoKey): Promise<SyncNoteVersion[]> {
    const versions: SyncNoteVersion[] = [];

    for (const note of notes) {
      // Get all versions for this note
      const noteVersions = await versionRepository.getVersionsForNote(note.id);

      // Convert to SyncNoteVersion format (versions are already encrypted in storage)
      for (const version of noteVersions) {
        // The sync format, as for notes. A version that can't be read stays on this
        // device: history isn't worth holding back the note it belongs to.
        let tags: string[];
        try {
          tags = await storageToSync(version.tags, masterKey);
        } catch (error) {
          console.warn(`[SyncService] Skipping version ${version.versionKey}, its tags can't be read:`, error);
          continue;
        }

        versions.push({
          versionKey: version.versionKey,
          noteId: version.noteId,
          version: version.version,
          createdAt: version.createdAt,
          syncedAt: version.syncedAt,
          content: version.content,
          tags,
          attachments: version.attachments,
          syntaxLanguage: version.syntaxLanguage,
          wordWrap: version.wordWrap,
          showPreview: version.showPreview,
          reason: version.reason,
          // Hash chain fields
          contentHash: version.contentHash,
          parentHash: version.parentHash,
        });
      }
    }

    return versions;
  }
}

export const syncService = new SyncService();
