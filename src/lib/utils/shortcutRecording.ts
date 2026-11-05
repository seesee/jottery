/**
 * What can be assigned to a shortcut.
 *
 * Note list shortcuts only fire outside text fields, so a bare key is fine there.
 * Every other shortcut fires while typing, so a bare letter would be swallowed each
 * time it was typed. Keys the keyboard itself depends on are never available.
 */
import type { KeyboardShortcut, KeyboardShortcuts } from '../types';

export type ShortcutProblem = 'needsModifier' | 'reserved';

const NOTE_LIST_SHORTCUTS = new Set<keyof KeyboardShortcuts>([
  'navigateUp', 'navigateDown', 'openNote', 'deleteNote', 'pinNote', 'selectAll',
]);

// Escape cancels and closes things everywhere, and Tab is how the keyboard gets around
const RESERVED_KEYS = new Set(['Escape', 'Tab']);

export function shortcutProblem(name: keyof KeyboardShortcuts, shortcut: KeyboardShortcut): ShortcutProblem | null {
  if (RESERVED_KEYS.has(shortcut.key)) {
    return 'reserved';
  }

  // Shift alone doesn't count: it is how capital letters are typed
  if (!NOTE_LIST_SHORTCUTS.has(name) && !shortcut.ctrl && !shortcut.alt) {
    return 'needsModifier';
  }

  return null;
}
