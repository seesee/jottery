/**
 * Conflict Resolution Service
 * Handles sync conflict resolution with four strategies:
 * - Keep Mine: Discard server version, push local
 * - Keep Server: Replace local with server version
 * - Keep Both: Create duplicate note from server version
 * - Merge: Save merged content as new version
 */

import type { Note, ConflictData, Attachment } from '../types';
import { noteRepository } from './noteRepository';
import { syncRepository } from './syncRepository';
import { noteService } from './noteService';
import { cryptoService, encryptStringArray, updateHashChain } from './crypto';
import { keyManager } from './keyManager';
import { ApplicationLockedError, NotFoundError } from '../errors';

export type ConflictResolution = 'keep-mine' | 'keep-server' | 'keep-both' | 'merge';

/**
 * Decrypt server tags from sync format (each tag individually encrypted)
 */
async function decryptSyncFormatTags(encryptedTags: string[], key: CryptoKey): Promise<string[]> {
  const tags: string[] = [];
  for (const encryptedTagJson of encryptedTags) {
    try {
      const encryptedTag = JSON.parse(encryptedTagJson);
      const tagJson = await cryptoService.decryptText(encryptedTag, key);
      const tag = JSON.parse(tagJson); // Tags are JSON-encoded in sync format
      if (typeof tag === 'string' && tag.trim().length > 0) {
        tags.push(tag);
      }
    } catch (error) {
      console.warn('[ConflictService] Failed to decrypt server tag:', error);
    }
  }
  return tags;
}

export interface ConflictInfo {
  noteId: string;
  localNote: Note;
  serverContent: string;           // Decrypted server content
  serverTags: string[];            // Decrypted server tags
  serverModifiedAt: string;
  serverPinned: boolean;
  serverSyntaxLanguage?: string;
  serverWordWrap?: boolean;
  detectedAt: string;
  // Hash chain fields for git-like conflict detection
  serverContentHash?: string;
  serverParentHash?: string | null;
  // Ancestor data for 3-way merge
  ancestorHash?: string;
  ancestorContent?: string;        // Decrypted ancestor content
  ancestorTags?: string[];         // Decrypted ancestor tags
}

/**
 * Get all notes currently in conflict state
 */
export async function getConflicts(): Promise<string[]> {
  const conflictIds: string[] = [];
  const notes = await noteRepository.getAllActive();

  for (const note of notes) {
    const syncMeta = await syncRepository.getNoteSyncMetadata(note.id);
    if (syncMeta?.lastSyncStatus === 'conflict' && syncMeta.conflictData) {
      conflictIds.push(note.id);
    }
  }

  return conflictIds;
}

/**
 * Get detailed conflict info for a specific note
 * Returns decrypted content for both local and server versions
 */
export async function getConflictInfo(noteId: string): Promise<ConflictInfo | null> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  const localNote = await noteRepository.getById(noteId);
  if (!localNote) {
    return null;
  }

  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  if (!syncMeta?.conflictData) {
    return null;
  }

  const conflict = syncMeta.conflictData;

  // Decrypt server content
  let serverContent: string;
  try {
    const encryptedContent = JSON.parse(conflict.serverContent);
    serverContent = await cryptoService.decryptText(encryptedContent, masterKey.key);
  } catch {
    serverContent = conflict.serverContent;
  }

  // Decrypt server tags (sync format: each tag individually encrypted)
  const serverTags = await decryptSyncFormatTags(conflict.serverTags, masterKey.key);

  // Decrypt ancestor content if available (for 3-way merge)
  let ancestorContent: string | undefined;
  let ancestorTags: string[] | undefined;

  if (conflict.ancestorContent) {
    try {
      const encryptedAncestorContent = JSON.parse(conflict.ancestorContent);
      ancestorContent = await cryptoService.decryptText(encryptedAncestorContent, masterKey.key);
    } catch {
      // Fall through - ancestor content unavailable
    }
  }

  if (conflict.ancestorTags) {
    ancestorTags = await decryptSyncFormatTags(conflict.ancestorTags, masterKey.key);
  }

  return {
    noteId,
    localNote,
    serverContent,
    serverTags,
    serverModifiedAt: conflict.serverModifiedAt,
    serverPinned: conflict.serverPinned,
    serverSyntaxLanguage: conflict.serverSyntaxLanguage,
    serverWordWrap: conflict.serverWordWrap,
    detectedAt: conflict.detectedAt,
    // Hash chain fields
    serverContentHash: conflict.serverContentHash,
    serverParentHash: conflict.serverParentHash,
    // Ancestor data for 3-way merge
    ancestorHash: conflict.ancestorHash,
    ancestorContent,
    ancestorTags,
  };
}

