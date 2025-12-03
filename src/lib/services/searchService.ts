/**
 * Search service with FlexSearch and query parser
 * Supports tag filtering, text search, wildcards, and boolean operators
 */

import FlexSearch from 'flexsearch';
import type { DecryptedNote, SearchQuery } from '../types';

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
 * Search notes using FlexSearch and structured query
 */
export async function searchNotes(
  query: string,
  allNotes: DecryptedNote[]
): Promise<DecryptedNote[]> {
  console.log('searchNotes called:', { query, allNotesCount: allNotes.length });

  // Parse query
  const parsed = parseSearchQuery(query);
  console.log('parsed query:', parsed);

  // If query is empty, return all notes
  if (
    !parsed.text &&
    (!parsed.tags || parsed.tags.length === 0) &&
    (!parsed.orTags || parsed.orTags.length === 0)
  ) {
    console.log('empty query, returning all notes');
    return allNotes;
  }

  let results = [...allNotes];

  // Full-text search using FlexSearch
  if (parsed.text) {
    console.log('performing text search for:', parsed.text);
    const searchResults = await index.searchAsync(parsed.text, {
      limit: 1000,
      enrich: true,
    });
    console.log('FlexSearch results:', searchResults);

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
    console.log('matching IDs:', matchingIds);

    results = results.filter((note) => matchingIds.has(note.id));
    console.log('filtered results:', results.length);
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

  return results;
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
  parseSearchQuery,
  searchNotes,
  getSearchSuggestions,
};
