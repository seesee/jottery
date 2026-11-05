import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';
import { FORBIDDEN_ELEMENTS, HOSTILE_HTML } from '../src/test/hostileHtml';

/**
 * Real browser checks for the things happy-dom can't show: whether script actually
 * runs. Hostile values are written straight into IndexedDB, which is also the state
 * of a device that pulled them before validation at the sync boundary existed.
 */

const PASSWORD = 'test-password-123';
const ATTACHMENT_ID = '3f2b8c1e-7d4a-4e9b-8a6f-1c2d3e4f5a6b';
const SET_FLAG = 'window.__pwned=1';

async function databaseName(page: Page): Promise<string> {
  return page.evaluate(async () => {
    const all = await indexedDB.databases();
    const found = all.find(db => db.name?.includes('jottery'));
    if (!found?.name) throw new Error('no jottery database');
    return found.name;
  });
}

/** Overwrite plaintext fields on the only stored note, as a sync server could. */
async function patchNote(page: Page, patch: Record<string, unknown>) {
  const name = await databaseName(page);
  await page.evaluate(async ({ name, patch }) => {
    const db: IDBDatabase = await new Promise((resolve, reject) => {
      const open = indexedDB.open(name);
      open.onsuccess = () => resolve(open.result);
      open.onerror = () => reject(open.error);
    });
    const store = db.transaction('notes', 'readwrite').objectStore('notes');
    const notes: Array<Record<string, unknown>> = await new Promise(resolve => {
      const all = store.getAll();
      all.onsuccess = () => resolve(all.result);
    });
    for (const note of notes) {
      store.put({ ...note, ...patch });
    }
    await new Promise(resolve => { store.transaction.oncomplete = resolve; });
    db.close();
  }, { name, patch });
}

async function reloadAndUnlock(page: Page) {
  await page.reload({ waitUntil: 'domcontentloaded' });
  const password = page.locator('#password');
  await expect(password).toBeVisible({ timeout: 30000 });
  await password.fill(PASSWORD);
  await password.press('Enter');
  await expect(page.locator('[data-testid="note-list-item"]').first()).toBeVisible({ timeout: 30000 });
}

async function selectNote(page: Page) {
  await page.locator('[data-testid="note-list-item"]').first().click();
}

async function pwned(page: Page): Promise<unknown> {
  // Long enough for a failed image load to fire its handler
  await page.waitForTimeout(500);
  return page.evaluate(() => (window as unknown as { __pwned?: unknown }).__pwned);
}

test.describe('Content Security Policy', () => {
  test.beforeEach(async ({ page }) => {
    await new JotteryPage(page).setup(PASSWORD);
  });

  test('the policy does not allow inline script', async ({ page }) => {
    const policy = await page.locator('meta[http-equiv="Content-Security-Policy"]').first().getAttribute('content');
    expect(policy).toContain("script-src 'self'");
    expect(policy).toContain("object-src 'none'");
    expect(policy).not.toMatch(/script-src[^;]*unsafe-inline/);
  });

  test('blocks an inline event handler', async ({ page }) => {
    await page.evaluate(flag => {
      const img = document.createElement('img');
      img.setAttribute('onerror', flag);
      img.src = 'data:image/png;base64,broken';
      document.body.appendChild(img);
    }, SET_FLAG);
    expect(await pwned(page)).toBeUndefined();
  });

  test('blocks an injected script element', async ({ page }) => {
    await page.evaluate(flag => {
      const script = document.createElement('script');
      script.textContent = flag;
      document.head.appendChild(script);
    }, SET_FLAG);
    expect(await pwned(page)).toBeUndefined();
  });

  test('blocks a javascript: link', async ({ page }) => {
    await page.evaluate(flag => {
      const link = document.createElement('a');
      link.href = `javascript:${flag}`;
      link.textContent = 'x';
      document.body.appendChild(link);
      link.click();
    }, SET_FLAG);
    expect(await pwned(page)).toBeUndefined();
  });
});

