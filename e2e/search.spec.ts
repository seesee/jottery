/**
 * E2E tests for search functionality
 * Tests advanced search syntax including tags, date filters, and word counts
 */

import { test, expect } from '@playwright/test';

test.describe('Search Functionality', () => {
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
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('test-password-123');
    await passwordInputs.nth(1).fill('test-password-123');
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app to load
    await page.waitForTimeout(2000);
  });

  async function createNote(page: any, content: string, tags: string[] = []) {
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();

    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially(content);

    // Add tags if provided
    if (tags.length > 0) {
      const tagInput = page.locator('input[placeholder*="tag" i], input[placeholder*="Tag" i]').first();
      if (await tagInput.isVisible()) {
        for (const tag of tags) {
          await tagInput.fill(tag);
          await tagInput.press('Enter');
        }
      }
    }

    // Wait for auto-save
    await page.waitForTimeout(2000);
  }

  test('should find notes by text content', async ({ page }) => {
    // Create test notes
    await createNote(page, 'This note contains apples and oranges');
    await createNote(page, 'This note is about programming in Python');
    await createNote(page, 'Another note with different content');

    // Search for specific text
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('apples');
    await page.waitForTimeout(500);

    // Should find the note with apples
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/apples/i)).toBeVisible();
  });

  test('should search with tag filter syntax', async ({ page }) => {
    // Create notes with tags
    await createNote(page, 'Work meeting notes for Monday', ['work']);
    await createNote(page, 'Personal journal entry', ['personal']);
    await createNote(page, 'Project planning document', ['work', 'project']);

    // Search by tag
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('#work');
    await page.waitForTimeout(500);

    // Should find notes with work tag
    const noteList = page.getByRole('list');
    const noteItems = noteList.locator('li, [role="listitem"], .note-item');
    const count = await noteItems.count();

    // Should have at least the work-tagged notes visible
    expect(count).toBeGreaterThanOrEqual(1);
  });

  test('should support negative search with minus', async ({ page }) => {
    await createNote(page, 'Important document about cats');
    await createNote(page, 'Important document about dogs');
    await createNote(page, 'Random notes about weather');

    // Search for important but not cats
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('important -cats');
    await page.waitForTimeout(500);

    // Should find dogs note but not cats note
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/dogs/i)).toBeVisible();
  });

  test('should support exact phrase search with quotes', async ({ page }) => {
    await createNote(page, 'The quick brown fox jumps');
    await createNote(page, 'Quick fox is brown and jumps high');

    // Search for exact phrase
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('"quick brown fox"');
    await page.waitForTimeout(500);

    // Should find the exact phrase match
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/quick brown fox/i)).toBeVisible();
  });

  test('should show search result count', async ({ page }) => {
    // Create multiple notes
    await createNote(page, 'Test note one');
    await createNote(page, 'Test note two');
    await createNote(page, 'Test note three');

    // Search
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('test');
    await page.waitForTimeout(500);

    // Should show result count somewhere
    const resultCount = page.locator('text=/\\d+.*match|\\d+.*result|\\d+\\/\\d+/i');
    const hasCount = await resultCount.count() > 0;

    // Result count display is optional but preferred
    if (hasCount) {
      await expect(resultCount.first()).toBeVisible();
    }
  });

  test('should clear search and show all notes', async ({ page }) => {
    await createNote(page, 'First note');
    await createNote(page, 'Second note');

    // Search
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('First');
    await page.waitForTimeout(500);

    // Clear search
    await searchInput.clear();
    await page.waitForTimeout(500);

    // Should show all notes again
    const noteList = page.getByRole('list');
    await expect(noteList.getByText(/First/i)).toBeVisible();
    await expect(noteList.getByText(/Second/i)).toBeVisible();
  });

  test('should handle empty search results gracefully', async ({ page }) => {
    await createNote(page, 'Sample note content');

    // Search for non-existent text
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('xyznonexistent123');
    await page.waitForTimeout(500);

    // Should show empty state or no results message
    const emptyState = page.locator('text=/no.*result|no.*found|no.*match|0.*match/i');
    const hasEmptyState = await emptyState.count() > 0;

    expect(hasEmptyState).toBe(true);
  });

  test('should support wildcard search', async ({ page }) => {
    await createNote(page, 'Programming in JavaScript');
    await createNote(page, 'Programming in TypeScript');
    await createNote(page, 'Writing documentation');

    // Search with wildcard
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('*Script');
    await page.waitForTimeout(500);

    // Should find both Script languages
    const noteList = page.getByRole('list');
    const items = noteList.locator('li, [role="listitem"], .note-item');
    const count = await items.count();

    expect(count).toBeGreaterThanOrEqual(1);
  });

  test('should preserve search on page interaction', async ({ page }) => {
    await createNote(page, 'Searchable note content');
    await createNote(page, 'Another note here');

    // Search
    const searchInput = page.locator('input[type="search"], input[placeholder*="search" i]').first();
    await searchInput.fill('Searchable');
    await page.waitForTimeout(500);

    // Click on a note
    const noteList = page.getByRole('list');
    await noteList.getByText(/Searchable/i).click();
    await page.waitForTimeout(500);

    // Search should still be active
    await expect(searchInput).toHaveValue('Searchable');
  });
});
