/**
 * E2E tests for the Visual (WYSIWYG) markdown editor mode.
 *
 * Switching to Visual mode, editing, and switching back must not rewrite
 * the parts of the note that were not edited.
 */

import { test, expect, Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

const NOTE = [
  '# Round trip',
  '',
  'Use snake_case_name and 2 * 3 = 6 with **bold** and *em*.',
  'Second line of the same paragraph.',
  '',
  '- one',
  '- two',
  '  - two a',
  '',
  '1. first',
  '2. second',
  '',
  '- [ ] todo',
  '- [x] done',
  '',
  '```js',
  'const x = a_b * c;',
  '```',
  '',
  '| Name | Value |',
  '| ---- | ----- |',
  '| a    | `b`   |',
].join('\n');

async function openMarkdownNote(page: Page, jp: JotteryPage, content: string) {
  await jp.newNoteButton.click();
  const editor = jp.editorContent;
  await expect(editor).toBeVisible({ timeout: 5000 });

  const languageSelect = page.locator('select').filter({ has: page.locator('option[value="markdown"]') });
  await languageSelect.selectOption('markdown');

  await editor.click();
  await page.waitForTimeout(300);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('Backspace');
  // insertText keeps newlines verbatim; pressing Enter would trigger list continuation
  await page.keyboard.insertText(content);
  await expect(editor).toContainText('Round trip');
}

function rawButton(page: Page) {
  return page.locator('button[title="Raw"]');
}

function visualButton(page: Page) {
  return page.locator('button[title="Visual"]');
}

async function rawEditorText(page: Page): Promise<string> {
  const lines = await page.locator('.cm-content .cm-line').allTextContents();
  return lines.join('\n').replace(/ /g, ' ');
}

test.describe('Visual editor mode', () => {
  test.beforeEach(async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
  });

  test('renders markdown structure in Visual mode', async ({ page }) => {
    const jp = new JotteryPage(page);
    await openMarkdownNote(page, jp, NOTE);

    await visualButton(page).click();
    const pm = page.locator('.ProseMirror');
    await expect(pm).toBeVisible();

    await expect(pm.locator('h1')).toHaveText('Round trip');
    await expect(pm.locator('strong')).toHaveText('bold');
    await expect(pm.locator('ul[data-type="taskList"] li')).toHaveCount(2);
    await expect(pm.locator('ul[data-type="taskList"] input[type="checkbox"]:checked')).toHaveCount(1);
    await expect(pm.locator('ol li')).toHaveCount(2);
    await expect(pm.locator('ul:not([data-type="taskList"]) ul li')).toHaveText('two a');
    await expect(pm.locator('pre code')).toContainText('const x = a_b * c;');
    await expect(pm.locator('table td code')).toHaveText('b');
    // prose is shown without escape backslashes
    await expect(pm.locator('p').first()).toContainText('snake_case_name and 2 * 3 = 6');
  });

  test('switching to Visual and back without editing leaves the note untouched', async ({ page }) => {
    const jp = new JotteryPage(page);
    await openMarkdownNote(page, jp, NOTE);

    await visualButton(page).click();
    await expect(page.locator('.ProseMirror h1')).toBeVisible();
    await rawButton(page).click();

    await expect(page.locator('.cm-content')).toBeVisible();
    expect(await rawEditorText(page)).toBe(NOTE);
  });

  test('editing in Visual mode only changes the edited text', async ({ page }) => {
    const jp = new JotteryPage(page);
    await openMarkdownNote(page, jp, NOTE);

    await visualButton(page).click();
    const heading = page.locator('.ProseMirror h1');
    await expect(heading).toBeVisible();

    // Append to the heading; this is the first keystroke, which serialises the whole note
    await heading.click();
    await page.keyboard.press('End');
    await page.keyboard.type(' edited');
    await expect(heading).toHaveText('Round trip edited');

    await rawButton(page).click();
    await expect(page.locator('.cm-content')).toBeVisible();

    expect(await rawEditorText(page)).toBe(NOTE.replace('# Round trip', '# Round trip edited'));
  });

  test('a second switch does not degrade nested lists', async ({ page }) => {
    const jp = new JotteryPage(page);
    await openMarkdownNote(page, jp, NOTE);

    for (let i = 0; i < 2; i++) {
      await visualButton(page).click();
      const heading = page.locator('.ProseMirror h1');
      await heading.click();
      await page.keyboard.press('End');
      await page.keyboard.type('!');
      await rawButton(page).click();
      await expect(page.locator('.cm-content')).toBeVisible();
    }

    expect(await rawEditorText(page)).toBe(NOTE.replace('# Round trip', '# Round trip!!'));
  });
});
