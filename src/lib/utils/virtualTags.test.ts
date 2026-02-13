/**
 * Tests for the Virtual Tag System
 *
 * Virtual tags are special tags with prefixes that have semantic meaning.
 * Currently supports:
 * - t: (title) - Custom note title
 */
import { describe, it, expect } from 'vitest';
import {
  VIRTUAL_TAGS,
  TITLE_TAG_PREFIX,
  isVirtualTag,
  getVirtualTagConfig,
  getVirtualTagDisplay,
  getRegularTags,
  getVirtualTags,
  getVirtualTagType,
} from './virtualTags';

describe('virtualTags', () => {
  describe('VIRTUAL_TAGS config', () => {
    it('should have title tag configuration', () => {
      expect(VIRTUAL_TAGS.title).toBeDefined();
      expect(VIRTUAL_TAGS.title.prefix).toBe('t:');
      expect(VIRTUAL_TAGS.title.displayKey).toBe('tags.virtual.title');
      expect(VIRTUAL_TAGS.title.singular).toBe(true);
      expect(VIRTUAL_TAGS.title.style).toBe('italic');
    });

    it('TITLE_TAG_PREFIX should match title config prefix', () => {
      expect(TITLE_TAG_PREFIX).toBe(VIRTUAL_TAGS.title.prefix);
    });
  });

  describe('isVirtualTag', () => {
    it('should return true for title tags', () => {
      expect(isVirtualTag('t:My Title')).toBe(true);
      expect(isVirtualTag('t:Another Title')).toBe(true);
      expect(isVirtualTag('t:')).toBe(true); // Empty title is still a virtual tag
    });

    it('should return false for regular tags', () => {
      expect(isVirtualTag('project')).toBe(false);
      expect(isVirtualTag('important')).toBe(false);
      expect(isVirtualTag('work')).toBe(false);
    });

    it('should return false for tags that look similar but are not virtual', () => {
      expect(isVirtualTag('title')).toBe(false);
      expect(isVirtualTag('t')).toBe(false);
      expect(isVirtualTag('title:something')).toBe(false);
    });

    it('should handle edge cases', () => {
      expect(isVirtualTag('')).toBe(false);
      expect(isVirtualTag('t::')).toBe(true); // Weird but valid
      expect(isVirtualTag('T:Title')).toBe(false); // Case-sensitive
    });
  });

  describe('getVirtualTagConfig', () => {
    it('should return config for title tags', () => {
      const config = getVirtualTagConfig('t:My Title');
      expect(config).toBeDefined();
      expect(config?.prefix).toBe('t:');
      expect(config?.displayKey).toBe('tags.virtual.title');
    });

    it('should return undefined for regular tags', () => {
      expect(getVirtualTagConfig('project')).toBeUndefined();
      expect(getVirtualTagConfig('important')).toBeUndefined();
    });

    it('should return undefined for non-matching prefixes', () => {
      expect(getVirtualTagConfig('x:something')).toBeUndefined();
      expect(getVirtualTagConfig('unknown:test')).toBeUndefined();
    });
  });

  describe('getVirtualTagDisplay', () => {
    it('should return display info for title tags', () => {
      const display = getVirtualTagDisplay('t:My Custom Title');
      expect(display).not.toBeNull();
      expect(display?.displayKey).toBe('tags.virtual.title');
      expect(display?.value).toBe('My Custom Title');
      expect(display?.style).toBe('italic');
    });

    it('should handle empty value after prefix', () => {
      const display = getVirtualTagDisplay('t:');
      expect(display).not.toBeNull();
      expect(display?.value).toBe('');
    });

    it('should preserve whitespace in title', () => {
      const display = getVirtualTagDisplay('t:  Spaced Title  ');
      expect(display?.value).toBe('  Spaced Title  ');
    });

    it('should return null for regular tags', () => {
      expect(getVirtualTagDisplay('project')).toBeNull();
      expect(getVirtualTagDisplay('important')).toBeNull();
    });
  });

  describe('getRegularTags', () => {
    it('should filter out virtual tags', () => {
      const tags = ['project', 't:My Title', 'important', 'work'];
      const regular = getRegularTags(tags);
      expect(regular).toEqual(['project', 'important', 'work']);
    });

    it('should return all tags if no virtual tags present', () => {
      const tags = ['project', 'important', 'work'];
      const regular = getRegularTags(tags);
      expect(regular).toEqual(['project', 'important', 'work']);
    });

    it('should return empty array if all tags are virtual', () => {
      const tags = ['t:Title 1', 't:Title 2'];
      const regular = getRegularTags(tags);
      expect(regular).toEqual([]);
    });

    it('should handle empty array', () => {
      const regular = getRegularTags([]);
      expect(regular).toEqual([]);
    });
  });

  describe('getVirtualTags', () => {
    it('should return only virtual tags', () => {
      const tags = ['project', 't:My Title', 'important', 'work'];
      const virtual = getVirtualTags(tags);
      expect(virtual).toEqual(['t:My Title']);
    });

    it('should return multiple virtual tags if present', () => {
      // Note: title tags are singular, but this function should still work
      const tags = ['project', 't:Title 1', 't:Title 2'];
      const virtual = getVirtualTags(tags);
      expect(virtual).toEqual(['t:Title 1', 't:Title 2']);
    });

    it('should return empty array if no virtual tags', () => {
      const tags = ['project', 'important'];
      const virtual = getVirtualTags(tags);
      expect(virtual).toEqual([]);
    });

    it('should handle empty array', () => {
      const virtual = getVirtualTags([]);
      expect(virtual).toEqual([]);
    });
  });

  describe('getVirtualTagType', () => {
    it('should return "title" for title tags', () => {
      expect(getVirtualTagType('t:My Title')).toBe('title');
      expect(getVirtualTagType('t:')).toBe('title');
    });

    it('should return undefined for regular tags', () => {
      expect(getVirtualTagType('project')).toBeUndefined();
      expect(getVirtualTagType('important')).toBeUndefined();
    });

    it('should return undefined for unknown prefixes', () => {
      expect(getVirtualTagType('x:something')).toBeUndefined();
    });
  });
});
