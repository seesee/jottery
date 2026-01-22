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
 * Find a color key by its display name (case-insensitive)
 * @param displayName - Display name to search for
 * @returns Color key or undefined if not found
 */
export function getColorKeyByDisplayName(displayName: string): string | undefined {
  const palette = getColorPalette();
  const normalizedSearch = displayName.toLowerCase().trim();

  // Search through palette for matching display name
  for (const [colorKey, colorValue] of Object.entries(palette)) {
    const colorDisplayName = colorValue.displayName || colorKey;
    if (colorDisplayName.toLowerCase() === normalizedSearch) {
      return colorKey;
    }
  }

  return undefined;
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

/**
 * Add opacity to a hex color code
 * @param hexColor - Hex color code (e.g., '#FF0000' or '#F00')
 * @param opacity - Opacity value between 0 and 1 (e.g., 0.3 for 30%)
 * @returns Hex color with alpha channel (e.g., '#FF000033' for 20% opacity)
 */
export function hexWithOpacity(hexColor: string | undefined, opacity: number): string | undefined {
  if (!hexColor) return undefined;

  // Remove # if present
  const hex = hexColor.replace('#', '');

  // Convert 3-digit hex to 6-digit
  const fullHex = hex.length === 3
    ? hex.split('').map(char => char + char).join('')
    : hex;

  // Convert opacity (0-1) to hex (00-FF)
  const alpha = Math.round(opacity * 255).toString(16).padStart(2, '0');

  return `#${fullHex}${alpha}`;
}