/**
 * Resolve conflict by keeping the local version
 * Clears conflict status and marks note for sync with updated timestamp
 */
export async function resolveKeepMine(noteId: string): Promise<void> {
  const note = await noteRepository.getById(noteId);
  if (!note) {
    throw new NotFoundError('Note', noteId);
  }

  // Get conflict data for server's hash info
  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  const conflict = syncMeta?.conflictData;

  // Recompute content hash for current local content
  const contentHash = await cryptoService.computeContentHash(note);

  // Set parentHash to server's contentHash (acknowledges server version)
  // This makes the next push a fast-forward from the server's perspective
  const serverChain = conflict?.serverHashChain || [];
  const parentHash = conflict?.serverContentHash || note.parentHash;

  // Merge hash chains: local content hash + server hash + server chain
  const hashChain = updateHashChain(contentHash,
    [conflict?.serverContentHash, ...serverChain].filter((h): h is string => !!h)
  );

  // Update note with current timestamp and corrected hash chain
  const now = new Date().toISOString();
  await noteRepository.update({
    ...note,
    modifiedAt: now,
    needsSync: true,
    contentHash,
    parentHash,
    hashChain,
  });

  // Clear conflict status
  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    lastSyncStatus: 'pending',
    errorMessage: undefined,
    conflictData: undefined,
  });
}

/**
 * Resolve conflict by keeping the server version
 * Replaces local note with server data
 */
export async function resolveKeepServer(noteId: string): Promise<void> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  if (!syncMeta?.conflictData) {
    throw new NotFoundError('ConflictData', noteId);
  }

  const conflict = syncMeta.conflictData;
  const existingNote = await noteRepository.getById(noteId);
  if (!existingNote) {
    throw new NotFoundError('Note', noteId);
  }

  // Update note with server data, including hash chain fields
  await noteRepository.update({
    ...existingNote,
    content: conflict.serverContent,
    tags: conflict.serverTags,
    modifiedAt: conflict.serverModifiedAt,
    pinned: conflict.serverPinned,
    syntaxLanguage: conflict.serverSyntaxLanguage,
    wordWrap: conflict.serverWordWrap,
    attachments: conflict.serverAttachments as Attachment[],
    needsSync: false,
    contentHash: conflict.serverContentHash,
    parentHash: conflict.serverParentHash,
    hashChain: conflict.serverHashChain || [],
  });

  // Clear conflict and mark as synced with matching syncHash
  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    syncedAt: new Date().toISOString(),
    syncHash: conflict.serverContentHash || '',
    serverVersion: conflict.serverVersion,
    lastSyncStatus: 'synced',
    errorMessage: undefined,
    conflictData: undefined,
  });
}

/**
 * Resolve conflict by keeping both versions
 * Creates a new note from server version, keeps local as-is
 */
export async function resolveKeepBoth(noteId: string): Promise<string> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  if (!syncMeta?.conflictData) {
    throw new NotFoundError('ConflictData', noteId);
  }

  const conflict = syncMeta.conflictData;

  // Decrypt server content for creating new note
  let serverContent: string;
  try {
    const encryptedContent = JSON.parse(conflict.serverContent);
    serverContent = await cryptoService.decryptText(encryptedContent, masterKey.key);
  } catch {
    serverContent = conflict.serverContent;
  }

  // Decrypt server tags (sync format: each tag individually encrypted)
  const serverTags = await decryptSyncFormatTags(conflict.serverTags, masterKey.key);

  // Add a tag to indicate this is a conflict copy
  serverTags.push('conflict-copy');

  // Create new note from server version
  const newNote = await noteService.createNote(serverContent, serverTags, {
    pinned: conflict.serverPinned,
    syntaxLanguage: conflict.serverSyntaxLanguage as 'markdown' | 'javascript' | 'python' | 'json' | 'css' | 'bash' | 'sql' | 'plain' | 'html' | undefined,
    wordWrap: conflict.serverWordWrap,
    // Note: Attachments would need to be copied separately if needed
  });

  // Update original note's hash chain to acknowledge server version
  const now = new Date().toISOString();
  const originalNote = await noteRepository.getById(noteId);
  if (originalNote) {
    const contentHash = await cryptoService.computeContentHash(originalNote);
    const serverChain = conflict.serverHashChain || [];
    const parentHash = conflict.serverContentHash || originalNote.parentHash;
    const hashChain = updateHashChain(contentHash,
      [conflict.serverContentHash, ...serverChain].filter((h): h is string => !!h)
    );

    await noteRepository.update({
      ...originalNote,
      modifiedAt: now,
      needsSync: true,
      contentHash,
      parentHash,
      hashChain,
    });
  }

  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    lastSyncStatus: 'pending',
    errorMessage: undefined,
    conflictData: undefined,
  });

  return newNote.id;
}