test.describe('The sanitiser in a real browser', () => {
  type Sanitise = (html: string) => string;

  /** Runs the app's own module here, because happy-dom can't show any of this. */
  async function runInBrowser<T>(page: Page, inputs: string[], forbidden: string): Promise<T> {
    return page.evaluate(async ({ inputs, forbidden, path }) => {
      const { sanitiseHtml } = await import(/* @vite-ignore */ path) as { sanitiseHtml: Sanitise };
      const host = document.createElement('div');
      document.body.appendChild(host);

      const results = [];
      for (const input of inputs) {
        const output = sanitiseHtml(input);
        // Mounted live, so anything left would really parse and load
        host.innerHTML = output;
        const elements = Array.from(host.querySelectorAll('*'));
        results.push({
          input,
          output,
          stable: sanitiseHtml(output) === output,
          forbidden: host.querySelectorAll(forbidden).length,
          handlers: elements.flatMap(el => Array.from(el.attributes).map(a => a.name).filter(n => n.startsWith('on'))),
          urls: elements.flatMap(el => ['href', 'src'].map(n => el.getAttribute(n) ?? '').filter(Boolean)),
        });
      }
      host.remove();
      return results;
    }, { inputs, forbidden, path: '/src/lib/utils/htmlSanitiser.ts' }) as Promise<T>;
  }

  test.beforeEach(async ({ page }) => {
    await new JotteryPage(page).setup(PASSWORD);
  });

  test('leaves nothing active in any hostile markup', async ({ page }) => {
    const results = await runInBrowser<Array<{
      input: string; output: string; stable: boolean; forbidden: number; handlers: string[]; urls: string[];
    }>>(page, HOSTILE_HTML, FORBIDDEN_ELEMENTS);

    expect(results).toHaveLength(HOSTILE_HTML.length);
    for (const result of results) {
      expect(result.handlers, result.input).toEqual([]);
      expect(result.forbidden, result.input).toBe(0);
      expect(result.stable, result.input).toBe(true);
      for (const url of result.urls) {
        expect(url, result.input).not.toMatch(/^[\s\u0000-\u001f]*(javascript|vbscript|data:text)/i);
      }
    }
    expect(await pwned(page)).toBeUndefined();
  });

  test('keeps everything the preview depends on', async ({ page }) => {
    const samples = [
      '<h1 id="title">x</h1>',
      '<a href="note:abc" data-note-id="Plans: 2026" class="note-link">y</a>',
      '<a href="https://example.com" target="_blank">z</a>',
      '<img src="data:image/png;base64,AAAA" alt="a: b" data-attachment-id="x" data-loaded="false">',
      '<img src="blob:http://localhost:5173/abc">',
      '<ul><li><input type="checkbox" checked=""> done</li></ul>',
      '<table><thead><tr><th scope="col">a</th></tr></thead><tbody><tr><td colspan="2">b</td></tr></tbody></table>',
      '<pre><code class="hljs language-ts"><span class="hljs-keyword">const</span></code></pre>',
      '<code>&lt;img src=x&gt;</code>',
    ];
    const results = await runInBrowser<Array<{ output: string }>>(page, samples, FORBIDDEN_ELEMENTS);
    const output = results.map(r => r.output);

    expect(output[0]).toBe('<h1 id="title">x</h1>');
    expect(output[1]).toContain('data-note-id="Plans: 2026"');
    expect(output[1]).toContain('href="note:abc"');
    expect(output[2]).toContain('rel="noopener noreferrer"');
    expect(output[3]).toContain('src="data:image/png;base64,AAAA"');
    expect(output[3]).toContain('alt="a: b"');
    expect(output[3]).toContain('data-attachment-id="x"');
    expect(output[4]).toBe(samples[4]);
    expect(output[5]).toContain('disabled');
    expect(output[5]).toContain('checked');
    expect(output[6]).toContain('<th scope="col">a</th>');
    expect(output[6]).toContain('colspan="2"');
    expect(output[7]).toBe(samples[7]);
    expect(output[8]).toBe(samples[8]);
  });
});

