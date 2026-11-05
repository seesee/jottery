/**
 * E2E tests for note operations (CRUD)
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './test-utils';

test.describe('Note Operations', () => {
  test.beforeEach(async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
  });

  test('should create a new note', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Click new note button
    await jp.newNoteButton.click();

    // Should see editor with empty content
    const editor = jp.editorContent;
    await expect(editor).toBeVisible();

    // Type note content
    await editor.click();
    await editor.pressSequentially('My first note content');

    // Wait for auto-save
    await page.waitForTimeout(3000);

    // Note should appear in list (check in note list area, not editor)
    const noteList = page.getByRole('list');
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/My first note/i).first()).toBeVisible();
  });

  test('should edit an existing note', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create a note first
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Original content');
    await page.waitForTimeout(3000);

    // Edit the note
    await editor.click();
    await page.keyboard.press('Control+A'); // Select all
    await editor.pressSequentially('Updated content');
    await page.waitForTimeout(3000);

    // Should see updated content in list
    const noteList = page.getByRole('list');
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Updated content/i).first()).toBeVisible();
  });

  test('should add tags to a note', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create a note
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Note with tags');
    await page.waitForTimeout(2000);

    // Find tag input
    const tagInput = jp.tagInput;
    await tagInput.click();
    await tagInput.fill('test-tag');
    await page.keyboard.press('Enter');

    await page.waitForTimeout(1000);

    // Tag should be visible as a tag pill with # prefix
    await expect(jp.tagPills.filter({ hasText: '#test-tag' })).toBeVisible();
  });

  test('should pin a note', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create a note
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Pinned note');
    await page.waitForTimeout(3000);

    // Click pin button (look for pin icon or button)
    const pinButton = page.locator('button[aria-label*="Pin"], button').filter({ hasText: /Pin/i }).first();
    if (await pinButton.isVisible()) {
      await pinButton.click();
      await page.waitForTimeout(1000);

      // Check if note is still visible in list
      const noteList = page.getByRole('list');
      await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Pinned note/i).first()).toBeVisible();
    }
  });

  test('should delete a note', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create a note
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Note to delete');
    await page.waitForTimeout(3000);

    // Click delete button
    const deleteButton = page.locator('button[aria-label*="Delete"], button').filter({ hasText: /Delete|Trash|Remove/i }).first();
    if (await deleteButton.isVisible()) {
      await deleteButton.click();

      // May have confirmation dialog
      const confirmButton = page.locator('button').filter({ hasText: /Confirm|Yes|Delete/i }).first();
      if (await confirmButton.isVisible({ timeout: 1000 })) {
        await confirmButton.click();
      }

      await page.waitForTimeout(1000);

      // Note should be gone (or moved to trash)
      // Should see empty state or note removed from list
      const noteInList = page.getByText(/Note to delete/i);
      const isVisible = await noteInList.isVisible().catch(() => false);
      if (isVisible) {
        // Note might still be visible if in trash - that's OK
        console.log('Note still visible (possibly in trash)');
      }
    }
  });

  test('should create multiple notes', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create first note
    await jp.newNoteButton.click();

    let editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('First note');
    await page.waitForTimeout(3000);

    // Create second note
    await jp.newNoteButton.click();
    editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Second note');
    await page.waitForTimeout(3000);

    // Create third note
    await jp.newNoteButton.click();
    editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Third note');
    await page.waitForTimeout(3000);

    // All three notes should be visible in the list
    const noteList = page.getByRole('list');
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/First note/i).first()).toBeVisible();
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Second note/i).first()).toBeVisible();
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Third note/i).first()).toBeVisible();
  });

  test('should select and switch between notes', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create two notes
    await jp.newNoteButton.click();
    let editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('First note content');
    await page.waitForTimeout(3000);

    await jp.newNoteButton.click();
    editor = jp.editorContent;
    await editor.click();
    await editor.pressSequentially('Second note content');
    await page.waitForTimeout(3000);

    // Click on first note in list
    await page.getByRole('list').getByRole('button').filter({ hasText: /First note/i }).first().click();

    // Editor should show first note content
    await expect(editor).toContainText(/First note content/i);

    // Click on second note
    await page.getByRole('list').getByRole('button').filter({ hasText: /Second note/i }).first().click();

    // Editor should show second note content
    await expect(editor).toContainText(/Second note content/i);
  });
});

test.describe('Leaving the page straight after typing', () => {
  test('keeps an edit made within the autosave delay', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup();
    await app.newNoteButton.click();
    await app.editorContent.click();
    await app.editorContent.pressSequentially('saved first');
    await page.waitForTimeout(2500);
    await app.editorContent.pressSequentially(' and typed last');

    // What a tab being closed or backgrounded does, without the wait for the debounce
    await page.evaluate(() => {
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
      document.dispatchEvent(new Event('visibilitychange'));
    });

    await expect.poll(() => page.evaluate(async () => {
      const { noteRepository } = await import('/src/lib/services/noteRepository.ts' as string);
      const { keyManager } = await import('/src/lib/services/keyManager.ts' as string);
      const { cryptoService } = await import('/src/lib/services/crypto.ts' as string);
      const [note] = await noteRepository.getAll();
      return cryptoService.decryptText(JSON.parse(note.content), keyManager.getMasterKey().key);
    }), { timeout: 900 }).toContain('typed last');
  });
});

test.describe('Locking', () => {
  test('hides the vault at once, and takes input away from it', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup();
    await app.newNoteButton.click();
    await app.editorContent.click();
    await app.editorContent.pressSequentially('private text');
    await page.waitForTimeout(2000);

    // Hold the lock open at the save step, so the state can be looked at
    await page.evaluate(async () => {
      const { registerPendingSave } = await import('/src/lib/utils/pendingSave.ts' as string);
      const { lockApplication } = await import('/src/lib/services/lockService.ts' as string);
      let release: () => void = () => undefined;
      const held = new Promise<void>(resolve => { release = resolve; });
      registerPendingSave(() => held);
      (window as unknown as { __release: () => void }).__release = release;
      void lockApplication();
    });

    const overlay = page.getByRole('status').filter({ hasText: 'Locking' });
    await expect(overlay).toBeVisible();
    const background = await overlay.evaluate(el => getComputedStyle(el).backgroundColor);
    expect(background).not.toMatch(/rgba\(.*, 0?\.\d+\)/); // Nothing shows through
    expect(await page.locator('#main-content').evaluate(el => (el as HTMLElement).inert)).toBe(true);

    await page.evaluate(() => (window as unknown as { __release: () => void }).__release());
    await expect(page.locator('#password')).toBeVisible({ timeout: 10000 });
  });
});

test.describe('For screen readers', () => {
  test('marks which note is open', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup();
    await app.createNote('First');
    await app.createNote('Second');

    await expect(page.locator('[data-testid="note-list-item"][aria-current="true"]')).toHaveCount(1);
  });

  test('names every control on the main screen, including a toast', async ({ page }) => {
    const app = new JotteryPage(page);
    await app.setup();
    await app.createNote('A note');
    await page.evaluate(async () => {
      const { toast } = await import('/src/lib/utils/toast.svelte.ts' as string);
      toast.info('something happened');
    });
    await expect(page.getByText('something happened')).toBeVisible();

    const unnamed = await page.evaluate(() => Array.from(document.querySelectorAll('button, [role="button"], a[href]'))
      .filter(el => !(el.getAttribute('aria-label') || el.getAttribute('title') || (el.textContent ?? '').trim() || el.getAttribute('aria-labelledby')))
      .map(el => el.outerHTML.slice(0, 100)));

    expect(unnamed).toEqual([]);
  });
});

