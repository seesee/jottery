/**
 * Tests for Note Title Utilities
 *
 * Tests title extraction, custom title tags, and tag manipulation functions.
 */
import { describe, it, expect } from 'vitest';
import {
  getNoteTitle,
  getTitleTagValue,
  hasCustomTitle,
  setTitleTag,
  removeTitleTag,
  type NoteTitleData,
} from './noteTitle';

describe('noteTitle', () => {
  describe('getNoteTitle', () => {
    it('should extract first line when no title tag present', () => {
      const note: NoteTitleData = {
        content: 'My First Line\nSecond line\nThird line',
        tags: ['project', 'work'],
      };
      expect(getNoteTitle(note)).toBe('My First Line');
    });

    it('should return custom title from t: tag', () => {
      const note: NoteTitleData = {
        content: 'This content is ignored\nAs a title',
        tags: ['project', 't:My Custom Title', 'work'],
      };
      expect(getNoteTitle(note)).toBe('My Custom Title');
    });

    it('should return custom title from title: tag', () => {
      const note: NoteTitleData = {
        content: 'This content is ignored\nAs a title',
        tags: ['project', 'title:My Custom Title', 'work'],
      };
      expect(getNoteTitle(note)).toBe('My Custom Title');
    });

    it('should trim whitespace from custom title', () => {
      const note: NoteTitleData = {
        content: 'Content',
        tags: ['t:  Spaced Title  '],
      };
      expect(getNoteTitle(note)).toBe('Spaced Title');
    });

    it('should fall back to first line if title tag is empty', () => {
      const note: NoteTitleData = {
        content: 'First Line Content',
        tags: ['t:', 'other'],
      };
      expect(getNoteTitle(note)).toBe('First Line Content');
    });

    it('should fall back to first line if title tag has only whitespace', () => {
      const note: NoteTitleData = {
        content: 'First Line Content',
        tags: ['t:   ', 'other'],
      };
      expect(getNoteTitle(note)).toBe('First Line Content');
    });

    it('should skip empty first lines', () => {
      const note: NoteTitleData = {
        content: '\n\n  \nActual First Line\nMore content',
        tags: [],
      };
      expect(getNoteTitle(note)).toBe('Actual First Line');
    });

    it('should return "Untitled" for empty content with no title tag', () => {
      const note: NoteTitleData = {
        content: '',
        tags: [],
      };
      expect(getNoteTitle(note)).toBe('Untitled');
    });

    it('should return "Untitled" for whitespace-only content', () => {
      const note: NoteTitleData = {
        content: '   \n  \n   ',
        tags: [],
      };
      expect(getNoteTitle(note)).toBe('Untitled');
    });

    it('should trim whitespace from first line', () => {
      const note: NoteTitleData = {
        content: '  Trimmed Line  \nSecond',
        tags: [],
      };
      expect(getNoteTitle(note)).toBe('Trimmed Line');
    });

    it('should use first title tag if multiple present', () => {
      // Note: This shouldn't happen with singular tags, but test the behaviour
      const note: NoteTitleData = {
        content: 'Content',
        tags: ['t:First Title', 't:Second Title'],
      };
      expect(getNoteTitle(note)).toBe('First Title');
    });
  });

  describe('getTitleTagValue', () => {
    it('should return title value for t: prefix', () => {
      const tags = ['project', 't:My Title', 'work'];
      expect(getTitleTagValue(tags)).toBe('My Title');
    });

    it('should return title value for title: prefix', () => {
      const tags = ['project', 'title:My Title', 'work'];
      expect(getTitleTagValue(tags)).toBe('My Title');
    });

    it('should return undefined when no title tag', () => {
      const tags = ['project', 'work'];
      expect(getTitleTagValue(tags)).toBeUndefined();
    });

    it('should return empty string for empty title tag', () => {
      const tags = ['t:', 'other'];
      expect(getTitleTagValue(tags)).toBe('');
    });

    it('should handle empty tags array', () => {
      expect(getTitleTagValue([])).toBeUndefined();
    });

    it('should return first title if multiple present', () => {
      const tags = ['t:First', 't:Second'];
      expect(getTitleTagValue(tags)).toBe('First');
    });
  });

  describe('hasCustomTitle', () => {
    it('should return true for t: prefix', () => {
      const tags = ['project', 't:My Title', 'work'];
      expect(hasCustomTitle(tags)).toBe(true);
    });

    it('should return true for title: prefix', () => {
      const tags = ['project', 'title:My Title', 'work'];
      expect(hasCustomTitle(tags)).toBe(true);
    });

    it('should return false when no title tag', () => {
      const tags = ['project', 'work'];
      expect(hasCustomTitle(tags)).toBe(false);
    });

    it('should return true even for empty title tag', () => {
      const tags = ['t:', 'other'];
      expect(hasCustomTitle(tags)).toBe(true);
    });

    it('should handle empty tags array', () => {
      expect(hasCustomTitle([])).toBe(false);
    });

    it('should not match similar non-title tags', () => {
      const tags = ['title', 'test', 't'];
      expect(hasCustomTitle(tags)).toBe(false);
    });
  });

  describe('setTitleTag', () => {
    it('should add title tag when none present', () => {
      const tags = ['project', 'work'];
      const result = setTitleTag(tags, 'New Title');
      expect(result).toEqual(['project', 'work', 't:New Title']);
    });

    it('should replace existing t: title tag', () => {
      const tags = ['project', 't:Old Title', 'work'];
      const result = setTitleTag(tags, 'New Title');
      expect(result).toEqual(['project', 'work', 't:New Title']);
    });

    it('should replace existing title: title tag with canonical t: prefix', () => {
      const tags = ['project', 'title:Old Title', 'work'];
      const result = setTitleTag(tags, 'New Title');
      expect(result).toEqual(['project', 'work', 't:New Title']);
    });

    it('should remove title tag when empty string provided', () => {
      const tags = ['project', 't:Old Title', 'work'];
      const result = setTitleTag(tags, '');
      expect(result).toEqual(['project', 'work']);
    });

    it('should remove title tag when whitespace-only provided', () => {
      const tags = ['project', 't:Old Title', 'work'];
      const result = setTitleTag(tags, '   ');
      expect(result).toEqual(['project', 'work']);
    });

    it('should trim the new title', () => {
      const tags = ['project'];
      const result = setTitleTag(tags, '  Trimmed  ');
      expect(result).toEqual(['project', 't:Trimmed']);
    });

    it('should not mutate original array', () => {
      const tags = ['project', 't:Old'];
      const result = setTitleTag(tags, 'New');
      expect(tags).toEqual(['project', 't:Old']);
      expect(result).not.toBe(tags);
    });

    it('should handle empty input array', () => {
      const result = setTitleTag([], 'New Title');
      expect(result).toEqual(['t:New Title']);
    });

    it('should remove multiple title tags and add new one', () => {
      // Shouldn't happen, but handle gracefully
      const tags = ['t:First', 'project', 't:Second'];
      const result = setTitleTag(tags, 'New');
      expect(result).toEqual(['project', 't:New']);
    });
  });

  describe('removeTitleTag', () => {
    it('should remove t: title tag', () => {
      const tags = ['project', 't:My Title', 'work'];
      const result = removeTitleTag(tags);
      expect(result).toEqual(['project', 'work']);
    });

    it('should remove title: title tag', () => {
      const tags = ['project', 'title:My Title', 'work'];
      const result = removeTitleTag(tags);
      expect(result).toEqual(['project', 'work']);
    });

    it('should return same tags if no title tag present', () => {
      const tags = ['project', 'work'];
      const result = removeTitleTag(tags);
      expect(result).toEqual(['project', 'work']);
    });

    it('should remove multiple title tags with different prefixes', () => {
      // Shouldn't happen, but handle gracefully
      const tags = ['t:First', 'project', 'title:Second'];
      const result = removeTitleTag(tags);
      expect(result).toEqual(['project']);
    });

    it('should not mutate original array', () => {
      const tags = ['project', 't:Title'];
      const result = removeTitleTag(tags);
      expect(tags).toEqual(['project', 't:Title']);
      expect(result).not.toBe(tags);
    });

    it('should handle empty array', () => {
      const result = removeTitleTag([]);
      expect(result).toEqual([]);
    });
  });
});
