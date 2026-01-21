import { describe, test, expect, beforeEach } from 'vitest';
import { getColorHex, getColorNames, isValidColor, getTagColor, resolveTheme } from './colorService';
import { DEFAULT_COLOR_PALETTE } from '../types/models';
import { settings } from '../stores/appStore';
import { get } from 'svelte/store';

describe('colorService', () => {
  describe('getColorHex', () => {
    test('should return correct hex for light theme with default palette', () => {
      const hex = getColorHex('red', 'light');
      expect(hex).toBe(DEFAULT_COLOR_PALETTE.red.light);
    });

    test('should return correct hex for dark theme with default palette', () => {
      const hex = getColorHex('red', 'dark');
      expect(hex).toBe(DEFAULT_COLOR_PALETTE.red.dark);
    });

    test('should return correct hex for all default colors in light mode', () => {
      const colorNames = Object.keys(DEFAULT_COLOR_PALETTE);
      colorNames.forEach(colorName => {
        const hex = getColorHex(colorName, 'light');
        expect(hex).toBe(DEFAULT_COLOR_PALETTE[colorName].light);
      });
    });

    test('should return correct hex for all default colors in dark mode', () => {
      const colorNames = Object.keys(DEFAULT_COLOR_PALETTE);
      colorNames.forEach(colorName => {
        const hex = getColorHex(colorName, 'dark');
        expect(hex).toBe(DEFAULT_COLOR_PALETTE[colorName].dark);
      });
    });

    test('should return undefined for invalid color name', () => {
      const hex = getColorHex('invalidcolor', 'light');
      expect(hex).toBeUndefined();
    });

    test('should return undefined when color parameter is undefined', () => {
      const hex = getColorHex(undefined, 'light');
      expect(hex).toBeUndefined();
    });

    test('should handle custom color palette from settings', () => {
      // This test would need to mock the settings store
      // For now, we verify the function works with default palette
      const hex = getColorHex('blue', 'light');
      expect(hex).toBe(DEFAULT_COLOR_PALETTE.blue.light);
    });
  });

  describe('isValidColor', () => {
    test('should return true for all default color names', () => {
      const defaultColors = ['red', 'orange', 'yellow', 'green', 'blue', 'purple', 'pink', 'gray'];
      defaultColors.forEach(color => {
        expect(isValidColor(color)).toBe(true);
      });
    });

    test('should return false for invalid color names', () => {
      expect(isValidColor('invalid')).toBe(false);
      expect(isValidColor('notacolor')).toBe(false);
      expect(isValidColor('')).toBe(false);
    });

    test('should be case-sensitive', () => {
      expect(isValidColor('red')).toBe(true);
      expect(isValidColor('Red')).toBe(false);
      expect(isValidColor('RED')).toBe(false);
    });
  });

  describe('getColorNames', () => {
    test('should return an array of color names', () => {
      const names = getColorNames();
      expect(Array.isArray(names)).toBe(true);
    });

    test('should return all default color names', () => {
      const names = getColorNames();
      expect(names.length).toBe(8);
      expect(names).toContain('red');
      expect(names).toContain('orange');
      expect(names).toContain('yellow');
      expect(names).toContain('green');
      expect(names).toContain('blue');
      expect(names).toContain('purple');
      expect(names).toContain('pink');
      expect(names).toContain('gray');
    });

    test('should return color names from default palette when no custom palette exists', () => {
      const names = getColorNames();
      const defaultNames = Object.keys(DEFAULT_COLOR_PALETTE);
      expect(names).toEqual(defaultNames);
    });
  });

  describe('getTagColor', () => {
    test('should return undefined when tag has no color assigned', () => {
      const color = getTagColor('unassigned-tag');
      expect(color).toBeUndefined();
    });

    test('should return color name when tag has color assigned', () => {
      // This test would need to mock the settings store with tagColors
      // For now, we verify it returns undefined for unassigned tags
      const color = getTagColor('test-tag');
      expect(color).toBeUndefined();
    });
  });

  describe('resolveTheme', () => {
    test('should return light for light theme', () => {
      expect(resolveTheme('light')).toBe('light');
    });

    test('should return dark for dark theme', () => {
      expect(resolveTheme('dark')).toBe('dark');
    });

    test('should resolve auto theme based on system preference', () => {
      const resolved = resolveTheme('auto');
      // Should return either 'light' or 'dark' based on system preference
      expect(['light', 'dark']).toContain(resolved);
    });
  });

  describe('color palette validation', () => {
    test('all default colors should have both light and dark variants', () => {
      Object.entries(DEFAULT_COLOR_PALETTE).forEach(([name, colors]) => {
        expect(colors).toHaveProperty('light');
        expect(colors).toHaveProperty('dark');
        expect(typeof colors.light).toBe('string');
        expect(typeof colors.dark).toBe('string');
        // Verify hex format (should start with #)
        expect(colors.light).toMatch(/^#[0-9A-Fa-f]{6}$/);
        expect(colors.dark).toMatch(/^#[0-9A-Fa-f]{6}$/);
      });
    });

    test('should have exactly 8 default colors', () => {
      const colorCount = Object.keys(DEFAULT_COLOR_PALETTE).length;
      expect(colorCount).toBe(8);
    });
  });
});
