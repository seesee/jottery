/**
 * Storage cleanup service for finding and removing orphaned/duplicate attachments
 */

import { noteRepository } from './noteRepository';
import { attachmentRepository } from './attachmentRepository';
import { getDB, STORES } from './db';
import type { NoteVersion } from '../types';

export interface StorageStats {
  totalAttachments: number;
  referencedAttachments: number;
  orphanedAttachments: number;
  orphanedIds: string[];
  totalSizeBytes: number;
  orphanedSizeBytes: number;
}

export interface CleanupResult {
  deletedCount: number;
  freedBytes: number;
  errors: string[];
}

/**
 * Collect all attachment IDs referenced by notes and versions
 */
async function getReferencedAttachmentIds(): Promise<Set<string>> {
  const referencedIds = new Set<string>();

  // Get all notes (including deleted ones, as they might still have attachments)
  const allNotes = await noteRepository.getAll();
  for (const note of allNotes) {
    if (note.attachments) {
      for (const attachment of note.attachments) {
        referencedIds.add(attachment.id);
      }
    }
  }

  // Also check note versions
  const db = getDB();
  const allVersions = await db.getAll(STORES.NOTE_VERSIONS) as NoteVersion[];
  for (const version of allVersions) {
    if (version.attachments) {
      for (const attachment of version.attachments) {
        referencedIds.add(attachment.id);
      }
    }
  }

  return referencedIds;
}

/**
 * Get storage statistics including orphaned attachments
 */
export async function getStorageStats(): Promise<StorageStats> {
  const allAttachmentIds = await attachmentRepository.listAllIds();
  const referencedIds = await getReferencedAttachmentIds();

  const orphanedIds: string[] = [];
  for (const id of allAttachmentIds) {
    if (!referencedIds.has(id)) {
      orphanedIds.push(id);
    }
  }

  // Calculate sizes
  let totalSizeBytes = 0;
  let orphanedSizeBytes = 0;

  for (const id of allAttachmentIds) {
    const blob = await attachmentRepository.getBlob(id);
    const thumbnail = await attachmentRepository.getThumbnail(id);
    const size = (blob?.byteLength || 0) + (thumbnail?.byteLength || 0);
    totalSizeBytes += size;

    if (orphanedIds.includes(id)) {
      orphanedSizeBytes += size;
    }
  }

  return {
    totalAttachments: allAttachmentIds.length,
    referencedAttachments: referencedIds.size,
    orphanedAttachments: orphanedIds.length,
    orphanedIds,
    totalSizeBytes,
    orphanedSizeBytes,
  };
}

/**
 * Delete orphaned attachments (not referenced by any note or version)
 */
export async function cleanupOrphanedAttachments(): Promise<CleanupResult> {
  console.log('[StorageCleanup] Starting orphaned attachment cleanup...');

  const stats = await getStorageStats();
  const errors: string[] = [];
  let deletedCount = 0;
  let freedBytes = 0;

  console.log(`[StorageCleanup] Found ${stats.orphanedAttachments} orphaned attachments (${formatBytes(stats.orphanedSizeBytes)})`);

  for (const id of stats.orphanedIds) {
    try {
      // Get size before deleting
      const blob = await attachmentRepository.getBlob(id);
      const thumbnail = await attachmentRepository.getThumbnail(id);
      const size = (blob?.byteLength || 0) + (thumbnail?.byteLength || 0);

      await attachmentRepository.deleteAll(id);
      deletedCount++;
      freedBytes += size;
    } catch (error) {
      const msg = `Failed to delete attachment ${id}: ${error instanceof Error ? error.message : 'Unknown error'}`;
      console.warn(`[StorageCleanup] ${msg}`);
      errors.push(msg);
    }
  }

  console.log(`[StorageCleanup] Cleanup complete: deleted ${deletedCount} attachments, freed ${formatBytes(freedBytes)}`);

  return {
    deletedCount,
    freedBytes,
    errors,
  };
}

/**
 * Format bytes to human-readable string
 */
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/**
 * Compute SHA-256 hash of ArrayBuffer
 */
async function computeHash(data: ArrayBuffer): Promise<string> {
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}

export interface DuplicateGroup {
  hash: string;
  ids: string[];
  sizeBytes: number;
}

export interface DuplicateStats {
  totalAttachments: number;
  uniqueByContent: number;
  duplicateGroups: DuplicateGroup[];
  potentialSavingsBytes: number;
}

/**
 * Find duplicate attachments by content hash
 * Note: This is computationally expensive for many attachments
 */
export async function findDuplicateAttachments(): Promise<DuplicateStats> {
  console.log('[StorageCleanup] Scanning for duplicate attachments...');

  const allAttachmentIds = await attachmentRepository.listAllIds();
  const hashToIds = new Map<string, { ids: string[]; size: number }>();

  for (const id of allAttachmentIds) {
    const blob = await attachmentRepository.getBlob(id);
    if (blob) {
      const hash = await computeHash(blob);
      const existing = hashToIds.get(hash);
      if (existing) {
        existing.ids.push(id);
      } else {
        hashToIds.set(hash, { ids: [id], size: blob.byteLength });
      }
    }
  }

  const duplicateGroups: DuplicateGroup[] = [];
  let potentialSavingsBytes = 0;

  for (const [hash, { ids, size }] of hashToIds.entries()) {
    if (ids.length > 1) {
      duplicateGroups.push({ hash, ids, sizeBytes: size });
      // We could save (n-1) * size bytes by deduplicating
      potentialSavingsBytes += (ids.length - 1) * size;
    }
  }

  console.log(`[StorageCleanup] Found ${duplicateGroups.length} groups of duplicates, potential savings: ${formatBytes(potentialSavingsBytes)}`);

  return {
    totalAttachments: allAttachmentIds.length,
    uniqueByContent: hashToIds.size,
    duplicateGroups,
    potentialSavingsBytes,
  };
}

export const storageCleanupService = {
  getStorageStats,
  cleanupOrphanedAttachments,
  findDuplicateAttachments,
};
