import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

/**
 * Cancel is focused when a confirmation opens, so pressing Enter has to mean Cancel.
 * It used to delete the note.
 */

test.describe('Confirmation dialogs', () => {
  let app: JotteryPage;

  async function openDeleteConfirmation(page: Page) {
    await app.noteListItems.first().click({ modifiers: ['ControlOrMeta'] });
    await page.locator('button').filter({ hasText: /^Delete$/i }).first().click();
    const cancel = app.dialog.getByRole('button', { name: /cancel/i });
    await expect(cancel).toBeFocused({ timeout: 5000 });
    return cancel;
  }

  test.beforeEach(async ({ page }) => {
    app = new JotteryPage(page);
    await app.setup();
    await app.createNote('A note that must survive');
  });

  test('Enter on the focused Cancel button cancels', async ({ page }) => {
    const cancel = await openDeleteConfirmation(page);

    await page.keyboard.press('Enter');

    await expect(cancel).toBeHidden({ timeout: 5000 });
    await expect(app.noteListItems).toHaveCount(1);
  });

  test('Enter on the Confirm button confirms, once', async ({ page }) => {
    await openDeleteConfirmation(page);

    await page.keyboard.press('Tab');
    await expect(app.dialog.getByRole('button', { name: /delete/i })).toBeFocused();
    await page.keyboard.press('Enter');

    await expect(app.noteListItems).toHaveCount(0, { timeout: 5000 });
  });

  test('Escape cancels', async ({ page }) => {
    const cancel = await openDeleteConfirmation(page);

    await page.keyboard.press('Escape');

    await expect(cancel).toBeHidden({ timeout: 5000 });
    await expect(app.noteListItems).toHaveCount(1);
  });
});
