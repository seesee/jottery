import { describe, it, expect } from 'vitest';
import { shortcutProblem } from './shortcutRecording';

describe('shortcutProblem', () => {
  it('refuses Escape and Tab for anything', () => {
    expect(shortcutProblem('lockApp', { key: 'Escape' })).toBe('reserved');
    expect(shortcutProblem('pinNote', { key: 'Escape' })).toBe('reserved');
    expect(shortcutProblem('newNote', { key: 'Tab', ctrl: true })).toBe('reserved');
  });

  it('needs a modifier for shortcuts that fire while typing', () => {
    expect(shortcutProblem('lockApp', { key: 'l' })).toBe('needsModifier');
    expect(shortcutProblem('exportNote', { key: 'x' })).toBe('needsModifier');
  });

  it('does not count Shift, which is how capitals are typed', () => {
    expect(shortcutProblem('lockApp', { key: 'l', shift: true })).toBe('needsModifier');
  });

  it('accepts Ctrl or Alt', () => {
    expect(shortcutProblem('lockApp', { key: 'l', ctrl: true })).toBeNull();
    expect(shortcutProblem('lockApp', { key: 'l', alt: true })).toBeNull();
  });

  it('allows a bare key in the note list, which only fires outside text fields', () => {
    expect(shortcutProblem('pinNote', { key: 'p' })).toBeNull();
    expect(shortcutProblem('deleteNote', { key: 'Delete' })).toBeNull();
  });
});
