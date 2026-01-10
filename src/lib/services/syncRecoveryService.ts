/**
 * Sync recovery service - creates a recovery note containing sync credentials
 *
 * When sync is configured, this service creates a note in the user's repository
 * containing their encrypted sync credentials. If the local credentials are lost,
 * the user can recover them from this note (if they have access via another device).
 */

import { noteService } from './noteService';
import { syncRepository } from './syncRepository';
import { encryptionRepository } from './encryptionRepository';
import { keyManager } from './keyManager';
import { cryptoService } from './crypto';

const RECOVERY_TAG = 'jottery:sync-recovery';
const RECOVERY_NOTE_TITLE = '🔐 Sync Recovery Credentials';

/**
 * Create or update a sync recovery note containing encrypted credentials
 *
 * This should be called after sync credentials are configured.
 * The note contains the same encrypted blob format as "Copy Credentials",
 * allowing recovery on a new device.
 */
export async function createSyncRecoveryNote(): Promise<void> {
  console.log('[SyncRecovery] Creating sync recovery note...');

  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    console.warn('[SyncRecovery] Cannot create recovery note - app is locked');
    return;
  }

  const metadata = await syncRepository.getMetadata();
  if (!metadata || !metadata.apiKey || !metadata.clientId || !metadata.syncEndpoint) {
    console.warn('[SyncRecovery] Cannot create recovery note - sync not configured');
    return;
  }

  const encryptionMeta = await encryptionRepository.getMetadata();
  if (!encryptionMeta) {
    console.warn('[SyncRecovery] Cannot create recovery note - encryption not initialised');
    return;
  }

  try {
    // Decrypt API key to re-encrypt in portable format
    const encryptedApiKey = JSON.parse(metadata.apiKey);
    const apiKey = await cryptoService.decryptText(encryptedApiKey, masterKey.key);

    // Generate encrypted credentials blob (same format as "Copy Credentials")
    const credentials = {
      endpoint: metadata.syncEndpoint,
      clientId: metadata.clientId,
      apiKey: apiKey,
    };
    const json = JSON.stringify(credentials);
    const encrypted = await cryptoService.encryptText(json, masterKey.key);
    const encryptedJson = JSON.stringify(encrypted);
    const credentialsBlob = `jottery:v1:${encryptionMeta.salt}.${btoa(encryptedJson)}`;

    // Build the recovery note content
    const noteContent = `${RECOVERY_NOTE_TITLE}

This note contains your sync credentials backup. Keep this note to recover sync access on a new device.

⚠️ IMPORTANT:
- This blob is encrypted with your master password
- You must use the SAME password on the new device
- Never share this blob publicly

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Server: ${metadata.syncEndpoint}
Device: ${metadata.clientId}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

CREDENTIALS BLOB (copy this entire line):

${credentialsBlob}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

To recover on a new device:
1. Install Jottery and set up with the SAME master password
2. Go to Settings → Sync → Import Credentials
3. Paste the credentials blob above

Generated: ${new Date().toISOString()}
`;

    // Check if a recovery note already exists
    const existingNote = await findRecoveryNote();

    if (existingNote) {
      // Update existing recovery note
      console.log('[SyncRecovery] Updating existing recovery note:', existingNote.id);
      await noteService.updateNote(existingNote.id, { content: noteContent });
    } else {
      // Create new recovery note
      console.log('[SyncRecovery] Creating new recovery note');
      await noteService.createNote(noteContent, [RECOVERY_TAG], {
        pinned: true,
      });
    }

    console.log('[SyncRecovery] Recovery note created/updated successfully');
  } catch (error) {
    console.error('[SyncRecovery] Failed to create recovery note:', error);
    // Don't throw - this is a non-critical feature
  }
}

/**
 * Find an existing sync recovery note by tag
 */
async function findRecoveryNote(): Promise<{ id: string } | null> {
  try {
    const allNotes = await noteService.getAllNotes('recent');

    // Look for note with recovery tag
    for (const note of allNotes) {
      if (note.tags.includes(RECOVERY_TAG)) {
        return { id: note.id };
      }
    }

    return null;
  } catch (error) {
    console.error('[SyncRecovery] Error finding recovery note:', error);
    return null;
  }
}

/**
 * Delete the sync recovery note (call when disconnecting from sync)
 */
export async function deleteSyncRecoveryNote(): Promise<void> {
  console.log('[SyncRecovery] Deleting sync recovery note...');

  const existingNote = await findRecoveryNote();
  if (existingNote) {
    await noteService.deleteNote(existingNote.id);
    console.log('[SyncRecovery] Recovery note deleted');
  }
}
