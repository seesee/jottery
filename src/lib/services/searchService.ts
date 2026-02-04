/**
 * Search service with FlexSearch and query parser
 * Supports tag filtering, text search, wildcards, and boolean operators
 */

import FlexSearch from 'flexsearch';
import type { DecryptedNote, SearchQuery, SortOrder, FlexSearchEnrichedResult, FlexSearchResultItem } from '../types';
import { getTagColor, getColorKeyByDisplayName } from './colorService';

// Create FlexSearch index
const index = new FlexSearch.Document({
  document: {
    id: 'id',
    index: ['content', 'tags'],
    store: true,
  },
  tokenize: 'forward',
  cache: true,
});

// Track indexed note IDs to properly remove stale entries
const indexedIds = new Set<string>();

/**
 * Index all notes for search
 * Removes entries for notes that no longer exist in the provided array
 */
export function indexNotes(notes: DecryptedNote[]): void {
  try {
    const newIds = new Set(notes.map(n => n.id));

    // Remove entries for notes that no longer exist
    for (const id of indexedIds) {
      if (!newIds.has(id)) {
        index.remove(id);
        indexedIds.delete(id);
      }
    }

    // Add/update all current notes
    notes.forEach((note) => {
      index.add({
        id: note.id,
        content: note.content,
        tags: note.tags.join(' '),
      });
      indexedIds.add(note.id);
    });
  } catch (error) {
    console.error('FlexSearch indexing error:', error);
    // Index state may be inconsistent, but search will fall back to basic filtering
  }
}

/**
 * Update a single note in the search index (incremental update)
 * This is much faster than re-indexing all notes
 */
export function updateNote(note: DecryptedNote): void {
  try {
    index.add({
      id: note.id,
      content: note.content,
      tags: note.tags.join(' '),
    });
    indexedIds.add(note.id);
  } catch (error) {
    console.error('FlexSearch update error:', error);
  }
}

/**
 * Remove a note from the search index
 */
export function removeNote(noteId: string): void {
  try {
    index.remove(noteId);
    indexedIds.delete(noteId);
  } catch (error) {
    console.error('FlexSearch remove error:', error);
  }
}

/**
 * Regex patterns for advanced search modifiers
 */
