import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

/**
 * The open editor holds its own copy of the note. It used to ignore a pull, so the
 * next keystroke autosaved the old text over the newer note.
 */

const ENDPOINT = 'https://sync.example.test';
const PULLED = 'Pulled from another device';
const TYPED = ' typed here';

type PageModules = {
  noteId: string;
  contentHash: string;
};

/** Answer as a sync server would, with whatever the test wants the next pull to return. */
async function fakeServer(page: Page, pull: { notes: unknown[] }) {
  await page.route(`${ENDPOINT}/**`, async route => {
    const request = route.request();
    const headers = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Headers': '*',
      'Access-Control-Allow-Methods': '*',
    };

    if (request.method() === 'OPTIONS') {
      return route.fulfill({ status: 204, headers });
    }
    if (request.url().endsWith('/sync/push')) {
      const body = request.postDataJSON() as { notes: Array<{ id: string }> };
      const accepted = body.notes.map(note => ({ id: note.id, serverVersion: 2, syncedAt: new Date().toISOString() }));
      return route.fulfill({ status: 200, headers, json: { accepted, rejected: [] } });
    }
    if (request.url().endsWith('/sync/pull')) {
      const notes = pull.notes;
      pull.notes = [];
      return route.fulfill({ status: 200, headers, json: { notes, attachments: [], versions: [], syncedAt: new Date().toISOString() } });
    }
    return route.fulfill({ status: 200, headers, json: { serverTime: new Date().toISOString(), version: '1', pendingNotes: 0, totalNotes: 0 } });
  });
}

/** Turn sync on through the app's own modules, and return the stored note's details. */
async function configureSync(page: Page): Promise<PageModules> {
  return page.evaluate(async endpoint => {
    const { syncRepository } = await import('/src/lib/services/syncRepository.ts' as string);
    const { noteRepository } = await import('/src/lib/services/noteRepository.ts' as string);
    const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
    const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
    const { settings } = await import('/src/lib/stores/appStore.ts' as string);

    await syncRepository.updateMetadata({
      apiKey: JSON.stringify(await cryptoService.encryptText('key', keyManager.getMasterKey().key)),
      clientId: 'client',
      syncEnabled: true,
      syncEndpoint: endpoint,
    });
    settings.update((s: object) => ({ ...s, syncEnabled: true }));

    const [note] = await noteRepository.getAll();
    return { noteId: note.id, contentHash: note.contentHash };
  }, ENDPOINT);
}

/** The note as another device would have pushed it: a new version of this one. */
async function remoteVersion(page: Page, local: PageModules, text: string) {
  return page.evaluate(async ({ local, text }) => {
    const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
    const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
    const now = new Date().toISOString();
    const hash = 'ZmFrZS1yZW1vdGUtaGFzaC1mb3ItdGhpcy1ub3RlLWFiYw==';
    return {
      id: local.noteId, createdAt: now, modifiedAt: now,
      content: JSON.stringify(await cryptoService.encryptText(text, keyManager.getMasterKey().key)),
      tags: [], attachments: [], pinned: false, archived: false, deleted: false, version: 3,
      syntaxLanguage: 'plain', wordWrap: true,
      contentHash: hash, parentHash: local.contentHash, hashChain: [hash, local.contentHash],
    };
  }, { local, text });
}

function syncNow(page: Page) {
  return page.evaluate(async () => {
    const { syncService } = await import('/src/lib/services/syncService.ts' as string);
    return syncService.syncNow();
  });
}

async function storedNote(page: Page, noteId: string) {
  return page.evaluate(async id => {
    const { noteRepository } = await import('/src/lib/services/noteRepository.ts' as string);
    const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
    const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
    const note = await noteRepository.getById(id);
    return {
      text: await cryptoService.decryptText(JSON.parse(note.content), keyManager.getMasterKey().key),
      needsSync: note.needsSync,
    };
  }, noteId);
}

