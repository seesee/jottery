/**
 * Search service and query parser
 * Supports tag filtering, text search, wildcards, and boolean operators
 *
 * Each term is evaluated against the note's text. A word index can't answer these
 * queries: a plain word has to match inside a longer word, and a phrase has to match
 * in order, so an index could only ever narrow the candidates, and would miss some.
 */

import type { DecryptedNote, SearchQuery, SearchTerm, SortOrder } from '../types';
import { getTagColor, getColorKeyByDisplayName } from './colorService';
import { getNoteTitle } from '../utils/noteTitle';
import { compareTimestamps } from '../utils/timestamps';
import { normaliseForComparison, localeIncludes, localeSort } from '../utils/stringUtils';

/** A note's text, normalised once so a search doesn't redo it for every keystroke. */
interface SearchableNote {
  source: DecryptedNote;
  content: string;
  tags: string[];
  text: string;
  words?: string[];
}

const searchCache = new Map<string, SearchableNote>();

function toSearchable(note: DecryptedNote): SearchableNote {
  const content = normaliseForComparison(note.content);
  const tags = note.tags.map(normaliseForComparison);
  return { source: note, content, tags, text: `${content} ${tags.join(' ')}` };
}

/** The cached form of a note. A different object means it has changed, so it is rebuilt. */
function searchableNote(note: DecryptedNote): SearchableNote {
  const cached = searchCache.get(note.id);
  if (cached?.source === note) {
    return cached;
  }

  const searchable = toSearchable(note);
  searchCache.set(note.id, searchable);
  return searchable;
}

/**
 * Index all notes for search
 * Removes entries for notes that no longer exist in the provided array
 */
export function indexNotes(notes: DecryptedNote[]): void {
  searchCache.clear();
  notes.forEach(note => searchCache.set(note.id, toSearchable(note)));
}

/**
 * Update a single note in the search index (incremental update)
 * This is much faster than re-indexing all notes
 */
export function updateNote(note: DecryptedNote): void {
  searchCache.set(note.id, toSearchable(note));
}

/**
 * Remove a note from the search index
 */
