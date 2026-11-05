/**
 * Inbox service for managing inbox items
 *
 * Inbox items are unencrypted notes submitted via the Inbox API.
 * They are accepted (encrypted into the main note corpus) or deleted.
 */

import type { InboxItem } from '../types';
import { inboxItems } from '../stores/appStore';
import { addNoteToStoreAndSearch } from '../stores/storeHelpers';
import { noteService } from './noteService';
import { syncRepository } from './syncRepository';
import { syncService } from './syncService';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { deleteInboxItem, normalizeEndpoint } from './syncClient';
import { toast } from '../utils/toast.svelte';
import { get } from 'svelte/store';
import { _ } from 'svelte-i18n';

/**
 * Get the sync endpoint and decrypted API key from metadata
 */
async function getSyncCredentials(): Promise<{ endpoint: string; apiKey: string } | null> {
  const metadata = await syncRepository.getMetadata();
  if (!metadata?.syncEnabled || !metadata.apiKey || !metadata.syncEndpoint) {
    return null;
  }

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) return null;

  const apiKeyEncrypted = JSON.parse(metadata.apiKey);
  const apiKey = await cryptoService.decryptText(apiKeyEncrypted, masterKey.key);
  return { endpoint: normalizeEndpoint(metadata.syncEndpoint), apiKey };
}

class InboxService {
  /**
   * Accept an inbox item — encrypt into main note corpus, then delete from server
   */
  async acceptItem(item: InboxItem): Promise<void> {
    const creds = await getSyncCredentials();
    if (!creds) throw new Error('Sync not configured');

    // Create note with showPreview forced to false (XSS prevention)
    const newNote = await noteService.createNote(item.content, item.tags ?? [], {
      showPreview: false,
    });

    // Add the note to the UI store and search index (same as normal note creation)
    if (newNote?.id) {
      const decryptedNote = await noteService.getNote(newNote.id);
      if (decryptedNote) {
        addNoteToStoreAndSearch(decryptedNote);
      }
    }

    // Delete from server — non-fatal if this fails (note is already created locally).
    // Until it works the server keeps sending the item, so it's remembered here: it
    // won't be offered again, and each sync tries the delete once more.
    await syncRepository.addAcceptedInboxId(item.id);
    try {
      await deleteInboxItem(creds.endpoint, creds.apiKey, item.id);
      await syncRepository.removeAcceptedInboxIds([item.id]);
    } catch (error) {
      console.warn('Failed to delete inbox item from server (will be retried on next sync):', error);
    }

    // Remove from inbox store
    inboxItems.update(items => items.filter(i => i.id !== item.id));

    toast.success(get(_)('inbox.accepted'));
  }

  /**
   * Delete an inbox item from the server
   */
  async deleteItem(itemId: string): Promise<void> {
    const creds = await getSyncCredentials();
    if (!creds) throw new Error('Sync not configured');

    await deleteInboxItem(creds.endpoint, creds.apiKey, itemId);

    // Remove from store
    inboxItems.update(items => items.filter(i => i.id !== itemId));

    toast.success(get(_)('inbox.deleted'));
  }

  /**
   * Accept all inbox items
   */
  async acceptAll(): Promise<void> {
    const items = get(inboxItems);
    const count = items.length;

    for (const item of items) {
      await this.acceptItem(item);
    }

    // Store should already be empty from individual acceptItem calls,
    // but ensure it's cleared
    inboxItems.set([]);

    // Trigger background sync to push newly created notes
    syncService.triggerBackgroundSync().catch(() => {});

    toast.success(get(_)('inbox.acceptedAll', { values: { count: String(count) } }));
  }

  /**
   * Delete all inbox items from the server
   */
  async deleteAll(): Promise<void> {
    const creds = await getSyncCredentials();
    if (!creds) throw new Error('Sync not configured');

    const response = await fetch(`${creds.endpoint}/api/v1/inbox`, {
      method: 'DELETE',
      headers: { 'Authorization': `Bearer ${creds.apiKey}` },
    });

    if (!response.ok) {
      throw new Error(`Failed to delete all inbox items: ${response.statusText}`);
    }

    inboxItems.set([]);
    toast.success(get(_)('inbox.deletedAll'));
  }
}

export const inboxService = new InboxService();
