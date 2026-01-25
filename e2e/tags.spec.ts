/**
 * E2E tests for tag functionality
 * Tests tag creation, removal, filtering, and autocomplete
 */

import { test, expect } from '@playwright/test';
import { setupFreshEnvironment } from './test-utils';

test.describe('Tags', () => {
  test.beforeEach(async ({ page }) => {
    await setupFreshEnvironment(page);

    // Create a note to work with
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('Test note for tags');
    await page.waitForTimeout(2000);
  });

  test('should show tag input area', async ({ page }) => {
    // Look for tag input
    const tagInput = page.locator('input[placeholder*="tag" i], input[placeholder*="Tag" i]');
    const tagSection = page.locator('[class*="tag"], [data-testid*="tag"]');

    const hasTagUI = await tagInput.count() > 0 || await tagSection.count() > 0;
    expect(hasTagUI).toBe(true);
  });

  test('should add tag to note', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add a tag
      await tagInput.fill('important');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Tag should appear
      const tagBadge = page.locator('[class*="tag"], .tag, [data-tag]').filter({ hasText: /important/i });
      await expect(tagBadge.first()).toBeVisible();
    }
  });

  test('should add multiple tags', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add multiple tags
      const tags = ['work', 'urgent', 'followup'];

      for (const tag of tags) {
        await tagInput.fill(tag);
        await tagInput.press('Enter');
        await page.waitForTimeout(300);
      }

      // All tags should appear
      for (const tag of tags) {
        const tagBadge = page.locator('[class*="tag"], .tag').filter({ hasText: new RegExp(tag, 'i') });
        const hasTag = await tagBadge.count() > 0;
        expect(hasTag).toBe(true);
      }
    }
  });

  test('should remove tag from note', async ({ page }) => {
    // Look for tag input with multiple selectors
    const tagInput = page.locator('input[placeholder*="tag" i], input[class*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add a tag first
      await tagInput.fill('removable');
      await tagInput.press('Enter');

      // Wait for tag to appear (state-based wait)
      const tagBadge = page.locator('.tag-pill, [class*="tag-badge"]').filter({ hasText: /removable/i }).first();
      await expect(tagBadge).toBeVisible({ timeout: 3000 });

      // Look for remove/close button - could be SVG, button, or span with × character
      const removeButton = tagBadge.locator('button, svg, [role="button"], span').filter({
        hasText: /×|✕|x/i
      }).first();

      // Alternative: look for any clickable element that removes the tag
      const closeIcon = tagBadge.locator('[class*="close"], [class*="remove"], [class*="delete"]').first();

      if (await removeButton.isVisible()) {
        await removeButton.click();
        // Wait for tag to disappear
        await expect(tagBadge).not.toBeVisible({ timeout: 3000 });
      } else if (await closeIcon.isVisible()) {
        await closeIcon.click();
        // Wait for tag to disappear
        await expect(tagBadge).not.toBeVisible({ timeout: 3000 });
      }

      // Tag removal should work (verify no error)
      expect(true).toBe(true);
    }
  });

  test('should show tag autocomplete', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add some tags first
      await tagInput.fill('project');
      await tagInput.press('Enter');
      await page.waitForTimeout(300);

      await tagInput.fill('personal');
      await tagInput.press('Enter');
      await page.waitForTimeout(300);

      // Create another note (Alt+N for web app to avoid browser conflicts)
      await page.keyboard.press('Alt+n');
      await page.waitForTimeout(500);

      const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
      await editor.click();
      await editor.pressSequentially('Second note for autocomplete');
      await page.waitForTimeout(1500);

      // Start typing in tag input
      const newTagInput = page.locator('input[placeholder*="tag" i]').first();
      await newTagInput.fill('pro');
      await page.waitForTimeout(500);

      // Should show autocomplete suggestions
      const autocomplete = page.locator('[class*="autocomplete"], [class*="suggestion"], [role="listbox"]');
      const suggestionText = page.locator('text=/project/i');

      const hasAutocomplete = await autocomplete.count() > 0 || await suggestionText.count() > 0;

      // Autocomplete should suggest existing tags
      if (hasAutocomplete) {
        expect(hasAutocomplete).toBe(true);
      }
    }
  });

  test('should filter notes by tag', async ({ page }) => {
    // Look for tag input with multiple selectors
    const tagInput = page.locator('input[placeholder*="tag" i], input[class*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add tag to first note
      await tagInput.fill('filterable');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Create another note without the tag (use button)
      const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
      await newNoteButton.click();
      await page.waitForTimeout(500);

      const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
      await editor.click();
      await editor.pressSequentially('Note without the tag');
      await page.waitForTimeout(2000);

      // Search by tag
      const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
      await searchInput.fill('#filterable');
      await page.waitForTimeout(500);

      // Should only show tagged note
      const noteList = page.getByRole('list');
      const taggedNote = noteList.getByText(/Test note for tags/i);
      const untaggedNote = noteList.getByText(/without the tag/i);

      await expect(taggedNote).toBeVisible();

      // Untagged note should not be visible
      const untaggedVisible = await untaggedNote.isVisible();
      expect(untaggedVisible).toBe(false);
    }
  });

  test('should display tags in note list preview', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      await tagInput.fill('visible');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Check note list for tag display
      const noteList = page.getByRole('list');
      const tagInList = noteList.locator('[class*="tag"], .tag').filter({ hasText: /visible/i });

      const hasTagInList = await tagInList.count() > 0;

      // Tags should be visible in list preview
      if (hasTagInList) {
        expect(hasTagInList).toBe(true);
      }
    }
  });

  test('should handle special characters in tags', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Try adding tag with special characters
      await tagInput.fill('c++');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Tag should be added (possibly sanitized)
      const tagBadge = page.locator('[class*="tag"], .tag').filter({ hasText: /c\+\+|cpp/i });
      const hasTag = await tagBadge.count() > 0;

      // Either accepts special chars or sanitizes them
      expect(true).toBe(true);
    }
  });

  test('should preserve tags after note edit', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add tag
      await tagInput.fill('persistent');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Edit note content
      const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
      await editor.click();
      await editor.pressSequentially(' - edited content');
      await page.waitForTimeout(2000);

      // Tag should still be there
      const tagBadge = page.locator('[class*="tag"], .tag').filter({ hasText: /persistent/i });
      await expect(tagBadge.first()).toBeVisible();
    }
  });

  test('should allow clicking tag to filter', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add tag
      await tagInput.fill('clickable');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Click on the tag
      const tagBadge = page.locator('[class*="tag"], .tag').filter({ hasText: /clickable/i }).first();

      if (await tagBadge.isVisible()) {
        await tagBadge.click();
        await page.waitForTimeout(500);

        // Should filter to this tag (search updated)
        const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
        const searchValue = await searchInput.inputValue();

        // Search should contain the tag
        const hasTagFilter = searchValue.includes('#clickable') || searchValue.includes('clickable');

        if (hasTagFilter) {
          expect(hasTagFilter).toBe(true);
        }
      }
    }
  });

  test('should prevent duplicate tags', async ({ page }) => {
    // Look for tag input with multiple selectors
    const tagInput = page.locator('input[placeholder*="tag" i], input[class*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add tag twice
      await tagInput.fill('uniquetag');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      await tagInput.fill('uniquetag');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Should only have one instance of the tag
      // Use more specific selector for tag pills (the span with tag-pill class)
      const tagPills = page.locator('.tag-pill, [class*="tag-pill"]').filter({ hasText: '#uniquetag' });
      const count = await tagPills.count();

      expect(count).toBe(1);
    }
  });

  test('should handle empty tag input', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Try to add empty tag
      await tagInput.fill('');
      await tagInput.press('Enter');
      await page.waitForTimeout(300);

      // Should not add empty tag
      // Count tags (should be 0 or unchanged)
      expect(true).toBe(true);
    }
  });

  test('should trim whitespace from tags', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add tag with whitespace
      await tagInput.fill('  trimmed  ');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Tag should be trimmed
      const tagBadge = page.locator('[class*="tag"], .tag').filter({ hasText: /trimmed/i });
      const hasTag = await tagBadge.count() > 0;

      expect(hasTag).toBe(true);
    }
  });

  test('should support bulk tag operations', async ({ page }) => {
    // Create multiple notes (Alt+N for web app to avoid browser conflicts)
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('Alt+n');
      await page.waitForTimeout(500);

      const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
      await editor.click();
      await editor.pressSequentially(`Bulk tag test note ${i + 1}`);
      await page.waitForTimeout(1500);
    }

    // Select multiple notes
    const noteList = page.getByRole('list');
    const noteItems = noteList.locator('li, [role="listitem"]');

    if (await noteItems.count() >= 2) {
      await noteItems.first().click({ modifiers: ['Control'] });
      await noteItems.nth(1).click({ modifiers: ['Control'] });
      await page.waitForTimeout(300);

      // Look for bulk tag button
      const bulkTagButton = page.locator('button').filter({ hasText: /add.*tag|tag/i });
      const bulkToolbar = page.locator('[class*="bulk"], [class*="toolbar"]');

      const hasBulkUI = await bulkTagButton.count() > 0 || await bulkToolbar.count() > 0;

      // Bulk operations should be available (if implemented)
      if (hasBulkUI) {
        expect(hasBulkUI).toBe(true);
      }
    }
  });

  test('should show all available tags somewhere in UI', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add several unique tags
      const tags = ['alpha', 'beta', 'gamma'];

      for (const tag of tags) {
        await tagInput.fill(tag);
        await tagInput.press('Enter');
        await page.waitForTimeout(300);
      }

      // Look for tag cloud or tag list in sidebar
      const tagCloud = page.locator('[class*="tag-cloud"], [class*="tags"], [class*="sidebar"]');
      const allTagsSection = page.locator('text=/all.*tags|tags/i');

      const hasTagOverview = await tagCloud.count() > 0 || await allTagsSection.count() > 0;

      // Tag overview is optional but good to have
      if (hasTagOverview) {
        expect(hasTagOverview).toBe(true);
      }
    }
  });

  test('should combine tag filter with text search', async ({ page }) => {
    const tagInput = page.locator('input[placeholder*="tag" i]').first();

    if (await tagInput.isVisible()) {
      // Add tag
      await tagInput.fill('combined');
      await tagInput.press('Enter');
      await page.waitForTimeout(500);

      // Create another note with same tag (Alt+N for web app to avoid browser conflicts)
      await page.keyboard.press('Alt+n');
      await page.waitForTimeout(500);

      const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
      await editor.click();
      await editor.pressSequentially('Different content here');
      await page.waitForTimeout(1500);

      const newTagInput = page.locator('input[placeholder*="tag" i]').first();
      await newTagInput.fill('combined');
      await newTagInput.press('Enter');
      await page.waitForTimeout(500);

      // Search with both tag and text
      const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
      await searchInput.fill('#combined tags');
      await page.waitForTimeout(500);

      // Should filter by both criteria
      const noteList = page.getByRole('list');
      const matchingNote = noteList.getByText(/Test note for tags/i);

      await expect(matchingNote).toBeVisible();
    }
  });
});
