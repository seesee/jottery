/**
 * Color service for managing note and tag colors
 * Handles color resolution, validation, and theme-aware hex code retrieval
 */

import { DEFAULT_COLOR_PALETTE, type ColorPalette } from '../types/models';
import { get } from 'svelte/store';
import { settings } from '../stores/appStore';

/**
 * Get the hex color code for a semantic color name
 * @param colorName - Semantic color name (e.g., 'red', 'blue')
 * @param theme - Current theme ('light' or 'dark')
 * @returns Hex color code or undefined if color name is invalid
 */
export function getColorHex(
  colorName: string | undefined,
  theme: 'light' | 'dark'
): string | undefined {
  if (!colorName) return undefined;

  const userSettings = get(settings);
  const palette = userSettings.colorPalette || DEFAULT_COLOR_PALETTE;

  return palette[colorName]?.[theme];
}

/**
 * Check if a color name is valid in the current palette
 * @param colorName - Color name to validate
 * @returns True if color exists in palette
 */
export function isValidColor(colorName: string): boolean {
  const userSettings = get(settings);
  const palette = userSettings.colorPalette || DEFAULT_COLOR_PALETTE;
  return colorName in palette;
}

/**
 * Get all available color names from the current palette
 * @returns Array of color names
 */
export function getColorNames(): string[] {
  const userSettings = get(settings);
  const palette = userSettings.colorPalette || DEFAULT_COLOR_PALETTE;
  return Object.keys(palette);
}

/**
 * Get the color assigned to a specific tag
 * @param tagName - Tag name (without # prefix)
 * @returns Color name or undefined if tag has no color
 */
export function getTagColor(tagName: string): string | undefined {
  const userSettings = get(settings);
  // Normalize tag name to lowercase for case-insensitive lookup
  const normalizedTag = tagName.toLowerCase().trim();
  return userSettings.tagColors?.[normalizedTag];
}

/**
 * Get the current color palette
 * @returns ColorPalette object
 */
export function getColorPalette(): ColorPalette {
  const userSettings = get(settings);
  return userSettings.colorPalette || DEFAULT_COLOR_PALETTE;
}

/**
 * Get the display name for a color
 * Returns the user-defined displayName if set, otherwise returns the color key
 * @param colorName - Color key name
 * @returns Display name or color key
 */
export function getColorDisplayName(colorName: string): string {
  const userSettings = get(settings);
  const palette = userSettings.colorPalette || DEFAULT_COLOR_PALETTE;
  return palette[colorName]?.displayName || colorName;
}

/**
 * Resolve the current theme to 'light' or 'dark'
 * Handles 'auto' theme by checking system preference
 * @param themePreference - User's theme preference
 * @returns Resolved theme ('light' or 'dark')
 */
export function resolveTheme(themePreference: 'light' | 'dark' | 'auto'): 'light' | 'dark' {
  if (themePreference === 'auto') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return themePreference;
}
