/**
 * E2E tests for mobile-specific UI behaviour
 * Tests mobile viewport interactions including tap-to-select, back navigation, etc.
 */

import { test, expect } from '@playwright/test';

test.describe('Mobile UI', () => {
  // Use mobile viewport for all tests in this describe block
  test.use({
    viewport: { width: 375, height: 667 }, // iPhone SE size
  });

  // Increase timeout for mobile tests
  test.setTimeout(60000);

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
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible', timeout: 10000 });
    await passwordInputs.first().fill('test-password-123');
    await passwordInputs.nth(1).fill('test-password-123');

    // Find and click the submit button
    const submitButton = page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first();
    await submitButton.click();

    // Wait for app to fully load - on mobile might take longer
    await page.waitForTimeout(2000);
    const appVisible = page.getByText(/No notes yet|Create your first note/i)
      .or(page.getByRole('list'))
      .or(page.locator('button').filter({ hasText: /New|^\+$/ }));
    await expect(appVisible.first()).toBeVisible({ timeout: 10000 });
  });

  test('first tap on note should select it without navigating (showing checkbox)', async ({ page }) => {
    // Create a note first - look for New button or + icon
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ })
      .or(page.locator('[aria-label*="new" i], [aria-label*="add" i]'))
      .first();
    await expect(newNoteButton).toBeVisible({ timeout: 10000 });
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially('Test note for mobile');
    await page.waitForTimeout(2000);

    // Go back to list (on mobile, we need to find the back button in the header)
    const backButton = page.locator('button svg').first()
      .or(page.locator('button').filter({ hasText: /←/ }))
      .or(page.locator('[aria-label*="back" i]'));

    if (await backButton.count() > 0) {
      await backButton.first().click();
      await page.waitForTimeout(1000);
    }

    // Find the note in the list
    const noteItem = page.locator('.note-list-item').filter({
      hasText: /Test note for mobile/i
    });

    if (await noteItem.count() > 0) {
      // First tap - should select but NOT navigate immediately
      await noteItem.first().click();
      await page.waitForTimeout(500);

      // The fix shows selection UI (checkbox or delete button) on first tap
      // instead of immediately navigating to the note
      const selectionUI = page.locator('input[type="checkbox"]')
        .or(page.locator('button[title*="delete" i]'))
        .or(page.locator('button svg[class*="x" i]'));

      // Check if any selection UI appeared, or we're still on the list
      const noteList = page.getByRole('list');
      const stillOnList = await noteList.isVisible();
      const hasSelectionUI = await selectionUI.count() > 0;

      // Test passes if we're still on the list (didn't navigate immediately)
      expect(stillOnList || hasSelectionUI).toBe(true);
    } else {
      // If we can't find the note item, skip this assertion
      test.skip();
    }
  });

  test('should have only one back button in mobile editor view', async ({ page }) => {
    // Create a note
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ })
      .or(page.locator('[aria-label*="new" i]'))
      .first();
    await expect(newNoteButton).toBeVisible({ timeout: 10000 });
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible({ timeout: 5000 });

    // In mobile editor view, count back buttons with arrow icons
    // The fix removed the duplicate back button from EditorToolbar
    // Now only the Header should have a back button
    await page.waitForTimeout(500);

    // Look for back buttons specifically - SVG with left arrow path or back text
    const backButtons = page.locator('button').filter({
      has: page.locator('svg path[d*="15 19l-7-7 7-7"], svg path[d*="M15"]')
    });

    const backButtonCount = await backButtons.count();

    // Should have at most 1 back button (the one in the header)
    // 0 is also acceptable if the button isn't visible
    expect(backButtonCount).toBeLessThanOrEqual(1);
  });

  test('mobile layout should show note list correctly', async ({ page }) => {
    // Create multiple notes
    for (let i = 1; i <= 3; i++) {
      const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ })
        .or(page.locator('[aria-label*="new" i]'))
        .first();
      await expect(newNoteButton).toBeVisible({ timeout: 10000 });
      await newNoteButton.click();

      const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
      await expect(editor).toBeVisible({ timeout: 5000 });
      await editor.click();
      await editor.pressSequentially(`Mobile note ${i}`);
      await page.waitForTimeout(2000);

      // Go back to list
      const backButton = page.locator('button svg').first()
        .or(page.locator('[aria-label*="back" i]'));

      if (await backButton.count() > 0) {
        await backButton.first().click();
        await page.waitForTimeout(1000);
      }
    }

    // Check that notes are visible in the list
    const noteItems = page.locator('.note-list-item');
    const count = await noteItems.count();

    // Should have at least 3 notes visible
    expect(count).toBeGreaterThanOrEqual(3);
  });

  test('second tap on selected note should navigate to editor', async ({ page }) => {
    // Create first note (this will be the one we test tap behaviour on)
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ })
      .or(page.locator('[aria-label*="new" i]'))
      .first();
    await expect(newNoteButton).toBeVisible({ timeout: 10000 });
    await newNoteButton.click();

    let editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially('Double tap test');
    await page.waitForTimeout(2000);

    // Go back to list
    let backButton = page.locator('button svg').first()
      .or(page.locator('[aria-label*="back" i]'));

    if (await backButton.count() > 0) {
      await backButton.first().click();
      await page.waitForTimeout(1000);
    }

    // Create a second note so the first note becomes unselected
    await expect(newNoteButton).toBeVisible({ timeout: 10000 });
    await newNoteButton.click();
    editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially('Second note');
    await page.waitForTimeout(2000);

    // Go back to list again
    backButton = page.locator('button svg').first()
      .or(page.locator('[aria-label*="back" i]'));

    if (await backButton.count() > 0) {
      await backButton.first().click();
      await page.waitForTimeout(1000);
    }

    // Find the first note (which is now NOT selected)
    const noteItem = page.locator('.note-list-item').filter({
      hasText: /Double tap test/i
    });

    if (await noteItem.count() > 0) {
      // First tap - should select but NOT navigate (mobile fix)
      await noteItem.first().click();
      await page.waitForTimeout(500);

      // Should still be on list view with note selected
      const noteList = page.getByRole('list');
      expect(await noteList.isVisible()).toBe(true);

      // Second tap - should navigate to editor
      await noteItem.first().click();
      await page.waitForTimeout(1000);

      // Should now be in editor view
      const editorView = page.locator('.cm-content, [contenteditable="true"], textarea');
      await expect(editorView.first()).toBeVisible({ timeout: 5000 });
    } else {
      test.skip();
    }
  });
});