/**
 * Resolve conflict with merged content
 * Saves the provided merged content as the new version
 */
export async function resolveMerge(
  noteId: string,
  mergedContent: string,
  mergedTags: string[]
): Promise<void> {
  const masterKey = keyManager.getMasterKey();
  if (!masterKey) {
    throw new ApplicationLockedError();
  }

  const note = await noteRepository.getById(noteId);
  if (!note) {
    throw new NotFoundError('Note', noteId);
  }

  // Get conflict data for server's hash info
  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  const conflict = syncMeta?.conflictData;

  // Encrypt merged content
  const encryptedContent = await cryptoService.encryptText(mergedContent, masterKey.key);
  const encryptedTags = await encryptStringArray(mergedTags, masterKey.key);

  // Build the updated note to compute its content hash
  const updatedNote = {
    ...note,
    content: JSON.stringify(encryptedContent),
    tags: [JSON.stringify(encryptedTags)],
  };

  // Compute content hash for the merged version
  const mergedContentHash = await cryptoService.computeContentHash(updatedNote);

  // Set parentHash to server's hash (acknowledges both versions)
  const serverChain = conflict?.serverHashChain || [];
  const parentHash = conflict?.serverContentHash || note.parentHash;
  const hashChain = updateHashChain(mergedContentHash,
    [conflict?.serverContentHash, ...serverChain].filter((h): h is string => !!h)
  );

  // Update note with merged content and corrected hash chain
  const now = new Date().toISOString();
  await noteRepository.update({
    ...updatedNote,
    modifiedAt: now,
    needsSync: true,
    contentHash: mergedContentHash,
    parentHash,
    hashChain,
  });

  // Clear conflict status
  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    lastSyncStatus: 'pending',
    errorMessage: undefined,
    conflictData: undefined,
  });
}

/**
 * Store conflict data when a push is rejected
 */
export async function storeConflict(
  noteId: string,
  rejected: {
    serverContent: string;
    serverTags: string[];
    serverModifiedAt: string;
    serverVersion: number;
    serverAttachments: { id: string; filename: string; mimeType: string; size: number; data: string }[];
    serverPinned: boolean;
    serverSyntaxLanguage?: string;
    serverWordWrap?: boolean;
    // Hash chain fields for git-like conflict detection
    serverContentHash?: string;
    serverParentHash?: string | null;
    serverHashChain?: string[];
    // Ancestor data for 3-way merge
    ancestorHash?: string;
    ancestorContent?: string;
    ancestorTags?: string[];
  }
): Promise<void> {
  const conflictData: ConflictData = {
    noteId,
    serverContent: rejected.serverContent,
    serverTags: rejected.serverTags,
    serverModifiedAt: rejected.serverModifiedAt,
    serverVersion: rejected.serverVersion,
    serverAttachments: rejected.serverAttachments,
    serverPinned: rejected.serverPinned,
    serverSyntaxLanguage: rejected.serverSyntaxLanguage,
    serverWordWrap: rejected.serverWordWrap,
    detectedAt: new Date().toISOString(),
    // Hash chain fields
    serverContentHash: rejected.serverContentHash,
    serverParentHash: rejected.serverParentHash,
    serverHashChain: rejected.serverHashChain,
    // Ancestor data for 3-way merge
    ancestorHash: rejected.ancestorHash,
    ancestorContent: rejected.ancestorContent,
    ancestorTags: rejected.ancestorTags,
  };

  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    lastSyncStatus: 'conflict',
    errorMessage: 'Conflict: diverged from common ancestor - manual resolution required',
    conflictData,
  });
}

export const conflictService = {
  getConflicts,
  getConflictInfo,
  resolveKeepMine,
  resolveKeepServer,
  resolveKeepBoth,
  resolveMerge,
  storeConflict,
};
