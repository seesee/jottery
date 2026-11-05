import { describe, it, expect } from 'vitest';
import { notePreview, stripMarkdownSignifiers } from './markdownStrip';

/**
 * Note text can come from anywhere, and the list renders on the main thread at every
 * unlock, so one note must not be able to hang the app. Generous limits: the old code
 * took seconds on these, and the fix takes milliseconds.
 */
describe('hostile note text', () => {
  const hostile = 'title\n' + '['.repeat(300_000);

  it('builds a preview without reading the whole note', () => {
    const started = performance.now();
    expect(notePreview(hostile, 'markdown', 100).length).toBeLessThanOrEqual(100);
    expect(performance.now() - started).toBeLessThan(500);
  });

  /**
   * The old code was still running after four minutes, and the fix takes milliseconds,
   * or a few seconds on a machine running a dozen other builds. The limit sits between
   * them, so this tells them apart however busy the machine is.
   */
  it.each([
    ['[', 200_000],
    ['![', 100_000],
    ['[a](', 50_000],
    ['[a][', 50_000],
  ])('strips unclosed %s in linear time', (unit, repeats) => {
    const started = performance.now();

    stripMarkdownSignifiers(unit.repeat(repeats));

    expect(performance.now() - started).toBeLessThan(15_000);
  }, 120_000);
});

describe('notePreview', () => {
  it('skips the first line, which is the title', () => {
    expect(notePreview('Title\nbody text\nmore', 'plain', 100)).toBe('body text more');
  });

  it('strips markdown only for markdown notes', () => {
    expect(notePreview('T\n**bold** [link](https://x.test)', 'markdown', 100)).toBe('bold link');
    expect(notePreview('T\n**bold**', 'plain', 100)).toBe('**bold**');
  });

  it('still fills the preview when markdown syntax is stripped away', () => {
    const long = '**word** '.repeat(50);
    expect(notePreview('T\n' + long, 'markdown', 40).length).toBe(40);
  });

  it('is empty for a note with one line', () => {
    expect(notePreview('Only a title', 'plain', 100)).toBe('');
  });
});
