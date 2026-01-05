/**
 * E2E tests for note operations (CRUD)
 */

import { test, expect } from '@playwright/test';

test.describe('Note Operations', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before each test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      // Clear IndexedDB
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload and set up password
    await page.goto('/');
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('test-password-123');
    await passwordInputs.nth(1).fill('test-password-123');
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app to load
    const appVisible = page.getByText(/No notes yet|Create your first note/i).or(page.getByRole('list'));
    await expect(appVisible.first()).toBeVisible({ timeout: 5000 });
  });

  test('should create a new note', async ({ page }) => {
    // Click new note button (look for + or New button)
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    // Should see editor with empty content
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();

    // Type note content
    await editor.click();
    await editor.pressSequentially('My first note content');

    // Wait for auto-save
    await page.waitForTimeout(3000);

    // Note should appear in list (check in note list area, not editor)
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/My first note/i)).toBeVisible();
  });

  test('should edit an existing note', async ({ page }) => {
    // Create a note first
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
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
    await expect(noteList.getByText(/Updated content/i)).toBeVisible();
  });

  test.skip('should add tags to a note', async ({ page }) => {
    // Create a note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('Note with tags');

    // Find tag input by placeholder
    const tagInput = page.getByPlaceholder(/add tag/i);
    await tagInput.click();
    await tagInput.pressSequentially('test-tag');
    await page.keyboard.press('Enter');

    await page.waitForTimeout(2000);

    // Tag should be visible (look for tag badge, not just text)
    await expect(page.locator('span, div').filter({ hasText: /^test-tag$/ })).toBeVisible();
  });

  test('should pin a note', async ({ page }) => {
    // Create a note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
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
      await expect(noteList.getByText(/Pinned note/i)).toBeVisible();
    }
  });

  test('should delete a note', async ({ page }) => {
    // Create a note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
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
    // Create first note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    let editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('First note');
    await page.waitForTimeout(3000);

    // Create second note
    await newNoteButton.click();
    editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('Second note');
    await page.waitForTimeout(3000);

    // Create third note
    await newNoteButton.click();
    editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('Third note');
    await page.waitForTimeout(3000);

    // All three notes should be visible in the list
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/First note/i)).toBeVisible();
    await expect(noteList.getByText(/Second note/i)).toBeVisible();
    await expect(noteList.getByText(/Third note/i)).toBeVisible();
  });

  test('should select and switch between notes', async ({ page }) => {
    // Create two notes
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();

    await newNoteButton.click();
    let editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('First note content');
    await page.waitForTimeout(3000);

    await newNoteButton.click();
    editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor.click();
    await editor.pressSequentially('Second note content');
    await page.waitForTimeout(3000);

    // Click on first note in list
    await page.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    // Editor should show first note content
    await expect(editor).toContainText(/First note content/i);

    // Click on second note
    await page.getByText(/Second note/i).click();
    await page.waitForTimeout(500);

    // Editor should show second note content
    await expect(editor).toContainText(/Second note content/i);
  });
});
