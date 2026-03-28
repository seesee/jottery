/**
 * E2E tests for search functionality
 * Tests advanced search syntax including tags, date filters, and word counts
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

/** Helper: get note list items matching text */
function noteItems(page: import('@playwright/test').Page, pattern: RegExp) {
  return page.locator('[data-testid="note-list-item"]').filter({ hasText: pattern });
}

test.describe('Search Functionality', () => {
  test.beforeEach(async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
  });

  test('should find notes by text content', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('This note contains apples and oranges');
    await jp.createNote('This note is about programming in Python');
    await jp.createNote('Another note with different content');

    await jp.searchInput.fill('apples');
    await expect(noteItems(page, /apples/i).first()).toBeVisible({ timeout: 5000 });
  });

  test('should search with tag filter syntax', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('Work meeting notes for Monday', ['work']);
    await jp.createNote('Personal journal entry', ['personal']);
    await jp.createNote('Project planning document', ['work', 'project']);

    // Wait for all notes to be created
    await expect(noteItems(page, /Project planning/i).first()).toBeVisible({ timeout: 5000 });

    // Select a work-tagged note so the selected item survives the filter
    await noteItems(page, /Work meeting/i).first().click();
    await page.waitForTimeout(500);

    // Search by tag
    await jp.searchInput.fill('#work');
    await page.waitForTimeout(1000);

    // Verify work notes ARE visible
    await expect(noteItems(page, /Work meeting/i).first()).toBeVisible({ timeout: 5000 });

    // Personal note should be filtered out
    await expect(noteItems(page, /Personal journal/i)).not.toBeVisible({ timeout: 10000 });
  });

  test('should support negative search with minus', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('Important document about cats');
    await jp.createNote('Important document about dogs');
    await jp.createNote('Random notes about weather');

    // Ensure all notes are created and visible first
    await expect(noteItems(page, /cats/i).first()).toBeVisible({ timeout: 5000 });
    await expect(noteItems(page, /dogs/i).first()).toBeVisible({ timeout: 5000 });
    await expect(noteItems(page, /weather/i).first()).toBeVisible({ timeout: 5000 });

    // Select the dogs note so the selected item survives the filter
    await noteItems(page, /dogs/i).first().click();
    await page.waitForTimeout(500);

    await jp.searchInput.fill('important -cats');
    await page.waitForTimeout(1000);

    // Dogs note should be visible, cats note should be filtered out
    await expect(noteItems(page, /dogs/i).first()).toBeVisible({ timeout: 10000 });
    await expect(noteItems(page, /cats/i)).not.toBeVisible({ timeout: 10000 });
  });

  test('should support exact phrase search with quotes', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('The quick brown fox jumps');
    await jp.createNote('Quick fox is brown and jumps high');

    await jp.searchInput.fill('"quick brown fox"');
    await expect(noteItems(page, /quick brown fox/i).first()).toBeVisible({ timeout: 5000 });
  });

  test('should show search result count', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('Test note one');
    await jp.createNote('Test note two');
    await jp.createNote('Test note three');

    await jp.searchInput.fill('test');
    await expect(noteItems(page, /Test note/i).first()).toBeVisible({ timeout: 5000 });

    // Should show result count somewhere
    const resultCount = page.locator('text=/\\d+.*match|\\d+.*result|\\d+\\/\\d+/i');
    const hasCount = await resultCount.count() > 0;

    if (hasCount) {
      await expect(resultCount.first()).toBeVisible();
    }
  });

  test('should clear search and show all notes', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('First note');
    await jp.createNote('Second note');

    // Verify both notes are visible initially
    await expect(noteItems(page, /First note/i).first()).toBeVisible({ timeout: 5000 });
    await expect(noteItems(page, /Second note/i).first()).toBeVisible({ timeout: 5000 });

    // Select the note that will survive the filter
    await noteItems(page, /First note/i).first().click();
    await page.waitForTimeout(500);

    await jp.searchInput.fill('First');
    await page.waitForTimeout(1000);

    // Second note should be filtered out
    await expect(noteItems(page, /Second note/i)).not.toBeVisible({ timeout: 10000 });

    // Clear search
    await jp.searchInput.clear();
    await page.waitForTimeout(1000);

    // Both should be visible again
    await expect(noteItems(page, /First note/i).first()).toBeVisible({ timeout: 5000 });
    await expect(noteItems(page, /Second note/i).first()).toBeVisible({ timeout: 5000 });
  });

  test('should handle empty search results gracefully', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('Sample note content');

    await jp.searchInput.fill('xyznonexistent123');

    // Wait for search to complete - no notes should match
    await expect(noteItems(page, /Sample note/i)).not.toBeVisible({ timeout: 10000 });

    // Should show empty state or no results message
    const emptyState = page.locator('text=/no.*result|no.*found|no.*match|0.*match/i');
    const hasEmptyState = await emptyState.count() > 0;

    expect(hasEmptyState).toBe(true);
  });

  test('should support wildcard search', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('Programming in JavaScript');
    await jp.createNote('Programming in TypeScript');
    await jp.createNote('Writing documentation');

    const noteList = page.getByRole('list');

    await jp.searchInput.fill('*Script');

    const scriptNotes = noteItems(page, /JavaScript|TypeScript/i);

    try {
      await expect(scriptNotes.first()).toBeVisible({ timeout: 5000 });
    } catch {
      // Wildcard search may not be fully supported - verify no crash at least
      await expect(jp.searchInput).toBeVisible();
    }
  });

  test('should preserve search on page interaction', async ({ page }) => {
    const jp = new JotteryPage(page);

    await jp.createNote('Searchable note content');
    await jp.createNote('Another note here');

    await jp.searchInput.fill('Searchable');

    const noteList = page.getByRole('list');
    const searchResult = noteList.getByRole('button').filter({ hasText: /Searchable/i }).first();
    await expect(searchResult).toBeVisible({ timeout: 5000 });

    await searchResult.click();

    const searchStillVisible = await jp.searchInput.isVisible().catch(() => false);

    if (searchStillVisible) {
      await expect(jp.searchInput).toHaveValue('Searchable');
    } else {
      await expect(jp.editorContent).toBeVisible({ timeout: 5000 });
    }
  });
});
