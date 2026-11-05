import { describe, it, expect } from 'vitest';
import { isScriptableType } from './safeObjectUrl';

describe('isScriptableType', () => {
  it.each([
    'image/svg+xml', 'IMAGE/SVG+XML', 'image/svg+xml; charset=utf-8', 'image/anything+xml',
    'text/html', 'application/xhtml+xml', 'text/xml', 'video/x+xml',
  ])('%s would run as a page', type => {
    expect(isScriptableType(type)).toBe(true);
  });

  it.each(['image/png', 'image/jpeg', 'image/webp', 'application/octet-stream', 'text/plain', ''])('%s would not', type => {
    expect(isScriptableType(type)).toBe(false);
  });
});
