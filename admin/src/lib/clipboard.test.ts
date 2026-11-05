import { describe, it, expect, vi, afterEach } from 'vitest';
import { copyText } from './clipboard';

function stubClipboard(clipboard: unknown) {
  Object.defineProperty(navigator, 'clipboard', { value: clipboard, configurable: true });
}

describe('copyText', () => {
  afterEach(() => {
    stubClipboard(undefined);
  });

  it('copies and says so', async () => {
    const writeText = vi.fn(async () => {});
    stubClipboard({ writeText });

    await expect(copyText('token')).resolves.toBe(true);
    expect(writeText).toHaveBeenCalledWith('token');
  });

  it('reports failure on plain HTTP, where browsers have no clipboard', async () => {
    stubClipboard(undefined);
    await expect(copyText('token')).resolves.toBe(false);
  });

  it('reports failure when the browser refuses', async () => {
    stubClipboard({ writeText: vi.fn(async () => { throw new Error('denied'); }) });
    await expect(copyText('token')).resolves.toBe(false);
  });
});