const MODIFIER_PATTERNS = {
  hasAttachment: /\bhas:attachment\b/gi,
  archived: /\barchived?:(true|false|yes|no|1|0)\b/gi,
  archivedSimple: /\barchived?\b/gi,
  createdAfter: /\bcreated:>(\d{4}-\d{2}-\d{2})\b/g,
  createdBefore: /\bcreated:<(\d{4}-\d{2}-\d{2})\b/g,
  createdRange: /\bcreated:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})\b/g,
  modifiedAfter: /\bmodified:>(\d{4}-\d{2}-\d{2})\b/g,
  modifiedBefore: /\bmodified:<(\d{4}-\d{2}-\d{2})\b/g,
  modifiedRange: /\bmodified:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})\b/g,
  wordsMin: /\bwords:>(\d+)\b/g,
  wordsMax: /\bwords:<(\d+)\b/g,
  wordsRange: /\bwords:(\d+)\.\.(\d+)\b/g,
  colorNoteQualifier: /\bcolou?r:([a-z0-9#]+)\s+note\b/gi,
  colorTagQualifier: /\bcolou?r:([a-z0-9#]+)\s+tag\b/gi,
  colorSimple: /\bcolou?r:([a-z0-9#]+)\b/gi,
  categoryNoteQualifier: /\bcategory:([\w\s-]+)\s+note\b/gi,
  categoryTagQualifier: /\bcategory:([\w\s-]+)\s+tag\b/gi,
  categorySimple: /\bcategory:([\w\s-]+)\b/gi,
};

// ============================================================================
// Modifier Parsing Helpers
// ============================================================================

/** Parse has:attachment modifier */
function parseHasAttachmentModifier(
  query: string,
  modifiers: Partial<SearchQuery>
): string {
  if (MODIFIER_PATTERNS.hasAttachment.test(query)) {
    modifiers.hasAttachment = true;
    return query.replace(MODIFIER_PATTERNS.hasAttachment, '');
  }
  return query;
}

/** Parse archived modifier (archived, archived:true, archived:false) */
function parseArchivedModifier(
  query: string,
  modifiers: Partial<SearchQuery>
): string {
  const explicitMatch = query.match(/\barchived?:(true|false|yes|no|1|0)\b/i);
  if (explicitMatch) {
    const value = explicitMatch[1].toLowerCase();
    modifiers.archived = value === 'true' || value === 'yes' || value === '1';
    return query.replace(/\barchived?:(true|false|yes|no|1|0)\b/gi, '');
  }

  if (/\barchived?\b/i.test(query)) {
    modifiers.archived = true;
    return query.replace(/\barchived?\b/gi, '');
  }

  return query;
}

/** Parse date range or comparison modifiers (created: or modified:) */
function parseDateModifier(
  query: string,
  modifiers: Partial<SearchQuery>,
  field: 'created' | 'modified'
): string {
  const afterKey = field === 'created' ? 'createdAfter' : 'modifiedAfter';
  const beforeKey = field === 'created' ? 'createdBefore' : 'modifiedBefore';

  // Check range first (START..END)
  const rangePattern = new RegExp(`\\b${field}:(\\d{4}-\\d{2}-\\d{2})\\.\\.(\\d{4}-\\d{2}-\\d{2})\\b`);
  const rangeMatch = query.match(rangePattern);
  if (rangeMatch) {
    modifiers[afterKey] = rangeMatch[1];
    modifiers[beforeKey] = rangeMatch[2];
    return query.replace(rangePattern, '');
  }

  // Check >DATE (after)
  const afterPattern = new RegExp(`\\b${field}:>(\\d{4}-\\d{2}-\\d{2})\\b`);
  const afterMatch = query.match(afterPattern);
  if (afterMatch) {
    modifiers[afterKey] = afterMatch[1];
    query = query.replace(afterPattern, '');
  }

  // Check <DATE (before)
  const beforePattern = new RegExp(`\\b${field}:<(\\d{4}-\\d{2}-\\d{2})\\b`);
  const beforeMatch = query.match(beforePattern);
  if (beforeMatch) {
    modifiers[beforeKey] = beforeMatch[1];
    query = query.replace(beforePattern, '');
  }

  return query;
}

/** Parse word count modifier (words:>N, words:<N, words:MIN..MAX) */
function parseWordCountModifier(
  query: string,
  modifiers: Partial<SearchQuery>
): string {
  // Check range first (MIN..MAX)
  const rangeMatch = query.match(/\bwords:(\d+)\.\.(\d+)\b/);
  if (rangeMatch) {
    modifiers.wordCountMin = parseInt(rangeMatch[1], 10);
    modifiers.wordCountMax = parseInt(rangeMatch[2], 10);
    return query.replace(/\bwords:\d+\.\.\d+\b/, '');
  }

  // Check >N (minimum)
  const minMatch = query.match(/\bwords:>(\d+)\b/);
  if (minMatch) {
    modifiers.wordCountMin = parseInt(minMatch[1], 10);
    query = query.replace(/\bwords:>\d+\b/, '');
  }

  // Check <N (maximum)
  const maxMatch = query.match(/\bwords:<(\d+)\b/);
  if (maxMatch) {
    modifiers.wordCountMax = parseInt(maxMatch[1], 10);
    query = query.replace(/\bwords:<\d+\b/, '');
  }

  return query;
}

/** Parse color modifier (color:red, color:red note, color:red tag) */
function parseColorModifier(
  query: string,
  modifiers: Partial<SearchQuery>
): string {
  // color:X note (note color only)
  const noteMatches = [...query.matchAll(/\bcolou?r:([a-z0-9#]+)\s+note\b/gi)];
  if (noteMatches.length > 0) {
    modifiers.colors = noteMatches.map(m => m[1].toLowerCase());
    modifiers.colorTarget = 'note';
    return query.replace(/\bcolou?r:[a-z0-9#]+\s+note\b/gi, '');
  }

  // color:X tag (tag color only)
  const tagMatches = [...query.matchAll(/\bcolou?r:([a-z0-9#]+)\s+tag\b/gi)];
  if (tagMatches.length > 0) {
    modifiers.colors = tagMatches.map(m => m[1].toLowerCase());
    modifiers.colorTarget = 'tag';
    return query.replace(/\bcolou?r:[a-z0-9#]+\s+tag\b/gi, '');
  }

  // color:X (both notes and tags)
  if (!modifiers.colorTarget) {
    const simpleMatches = [...query.matchAll(/\bcolou?r:([a-z0-9#]+)\b/gi)];
    if (simpleMatches.length > 0) {
      modifiers.colors = simpleMatches.map(m => m[1].toLowerCase());
      modifiers.colorTarget = 'both';
      return query.replace(/\bcolou?r:[a-z0-9#]+\b/gi, '');
    }
  }

  return query;
}

/** Parse category modifier (uses display names instead of color keys) */
function parseCategoryModifier(
  query: string,
  modifiers: Partial<SearchQuery>
): string {
  // category:X note
  const noteMatches = [...query.matchAll(/\bcategory:([\w\s-]+)\s+note\b/gi)];
  if (noteMatches.length > 0) {
    const colorKeys = noteMatches
      .map(m => getColorKeyByDisplayName(m[1].trim()))
      .filter(Boolean) as string[];
    if (colorKeys.length > 0) {
      modifiers.colors = colorKeys;
      modifiers.colorTarget = 'note';
    }
    return query.replace(/\bcategory:[\w\s-]+\s+note\b/gi, '');
  }

  // category:X tag
  const tagMatches = [...query.matchAll(/\bcategory:([\w\s-]+)\s+tag\b/gi)];
  if (tagMatches.length > 0) {
    const colorKeys = tagMatches
      .map(m => getColorKeyByDisplayName(m[1].trim()))
      .filter(Boolean) as string[];
    if (colorKeys.length > 0) {
      modifiers.colors = colorKeys;
      modifiers.colorTarget = 'tag';
    }
    return query.replace(/\bcategory:[\w\s-]+\s+tag\b/gi, '');
  }

  // category:X (both)
  if (!modifiers.colorTarget) {
    const simpleMatches = [...query.matchAll(/\bcategory:([\w\s-]+)\b/gi)];
    if (simpleMatches.length > 0) {
      const colorKeys = simpleMatches
        .map(m => getColorKeyByDisplayName(m[1].trim()))
        .filter(Boolean) as string[];
      if (colorKeys.length > 0) {
        modifiers.colors = colorKeys;
        modifiers.colorTarget = 'both';
      }
      return query.replace(/\bcategory:[\w\s-]+\b/gi, '');
    }
  }

  return query;
}

/**
 * Parse advanced search modifiers from query string
 * Returns the modifiers and the remaining query with modifiers removed
 */
function parseAdvancedModifiers(query: string): { modifiers: Partial<SearchQuery>; remainingQuery: string } {
  const modifiers: Partial<SearchQuery> = {};
  let remaining = query;

  remaining = parseHasAttachmentModifier(remaining, modifiers);
  remaining = parseArchivedModifier(remaining, modifiers);
  remaining = parseDateModifier(remaining, modifiers, 'created');
  remaining = parseDateModifier(remaining, modifiers, 'modified');
  remaining = parseWordCountModifier(remaining, modifiers);
  remaining = parseColorModifier(remaining, modifiers);
  remaining = parseCategoryModifier(remaining, modifiers);

  // Clean up extra whitespace
  remaining = remaining.replace(/\s+/g, ' ').trim();

  return { modifiers, remainingQuery: remaining };
}

/**
 * Parse search query string into structured query
 * Supports:
 * - #tag - Notes with tag
 * - #tag1 #tag2 - Notes with both tags (AND)
 * - #tag1 | #tag2 - Notes with either tag (OR)
 * - text - Contains text
 * - "exact phrase" - Exact phrase match
 * - text1 text2 - Contains both (AND)
 * - text1 | text2 - Contains either (OR)
 * - -text - Does NOT contain text
 * - -#tag - Does NOT have tag
 * - has:attachment - Notes with attachments
 * - created:>DATE, created:<DATE, created:DATE..DATE - Created date filters
 * - modified:>DATE, modified:<DATE, modified:DATE..DATE - Modified date filters
 * - words:>N, words:<N, words:N..N - Word count filters
 * - color:KEY, color:KEY note, color:KEY tag - Search by color key (e.g., red, blue)
 * - category:NAME, category:NAME note, category:NAME tag - Search by category display name
 */
export function parseSearchQuery(query: string): SearchQuery {
  const parsed: SearchQuery = {
    text: undefined,
    tags: [],
    orTags: [],
    excludeText: [],
    excludeTags: [],
  };

  if (!query.trim()) return parsed;

  const tokens: string[] = [];
  let current = '';
  let inQuotes = false;

  // Tokenize
  for (let i = 0; i < query.length; i++) {
    const char = query[i];

    if (char === '"') {
      if (inQuotes) {
        tokens.push(current);
        current = '';
      }
      inQuotes = !inQuotes;
    } else if (char === ' ' && !inQuotes) {
      if (current) {
        tokens.push(current);
        current = '';
      }
    } else {
      current += char;
    }
  }

  if (current) tokens.push(current);

  // Parse tokens
  const textTerms: string[] = [];
  const orTextTerms: string[] = [];
  let expectOr = false;

  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];

    // Handle OR operator
    if (token === '|') {
      expectOr = true;
      continue;
    }

    // Exclude operator
    if (token.startsWith('-')) {
      const term = token.slice(1);
      if (term.startsWith('#')) {
        parsed.excludeTags!.push(term.slice(1));
      } else {
        parsed.excludeText!.push(term);
      }
      expectOr = false;
      continue;
    }

    // Tag
    if (token.startsWith('#')) {
      const tag = token.slice(1);
      if (expectOr) {
        parsed.orTags!.push(tag);
      } else {
        parsed.tags!.push(tag);
      }
      expectOr = false;
      continue;
    }

    // Regular text
    if (expectOr) {
      orTextTerms.push(token);
    } else {
      textTerms.push(token);
    }
    expectOr = false;
  }

  // Combine text terms
  if (textTerms.length > 0 || orTextTerms.length > 0) {
    parsed.text = [...textTerms, ...orTextTerms].join(' ');
  }

  return parsed;
}

/**
 * Count words in a string
 */
function countWords(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
}

/**
 * Compare ISO date strings (YYYY-MM-DD format)
 * Returns true if noteDate >= filterDate for "after" comparisons
 * Returns true if noteDate <= filterDate for "before" comparisons
 */
function compareDates(noteDate: string, filterDate: string, comparison: 'after' | 'before'): boolean {
  // Extract just the date part from ISO timestamp
  const noteDatePart = noteDate.slice(0, 10);
  if (comparison === 'after') {
    return noteDatePart >= filterDate;
  }
  return noteDatePart <= filterDate;
}

/**
 * Sort notes by specified order, with pinned notes always first
 */
function sortNotes(notes: DecryptedNote[], sortOrder: SortOrder): DecryptedNote[] {
  const sorted = [...notes];

  // Separate pinned and unpinned
  const pinned = sorted.filter(n => n.pinned);
  const unpinned = sorted.filter(n => !n.pinned);

  // Get sort function based on sort order
  let sortFn: (a: DecryptedNote, b: DecryptedNote) => number;
  switch (sortOrder) {
    case 'recent':
      sortFn = (a, b) => b.modifiedAt.localeCompare(a.modifiedAt);
      break;
    case 'oldest':
      sortFn = (a, b) => a.modifiedAt.localeCompare(b.modifiedAt);
      break;
    case 'created':
      sortFn = (a, b) => b.createdAt.localeCompare(a.createdAt);
      break;
    case 'alpha':
      sortFn = (a, b) => {
        const aTitle = a.content.split('\n')[0].toLowerCase();
        const bTitle = b.content.split('\n')[0].toLowerCase();
        return aTitle.localeCompare(bTitle);
      };
      break;
    default:
      sortFn = (a, b) => b.modifiedAt.localeCompare(a.modifiedAt);
  }

  // Sort each group
  pinned.sort(sortFn);
  unpinned.sort(sortFn);

  // Pinned notes always come first
  return [...pinned, ...unpinned];
}

// ============================================================================
// Search Filter Helpers
// ============================================================================

/** Perform full-text search using FlexSearch */
async function filterByFullText(
  notes: DecryptedNote[],
  searchText: string
): Promise<DecryptedNote[]> {
  try {
    const searchResults = await index.searchAsync(searchText, {
      limit: 1000,
      enrich: true,
    });

    const matchingIds = new Set<string>();
    if (Array.isArray(searchResults)) {
      (searchResults as unknown as FlexSearchEnrichedResult[]).forEach((result) => {
        if (result.result) {
          result.result.forEach((item: string | FlexSearchResultItem) => {
            const id = typeof item === 'string' ? item : item.id;
            if (id) {
              matchingIds.add(id);
            }
          });
        }
      });
    }

    return notes.filter((note) => matchingIds.has(note.id));
  } catch (error) {
    console.error('FlexSearch search error, falling back to basic filter:', error);
    // Graceful degradation: fall back to basic case-insensitive text matching
    const lowerSearchText = searchText.toLowerCase();
    return notes.filter((note) =>
      note.content.toLowerCase().includes(lowerSearchText) ||
      note.tags.some((tag) => tag.toLowerCase().includes(lowerSearchText))
    );
  }
}

/** Filter notes by required tags (AND logic) */
function filterByRequiredTags(
  notes: DecryptedNote[],
  tags: string[]
): DecryptedNote[] {
  return notes.filter((note) =>
    tags.every((tag) =>
      note.tags.some((noteTag) => noteTag.toLowerCase().includes(tag.toLowerCase()))
    )
  );
}

/** Filter notes by optional tags (OR logic) */
function filterByOptionalTags(
  notes: DecryptedNote[],
  tags: string[]
): DecryptedNote[] {
  return notes.filter((note) =>
    tags.some((tag) =>
      note.tags.some((noteTag) => noteTag.toLowerCase().includes(tag.toLowerCase()))
    )
  );
}

/** Filter out notes containing excluded text */
function filterByExcludedText(
  notes: DecryptedNote[],
  excludeTerms: string[]
): DecryptedNote[] {
  return notes.filter((note) =>
    excludeTerms.every((term) => !note.content.toLowerCase().includes(term.toLowerCase()))
  );
}

/** Filter out notes with excluded tags */
function filterByExcludedTags(
  notes: DecryptedNote[],
  excludeTags: string[]
): DecryptedNote[] {
  return notes.filter((note) =>
    excludeTags.every((tag) =>
      !note.tags.some((noteTag) => noteTag.toLowerCase().includes(tag.toLowerCase()))
    )
  );
}

/** Filter notes by date constraints */
function filterByDateConstraints(
  notes: DecryptedNote[],
  parsed: SearchQuery
): DecryptedNote[] {
  let results = notes;

  if (parsed.createdAfter) {
    results = results.filter((note) => compareDates(note.createdAt, parsed.createdAfter!, 'after'));
  }
  if (parsed.createdBefore) {
    results = results.filter((note) => compareDates(note.createdAt, parsed.createdBefore!, 'before'));
  }
  if (parsed.modifiedAfter) {
    results = results.filter((note) => compareDates(note.modifiedAt, parsed.modifiedAfter!, 'after'));
  }
  if (parsed.modifiedBefore) {
    results = results.filter((note) => compareDates(note.modifiedAt, parsed.modifiedBefore!, 'before'));
  }

  return results;
}

/** Filter notes by word count constraints */
function filterByWordCount(
  notes: DecryptedNote[],
  minWords?: number,
  maxWords?: number
): DecryptedNote[] {
  let results = notes;

  if (minWords !== undefined) {
    results = results.filter((note) => countWords(note.content) >= minWords);
  }
  if (maxWords !== undefined) {
    results = results.filter((note) => countWords(note.content) <= maxWords);
  }

  return results;
}

/** Check if a note matches color criteria */
function noteMatchesColor(
  note: DecryptedNote,
  colors: string[],
  target: 'note' | 'tag' | 'both'
): boolean {
  const noteHasColor = colors.some(
    color => note.color?.toLowerCase() === color.toLowerCase()
  );

  let tagHasColor = false;
  if (target === 'tag' || target === 'both') {
    tagHasColor = note.tags.some(tag => {
      const tagColor = getTagColor(tag);
      return tagColor && colors.some(
        color => tagColor.toLowerCase() === color.toLowerCase()
      );
    });
  }

  if (target === 'note') return noteHasColor;
  if (target === 'tag') return tagHasColor;
  return noteHasColor || tagHasColor;
}

/** Filter notes by color */
function filterByColor(
  notes: DecryptedNote[],
  colors: string[],
  target: 'note' | 'tag' | 'both'
): DecryptedNote[] {
  return notes.filter((note) => noteMatchesColor(note, colors, target));
}

/** Filter out notes with excluded colors */
function filterByExcludedColors(
  notes: DecryptedNote[],
  excludeColors: string[]
): DecryptedNote[] {
  return notes.filter((note) => {
    const noteColorExcluded = excludeColors.some(
      color => note.color?.toLowerCase() === color.toLowerCase()
    );

    const tagColorExcluded = note.tags.some(tag => {
      const tagColor = getTagColor(tag);
      return tagColor && excludeColors.some(
        color => tagColor.toLowerCase() === color.toLowerCase()
      );
    });

    return !noteColorExcluded && !tagColorExcluded;
  });
}

/** Check if the parsed query has any modifiers set */
function hasSearchModifiers(parsed: SearchQuery): boolean {
  return !!(
    parsed.hasAttachment ||
    parsed.archived !== undefined ||
    parsed.createdAfter ||
    parsed.createdBefore ||
    parsed.modifiedAfter ||
    parsed.modifiedBefore ||
    parsed.wordCountMin !== undefined ||
    parsed.wordCountMax !== undefined ||
    parsed.colors?.length ||
    parsed.excludeColors?.length
  );
}

/** Check if the parsed query is empty (no search criteria) */
function isEmptyQuery(parsed: SearchQuery): boolean {
  return (
    !parsed.text &&
    (!parsed.tags || parsed.tags.length === 0) &&
    (!parsed.orTags || parsed.orTags.length === 0) &&
    (!parsed.excludeText || parsed.excludeText.length === 0) &&
    (!parsed.excludeTags || parsed.excludeTags.length === 0) &&
    !hasSearchModifiers(parsed)
  );
}

/**
 * Search notes using FlexSearch and structured query
 */
export async function searchNotes(
  query: string,
  allNotes: DecryptedNote[],
  sortOrder: SortOrder = 'recent',
  archiveMode: boolean = false
): Promise<DecryptedNote[]> {
  // Parse query into structured form
  const { modifiers, remainingQuery } = parseAdvancedModifiers(query);
  const parsed = parseSearchQuery(remainingQuery);
  Object.assign(parsed, modifiers);

  // Empty query: return all notes filtered by archive mode
  if (isEmptyQuery(parsed)) {
    const filteredNotes = allNotes.filter((note) => note.archived === archiveMode);
    return sortNotes(filteredNotes, sortOrder);
  }

  let results = [...allNotes];

  // Apply text and tag filters
  if (parsed.text) {
    results = await filterByFullText(results, parsed.text);
  }
  if (parsed.tags && parsed.tags.length > 0) {
    results = filterByRequiredTags(results, parsed.tags);
  }
  if (parsed.orTags && parsed.orTags.length > 0) {
    results = filterByOptionalTags(results, parsed.orTags);
  }
  if (parsed.excludeText && parsed.excludeText.length > 0) {
    results = filterByExcludedText(results, parsed.excludeText);
  }
  if (parsed.excludeTags && parsed.excludeTags.length > 0) {
    results = filterByExcludedTags(results, parsed.excludeTags);
  }

  // Apply modifier filters
  if (parsed.hasAttachment) {
    results = results.filter((note) => note.attachments && note.attachments.length > 0);
  }

  // Archive filter (explicit modifier overrides archiveMode)
  if (parsed.archived !== undefined) {
    results = results.filter((note) => note.archived === parsed.archived);
  } else {
    results = results.filter((note) => note.archived === archiveMode);
  }

  // Date and word count filters
  results = filterByDateConstraints(results, parsed);
  results = filterByWordCount(results, parsed.wordCountMin, parsed.wordCountMax);

  // Color filters
  if (parsed.colors && parsed.colors.length > 0) {
    results = filterByColor(results, parsed.colors, parsed.colorTarget || 'both');
  }
  if (parsed.excludeColors && parsed.excludeColors.length > 0) {
    results = filterByExcludedColors(results, parsed.excludeColors);
  }

  return sortNotes(results, sortOrder);
}

/**
 * Get search suggestions based on partial query
 */
export function getSearchSuggestions(
  query: string,
  allNotes: DecryptedNote[]
): string[] {
  const suggestions: string[] = [];

  // If query starts with #, suggest tags
  if (query.startsWith('#')) {
    const tagQuery = query.slice(1).toLowerCase();
    const allTags = new Set<string>();

    allNotes.forEach((note) => {
      note.tags.forEach((tag) => {
        if (tag.toLowerCase().includes(tagQuery)) {
          allTags.add(`#${tag}`);
        }
      });
    });

    suggestions.push(...Array.from(allTags).slice(0, 5));
  }

  return suggestions;
}

/**
 * Count search results in the opposite archive mode
 * Used to show "x notes match in archive" hint
 */
export async function countCrossModeMatches(
  query: string,
  allNotes: DecryptedNote[],
  currentArchiveMode: boolean
): Promise<number> {
  const oppositeMode = !currentArchiveMode;
  const results = await searchNotes(query, allNotes, 'recent', oppositeMode);
  return results.length;
}

/**
 * Export for use in stores and components
 */
export const searchService = {
  indexNotes,
  updateNote,
  removeNote,
  parseSearchQuery,
  searchNotes,
  getSearchSuggestions,
  countCrossModeMatches,
};
