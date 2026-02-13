/**
 * Note Title Utilities
 *
 * Provides functions for extracting and managing note titles.
 * Supports custom titles via t: or title: tag prefixes, falling back to first line extraction.
 */

import { TITLE_TAG_PREFIX, isTitleTag, getTitleFromTag } from './virtualTags';

/**
 * Interface for note data needed for title extraction.
 */
export interface NoteTitleData {
  content: string;
  tags: string[];
}

/**
 * Extract the title from a note.
 * Checks for a t: or title: tag first, then falls back to the first non-empty line.
 *
 * @param note The note data (content and tags)
 * @returns The note title
 */
export function getNoteTitle(note: NoteTitleData): string {
  // Check for title tag first (supports t: and title: prefixes)
  const titleTag = note.tags.find((tag) => isTitleTag(tag));
  if (titleTag) {
    const title = getTitleFromTag(titleTag)?.trim();
    if (title) return title;
  }

  // Fall back to first non-empty line
  const firstLine = note.content.split('\n').find((line) => line.trim());
  return firstLine?.trim() || 'Untitled';
}

/**
 * Get the title tag value (without prefix), or undefined if not present.
 *
 * @param tags Array of tags
 * @returns The title value or undefined
 */
export function getTitleTagValue(tags: string[]): string | undefined {
  const titleTag = tags.find((tag) => isTitleTag(tag));
  return titleTag ? getTitleFromTag(titleTag) : undefined;
}

/**
 * Check if a note has a custom title tag.
 *
 * @param tags Array of tags
 * @returns true if the note has a t: or title: tag
 */
export function hasCustomTitle(tags: string[]): boolean {
  return tags.some((tag) => isTitleTag(tag));
}

/**
 * Set the title tag, removing any existing title tags.
 * Returns a new tags array (does not mutate the input).
 * Always uses the canonical 't:' prefix for new tags.
 *
 * @param tags Current tags array
 * @param title The new title (empty string removes the tag)
 * @returns New tags array with the title tag set
 */
export function setTitleTag(tags: string[], title: string): string[] {
  // Filter out any existing title tags (both t: and title: prefixes)
  const filtered = tags.filter((tag) => !isTitleTag(tag));

  // Add new title tag if title is non-empty (using canonical t: prefix)
  if (title.trim()) {
    return [...filtered, `${TITLE_TAG_PREFIX}${title.trim()}`];
  }

  return filtered;
}

/**
 * Remove the title tag from tags array.
 * Returns a new tags array (does not mutate the input).
 *
 * @param tags Current tags array
 * @returns New tags array without any title tags
 */
export function removeTitleTag(tags: string[]): string[] {
  return tags.filter((tag) => !isTitleTag(tag));
}
