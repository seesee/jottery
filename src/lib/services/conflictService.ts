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
import { cryptoService, decryptStringArray, encryptStringArray } from './crypto';
import { keyManager } from './keyManager';

export type ConflictResolution = 'keep-mine' | 'keep-server' | 'keep-both' | 'merge';

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
    throw new Error('Application is locked');
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

  // Decrypt server tags
  let serverTags: string[] = [];
  if (conflict.serverTags.length > 0 && conflict.serverTags[0]) {
    try {
      const encryptedTags = JSON.parse(conflict.serverTags[0]);
      serverTags = await decryptStringArray(encryptedTags, masterKey.key);
    } catch {
      serverTags = conflict.serverTags;
    }
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
  };
}

/**
 * Resolve conflict by keeping the local version
 * Clears conflict status and marks note for sync with updated timestamp
 */
export async function resolveKeepMine(noteId: string): Promise<void> {
  const note = await noteRepository.getById(noteId);
  if (!note) {
    throw new Error(`Note ${noteId} not found`);
  }

  // Update note with current timestamp to ensure it wins on next push
  const now = new Date().toISOString();
  await noteRepository.update({
    ...note,
    modifiedAt: now,
    needsSync: true,
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
    throw new Error('Application is locked');
  }

  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  if (!syncMeta?.conflictData) {
    throw new Error(`No conflict data for note ${noteId}`);
  }

  const conflict = syncMeta.conflictData;
  const existingNote = await noteRepository.getById(noteId);
  if (!existingNote) {
    throw new Error(`Note ${noteId} not found`);
  }

  // Update note with server data
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
  });

  // Clear conflict and mark as synced
  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    syncedAt: new Date().toISOString(),
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
    throw new Error('Application is locked');
  }

  const syncMeta = await syncRepository.getNoteSyncMetadata(noteId);
  if (!syncMeta?.conflictData) {
    throw new Error(`No conflict data for note ${noteId}`);
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

  // Decrypt server tags
  let serverTags: string[] = [];
  if (conflict.serverTags.length > 0 && conflict.serverTags[0]) {
    try {
      const encryptedTags = JSON.parse(conflict.serverTags[0]);
      serverTags = await decryptStringArray(encryptedTags, masterKey.key);
    } catch {
      serverTags = conflict.serverTags;
    }
  }

  // Add a tag to indicate this is a conflict copy
  serverTags.push('conflict-copy');

  // Create new note from server version
  const newNote = await noteService.createNote(serverContent, serverTags, {
    pinned: conflict.serverPinned,
    syntaxLanguage: conflict.serverSyntaxLanguage as 'markdown' | 'javascript' | 'python' | 'json' | 'css' | 'bash' | 'sql' | 'plain' | 'html' | undefined,
    wordWrap: conflict.serverWordWrap,
    // Note: Attachments would need to be copied separately if needed
  });

  // Clear conflict on original note and mark for sync
  const now = new Date().toISOString();
  const originalNote = await noteRepository.getById(noteId);
  if (originalNote) {
    await noteRepository.update({
      ...originalNote,
      modifiedAt: now,
      needsSync: true,
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
    throw new Error('Application is locked');
  }

  const note = await noteRepository.getById(noteId);
  if (!note) {
    throw new Error(`Note ${noteId} not found`);
  }

  // Encrypt merged content
  const encryptedContent = await cryptoService.encryptText(mergedContent, masterKey.key);
  const encryptedTags = await encryptStringArray(mergedTags, masterKey.key);

  // Update note with merged content
  const now = new Date().toISOString();
  await noteRepository.update({
    ...note,
    content: JSON.stringify(encryptedContent),
    tags: [JSON.stringify(encryptedTags)],
    modifiedAt: now,
    needsSync: true,
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
  };

  await syncRepository.updateNoteSyncMetadata(noteId, {
    noteId,
    lastSyncStatus: 'conflict',
    errorMessage: 'Server version is newer - manual resolution required',
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
