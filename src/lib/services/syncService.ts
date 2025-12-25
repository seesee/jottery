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
} from '../types';
import { syncRepository } from './syncRepository';
import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { settingsRepository } from './settingsRepository';
import { keyManager } from './keyManager';
import { versionRepository } from './versionRepository';
import { cryptoService, encryptJSON, decryptJSON } from './crypto';
import { noteService } from './noteService';
import { searchService } from './searchService';
import { notes, settings } from '../stores/appStore';

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
  }

  /**
   * Perform full bidirectional sync
   */
  async syncNow(): Promise<{ success: boolean; error?: string }> {
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

      // 2. Push local changes
      await this.push(metadata.syncEndpoint, apiKey);

      // 3. Pull remote changes
      await this.pull(metadata.syncEndpoint, apiKey);

      // 4. Update last sync timestamp
      await syncRepository.updateMetadata({
        lastSyncAt: new Date().toISOString(),
      });

      // 5. Reload notes into app state and rebuild search index
      console.log('[SyncService] Reloading notes and rebuilding search index...');
      let currentSettings: any;
      settings.subscribe(s => currentSettings = s)();
      const allNotes = await noteService.getAllNotes(currentSettings.sortOrder);
      notes.set(allNotes);
      searchService.indexNotes(allNotes);
      console.log('[SyncService] UI refreshed');

      return { success: true };
    } catch (error) {
      console.error('Sync failed:', error);
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
  private async push(endpoint: string, apiKey: string): Promise<void> {
    endpoint = this.normalizeEndpoint(endpoint);
    const metadata = await syncRepository.getMetadata();
    const lastSyncAt = metadata?.lastSyncAt || '1970-01-01T00:00:00Z';

    // Get notes modified since last sync
    const modifiedNotes = await noteRepository.getModifiedAfter(lastSyncAt);

    if (modifiedNotes.length === 0) {
      console.log('[SyncService] No notes to push');
      return; // Nothing to push
    }

    console.log(`[SyncService] Pushing ${modifiedNotes.length} notes`);

    // Collect attachments for all modified notes
    const attachmentMap = new Map<string, string>();
    for (const note of modifiedNotes) {
      for (const attachment of note.attachments) {
        if (!attachmentMap.has(attachment.id)) {
          const blob = await attachmentRepository.getBlob(attachment.data);
          if (blob) {
            const base64 = this.arrayBufferToBase64(blob);
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
        let individuallyEncryptedTags: string[];

        // Handle tags conversion - support both old and new storage formats
        if (!note.tags || note.tags.length === 0) {
          // No tags
          individuallyEncryptedTags = [];
        } else if (note.tags.length === 1) {
          // Single-blob format: tags[0] is a JSON string containing encrypted array
          try {
            const encryptedTagsBlob = JSON.parse(note.tags[0]);
            const decryptedTagsArray = await decryptJSON<string[]>(
              encryptedTagsBlob,
              masterKey.key
            );

            // Filter out invalid tags (empty strings, non-strings, empty arrays, etc.)
            const validTags = decryptedTagsArray.filter(tag => {
              if (typeof tag !== 'string') {
                console.warn('[SyncService] Skipping non-string tag:', tag);
                return false;
              }
              if (tag.trim().length === 0) {
                console.warn('[SyncService] Skipping empty tag');
                return false;
              }
              return true;
            });

            // Encrypt each valid tag individually with JSON encoding (as required by sync protocol)
            individuallyEncryptedTags = await Promise.all(
              validTags.map(async tag => {
                const tagJson = JSON.stringify(tag);  // JSON-encode the tag
                const encrypted = await cryptoService.encryptText(tagJson, masterKey.key);
                return JSON.stringify(encrypted);  // Stringify the EncryptionResult
              })
            );
          } catch (error) {
            console.error('[SyncService] Failed to parse tags, assuming empty:', error);
            individuallyEncryptedTags = [];
          }
        } else {
          // Array format: already individually encrypted (shouldn't happen, but handle it)
          console.warn('[SyncService] Note has multiple tag entries, using as-is');
          individuallyEncryptedTags = note.tags;
        }

        return {
          id: note.id,
          createdAt: note.createdAt,
          modifiedAt: note.modifiedAt,
          content: note.content,
          tags: individuallyEncryptedTags,
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

      // Create version snapshot after successful push
      const note = await noteRepository.getById(accepted.id);
      if (note) {
        await versionRepository.createVersion(note, {
          syncedAt: accepted.syncedAt,
          reason: 'sync',
        });
      }
    }

    // Handle rejected notes (conflicts)
    for (const rejected of result.rejected) {
      console.warn(`[SyncService] Note ${rejected.id} rejected: ${rejected.reason}`);
      await syncRepository.updateNoteSyncMetadata(rejected.id, {
        noteId: rejected.id,
        lastSyncStatus: 'conflict',
        errorMessage: rejected.reason,
      });
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
      throw new Error(`Pull failed: ${response.statusText} - ${errorText}`);
    }

    const result: SyncPullResponse = await response.json();

    console.log(`[SyncService] Pull complete: ${result.notes.length} notes, ${result.attachments.length} attachments, ${result.deletions.length} deletions`);

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
      // Convert tags from sync format (array of individually encrypted tags) to storage format (single encrypted blob)
      console.log(`[SyncService] Pull - Processing note ${remoteNote.id}`);
      console.log(`[SyncService] Pull - remoteNote.tags:`, {
        isArray: Array.isArray(remoteNote.tags),
        length: remoteNote.tags?.length,
        type: typeof remoteNote.tags,
        sample: remoteNote.tags?.[0]?.substring(0, 100)
      });

      let tagsForStorage: string[];

      if (remoteNote.tags && remoteNote.tags.length > 0) {
        // Decrypt each individually encrypted tag
        const decryptedTags: string[] = [];
        for (let i = 0; i < remoteNote.tags.length; i++) {
          const encryptedTagJson = remoteNote.tags[i];
          try {
            console.log(`[SyncService] Pull - Tag[${i}] encrypted:`, encryptedTagJson?.substring(0, 100));
            const encryptedTag = JSON.parse(encryptedTagJson);
            const tagJson = await cryptoService.decryptText(encryptedTag, masterKey.key);
            console.log(`[SyncService] Pull - Tag[${i}] decrypted:`, tagJson);
            const tag = JSON.parse(tagJson); // Parse the JSON-encoded tag
            console.log(`[SyncService] Pull - Tag[${i}] parsed:`, tag, 'type:', typeof tag, 'isArray:', Array.isArray(tag));

            // Handle legacy format: if tag is an array, extract individual tags
            if (Array.isArray(tag)) {
              console.warn('[SyncService] Pull - Found legacy array tag format, extracting individual tags:', tag);
              for (const individualTag of tag) {
                if (typeof individualTag === 'string' && individualTag.trim().length > 0) {
                  decryptedTags.push(individualTag);
                } else {
                  console.warn('[SyncService] Pull - Skipping invalid tag in array:', individualTag);
                }
              }
              continue;
            }

            // Validate tag before adding
            if (typeof tag !== 'string') {
              console.warn('[SyncService] Pull - Skipping non-string tag:', tag);
              continue;
            }
            if (tag.trim().length === 0) {
              console.warn('[SyncService] Pull - Skipping empty tag');
              continue;
            }

            decryptedTags.push(tag);
          } catch (error) {
            console.error('[SyncService] Pull - Failed to decrypt tag:', error);
          }
        }

        console.log(`[SyncService] Pull - Collected ${decryptedTags.length} decrypted tags:`, decryptedTags);

        // Re-encrypt as a single blob for storage
        const encryptedTagsBlob = await encryptJSON(decryptedTags, masterKey.key);
        tagsForStorage = [JSON.stringify(encryptedTagsBlob)];

        console.log(`[SyncService] Pull - tagsForStorage:`, {
          isArray: Array.isArray(tagsForStorage),
          length: tagsForStorage.length,
          sample: tagsForStorage[0]?.substring(0, 100)
        });
      } else {
        tagsForStorage = [];
        console.log(`[SyncService] Pull - No tags on note`);
      }

      const noteForStorage = {
        ...remoteNote,
        tags: tagsForStorage,
        syncedAt: result.syncedAt,
      };

      // Debug logging for tag storage
      console.log(`[SyncService] Pull - Storing note ${remoteNote.id} with tags:`, {
        tagsIsArray: Array.isArray(tagsForStorage),
        tagsLength: tagsForStorage?.length,
        tagsType: typeof tagsForStorage,
        firstTagSample: tagsForStorage?.[0]?.substring(0, 100)
      });

      const localNote = await noteRepository.getById(remoteNote.id);

      if (!localNote) {
        // New note from server - create locally
        console.log(`[SyncService] Creating new note from server: ${remoteNote.id}`);
        await noteRepository.create(noteForStorage);
      } else {
        // Conflict resolution: Last-Write-Wins by modifiedAt
        if (remoteNote.modifiedAt > localNote.modifiedAt) {
          // Capture local version BEFORE overwriting with server version
          await versionRepository.createVersion(localNote, {
            syncedAt: result.syncedAt,
            reason: 'sync',
          });

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
        const blob = this.base64ToArrayBuffer(attachment.data);
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
        // Convert SyncNoteVersion to NoteVersion format for storage
        const versionToStore: NoteVersion = {
          versionKey: serverVersion.versionKey,
          noteId: serverVersion.noteId,
          version: serverVersion.version,
          createdAt: serverVersion.createdAt,
          syncedAt: serverVersion.syncedAt,
          content: serverVersion.content,
          tags: serverVersion.tags,
          attachments: serverVersion.attachments,
          syntaxLanguage: serverVersion.syntaxLanguage,
          wordWrap: serverVersion.wordWrap,
          reason: serverVersion.reason,
        };

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
          { syncedAt: serverVersion.syncedAt, reason: serverVersion.reason as 'sync' | 'manual-sync' }
        );

        console.log(`[SyncService] Pull - Stored new version from server: ${serverVersion.versionKey}`);
      }
    }

    // Handle deletions
    for (const deletion of result.deletions) {
      const localNote = await noteRepository.getById(deletion.id);
      if (localNote && !localNote.deleted) {
        // Server says deleted - soft delete locally
        console.log(`[SyncService] Soft deleting note from server: ${deletion.id}`);
        await noteRepository.softDelete(deletion.id);
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

  // Helper methods for base64 conversion
  private arrayBufferToBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    let binary = '';
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  private base64ToArrayBuffer(base64: string): ArrayBuffer {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes.buffer;
  }
}

export const syncService = new SyncService();
