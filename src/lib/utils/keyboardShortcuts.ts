import { DEFAULT_KEYBOARD_SHORTCUTS, type KeyboardShortcut, type KeyboardShortcuts } from '../types';

/**
 * Detect if the user is on a Mac
 */
export function isMac(): boolean {
  return typeof navigator !== 'undefined' && navigator.platform.toUpperCase().indexOf('MAC') >= 0;
}

/** Saved shortcuts, with defaults for any added since they were saved */
export function withDefaultShortcuts(saved: Partial<KeyboardShortcuts> | undefined): KeyboardShortcuts {
  return { ...DEFAULT_KEYBOARD_SHORTCUTS, ...saved };
}

/** Whether a key press is a shortcut, with Cmd counting as Ctrl */
export function matchesShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut | undefined): boolean {
  if (!shortcut?.key) return false;

  return (shortcut.ctrl === true) === (event.ctrlKey || event.metaKey)
    && (shortcut.alt === true) === event.altKey
    && (shortcut.shift === true) === event.shiftKey
    && event.key.toLowerCase() === shortcut.key.toLowerCase();
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
