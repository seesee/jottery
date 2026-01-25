/**
 * E2E tests for bulk operations (multi-select actions)
 */

import { test, expect } from '@playwright/test';

test.describe('Bulk Operations', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before each test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload and set up password
    await page.goto('/');

    // Handle landing page for new users - click "Try It Out"
    const getStartedButton = page.locator('button').filter({ hasText: /Try It Out/i }).first();
    const passwordInput = page.locator('input[type="password"]').first();

    // Wait for either landing page button or password input
    await Promise.race([
      getStartedButton.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {}),
      passwordInput.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {})
    ]);

    // Click Try It Out if visible
    if (await getStartedButton.isVisible()) {
      await getStartedButton.click();
    }

    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('test-password-123');
    await passwordInputs.nth(1).fill('test-password-123');
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app to load
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 5000 });
  });

  async function createNote(page: import('@playwright/test').Page, content: string) {
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await page.keyboard.press('Control+A');
    await editor.pressSequentially(content);
    await page.waitForTimeout(3000);
  }

  test('should combine two notes into one', async ({ page }) => {
    // Create first note (will be oldest)
    await createNote(page, 'First note content - should appear first');

    // Create second note (will be newest)
    await createNote(page, 'Second note content - should appear last');

    // Verify both notes exist
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/First note content/i)).toBeVisible();
    await expect(noteList.getByText(/Second note content/i)).toBeVisible();

    // Enter multi-select mode with Ctrl+click on first note
    const firstNoteItem = noteList.locator('.note-list-item').filter({ hasText: /First note content/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = noteList.locator('.note-list-item').filter({ hasText: /Second note content/i });
    const secondCheckbox = secondNoteItem.locator('[role="checkbox"]');
    await secondCheckbox.click();

    // Bulk toolbar should appear with "2 selected"
    await expect(page.getByText(/2.*selected/i)).toBeVisible();

    // Click Combine button
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await expect(combineButton).toBeVisible();
    await combineButton.click();

    // Confirmation modal should appear
    await expect(page.getByText(/Combine.*notes/i)).toBeVisible();

    // Click confirm
    const confirmButton = page.locator('button').filter({ hasText: /Combine/i }).last();
    await confirmButton.click();

    // Wait for operation to complete
    await page.waitForTimeout(2000);

    // Should now have only one note containing content from both
    // The combined note should contain a horizontal rule separator
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toContainText(/First note content/i);
    await expect(editor).toContainText(/Second note content/i);

    // Original notes should be gone (moved to recycle bin)
    // There should be only one note in the list now
    const noteItems = await noteList.locator('.note-list-item').count();
    expect(noteItems).toBe(1);
  });

  test('should combine notes with tags merged by default', async ({ page }) => {
    // Create first note with tags
    await createNote(page, 'Note with tag A');
    const tagInput = page.locator('.tag-input-container input[type="text"]').first();
    await tagInput.click();
    await tagInput.fill('tag-a');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Create second note with different tags
    await createNote(page, 'Note with tag B');
    await tagInput.click();
    await tagInput.fill('tag-b');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Select both notes - Ctrl+click first to enter multi-select mode
    const noteList = page.getByRole('list');
    const firstNoteItem = noteList.locator('.note-list-item').filter({ hasText: /Note with tag A/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = noteList.locator('.note-list-item').filter({ hasText: /Note with tag B/i });
    await secondNoteItem.locator('[role="checkbox"]').click();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Confirm (merge tags checkbox is checked by default)
    const confirmButton = page.locator('button').filter({ hasText: /Combine/i }).last();
    await confirmButton.click();

    await page.waitForTimeout(2000);

    // Combined note should have both tags
    await expect(page.locator('.tag-pill').filter({ hasText: '#tag-a' })).toBeVisible();
    await expect(page.locator('.tag-pill').filter({ hasText: '#tag-b' })).toBeVisible();
  });

  test('should combine notes without merging tags when checkbox unchecked', async ({ page }) => {
    // Create first note (oldest) with tag-first
    await createNote(page, 'First note oldest');
    const tagInput = page.locator('.tag-input-container input[type="text"]').first();
    await tagInput.click();
    await tagInput.fill('tag-first');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Create second note (newest) with tag-second
    await createNote(page, 'Second note newest');
    await tagInput.click();
    await tagInput.fill('tag-second');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Select both notes - Ctrl+click first to enter multi-select mode
    const noteList = page.getByRole('list');
    const firstNoteItem = noteList.locator('.note-list-item').filter({ hasText: /First note oldest/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = noteList.locator('.note-list-item').filter({ hasText: /Second note newest/i });
    await secondNoteItem.locator('[role="checkbox"]').click();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Wait for modal
    const modal = page.locator('[role="dialog"]');
    await expect(modal).toBeVisible();

    // Uncheck the merge tags checkbox
    const mergeTagsCheckbox = modal.locator('input[type="checkbox"]');
    await mergeTagsCheckbox.uncheck();

    // Confirm
    const confirmButton = modal.locator('button').filter({ hasText: /Combine/i });
    await confirmButton.click();

    await page.waitForTimeout(2000);

    // Combined note should only have tag from first (oldest) note
    await expect(page.locator('.tag-pill').filter({ hasText: '#tag-first' })).toBeVisible();
    // Second note's tag should NOT be present
    await expect(page.locator('.tag-pill').filter({ hasText: '#tag-second' })).not.toBeVisible();
  });

  test('should not show combine button with only one note selected', async ({ page }) => {
    // Create one note
    await createNote(page, 'Single note');

    // Select it with Ctrl+click
    const noteList = page.getByRole('list');
    const noteItem = noteList.locator('.note-list-item').filter({ hasText: /Single note/i });
    await noteItem.click({ modifiers: ['ControlOrMeta'] });

    // Bulk toolbar should appear with "1 selected"
    await expect(page.getByText(/1.*selected/i)).toBeVisible();

    // Combine button should NOT be visible (requires 2+ notes)
    const combineButton = page.locator('button').filter({ hasText: /Combine/i });
    await expect(combineButton).not.toBeVisible();
  });

  test('should cancel combine operation', async ({ page }) => {
    // Create two notes
    await createNote(page, 'Note one');
    await createNote(page, 'Note two');

    // Select both notes - Ctrl+click first to enter multi-select mode
    const noteList = page.getByRole('list');
    const firstNoteItem = noteList.locator('.note-list-item').filter({ hasText: /Note one/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = noteList.locator('.note-list-item').filter({ hasText: /Note two/i });
    await secondNoteItem.locator('[role="checkbox"]').click();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Confirmation modal should appear
    const modal = page.locator('[role="dialog"]');
    await expect(modal).toBeVisible();
    await expect(modal.getByText(/Combine.*notes/i)).toBeVisible();

    // Click Cancel button inside the modal
    const cancelButton = modal.locator('button').filter({ hasText: /Cancel/i });
    await cancelButton.click();

    // Modal should close, both notes should still exist
    await expect(noteList.getByText(/Note one/i)).toBeVisible();
    await expect(noteList.getByText(/Note two/i)).toBeVisible();

    // Count should still be 2
    const noteItems = await noteList.locator('.note-list-item').count();
    expect(noteItems).toBe(2);
  });

  test('should combine three notes in creation order', async ({ page }) => {
    // Create three notes with distinct content
    await createNote(page, 'AAA First created');
    await page.waitForTimeout(100); // Ensure different creation times
    await createNote(page, 'BBB Second created');
    await page.waitForTimeout(100);
    await createNote(page, 'CCC Third created');

    // Select all three notes - Ctrl+click first to enter multi-select mode
    const noteList = page.getByRole('list');

    const firstNote = noteList.locator('.note-list-item').filter({ hasText: /AAA First/i });
    await firstNote.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click remaining notes' checkboxes
    const secondNote = noteList.locator('.note-list-item').filter({ hasText: /BBB Second/i });
    await secondNote.locator('[role="checkbox"]').click();

    const thirdNote = noteList.locator('.note-list-item').filter({ hasText: /CCC Third/i });
    await thirdNote.locator('[role="checkbox"]').click();

    // Should show 3 selected
    await expect(page.getByText(/3.*selected/i)).toBeVisible();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Confirm
    const confirmButton = page.locator('button').filter({ hasText: /Combine/i }).last();
    await confirmButton.click();

    await page.waitForTimeout(2000);

    // Check combined note has content in creation order (oldest first)
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    const content = await editor.textContent();

    // AAA should come before BBB, and BBB before CCC
    const posA = content?.indexOf('AAA') ?? -1;
    const posB = content?.indexOf('BBB') ?? -1;
    const posC = content?.indexOf('CCC') ?? -1;

    expect(posA).toBeLessThan(posB);
    expect(posB).toBeLessThan(posC);

    // Should only have 1 note now
    const noteItems = await noteList.locator('.note-list-item').count();
    expect(noteItems).toBe(1);
  });
});
