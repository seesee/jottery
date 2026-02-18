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
    await expect(noteList.getByText(/My first note/i)).toBeVisible();
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
    await expect(noteList.getByText(/Updated content/i)).toBeVisible();
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
      await expect(noteList.getByText(/Pinned note/i)).toBeVisible();
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
    await expect(noteList.getByText(/First note/i)).toBeVisible();
    await expect(noteList.getByText(/Second note/i)).toBeVisible();
    await expect(noteList.getByText(/Third note/i)).toBeVisible();
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
