/**
 * Tag management service
 * Provides utilities for working with tags
 */

import type { DecryptedNote } from '../types';

/**
 * Get all unique tags from notes
 */
export function getAllTags(notes: DecryptedNote[]): string[] {
  const tagSet = new Set<string>();

  notes.forEach(note => {
    note.tags.forEach(tag => {
      if (tag.trim()) {
        tagSet.add(tag);
      }
    });
  });

  return Array.from(tagSet).sort();
}

/**
 * Get tag usage statistics
 */
export function getTagStats(notes: DecryptedNote[]): Map<string, number> {
  const tagCounts = new Map<string, number>();

  notes.forEach(note => {
    note.tags.forEach(tag => {
      if (tag.trim()) {
        tagCounts.set(tag, (tagCounts.get(tag) || 0) + 1);
      }
    });
  });

  return tagCounts;
}

/**
 * Get most popular tags
 */
export function getPopularTags(notes: DecryptedNote[], limit: number = 10): Array<{tag: string, count: number}> {
  const stats = getTagStats(notes);

  return Array.from(stats.entries())
    .map(([tag, count]) => ({ tag, count }))
    .sort((a, b) => b.count - a.count)
    .slice(0, limit);
}

/**
 * Get notes by tag
 */
export function getNotesByTag(notes: DecryptedNote[], tag: string): DecryptedNote[] {
  return notes.filter(note =>
    note.tags.some(noteTag => noteTag.toLowerCase() === tag.toLowerCase())
  );
}

/**
 * Get notes with multiple tags (AND logic)
 */
export function getNotesByTags(notes: DecryptedNote[], tags: string[]): DecryptedNote[] {
  return notes.filter(note =>
    tags.every(tag =>
      note.tags.some(noteTag => noteTag.toLowerCase() === tag.toLowerCase())
    )
  );
}

/**
 * Normalize tag (trim, lowercase for comparison)
 */
export function normalizeTag(tag: string): string {
  return tag.trim().toLowerCase();
}

/**
 * Validate tag name
 */
export function isValidTag(tag: string): boolean {
  if (!tag.trim()) return false;
  if (tag.length > 50) return false;
  // No special characters that might break search
  if (/[|()]/.test(tag)) return false;
  return true;
}

/**
 * Parse tags from string (comma or space separated)
 */
export function parseTagString(tagString: string): string[] {
  return tagString
    .split(/[,\s]+/)
    .map(tag => tag.trim())
    .filter(tag => tag.length > 0 && isValidTag(tag));
}

/**
 * Export tag service
 */
export const tagService = {
  getAllTags,
  getTagStats,
  getPopularTags,
  getNotesByTag,
  getNotesByTags,
  normalizeTag,
  isValidTag,
  parseTagString,
};
