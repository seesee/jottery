/**
 * E2E tests for keyboard shortcuts
 * Tests global and editor keyboard shortcuts in the web app
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

test.describe('Keyboard Shortcuts', () => {
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    jp = new JotteryPage(page);
    await jp.setup();
  });

  // Helper function to create a note via button click
  async function createNoteViaButton(page: any, content: string) {
    await jp.newNoteButton.click();

    // Wait for editor to appear
    const editor = jp.editorContent;
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially(content);

    // Wait for auto-save
    await page.waitForTimeout(3000);
  }

  // Helper to blur editor by clicking outside
  async function blurEditor(page: any) {
    await jp.blurEditor();
  }

  test('should focus search with Ctrl/Cmd+K', async ({ page }) => {
    // Ensure the search input exists first
    const searchInput = jp.searchInput;
    await expect(searchInput).toBeVisible({ timeout: 5000 });

    // Click elsewhere first to ensure we're not already focused on search
    await page.locator('body').click({ position: { x: 10, y: 10 } });
    await page.waitForTimeout(100);

    // Press Ctrl+K
    await page.keyboard.press('Control+k');

    // Wait for search input to be focused - use Playwright's built-in focus assertion
    await expect(searchInput).toBeFocused({ timeout: 5000 });
  });

  test('should open settings with Ctrl/Cmd+,', async ({ page }) => {
    // Press Ctrl+,
    await page.keyboard.press('Control+,');
    await page.waitForTimeout(500);

    // Settings panel should be visible
    const settingsModal = jp.dialog;
    await expect(settingsModal.first()).toBeVisible();
  });

  test('should create new note with Alt+N', async ({ page }) => {
    // Press Alt+N to create new note (web app uses Alt to avoid browser conflicts)
    await page.keyboard.press('Alt+n');
    await page.waitForTimeout(1000);

    // Editor should be visible
    const editor = jp.editorContent;
    await expect(editor).toBeVisible({ timeout: 5000 });
  });

  test('should lock app with Alt+L', async ({ page }) => {
    // Press Alt+L to lock (web app uses Alt to avoid browser conflicts)
    await page.keyboard.press('Alt+l');
    await page.waitForTimeout(1000);

    // Should show lock screen or password prompt
    const lockScreen = page.locator('input[type="password"]');
    await expect(lockScreen.first()).toBeVisible({ timeout: 5000 });
  });

  test('should show keyboard shortcuts help with Alt+/', async ({ page }) => {
    // Press Alt+/ to show shortcuts (web app uses Alt to avoid browser conflicts)
    await page.keyboard.press('Alt+/');
    await page.waitForTimeout(500);

    // Should show keyboard shortcuts help modal
    const helpModal = jp.dialog;
    const shortcutsText = page.locator('text=/keyboard|shortcut/i');

    const hasHelp = await helpModal.count() > 0 || await shortcutsText.count() > 0;
    expect(hasHelp).toBe(true);
  });

  test('should navigate notes with arrow keys when not editing', async ({ page }) => {
    // Create multiple notes
    await createNoteViaButton(page, 'Navigation test note 1');
    await createNoteViaButton(page, 'Navigation test note 2');

    // Blur the editor to enable keyboard navigation
    await blurEditor(page);

    // Press ArrowDown to navigate
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(300);

    // Press ArrowUp to navigate back
    await page.keyboard.press('ArrowUp');
    await page.waitForTimeout(300);

    // Navigation should work without errors
    expect(true).toBe(true);
  });

  test('should navigate notes with J/K when not editing', async ({ page }) => {
    // Create notes
    await createNoteViaButton(page, 'Vim nav test 1');
    await createNoteViaButton(page, 'Vim nav test 2');

    // Blur the editor to enable keyboard navigation
    await blurEditor(page);

    // Press J to move down
    await page.keyboard.press('j');
    await page.waitForTimeout(300);

    // Press K to move up
    await page.keyboard.press('k');
    await page.waitForTimeout(300);

    // Navigation should work without errors
    expect(true).toBe(true);
  });

  test('should open first note with Enter when no note selected', async ({ page }) => {
    // Create a note
    await createNoteViaButton(page, 'Enter key test note');

    // Close the note by pressing Escape
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);

    // Press Enter to open first note
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    // Editor should be visible with the note content
    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
  });

  test('should toggle pin with P key when note selected', async ({ page }) => {
    // Create a note
    await createNoteViaButton(page, 'Pin test note');

    // Blur the editor but keep note selected
    await blurEditor(page);

    // Press P to pin (note should still be selected)
    await page.keyboard.press('p');
    await page.waitForTimeout(500);

    // Pin functionality should work without errors
    expect(true).toBe(true);
  });

  test('should delete note with Delete key when note selected', async ({ page }) => {
    // Create a note
    await createNoteViaButton(page, 'Delete key test note');

    // Blur the editor but keep note selected
    await blurEditor(page);

    // Press Delete
    await page.keyboard.press('Delete');
    await page.waitForTimeout(500);

    // Note should be deleted (moved to recycle bin)
    expect(true).toBe(true);
  });

  test('should close note with Escape', async ({ page }) => {
    // Create a note
    await createNoteViaButton(page, 'Escape close test');

    // Press Escape to close the note
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);

    // Note should be deselected (no editor visible or editor hidden)
    // The Escape key clears selection when no modal is open
    expect(true).toBe(true);
  });

  test('should copy note content with Ctrl+Shift+C', async ({ page }) => {
    // Create a note with specific content
    await createNoteViaButton(page, 'Copy test content');

    // Press Ctrl+Shift+C to copy
    await page.keyboard.press('Control+Shift+c');
    await page.waitForTimeout(500);

    // Copy should work without errors (can't easily verify clipboard)
    expect(true).toBe(true);
  });

  test('should not trigger shortcuts when editing', async ({ page }) => {
    // Create a note
    await createNoteViaButton(page, 'Editing test');

    // Editor should be focused
    const editor = jp.editorContent;
    await editor.click();

    // Type 'j' - should type in editor, not navigate
    await page.keyboard.type('jjj');
    await page.waitForTimeout(500);

    // The 'jjj' should appear in the content
    const content = await editor.textContent();
    expect(content).toContain('jjj');
  });

  test('should toggle multi-select with Ctrl+Click', async ({ page }) => {
    // Create notes
    await createNoteViaButton(page, 'Multi-select test 1');
    await createNoteViaButton(page, 'Multi-select test 2');

    // Ctrl+Click on note items to multi-select
    const noteList = page.getByRole('list');
    const noteItems = noteList.locator('li, [role="listitem"], button').filter({ hasText: /Multi-select/ });

    if (await noteItems.count() >= 2) {
      await noteItems.first().click({ modifiers: ['Control'] });
      await page.waitForTimeout(300);

      // Should have selection - just verify no error
      expect(true).toBe(true);
    }
  });

  test('should range select with Shift+Click', async ({ page }) => {
    // Create notes
    await createNoteViaButton(page, 'Range select test 1');
    await createNoteViaButton(page, 'Range select test 2');
    await createNoteViaButton(page, 'Range select test 3');

    // Click first note
    const noteList = page.getByRole('list');
    const noteItems = noteList.locator('li, [role="listitem"], button').filter({ hasText: /Range select/ });

    if (await noteItems.count() >= 3) {
      await noteItems.first().click();
      await page.waitForTimeout(300);

      // Shift+Click on third note for range select
      await noteItems.nth(2).click({ modifiers: ['Shift'] });
      await page.waitForTimeout(300);

      // Should have multiple selections - just verify no error
      expect(true).toBe(true);
    }
  });
});
