import type { KeyboardShortcut } from '../types';

/**
 * Detect if the user is on a Mac
 */
export function isMac(): boolean {
  return typeof navigator !== 'undefined' && navigator.platform.toUpperCase().indexOf('MAC') >= 0;
}

/**
 * Format a keyboard shortcut for display
 * @param shortcut The keyboard shortcut to format
 * @param usePlatformSymbols Whether to use platform-specific symbols (⌘ on Mac) or text (Ctrl)
 * @returns Formatted shortcut string (e.g., "Ctrl+K" or "⌘K")
 */
export function formatShortcut(shortcut: KeyboardShortcut | undefined, usePlatformSymbols = true): string {
  if (!shortcut || !shortcut.key) return '';

  const parts: string[] = [];
  const mac = isMac();

  // Add modifiers in standard order: Ctrl/Cmd, Alt/Option, Shift
  if (shortcut.ctrl) {
    if (usePlatformSymbols && mac) {
      parts.push('⌘'); // Command symbol
    } else {
      parts.push('Ctrl');
    }
  }

  if (shortcut.alt) {
    if (usePlatformSymbols && mac) {
      parts.push('⌥'); // Option symbol
    } else {
      parts.push('Alt');
    }
  }

  if (shortcut.shift) {
    if (usePlatformSymbols && mac) {
      parts.push('⇧'); // Shift symbol
    } else {
      parts.push('Shift');
    }
  }

  // Add the key (uppercase for single letters)
  const key = shortcut.key.length === 1 ? shortcut.key.toUpperCase() : shortcut.key;
  parts.push(key);

  // Join with + for text, or without separator for symbols
  if (usePlatformSymbols && mac) {
    return parts.join('');
  } else {
    return parts.join('+');
  }
}

/**
 * Format a keyboard shortcut with consistent styling for UI display
 * Returns a string with keyboard shortcut in parentheses
 * @param shortcut The keyboard shortcut to format
 * @returns Formatted string like "(Ctrl+K)" or "(⌘K)"
 */
export function formatShortcutForUI(shortcut: KeyboardShortcut | undefined): string {
  const formatted = formatShortcut(shortcut);
  return formatted ? `(${formatted})` : '';
}

/**
 * Format just the shortcut without parentheses (for tooltips)
 * @param shortcut The keyboard shortcut to format
 * @returns Formatted string like "Ctrl+K" or "⌘K"
 */
export function formatShortcutForTooltip(shortcut: KeyboardShortcut | undefined): string {
  return formatShortcut(shortcut);
}