test.describe('The open editor and a pull', () => {
  let app: JotteryPage;

  test.beforeEach(async ({ page }) => {
    app = new JotteryPage(page);
    await app.setup();
    await app.createNote('Original text of the note');
    await page.waitForTimeout(1500); // Let the autosave finish, so the editor is clean
  });

  test('a clean editor shows the pulled text, and leaves the note alone', async ({ page }) => {
    const local = await configureSync(page);
    const pull = { notes: [await remoteVersion(page, local, PULLED)] };
    await fakeServer(page, pull);

    await syncNow(page);

    await expect(app.editorContent).toContainText(PULLED, { timeout: 5000 });

    await page.waitForTimeout(1500); // Longer than the autosave delay
    expect(await storedNote(page, local.noteId)).toEqual({ text: PULLED, needsSync: false });
  });

  test('the next keystroke builds on the pulled text', async ({ page }) => {
    const local = await configureSync(page);
    const pull = { notes: [await remoteVersion(page, local, PULLED)] };
    await fakeServer(page, pull);
    await syncNow(page);
    await expect(app.editorContent).toContainText(PULLED, { timeout: 5000 });

    await app.editorContent.click();
    await page.keyboard.press('ControlOrMeta+End');
    await page.keyboard.type(TYPED);

    await expect.poll(async () => (await storedNote(page, local.noteId)).text, { timeout: 5000 }).toBe(PULLED + TYPED);
  });

  test.describe('with unsaved text', () => {
    async function pullWhileTyping(page: Page) {
      const local = await configureSync(page);
      const pull = { notes: [await remoteVersion(page, local, PULLED)] };
      await fakeServer(page, pull);

      await app.editorContent.click();
      await page.keyboard.press('ControlOrMeta+End');
      await page.keyboard.type(TYPED);
      await syncNow(page);
      return local;
    }

    test('keeps what was typed', async ({ page }) => {
      const local = await pullWhileTyping(page);

      await expect(app.editorContent).toContainText(TYPED, { timeout: 5000 });
      await expect.poll(async () => (await storedNote(page, local.noteId)).text, { timeout: 5000 }).toContain(TYPED);
    });

    test('raises a conflict that still holds the pulled text', async ({ page }) => {
      await pullWhileTyping(page);

      await expect(page.getByText(/conflict/i).first()).toBeVisible({ timeout: 5000 });
      await page.getByRole('button', { name: /resolve/i }).first().click();

      const dialog = page.getByRole('dialog');
      await expect(dialog).toContainText(PULLED, { timeout: 5000 });
      await expect(dialog).toContainText(TYPED);
    });

    test('Keep Server puts the pulled text in the editor', async ({ page }) => {
      await pullWhileTyping(page);

      await page.getByRole('button', { name: /resolve/i }).first().click();
      await page.getByRole('dialog').getByRole('button', { name: /server/i }).click();

      await expect(app.editorContent).toContainText(PULLED, { timeout: 5000 });
      await expect(app.editorContent).not.toContainText(TYPED);
    });
  });
});

test.describe('A save still running when the note changes', () => {
  test('keeps the first note\'s text in the list, not the one now open', async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
    await jp.createNote('Second note');
    await jp.createNote('First note');

    await page.evaluate(async () => {
      const { noteService } = await import('/src/lib/services/noteService.ts' as string);
      const state = window as unknown as { saving?: boolean; saved?: boolean };
      const update = noteService.updateNote.bind(noteService);
      noteService.updateNote = async (...args: unknown[]) => {
        state.saving = true;
        const result = await update(...args);
        await new Promise(resolve => setTimeout(resolve, 1000));
        state.saved = true;
        return result;
      };
    });

    await jp.editorContent.click();
    await page.keyboard.insertText(' edited');
    await page.waitForFunction(() => (window as unknown as { saving?: boolean }).saving === true);

    await jp.noteListItem(/Second note/).first().click();
    await page.waitForFunction(() => (window as unknown as { saved?: boolean }).saved === true);

    const contents = await page.evaluate(async () => {
      const { notes } = await import('/src/lib/stores/appStore.ts' as string);
      let list: Array<{ content: string }> = [];
      notes.subscribe((current: Array<{ content: string }>) => { list = current; })();
      return list.map(note => note.content);
    });

    expect(contents.filter(content => content === 'Second note')).toHaveLength(1);
    expect(contents.some(content => content.includes('edited'))).toBe(true);
  });
});
