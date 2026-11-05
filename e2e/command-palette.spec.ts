import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

/**
 * Palette actions used to dispatch an event nothing listened for: choosing Lock
 * closed the palette and left the vault unlocked.
 */

async function runCommand(page: Page, search: string) {
  await page.evaluate(() => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, shiftKey: true, bubbles: true }));
  });
  const input = page.getByRole('dialog').locator('input').first();
  await expect(input).toBeVisible({ timeout: 5000 });
  await input.fill(search);
  await input.press('Enter');
}

test.describe('Command palette', () => {
  let app: JotteryPage;

  test.beforeEach(async ({ page }) => {
    app = new JotteryPage(page);
    await app.setup();
  });

  test('Lock locks the vault', async ({ page }) => {
    await runCommand(page, 'Lock Application');
    await expect(page.locator('#password')).toBeVisible({ timeout: 10000 });
  });

  test.describe('with a note open', () => {
    test.beforeEach(async () => {
      await app.createNote('Note for the command palette');
    });

    test('Duplicate runs once', async ({ page }) => {
      await runCommand(page, 'Duplicate');
      await expect(app.noteListItems).toHaveCount(2, { timeout: 10000 });
      await expect(app.noteListItems).toHaveCount(2);
    });

    test('Note info opens', async ({ page }) => {
      await runCommand(page, 'Note Info');
      await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });
    });

    test('Note info closes with Escape', async ({ page }) => {
      await runCommand(page, 'Note Info');
      await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });

      await page.keyboard.press('Escape');

      await expect(page.getByRole('dialog')).toHaveCount(0);
      await expect(page.getByRole('region', { name: 'Note editor' })).toBeVisible();
    });

    test('Version history opens', async ({ page }) => {
      await runCommand(page, 'Version History');
      await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });
    });

    test('Version history closes with Escape, wherever focus went', async ({ page }) => {
      await runCommand(page, 'Version History');
      await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });

      await page.keyboard.press('Escape');

      await expect(page.getByRole('dialog')).toHaveCount(0);
      await expect(page.getByRole('region', { name: 'Note editor' })).toBeVisible();
    });

    test('Delete asks first', async ({ page }) => {
      await runCommand(page, 'Delete Note');
      await expect(page.getByRole('dialog').getByRole('button', { name: /delete/i })).toBeVisible({ timeout: 5000 });
      await expect(app.noteListItems).toHaveCount(1);
    });

    test('Archive removes it from the list', async ({ page }) => {
      await runCommand(page, 'Archive');
      await expect(app.noteListItems).toHaveCount(0, { timeout: 10000 });
    });

    test('Lock note makes it read-only', async ({ page }) => {
      await runCommand(page, 'Lock/Unlock');
      await expect(page.getByText(/this note is locked/i)).toBeVisible({ timeout: 5000 });
    });

    test('Set category opens the colour picker', async ({ page }) => {
      await runCommand(page, 'Set Note Category');
      await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 });
    });

    test('Export downloads the note', async ({ page }) => {
      const download = page.waitForEvent('download', { timeout: 5000 });
      await runCommand(page, 'Export to File');
      await download;
    });
  });
});
