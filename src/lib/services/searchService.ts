/**
 * Search service with FlexSearch and query parser
 * Supports tag filtering, text search, wildcards, and boolean operators
 */

import FlexSearch from 'flexsearch';
import type { DecryptedNote, SearchQuery, SortOrder } from '../types';
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

/**
 * Index all notes for search
 */
export function indexNotes(notes: DecryptedNote[]): void {
  // Note: FlexSearch Document index doesn't have a clear method
  // So we just re-add all documents (it will update existing ones)

  // Add all notes to index
  notes.forEach((note) => {
    index.add({
      id: note.id,
      content: note.content,
      tags: note.tags.join(' '),
    });
  });
}

/**
 * Update a single note in the search index (incremental update)
 * This is much faster than re-indexing all notes
 */
export function updateNote(note: DecryptedNote): void {
  index.add({
    id: note.id,
    content: note.content,
    tags: note.tags.join(' '),
  });
}

/**
 * Remove a note from the search index
 */
export function removeNote(noteId: string): void {
  index.remove(noteId);
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

/**
 * Parse advanced search modifiers from query string
 * Returns the modifiers and the remaining query with modifiers removed
 */
function parseAdvancedModifiers(query: string): { modifiers: Partial<SearchQuery>; remainingQuery: string } {
  const modifiers: Partial<SearchQuery> = {};
  let remaining = query;

  // has:attachment
  if (MODIFIER_PATTERNS.hasAttachment.test(remaining)) {
    modifiers.hasAttachment = true;
    remaining = remaining.replace(MODIFIER_PATTERNS.hasAttachment, '');
  }

  // archive:true or archive:false (explicit value)
  const archivedMatch = remaining.match(/\barchived?:(true|false|yes|no|1|0)\b/i);
  if (archivedMatch) {
    const value = archivedMatch[1].toLowerCase();
    modifiers.archived = value === 'true' || value === 'yes' || value === '1';
    remaining = remaining.replace(/\barchived?:(true|false|yes|no|1|0)\b/gi, '');
  }
  // archive (shorthand for archive:true)
  else if (/\barchived?\b/i.test(remaining)) {
    modifiers.archived = true;
    remaining = remaining.replace(/\barchived?\b/gi, '');
  }

  // created:START..END (range - check first before single comparisons)
  const createdRangeMatch = remaining.match(/\bcreated:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})\b/);
  if (createdRangeMatch) {
    modifiers.createdAfter = createdRangeMatch[1];
    modifiers.createdBefore = createdRangeMatch[2];
    remaining = remaining.replace(/\bcreated:\d{4}-\d{2}-\d{2}\.\.\d{4}-\d{2}-\d{2}\b/, '');
  } else {
    // created:>DATE
    const createdAfterMatch = remaining.match(/\bcreated:>(\d{4}-\d{2}-\d{2})\b/);
    if (createdAfterMatch) {
      modifiers.createdAfter = createdAfterMatch[1];
      remaining = remaining.replace(/\bcreated:>\d{4}-\d{2}-\d{2}\b/, '');
    }

    // created:<DATE
    const createdBeforeMatch = remaining.match(/\bcreated:<(\d{4}-\d{2}-\d{2})\b/);
    if (createdBeforeMatch) {
      modifiers.createdBefore = createdBeforeMatch[1];
      remaining = remaining.replace(/\bcreated:<\d{4}-\d{2}-\d{2}\b/, '');
    }
  }

  // modified:START..END (range - check first before single comparisons)
  const modifiedRangeMatch = remaining.match(/\bmodified:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})\b/);
  if (modifiedRangeMatch) {
    modifiers.modifiedAfter = modifiedRangeMatch[1];
    modifiers.modifiedBefore = modifiedRangeMatch[2];
    remaining = remaining.replace(/\bmodified:\d{4}-\d{2}-\d{2}\.\.\d{4}-\d{2}-\d{2}\b/, '');
  } else {
    // modified:>DATE
    const modifiedAfterMatch = remaining.match(/\bmodified:>(\d{4}-\d{2}-\d{2})\b/);
    if (modifiedAfterMatch) {
      modifiers.modifiedAfter = modifiedAfterMatch[1];
      remaining = remaining.replace(/\bmodified:>\d{4}-\d{2}-\d{2}\b/, '');
    }

    // modified:<DATE
    const modifiedBeforeMatch = remaining.match(/\bmodified:<(\d{4}-\d{2}-\d{2})\b/);
    if (modifiedBeforeMatch) {
      modifiers.modifiedBefore = modifiedBeforeMatch[1];
      remaining = remaining.replace(/\bmodified:<\d{4}-\d{2}-\d{2}\b/, '');
    }
  }

  // words:MIN..MAX (range - check first before single comparisons)
  const wordsRangeMatch = remaining.match(/\bwords:(\d+)\.\.(\d+)\b/);
  if (wordsRangeMatch) {
    modifiers.wordCountMin = parseInt(wordsRangeMatch[1], 10);
    modifiers.wordCountMax = parseInt(wordsRangeMatch[2], 10);
    remaining = remaining.replace(/\bwords:\d+\.\.\d+\b/, '');
  } else {
    // words:>N
    const wordsMinMatch = remaining.match(/\bwords:>(\d+)\b/);
    if (wordsMinMatch) {
      modifiers.wordCountMin = parseInt(wordsMinMatch[1], 10);
      remaining = remaining.replace(/\bwords:>\d+\b/, '');
    }

    // words:<N
    const wordsMaxMatch = remaining.match(/\bwords:<(\d+)\b/);
    if (wordsMaxMatch) {
      modifiers.wordCountMax = parseInt(wordsMaxMatch[1], 10);
      remaining = remaining.replace(/\bwords:<\d+\b/, '');
    }
  }

  // color:red note (notes with red color)
  const colorNoteMatches = [...remaining.matchAll(/\bcolou?r:([a-z0-9#]+)\s+note\b/gi)];
  if (colorNoteMatches.length > 0) {
    modifiers.colors = colorNoteMatches.map(m => m[1].toLowerCase());
    modifiers.colorTarget = 'note';
    remaining = remaining.replace(/\bcolou?r:[a-z0-9#]+\s+note\b/gi, '');
  }

  // color:red tag (notes with red-colored tags)
  const colorTagMatches = [...remaining.matchAll(/\bcolou?r:([a-z0-9#]+)\s+tag\b/gi)];
  if (colorTagMatches.length > 0) {
    modifiers.colors = colorTagMatches.map(m => m[1].toLowerCase());
    modifiers.colorTarget = 'tag';
    remaining = remaining.replace(/\bcolou?r:[a-z0-9#]+\s+tag\b/gi, '');
  }

  // color:red (both notes and tags) - only if not already set by qualifiers
  if (!modifiers.colorTarget) {
    const colorSimpleMatches = [...remaining.matchAll(/\bcolou?r:([a-z0-9#]+)\b/gi)];
    if (colorSimpleMatches.length > 0) {
      modifiers.colors = colorSimpleMatches.map(m => m[1].toLowerCase());
      modifiers.colorTarget = 'both';
      remaining = remaining.replace(/\bcolou?r:[a-z0-9#]+\b/gi, '');
    }
  }

  // category:important note (notes with category named "important")
  // Category uses display names instead of color keys
  const categoryNoteMatches = [...remaining.matchAll(/\bcategory:([\w\s-]+)\s+note\b/gi)];
  if (categoryNoteMatches.length > 0) {
    const colorKeys = categoryNoteMatches
      .map(m => getColorKeyByDisplayName(m[1].trim()))
      .filter(Boolean) as string[];

    if (colorKeys.length > 0) {
      modifiers.colors = colorKeys;
      modifiers.colorTarget = 'note';
    }
    remaining = remaining.replace(/\bcategory:[\w\s-]+\s+note\b/gi, '');
  }

  // category:important tag (notes with tags in category named "important")
  const categoryTagMatches = [...remaining.matchAll(/\bcategory:([\w\s-]+)\s+tag\b/gi)];
  if (categoryTagMatches.length > 0) {
    const colorKeys = categoryTagMatches
      .map(m => getColorKeyByDisplayName(m[1].trim()))
      .filter(Boolean) as string[];

    if (colorKeys.length > 0) {
      modifiers.colors = colorKeys;
      modifiers.colorTarget = 'tag';
    }
    remaining = remaining.replace(/\bcategory:[\w\s-]+\s+tag\b/gi, '');
  }

  // category:important (both notes and tags) - only if not already set by qualifiers
  if (!modifiers.colorTarget) {
    const categorySimpleMatches = [...remaining.matchAll(/\bcategory:([\w\s-]+)\b/gi)];
    if (categorySimpleMatches.length > 0) {
      const colorKeys = categorySimpleMatches
        .map(m => getColorKeyByDisplayName(m[1].trim()))
        .filter(Boolean) as string[];

      if (colorKeys.length > 0) {
        modifiers.colors = colorKeys;
        modifiers.colorTarget = 'both';
      }
      remaining = remaining.replace(/\bcategory:[\w\s-]+\b/gi, '');
    }
  }

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

/**
 * Search notes using FlexSearch and structured query
 */
export async function searchNotes(
  query: string,
  allNotes: DecryptedNote[],
  sortOrder: SortOrder = 'recent'
): Promise<DecryptedNote[]> {
  // First extract advanced modifiers from query
  const { modifiers, remainingQuery } = parseAdvancedModifiers(query);

  // Parse remaining query for text/tags
  const parsed = parseSearchQuery(remainingQuery);

  // Merge modifiers into parsed query
  Object.assign(parsed, modifiers);

  // If query is empty (no text, tags, or modifiers), return all notes
  const hasModifiers =
    parsed.hasAttachment ||
    parsed.archived !== undefined ||
    parsed.createdAfter ||
    parsed.createdBefore ||
    parsed.modifiedAfter ||
    parsed.modifiedBefore ||
    parsed.wordCountMin !== undefined ||
    parsed.wordCountMax !== undefined ||
    parsed.colors?.length ||
    parsed.excludeColors?.length;

  if (
    !parsed.text &&
    (!parsed.tags || parsed.tags.length === 0) &&
    (!parsed.orTags || parsed.orTags.length === 0) &&
    (!parsed.excludeText || parsed.excludeText.length === 0) &&
    (!parsed.excludeTags || parsed.excludeTags.length === 0) &&
    !hasModifiers
  ) {
    // No query: return all notes, excluding archived by default
    const filteredNotes = allNotes.filter((note) => !note.archived);
    return sortNotes(filteredNotes, sortOrder);
  }

  let results = [...allNotes];

  // Full-text search using FlexSearch
  if (parsed.text) {
    const searchResults = await index.searchAsync(parsed.text, {
      limit: 1000,
      enrich: true,
    });

    const matchingIds = new Set<string>();
    if (Array.isArray(searchResults)) {
      searchResults.forEach((result: any) => {
        if (result.result) {
          result.result.forEach((item: any) => {
            // FlexSearch with enrich: true returns objects with id property
            const id = typeof item === 'string' ? item : item.id;
            if (id) {
              matchingIds.add(id);
            }
          });
        }
      });
    }

    results = results.filter((note) => matchingIds.has(note.id));
  }

  // Filter by tags (AND logic)
  if (parsed.tags && parsed.tags.length > 0) {
    results = results.filter((note) =>
      parsed.tags!.every((tag) =>
        note.tags.some((noteTag) => noteTag.toLowerCase().includes(tag.toLowerCase()))
      )
    );
  }

  // Filter by tags (OR logic)
  if (parsed.orTags && parsed.orTags.length > 0) {
    results = results.filter((note) =>
      parsed.orTags!.some((tag) =>
        note.tags.some((noteTag) => noteTag.toLowerCase().includes(tag.toLowerCase()))
      )
    );
  }

  // Exclude text
  if (parsed.excludeText && parsed.excludeText.length > 0) {
    results = results.filter((note) =>
      parsed.excludeText!.every(
        (term) => !note.content.toLowerCase().includes(term.toLowerCase())
      )
    );
  }

  // Exclude tags
  if (parsed.excludeTags && parsed.excludeTags.length > 0) {
    results = results.filter((note) =>
      parsed.excludeTags!.every(
        (tag) => !note.tags.some((noteTag) => noteTag.toLowerCase().includes(tag.toLowerCase()))
      )
    );
  }

  // Advanced modifier filters

  // has:attachment
  if (parsed.hasAttachment) {
    results = results.filter((note) => note.attachments && note.attachments.length > 0);
  }

  // archive: or archive:true/false
  // Default behavior: exclude archived notes unless explicitly requested
  if (parsed.archived !== undefined) {
    results = results.filter((note) => note.archived === parsed.archived);
  } else {
    // No archive modifier specified: exclude archived notes by default
    results = results.filter((note) => !note.archived);
  }

  // created:>DATE (created after)
  if (parsed.createdAfter) {
    results = results.filter((note) => compareDates(note.createdAt, parsed.createdAfter!, 'after'));
  }

  // created:<DATE (created before)
  if (parsed.createdBefore) {
    results = results.filter((note) => compareDates(note.createdAt, parsed.createdBefore!, 'before'));
  }

  // modified:>DATE (modified after)
  if (parsed.modifiedAfter) {
    results = results.filter((note) => compareDates(note.modifiedAt, parsed.modifiedAfter!, 'after'));
  }

  // modified:<DATE (modified before)
  if (parsed.modifiedBefore) {
    results = results.filter((note) => compareDates(note.modifiedAt, parsed.modifiedBefore!, 'before'));
  }

  // words:>N (minimum word count)
  if (parsed.wordCountMin !== undefined) {
    results = results.filter((note) => countWords(note.content) >= parsed.wordCountMin!);
  }

  // words:<N (maximum word count)
  if (parsed.wordCountMax !== undefined) {
    results = results.filter((note) => countWords(note.content) <= parsed.wordCountMax!);
  }

  // Color filter
  if (parsed.colors && parsed.colors.length > 0) {
    const target = parsed.colorTarget || 'both';

    results = results.filter((note) => {
      const noteHasColor = parsed.colors!.some(
        color => note.color?.toLowerCase() === color.toLowerCase()
      );

      let tagHasColor = false;
      if (target === 'tag' || target === 'both') {
        // Check if any of the note's tags have matching color
        tagHasColor = note.tags.some(tag => {
          const tagColor = getTagColor(tag);
          return tagColor && parsed.colors!.some(
            color => tagColor.toLowerCase() === color.toLowerCase()
          );
        });
      }

      if (target === 'note') return noteHasColor;
      if (target === 'tag') return tagHasColor;
      return noteHasColor || tagHasColor;  // 'both'
    });
  }

  // Exclude colors
  if (parsed.excludeColors && parsed.excludeColors.length > 0) {
    results = results.filter((note) => {
      const noteColorExcluded = parsed.excludeColors!.some(
        color => note.color?.toLowerCase() === color.toLowerCase()
      );

      const tagColorExcluded = note.tags.some(tag => {
        const tagColor = getTagColor(tag);
        return tagColor && parsed.excludeColors!.some(
          color => tagColor.toLowerCase() === color.toLowerCase()
        );
      });

      return !noteColorExcluded && !tagColorExcluded;
    });
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
 * Export for use in stores and components
 */
export const searchService = {
  indexNotes,
  updateNote,
  removeNote,
  parseSearchQuery,
  searchNotes,
  getSearchSuggestions,
};
