/**
 * Printing used to write note content into a same-origin window. For html and
 * xml notes that was the raw source, so anything hostile in it ran with full
 * access to the vault.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('../services', () => ({
  attachmentService: { getAttachmentData: vi.fn() },
  cryptoService: { decryptText: vi.fn() },
  keyManager: { getMasterKey: vi.fn(() => ({ key: {} })) },
}));

import { generatePrintHtml, printNote } from './noteExport';
import { removePrintFrames } from './printFrames';

const HOSTILE = '<img src=x onerror="window.__pwned=1"><script>window.__pwned=1</script>Visible';

function body(html: string): HTMLElement {
  return new DOMParser().parseFromString(html, 'text/html').body;
}

function hasLiveContent(html: string): boolean {
  const root = body(html);
  if (root.querySelector('script, iframe, object, embed, svg, meta, base')) return true;
  return Array.from(root.querySelectorAll('*')).some(el =>
    Array.from(el.attributes).some(a => a.name.toLowerCase().startsWith('on'))
  );
}

describe('generatePrintHtml', () => {
  for (const language of ['html', 'xml']) {
    it(`removes anything that could run from a ${language} note`, async () => {
      const html = await generatePrintHtml({ content: HOSTILE, language });
      expect(html).not.toBeNull();
      expect(hasLiveContent(html as string)).toBe(false);
      expect(body(html as string).textContent).toContain('Visible');
    });
  }

  it('renders a markdown note with the preview off, which leaves no preview HTML', async () => {
    const html = await generatePrintHtml({ content: '# Title\n\nSome **bold** text', language: 'markdown', previewHtml: '' });

    const root = body(html as string);
    expect(root.querySelector('h1')?.textContent).toBe('Title');
    expect(root.querySelector('strong')?.textContent).toBe('bold');
    expect(root.querySelector('pre')).toBeNull();
  });

  it('keeps a markdown note it renders itself from running anything', async () => {
    const html = await generatePrintHtml({ content: HOSTILE, language: 'markdown' });

    expect(hasLiveContent(html as string)).toBe(false);
  });

  it('does not trust the preview HTML it is given', async () => {
    const html = await generatePrintHtml({ content: 'x', language: 'markdown', previewHtml: HOSTILE });
    expect(hasLiveContent(html as string)).toBe(false);
  });

  it('escapes plain text', async () => {
    const html = await generatePrintHtml({ content: HOSTILE, language: 'plain' });
    expect(hasLiveContent(html as string)).toBe(false);
    expect(body(html as string).textContent).toContain('<script>');
  });

  it('keeps a hostile first line inside the title', async () => {
    const html = await generatePrintHtml({ content: '</title><script>window.__pwned=1</script>', language: 'plain' });
    const doc = new DOMParser().parseFromString(html as string, 'text/html');
    expect(doc.querySelector('script')).toBeNull();
    expect(doc.title).toContain('</title>');
  });
});

describe('printNote', () => {
  let openSpy: ReturnType<typeof vi.spyOn>;
  let appendSpy: ReturnType<typeof vi.spyOn>;
  let frame: HTMLIFrameElement | undefined;

  beforeEach(() => {
    frame = undefined;
    openSpy = vi.spyOn(window, 'open').mockImplementation(() => null);
    // Hold the frame unattached, so the tests decide when it finishes loading
    appendSpy = vi.spyOn(document.body, 'appendChild').mockImplementation(<T extends Node>(node: T) => {
      frame = node as unknown as HTMLIFrameElement;
      return node;
    });
  });

  afterEach(() => {
    openSpy.mockRestore();
    appendSpy.mockRestore();
  });

  async function startPrint(options = { content: HOSTILE, language: 'html' }) {
    const result = printNote(options);
    const built = await vi.waitFor(() => {
      if (!frame) throw new Error('no frame yet');
      return frame;
    });
    return { result, built };
  }

  it('leaves nothing behind if the app locks while the dialog is open', async () => {
    const { built } = await startPrint();
    const remove = vi.spyOn(built, 'remove');

    removePrintFrames();

    expect(remove).toHaveBeenCalledOnce();
  });

  function stubWindow(built: HTMLIFrameElement) {
    const listeners: Record<string, () => void> = {};
    const stub = {
      addEventListener: vi.fn((name: string, listener: () => void) => { listeners[name] = listener; }),
      focus: vi.fn(),
      print: vi.fn(),
    };
    Object.defineProperty(built, 'contentWindow', { value: stub });
    return { stub, listeners };
  }

  it('never opens a window that shares the app origin', async () => {
    await startPrint();
    expect(openSpy).not.toHaveBeenCalled();
  });

  it('prints from a frame that cannot run script', async () => {
    const { built } = await startPrint();
    expect(built.hasAttribute('sandbox')).toBe(true);
    expect((built.getAttribute('sandbox') as string).split(/\s+/)).not.toContain('allow-scripts');
  });

  it('does not put raw note content in the frame', async () => {
    const { built } = await startPrint();
    const source = built.srcdoc;
    expect(source).toContain('Visible');
    expect(hasLiveContent(source)).toBe(false);
  });

  it('prints once loaded, then removes the frame', async () => {
    const { result, built } = await startPrint();
    const removeSpy = vi.spyOn(built, 'remove');
    const { stub, listeners } = stubWindow(built);

    built.dispatchEvent(new Event('load'));
    await expect(result).resolves.toBe(true);
    expect(stub.print).toHaveBeenCalledOnce();
    expect(removeSpy).not.toHaveBeenCalled();

    listeners.afterprint();
    expect(removeSpy).toHaveBeenCalledOnce();
  });

  it('removes the frame and reports failure when printing throws', async () => {
    const { result, built } = await startPrint();
    const removeSpy = vi.spyOn(built, 'remove');
    const { stub } = stubWindow(built);
    stub.print.mockImplementation(() => { throw new Error('blocked'); });

    built.dispatchEvent(new Event('load'));
    await expect(result).resolves.toBe(false);
    expect(removeSpy).toHaveBeenCalledOnce();
  });
});