test.describe('Note previews', () => {
  let app: JotteryPage;

  test.beforeEach(async ({ page }) => {
    app = new JotteryPage(page);
    await app.setup(PASSWORD);
  });

  const PAYLOAD = `"><img src=x onerror="${SET_FLAG}">`;

  // Each case has to reach the template that interpolates the hostile value, so each
  // also checks that the element it should produce is really there. Without that, a
  // note that fails to render would look like a pass.
  const hostileAttachments = [
    {
      name: 'data in an image',
      content: `Note with a hostile attachment ![pic](attachment:${ATTACHMENT_ID})`,
      attachment: { id: ATTACHMENT_ID, data: PAYLOAD, mimeType: 'image/png' },
      rendered: 'img.attachment-image',
    },
    {
      name: 'data in a download link',
      content: `Note with a hostile attachment [file](attachment:${ATTACHMENT_ID})`,
      attachment: { id: ATTACHMENT_ID, data: PAYLOAD, mimeType: 'application/pdf' },
      rendered: '.attachment-download',
    },
    {
      name: 'id in a download link',
      content: `Note with a hostile attachment [file](attachment:${ATTACHMENT_ID})`,
      attachment: { id: PAYLOAD, data: ATTACHMENT_ID, mimeType: 'application/pdf' },
      rendered: '.attachment-download',
    },
  ];

  for (const { name, content, attachment, rendered } of hostileAttachments) {
    test(`hostile attachment ${name} is inert in a markdown preview`, async ({ page }) => {
      await app.createNote(content);
      await patchNote(page, {
        syntaxLanguage: 'markdown',
        showPreview: true,
        attachments: [{ filename: '{}', size: 1, ...attachment }],
      });

      await reloadAndUnlock(page);
      await selectNote(page);

      const preview = page.locator('[data-preview-container]');
      await expect(preview.locator(rendered)).toHaveCount(1, { timeout: 10000 });
      expect(await pwned(page)).toBeUndefined();
      await expect(preview.locator('[onerror]')).toHaveCount(0);
      await expect(preview.locator('img[src="x"]')).toHaveCount(0);
    });
  }

  test('a wiki link finds a note by its custom title', async ({ page }) => {
    await app.createNote('See [[Project Plan]]');
    await patchNote(page, { syntaxLanguage: 'markdown', showPreview: true });

    await reloadAndUnlock(page);
    await selectNote(page);
    await page.evaluate(async id => {
      const { notes } = await import('/src/lib/stores/appStore.ts' as string);
      notes.update((list: unknown[]) => [...list, { id, content: 'Draft', tags: ['t:Project Plan'], attachments: [] }]);
    }, ATTACHMENT_ID);

    await expect(page.locator('[data-preview-container] a.note-link')).toContainText('Project Plan', { timeout: 10000 });
  });

  test('raw HTML in a markdown note cannot run', async ({ page }) => {
    await app.createNote(`Visible <img src=x onerror="${SET_FLAG}"> <a href="javascript:${SET_FLAG}">link</a>`);
    await patchNote(page, { syntaxLanguage: 'markdown', showPreview: true });

    await reloadAndUnlock(page);
    await selectNote(page);

    const preview = page.locator('[data-preview-container]');
    await expect(preview).toContainText('Visible', { timeout: 10000 });
    expect(await pwned(page)).toBeUndefined();
    await expect(preview.locator('[onerror]')).toHaveCount(0);
    await expect(preview.locator('a[href^="javascript"]')).toHaveCount(0);
  });

  test('an HTML note renders statically in an empty sandbox', async ({ page }) => {
    await app.createNote(`<p id="visible">Visible</p><img src=x onerror="parent.__pwned=1"><script>parent.__pwned=1</script>`);
    await patchNote(page, { syntaxLanguage: 'html', showPreview: true });

    await reloadAndUnlock(page);
    await selectNote(page);

    const iframe = page.locator('iframe[title="HTML Preview"]');
    await expect(iframe).toBeVisible({ timeout: 10000 });
    expect(await iframe.getAttribute('sandbox')).toBe('');
    await expect(page.frameLocator('iframe[title="HTML Preview"]').locator('#visible')).toHaveText('Visible');
    expect(await pwned(page)).toBeUndefined();
  });

  test('the HTML preview frame has no access to the app', async ({ page }) => {
    await app.createNote('<p id="visible">Visible</p>');
    await patchNote(page, { syntaxLanguage: 'html', showPreview: true });

    await reloadAndUnlock(page);
    await selectNote(page);

    await expect(page.locator('iframe[title="HTML Preview"]')).toBeVisible({ timeout: 10000 });
    const reachable = await page.evaluate(() => {
      const frame = document.querySelector('iframe[title="HTML Preview"]') as HTMLIFrameElement;
      return frame.contentDocument !== null;
    });
    expect(reachable).toBe(false);
  });
});

