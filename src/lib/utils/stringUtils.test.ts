import { describe, it, expect } from 'vitest';
import {
  normaliseForComparison,
  localeEquals,
  localeIncludes,
  localeSort,
} from './stringUtils';

describe('stringUtils', () => {
  describe('normaliseForComparison', () => {
    it('should lowercase ASCII text', () => {
      expect(normaliseForComparison('Hello')).toBe('hello');
      expect(normaliseForComparison('WORLD')).toBe('world');
    });

    it('should handle accented characters', () => {
      expect(normaliseForComparison('Café')).toBe('café');
      expect(normaliseForComparison('RÉSUMÉ')).toBe('résumé');
    });

    it('should normalise composed and decomposed Unicode to the same form', () => {
      // é as single codepoint (U+00E9) vs e + combining acute (U+0065 U+0301)
      const composed = '\u00E9'; // é
      const decomposed = '\u0065\u0301'; // e + ́
      expect(normaliseForComparison(composed)).toBe(
        normaliseForComparison(decomposed)
      );
    });

    it('should handle German ß', () => {
      const result = normaliseForComparison('Straße');
      expect(result).toBe('straße');
    });

    it('should handle CJK characters unchanged', () => {
      expect(normaliseForComparison('日本語')).toBe('日本語');
    });

    it('should handle empty string', () => {
      expect(normaliseForComparison('')).toBe('');
    });
  });

  describe('localeEquals', () => {
    it('should match identical strings', () => {
      expect(localeEquals('hello', 'hello')).toBe(true);
    });

    it('should match case-insensitively', () => {
      expect(localeEquals('Hello', 'hello')).toBe(true);
      expect(localeEquals('HELLO', 'hello')).toBe(true);
    });

    it('should match composed vs decomposed Unicode', () => {
      const composed = 'caf\u00E9'; // café with single-char é
      const decomposed = 'cafe\u0301'; // café with combining acute
      expect(localeEquals(composed, decomposed)).toBe(true);
    });

    it('should not match different strings', () => {
      expect(localeEquals('hello', 'world')).toBe(false);
      expect(localeEquals('café', 'cafe')).toBe(false);
    });

    it('should match accented characters case-insensitively', () => {
      expect(localeEquals('Ñoño', 'ñoño')).toBe(true);
      expect(localeEquals('ÜBER', 'über')).toBe(true);
    });
  });

  describe('localeIncludes', () => {
    it('should find substring case-insensitively', () => {
      expect(localeIncludes('Hello World', 'hello')).toBe(true);
      expect(localeIncludes('Hello World', 'WORLD')).toBe(true);
    });

    it('should find accented substrings', () => {
      expect(localeIncludes('Café Latte', 'café')).toBe(true);
      expect(localeIncludes('CAFÉ LATTE', 'café')).toBe(true);
    });

    it('should find composed/decomposed matches', () => {
      const text = 'caf\u00E9 latte'; // composed é
      const query = 'cafe\u0301'; // decomposed é
      expect(localeIncludes(text, query)).toBe(true);
    });

    it('should return false when substring is absent', () => {
      expect(localeIncludes('Hello World', 'xyz')).toBe(false);
    });

    it('should handle empty needle', () => {
      expect(localeIncludes('Hello', '')).toBe(true);
    });

    it('should handle CJK text', () => {
      expect(localeIncludes('日本語テスト', '日本')).toBe(true);
      expect(localeIncludes('日本語テスト', '英語')).toBe(false);
    });
  });

  describe('localeSort', () => {
    it('should sort ASCII strings alphabetically', () => {
      const arr = ['banana', 'apple', 'cherry'];
      arr.sort(localeSort);
      expect(arr).toEqual(['apple', 'banana', 'cherry']);
    });

    it('should sort case-insensitively', () => {
      const arr = ['Banana', 'apple', 'Cherry'];
      arr.sort(localeSort);
      expect(arr).toEqual(['apple', 'Banana', 'Cherry']);
    });

    it('should sort accented characters near their base letters', () => {
      const arr = ['zebra', 'über', 'apple'];
      arr.sort(localeSort);
      // ü should sort near u, so between apple and zebra
      expect(arr[0]).toBe('apple');
      expect(arr[2]).toBe('zebra');
    });

    it('should handle numeric sorting', () => {
      const arr = ['item2', 'item10', 'item1'];
      arr.sort(localeSort);
      expect(arr).toEqual(['item1', 'item2', 'item10']);
    });

    it('should handle CJK characters', () => {
      // Just verify it doesn't throw — CJK sort order is locale-dependent
      const arr = ['日本', '中国', '韓国'];
      expect(() => arr.sort(localeSort)).not.toThrow();
    });
  });
});