export function removeNote(noteId: string): void {
  searchCache.delete(noteId);
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

type QueryToken =
  | { kind: 'or' }
  | { kind: 'term'; term: SearchTerm }
  | { kind: 'excludeText'; value: string }
  | { kind: 'excludeTag'; value: string };

/** Classify a word by its wildcards. A word of only asterisks is a search for them. */
function classifyWord(word: string): SearchTerm {
  const value = normaliseForComparison(word.replace(/^\*+|\*+$/g, ''));
  if (!value) return { type: 'contains', value: word };

  const leading = word.startsWith('*');
  const trailing = word.endsWith('*');
  if (leading && !trailing) return { type: 'suffix', value };
  if (trailing && !leading) return { type: 'prefix', value };
  return { type: 'contains', value };
}

function tokeniseWord(word: string): QueryToken {
  if (word.startsWith('-#') && word.length > 2) {
    return { kind: 'excludeTag', value: normaliseForComparison(word.slice(2)) };
  }
  if (word.startsWith('#') && word.length > 1) {
    return { kind: 'term', term: { type: 'tag', value: normaliseForComparison(word.slice(1)) } };
  }
  if (word.startsWith('-') && word.length > 1) {
    return { kind: 'excludeText', value: normaliseForComparison(word.slice(1)) };
  }
  return { kind: 'term', term: classifyWord(word) };
}

function tokeniseQuery(query: string): QueryToken[] {
  const tokens: QueryToken[] = [];
  let i = 0;

  while (i < query.length) {
    if (/\s/.test(query[i])) {
      i++;
      continue;
    }

    if (query[i] === '|') {
      tokens.push({ kind: 'or' });
      i++;
      continue;
    }

    const negatedPhrase = query.startsWith('-"', i);
    if (query[i] === '"' || negatedPhrase) {
      // An unclosed quote runs to the end of the query, so a phrase can be typed
      // and searched for before the closing quote is reached
      const start = i + (negatedPhrase ? 2 : 1);
      const end = query.indexOf('"', start);
      const value = normaliseForComparison(end === -1 ? query.slice(start) : query.slice(start, end));
      i = end === -1 ? query.length : end + 1;

      if (value) {
        tokens.push(negatedPhrase ? { kind: 'excludeText', value } : { kind: 'term', term: { type: 'phrase', value } });
      }
      continue;
    }

    let end = i;
    while (end < query.length && !/\s/.test(query[end]) && query[end] !== '|') {
      end++;
    }
    tokens.push(tokeniseWord(query.slice(i, end)));
    i = end;
  }

  return tokens;
}

/**
 * Parse search query string into structured query. Every client follows the same
 * rules, which are written down with test cases in docs/search-query-fixture.json.
 *
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
 * - text*, *text, *text* - Starts with, ends with, contains anywhere
 *
 * Handled before this, by parseAdvancedModifiers:
 * - has:attachment - Notes with attachments
 * - created:>DATE, created:<DATE, created:DATE..DATE - Created date filters
 * - modified:>DATE, modified:<DATE, modified:DATE..DATE - Modified date filters
 * - words:>N, words:<N, words:N..N - Word count filters
 * - color:KEY, color:KEY note, color:KEY tag - Search by color key (e.g., red, blue)
 * - category:NAME, category:NAME note, category:NAME tag - Search by category display name
 */
export function parseSearchQuery(query: string): SearchQuery {
  const parsed: SearchQuery = {
    groups: [],
    excludeText: [],
    excludeTags: [],
  };

  let currentGroup: SearchTerm[] | undefined;
  let joinToCurrent = false;

  for (const token of tokeniseQuery(query)) {
    if (token.kind === 'or') {
      joinToCurrent = currentGroup !== undefined;
    } else if (token.kind === 'term') {
      if (joinToCurrent && currentGroup) {
        currentGroup.push(token.term);
      } else {
        currentGroup = [token.term];
        parsed.groups!.push(currentGroup);
      }
      joinToCurrent = false;
    } else {
      // An exclusion always applies, so it ends any OR group
      (token.kind === 'excludeTag' ? parsed.excludeTags! : parsed.excludeText!).push(token.value);
      joinToCurrent = false;
    }
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
      sortFn = (a, b) => compareTimestamps(b.modifiedAt, a.modifiedAt);
      break;
    case 'oldest':
      sortFn = (a, b) => compareTimestamps(a.modifiedAt, b.modifiedAt);
      break;
    case 'created':
      sortFn = (a, b) => compareTimestamps(b.createdAt, a.createdAt);
      break;
    case 'alpha':
      sortFn = (a, b) => localeSort(getNoteTitle(a), getNoteTitle(b));
      break;
    default:
      sortFn = (a, b) => compareTimestamps(b.modifiedAt, a.modifiedAt);
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

/** The words of a note, split as every client splits them. */
function wordsOf(note: SearchableNote): string[] {
  note.words ??= note.text.split(/[\s\p{P}]+/u).filter(Boolean);
  return note.words;
}

function matchesTerm(note: SearchableNote, term: SearchTerm): boolean {
  switch (term.type) {
    case 'tag':
      return note.tags.some(tag => tag.includes(term.value));
    case 'prefix':
      return wordsOf(note).some(word => word.startsWith(term.value));
    case 'suffix':
      return wordsOf(note).some(word => word.endsWith(term.value));
    case 'contains':
    case 'phrase':
      return note.text.includes(term.value);
  }
}

/** Every group has to match, through any one of its terms. */
function matchesGroups(note: SearchableNote, groups: SearchTerm[][]): boolean {
  return groups.every(group => group.some(term => matchesTerm(note, term)));
}

function matchesExclusions(note: SearchableNote, excludeText: string[], excludeTags: string[]): boolean {
  return excludeText.every(term => !note.content.includes(term))
    && excludeTags.every(term => !note.tags.some(tag => tag.includes(term)));
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
  const normColors = colors.map(normaliseForComparison);
  const noteHasColor = note.color
    ? normColors.includes(normaliseForComparison(note.color))
    : false;

  let tagHasColor = false;
  if (target === 'tag' || target === 'both') {
    tagHasColor = note.tags.some(tag => {
      const tagColor = getTagColor(tag);
      return tagColor && normColors.includes(normaliseForComparison(tagColor));
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
  const normExcluded = excludeColors.map(normaliseForComparison);
  return notes.filter((note) => {
    const noteColorExcluded = note.color
      ? normExcluded.includes(normaliseForComparison(note.color))
      : false;

    const tagColorExcluded = note.tags.some(tag => {
      const tagColor = getTagColor(tag);
      return tagColor && normExcluded.includes(normaliseForComparison(tagColor));
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
    (!parsed.groups || parsed.groups.length === 0) &&
    (!parsed.excludeText || parsed.excludeText.length === 0) &&
    (!parsed.excludeTags || parsed.excludeTags.length === 0) &&
    !hasSearchModifiers(parsed)
  );
}

/**
 * Search notes using a structured query
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

  const groups = parsed.groups ?? [];
  const excludeText = parsed.excludeText ?? [];
  const excludeTags = parsed.excludeTags ?? [];

  // Apply text and tag filters
  let results = allNotes.filter((candidate) => {
    const note = searchableNote(candidate);
    return matchesGroups(note, groups) && matchesExclusions(note, excludeText, excludeTags);
  });

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
    const tagQuery = query.slice(1);
    const allTags = new Set<string>();

    allNotes.forEach((note) => {
      note.tags.forEach((tag) => {
        if (localeIncludes(tag, tagQuery)) {
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
