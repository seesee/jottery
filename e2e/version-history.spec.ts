/**
 * E2E tests for version history functionality
 * Tests that versions are created when editing notes and switching between them
 */

import { test, expect } from '@playwright/test';
import { setupFreshEnvironment } from './test-utils';

test.describe('Version History', () => {
  test.beforeEach(async ({ page }) => {
    await setupFreshEnvironment(page);
  });

  async function openVersionHistory(page: any) {
    // Open the "More actions" menu - look for button with 3 dots SVG
    const moreButton = page.locator('button').filter({ has: page.locator('svg path[d*="12 8c1.1"]') });
    await expect(moreButton).toBeVisible({ timeout: 5000 });
    await moreButton.click();

    // Click version history option (has 📜 emoji)
    const versionHistoryButton = page.locator('button').filter({ hasText: '📜' });
    await expect(versionHistoryButton).toBeVisible();
    await versionHistoryButton.click();

    // Wait for modal
    const versionHistoryModal = page.locator('[role="dialog"]');
    await expect(versionHistoryModal).toBeVisible({ timeout: 5000 });
    return versionHistoryModal;
  }

  test('should create version when switching notes after editing', async ({ page }) => {
    // Create first note (use same selector as working tests)
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('First note original content');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Create second note (this should trigger version creation for first note)
    await newNoteButton.click();
    await page.waitForTimeout(500);

    const editor2 = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor2.click();
    await editor2.pressSequentially('Second note content');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Edit the first note by clicking on it in the list
    const noteList = page.getByRole('list');
    await noteList.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    const editorAfterSwitch = page.locator('.cm-content, [contenteditable="true"], textarea').first();
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
    const versionHistoryModal = await openVersionHistory(page);

    // Verify at least one version exists (should see "v1" or higher)
    const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
    await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
  });

  test('should show no versions message for new note without edits', async ({ page }) => {
    // Create a new note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('New note without version');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Open version history
    const versionHistoryModal = await openVersionHistory(page);

    // Should show "no version history" message
    await expect(versionHistoryModal.getByText(/No version history/i)).toBeVisible({ timeout: 5000 });
  });

  test('should create version when closing note after editing', async ({ page }) => {
    // Create a note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('Note to close');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Edit the note
    await editor.pressSequentially(' - with more content');
    await page.waitForTimeout(2000); // Wait for auto-save

    // Close the note by clicking the X button (close icon)
    const closeButton = page.locator('button').filter({ has: page.locator('svg path[d*="M6 18L18 6"]') });
    if (await closeButton.isVisible()) {
      await closeButton.click();
      await page.waitForTimeout(1000); // Wait for version creation

      // Re-select the note from the list
      const noteList = page.getByRole('list');
      await noteList.getByText(/Note to close/i).click();
      await page.waitForTimeout(500);

      // Open version history
      const versionHistoryModal = await openVersionHistory(page);

      // Verify version exists
      const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
      await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
    }
  });

  test('should create version after auto-save completes and note is switched', async ({ page }) => {
    // This is the critical test for the bug where hasContentChanged is reset
    // after auto-save, preventing version creation when switching notes

    // Create first note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('First note');
    await page.waitForTimeout(2000); // Wait for initial save

    // Create second note
    await newNoteButton.click();
    await page.waitForTimeout(500);

    const editor2 = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await editor2.click();
    await editor2.pressSequentially('Second note');
    await page.waitForTimeout(2000);

    // Go back to first note
    const noteList = page.getByRole('list');
    await noteList.getByText(/First note/i).click();
    await page.waitForTimeout(500);

    // Edit the first note
    const editorFirst = page.locator('.cm-content, [contenteditable="true"], textarea').first();
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
    const versionHistoryModal = await openVersionHistory(page);

    // Verify version exists - this test would FAIL before the bug fix
    const versionItem = versionHistoryModal.locator('button').filter({ hasText: /^v\d+/ });
    await expect(versionItem.first()).toBeVisible({ timeout: 5000 });
  });
});
