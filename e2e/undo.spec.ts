/**
 * E2E tests for undo in the editor. One editor is reused for every note, so its
 * history must not carry over from one note to the next.
 */

import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

const PASSWORD = 'test-password-123';
const AUTOSAVE_MS = 1500;
const NEW_UNDO_STEP_MS = 800; // CodeMirror groups edits made within 500ms

async function editorText(jp: JotteryPage) {
  return (await jp.editorContent.first().innerText()).replace(/\s+/g, ' ').trim();
}

async function typeAtEnd(page: Page, jp: JotteryPage, text: string) {
  await jp.editor.first().click();
  await page.keyboard.press('ControlOrMeta+ArrowDown');
  await page.keyboard.type(text, { delay: 15 });
}

async function openNote(page: Page, jp: JotteryPage, title: RegExp) {
  await jp.noteListItem(title).first().click();
  await page.waitForTimeout(500);
}

test.describe('Undo', () => {
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    jp = new JotteryPage(page);
    await jp.setup(PASSWORD);
  });

  test('one undo reverts one step', async ({ page }) => {
    await jp.createNote('Start');
    await page.waitForTimeout(AUTOSAVE_MS);

    await typeAtEnd(page, jp, ' first');
    await page.waitForTimeout(NEW_UNDO_STEP_MS);
    await page.keyboard.type(' second', { delay: 15 });
    await page.waitForTimeout(NEW_UNDO_STEP_MS);

    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => editorText(jp)).toBe('Start first');
  });

  test('undo never brings in another note\'s text', async ({ page }) => {
    await jp.createNote('ALPHA note');
    await page.waitForTimeout(AUTOSAVE_MS);
    await jp.createNote('BETA note');
    await page.waitForTimeout(AUTOSAVE_MS);

    await openNote(page, jp, /ALPHA/);
    await openNote(page, jp, /BETA/);
    await typeAtEnd(page, jp, ' extra');
    await page.waitForTimeout(AUTOSAVE_MS);

    // More undos than there are edits: the last ones must do nothing.
    for (let i = 0; i < 4; i++) {
      await page.keyboard.press('ControlOrMeta+z');
      await page.waitForTimeout(150);
    }
    await page.waitForTimeout(AUTOSAVE_MS);
    expect(await editorText(jp)).toBe('BETA note');

    await page.reload();
    await page.locator('#password').fill(PASSWORD);
    await page.locator('#password').press('Enter');
    await expect(jp.noteListItem(/BETA/).first()).toBeVisible({ timeout: 30000 });
    await expect(jp.noteListItem(/ALPHA/).first()).toBeVisible();
    await expect(jp.noteListItems).toHaveCount(2);
  });
});
