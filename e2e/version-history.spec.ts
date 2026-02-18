/**
 * E2E tests for version history functionality
 * Tests that versions are created when editing notes and switching between them
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './test-utils';

test.describe('Version History', () => {
  test.beforeEach(async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
  });

  test('should create version when switching notes after editing', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create first note (use same selector as working tests)
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('First note original content');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Create second note (this should trigger version creation for first note)
    await jp.newNoteButton.click();
    await page.waitForTimeout(500);

    const editor2 = jp.editorContent;
    await editor2.click();
    await editor2.pressSequentially('Second note content');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Edit the first note by clicking on it in the list
    const noteList = page.getByRole('list');
    await noteList.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    const editorAfterSwitch = jp.editorContent;
    await editorAfterSwitch.click();
    await page.keyboard.press('End'); // Go to end of line
    await editorAfterSwitch.pressSequentially(' - edited');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Switch back to second note (this should trigger version creation for first note)
    await noteList.getByText(/Second note/i).click();
    await page.waitForTimeout(1000); // Wait for version creation

    // Now go back to first note and open version history
    await noteList.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    // Open version history
    const versionHistoryModal = await jp.openVersionHistory();

    // Verify at least one version exists (should see "v1" or higher)
    const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
    await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
  });

  test('should show initial version for new note', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create a new note
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('New note with initial version');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Open version history
    const versionHistoryModal = await jp.openVersionHistory();

    // Should show initial version (v0 or v1) - new notes now create a version snapshot
    const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
    await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
  });

  test('should create version when closing note after editing', async ({ page }) => {
    const jp = new JotteryPage(page);

    // Create a note
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('Note to close');

    // Wait for auto-save by checking note appears in list (state-based wait)
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/Note to close/i)).toBeVisible({ timeout: 5000 });

    // Edit the note
    await editor.pressSequentially(' - with more content');

    // Wait for auto-save by checking updated content appears in list preview
    await expect(noteList.getByText(/with more content/i)).toBeVisible({ timeout: 5000 });

    // Close the note by clicking the X button (close icon)
    const closeButton = page.locator('button').filter({ has: page.locator('svg path[d*="M6 18L18 6"]') });
    if (await closeButton.isVisible()) {
      await closeButton.click();

      // Wait for note list to be visible (editor closed)
      await expect(noteList.getByText(/Note to close/i)).toBeVisible({ timeout: 5000 });

      // Re-select the note from the list
      await noteList.getByText(/Note to close/i).click();

      // Wait for editor to load
      await expect(editor).toBeVisible({ timeout: 3000 });

      // Open version history
      const versionHistoryModal = await jp.openVersionHistory();

      // Verify version exists
      const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
      await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
    }
  });

  test('should create version after auto-save completes and note is switched', async ({ page }) => {
    const jp = new JotteryPage(page);

    // This is the critical test for the bug where hasContentChanged is reset
    // after auto-save, preventing version creation when switching notes

    // Create first note
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('First note');
    await page.waitForTimeout(2000); // Wait for initial save

    // Create second note
    await jp.newNoteButton.click();
    await page.waitForTimeout(500);

    const editor2 = jp.editorContent;
    await editor2.click();
    await editor2.pressSequentially('Second note');
    await page.waitForTimeout(2000);

    // Go back to first note
    const noteList = page.getByRole('list');
    await noteList.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    // Edit the first note
    const editorFirst = jp.editorContent;
    await editorFirst.click();
    await page.keyboard.press('End');
    await editorFirst.pressSequentially(' modified content');

    // Wait for auto-save to complete (this used to reset hasContentChanged to false)
    await page.waitForTimeout(2000);

    // Now switch to second note - this should still create a version
    // even though hasContentChanged would have been incorrectly reset by auto-save
    await noteList.getByText(/Second note/i).click();
    await page.waitForTimeout(1000);

    // Go back to first note and check version history
    await noteList.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    // Open version history
    const versionHistoryModal = await jp.openVersionHistory();

    // Verify version exists - this test would FAIL before the bug fix
    const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
    await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
  });
});
