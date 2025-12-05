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
import { cryptoService } from './crypto';
import { noteService } from './noteService';
import { searchService } from './searchService';

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
      await noteService.loadAllNotes();
      await searchService.rebuildIndex();
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

    // Build push request
    const pushRequest: SyncPushRequest = {
      notes: modifiedNotes.map(note => ({
        id: note.id,
        createdAt: note.createdAt,
        modifiedAt: note.modifiedAt,
        content: note.content,
        tags: note.tags,
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
      })),
      attachments: Array.from(attachmentMap.entries()).map(([id, data]) => ({ id, data })),
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

    // Get all known note IDs
    const allNotes = await noteRepository.getAll();
    const knownIds = allNotes.map(n => n.id);

    const pullRequest: SyncPullRequest = {
      lastSyncAt,
      knownNoteIds: knownIds,
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

    // Apply remote changes with Last-Write-Wins conflict resolution
    for (const remoteNote of result.notes) {
      const localNote = await noteRepository.getById(remoteNote.id);

      if (!localNote) {
        // New note from server - create locally
        console.log(`[SyncService] Creating new note from server: ${remoteNote.id}`);
        await noteRepository.create({
          ...remoteNote,
          syncedAt: result.syncedAt,
        });
      } else {
        // Conflict resolution: Last-Write-Wins by modifiedAt
        if (remoteNote.modifiedAt > localNote.modifiedAt) {
          // Server version is newer - update local
          console.log(`[SyncService] Updating note with server version (newer): ${remoteNote.id}`);
          await noteRepository.update({
            ...remoteNote,
            syncedAt: result.syncedAt,
          });
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