test.describe('Connections', () => {
  const SERVER = 'https://sync.example.test';
  const OTHER = 'https://elsewhere.example.test';

  /** Answer for any host, so a blocked request can only be down to the policy. */
  async function answerEverything(page: Page) {
    const hits: string[] = [];
    await page.route(/example\.test/, route => {
      hits.push(new URL(route.request().url()).origin);
      return route.fulfill({ status: 200, headers: { 'Access-Control-Allow-Origin': '*' }, body: '{}' });
    });
    return hits;
  }

  async function canReach(page: Page, url: string): Promise<boolean> {
    return page.evaluate(async target => {
      try {
        await fetch(target);
        return true;
      } catch {
        return false;
      }
    }, url);
  }

  async function configureSync(page: Page) {
    const name = await databaseName(page);
    await page.evaluate(async ({ name, endpoint }) => {
      const db: IDBDatabase = await new Promise((resolve, reject) => {
        const open = indexedDB.open(name);
        open.onsuccess = () => resolve(open.result);
        open.onerror = () => reject(open.error);
      });
      const store = db.transaction('sync_metadata', 'readwrite').objectStore('sync_metadata');
      store.put({ syncEnabled: true, syncEndpoint: endpoint, clientId: 'test' }, 'metadata');
      await new Promise(resolve => { store.transaction.oncomplete = resolve; });
      db.close();
    }, { name, endpoint: SERVER });
  }

  test('any server can be reached until sync is configured', async ({ page }) => {
    await answerEverything(page);
    await new JotteryPage(page).setup(PASSWORD);

    expect(await canReach(page, `${OTHER}/health`)).toBe(true);
  });

  test('after unlock, only the configured sync server can be reached', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup(PASSWORD);
    await app.createNote('Some note');
    await configureSync(page);

    const hits = await answerEverything(page);
    await reloadAndUnlock(page);

    expect(await canReach(page, `${SERVER}/health`)).toBe(true);
    expect(await canReach(page, `${OTHER}/steal`)).toBe(false);
    expect(hits).not.toContain(OTHER);
    expect(hits).toContain(SERVER);
  });

  test('the app itself is still reachable, for update checks', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup(PASSWORD);
    await app.createNote('Some note');
    await configureSync(page);
    await answerEverything(page);
    await reloadAndUnlock(page);

    expect(await canReach(page, '/version.json')).toBe(true);
  });
});

test.describe('Printing', () => {
  test('prints in real Chromium from a frame where script is off', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup(PASSWORD);

    const result = await page.evaluate(async flag => {
      const { printNote } = await import('/src/lib/utils/noteExport.ts' as string);
      const appended: HTMLIFrameElement[] = [];
      const observer = new MutationObserver(records => {
        for (const record of records) {
          record.addedNodes.forEach(node => {
            if (node instanceof HTMLIFrameElement) appended.push(node);
          });
        }
      });
      observer.observe(document.body, { childList: true });

      const printed = await printNote({
        content: `<p>Visible</p><img src=x onerror="parent.__pwned=1"><script>${flag}</script>`,
        language: 'html',
      });

      observer.disconnect();
      const frame = appended[0];
      return {
        printed,
        sandbox: frame?.getAttribute('sandbox'),
        text: frame?.contentDocument?.body?.textContent ?? '',
      };
    }, SET_FLAG);

    expect(result.printed).toBe(true);
    expect(result.sandbox).toBe('allow-same-origin allow-modals');
    expect(result.text).toContain('Visible');
    expect(await pwned(page)).toBeUndefined();
  });
});
