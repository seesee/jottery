import { describe, it, expect } from 'vitest';
import { matchesShortcut, withDefaultShortcuts } from './keyboardShortcuts';
import { DEFAULT_KEYBOARD_SHORTCUTS } from '../types';

const press = (key: string, modifiers: Partial<KeyboardEventInit> = {}) => new KeyboardEvent('keydown', { key, ...modifiers });

describe('matchesShortcut', () => {
  it('needs the same key and exactly the same modifiers', () => {
    expect(matchesShortcut(press('Tab'), { key: 'Tab' })).toBe(true);
    expect(matchesShortcut(press('Tab', { shiftKey: true }), { key: 'Tab' })).toBe(false);
    expect(matchesShortcut(press('Tab'), { key: 'Tab', shift: true })).toBe(false);
    expect(matchesShortcut(press('ArrowUp', { altKey: true }), { key: 'ArrowUp' })).toBe(false);
  });

  it('takes Ctrl or Cmd for ctrl', () => {
    expect(matchesShortcut(press('.', { ctrlKey: true }), { key: '.', ctrl: true })).toBe(true);
    expect(matchesShortcut(press('.', { metaKey: true }), { key: '.', ctrl: true })).toBe(true);
  });

  it('ignores case, as Shift changes it', () => {
    expect(matchesShortcut(press('I', { altKey: true, shiftKey: true }), { key: 'i', alt: true, shift: true })).toBe(true);
  });

  it('matches nothing without a shortcut', () => {
    expect(matchesShortcut(press('a'), undefined)).toBe(false);
  });
});

describe('withDefaultShortcuts', () => {
  it('fills in shortcuts added since the settings were saved', () => {
    const saved = { focusSearch: { key: 'j', ctrl: true } };

    const shortcuts = withDefaultShortcuts(saved as never);

    expect(shortcuts.focusSearch).toEqual({ key: 'j', ctrl: true });
    expect(shortcuts.outlinerIndent).toEqual({ key: 'Tab' });
  });

  it('gives every default when nothing is saved', () => {
    expect(withDefaultShortcuts(undefined)).toEqual(DEFAULT_KEYBOARD_SHORTCUTS);
  });
});
