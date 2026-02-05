/**
 * Inbox service for managing inbox items
 *
 * Inbox items are unencrypted notes submitted via the Inbox API.
 * They are accepted (encrypted into the main note corpus) or deleted.
 */

import type { InboxItem } from '../types';
import { inboxItems } from '../stores/appStore';
import { noteService } from './noteService';
import { syncRepository } from './syncRepository';
import { cryptoService } from './crypto';
import { keyManager } from './keyManager';
import { normalizeEndpoint } from './syncClient';
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

/**
 * Delete an inbox item from the server
 */
async function deleteFromServer(endpoint: string, apiKey: string, itemId: string): Promise<void> {
  const response = await fetch(`${endpoint}/api/v1/inbox/${itemId}`, {
    method: 'DELETE',
    headers: { 'Authorization': `Bearer ${apiKey}` },
  });
  if (!response.ok && response.status !== 404) {
    throw new Error(`Failed to delete inbox item: ${response.statusText}`);
  }
}

class InboxService {
  /**
   * Accept an inbox item — encrypt into main note corpus, then delete from server
   */
  async acceptItem(item: InboxItem): Promise<void> {
    const creds = await getSyncCredentials();
    if (!creds) throw new Error('Sync not configured');

    // Create note with showPreview forced to false (XSS prevention)
    await noteService.createNote(item.content, item.tags, {
      showPreview: false,
    });

    // Delete from server
    await deleteFromServer(creds.endpoint, creds.apiKey, item.id);

    // Remove from store
    inboxItems.update(items => items.filter(i => i.id !== item.id));

    toast.success(get(_)('inbox.accepted'));
  }

  /**
   * Delete an inbox item from the server
   */
  async deleteItem(itemId: string): Promise<void> {
    const creds = await getSyncCredentials();
    if (!creds) throw new Error('Sync not configured');

    await deleteFromServer(creds.endpoint, creds.apiKey, itemId);

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
