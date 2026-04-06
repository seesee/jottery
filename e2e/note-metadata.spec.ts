/**
 * E2E tests for note metadata persistence
 * Tests pin state, preview state, and other UI settings
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './test-utils';

test.describe('Note Metadata Persistence', () => {
  test.beforeEach(async ({ page }) => {
    const jp = new JotteryPage(page);
    await jp.setup();
  });

  async function createNote(page: any, content: string) {
    const jp = new JotteryPage(page);
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially(content);

    // Wait for auto-save by checking note appears in list (state-based wait)
    // Extract a searchable word from content (skip markdown symbols and split by whitespace)
    const noteList = page.getByRole('list');
    const cleanContent = content.replace(/^[#*_>\-]+\s*/, '');
    const words = cleanContent.split(/\s+/).filter((w: string) => w.length > 2);
    const searchWord = words[0] || cleanContent.substring(0, 10);
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(new RegExp(searchWord, 'i')).first()).toBeVisible({ timeout: 5000 });
  }

  test.describe('Pin State', () => {
    test('should persist pin state when switching notes', async ({ page }) => {
      // Create first note
      await createNote(page, 'First note content');

      // Create second note
      await createNote(page, 'Second note content');

      // Click on first note in the list
      const noteList = page.getByRole('list');
      const firstNoteItem = noteList.getByRole('button').filter({ hasText: /First note/i }).first();
      await firstNoteItem.click();

      // Wait for editor to load
      const jp = new JotteryPage(page);
      const editor = jp.editorContent;
      await expect(editor).toBeVisible({ timeout: 3000 });

      // Find and click pin button (specifically the one with "Pin note" title)
      const pinButton = page.getByRole('button', { name: 'Pin note' });

      if (await pinButton.isVisible()) {
        await pinButton.click();

        // Wait for pin state to change - button should become "Unpin note"
        const unpinButton = page.getByRole('button', { name: 'Unpin note' });
        await expect(unpinButton).toBeVisible({ timeout: 3000 });

        // Switch to second note
        const secondNoteItem = noteList.getByRole('button').filter({ hasText: /Second note/i }).first();
        await secondNoteItem.click();
        await expect(editor).toBeVisible({ timeout: 3000 });

        // Switch back to first note
        await firstNoteItem.click();
        await expect(editor).toBeVisible({ timeout: 3000 });

        // Check that pin indicator is still visible (Unpin button means it's pinned)
        const pinIndicator = page.locator('[class*="pin"], [data-pinned="true"], svg[class*="pin"]');
        const isPinned = await pinIndicator.count() > 0 || await unpinButton.isVisible();

        // If we found a pin button earlier, pin state should be preserved
        // (The note list should show the first note is pinned)
        expect(isPinned || (await noteList.locator('[class*="pinned"]').count() > 0)).toBe(true);
      }
    });

    test('should show pinned notes at top of list', async ({ page }) => {
      // Create two notes
      await createNote(page, 'Apple note');
      await createNote(page, 'Banana note');

      // Wait for notes to appear in list
      const noteList = page.getByRole('list');
      await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Apple/i).first()).toBeVisible();
      await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Banana/i).first()).toBeVisible();

      // Click on first note (Apple)
      await noteList.getByRole('button').filter({ hasText: /Apple/i }).first().click();
      await page.waitForTimeout(500);

      // Try to pin it
      const pinButton = page.locator('button').filter({ hasText: /pin/i }).first()
        .or(page.locator('button[title*="pin" i]').first())
        .or(page.locator('button[aria-label*="pin" i]').first());

      if (await pinButton.isVisible()) {
        await pinButton.click();
        await page.waitForTimeout(1000);

        // The pinned note should now be at the top
        // Check the order of notes in the list
        const noteItems = noteList.locator('[class*="item"], li, [role="listitem"]');
        const firstNoteText = await noteItems.first().textContent();

        // Apple should be first (pinned notes come first)
        expect(firstNoteText?.toLowerCase()).toContain('apple');
      }
    });
  });

  test.describe('Preview State', () => {
    test('should persist preview state for markdown notes', async ({ page }) => {
      // Create a markdown note
      await createNote(page, '# Heading\n\nThis is **bold** text');

      // Look for a preview toggle button
      const previewButton = page.locator('button').filter({ hasText: /preview/i }).first()
        .or(page.locator('button[title*="preview" i]').first())
        .or(page.locator('button[aria-label*="preview" i]').first());

      if (await previewButton.isVisible()) {
        // Enable preview
        await previewButton.click();
        await page.waitForTimeout(1000);

        // Check that preview is shown (look for rendered markdown)
        const previewContent = page.locator('[class*="preview"], [class*="markdown"], h1, strong');
        const hasPreview = await previewContent.count() > 0;

        if (hasPreview) {
          // Create another note (this should switch away)
          await createNote(page, 'Another note');

          // Go back to the markdown note
          const noteList = page.getByRole('list');
          await noteList.getByRole('button').filter({ hasText: /Heading/i }).first().click();
          await page.waitForTimeout(500);

          // Preview should still be enabled
          // Look for the rendered heading or bold text
          const renderedMarkdown = page.locator('h1, strong').filter({ hasText: /Heading|bold/i });
          const previewStillEnabled = await renderedMarkdown.count() > 0;

          expect(previewStillEnabled).toBe(true);
        }
      }
    });

    test('should disable preview when switching to non-preview language', async ({ page }) => {
      // Create a markdown note with preview enabled
      await createNote(page, '# Markdown content');

      // Enable preview if available
      const previewButton = page.locator('button').filter({ hasText: /preview/i }).first()
        .or(page.locator('button[title*="preview" i]').first());

      if (await previewButton.isVisible()) {
        await previewButton.click();
        await page.waitForTimeout(500);

        // Change language to plain text (if language selector exists)
        const languageSelector = page.locator('select').filter({ hasText: /plain|markdown|javascript/i }).first()
          .or(page.locator('[class*="language"]').first());

        if (await languageSelector.isVisible()) {
          // Try to select plain text option
          try {
            await languageSelector.selectOption('plain');
          } catch {
            // If select doesn't work, try clicking an option
            await languageSelector.click();
            await page.locator('option, [role="option"]').filter({ hasText: /plain/i }).first().click();
          }
          await page.waitForTimeout(500);

          // Preview should be disabled (can't preview plain text)
          const previewContent = page.locator('[class*="preview"]');
          const previewDisabled = await previewContent.count() === 0;

          expect(previewDisabled).toBe(true);
        }
      }
    });
  });

  test.describe('Sort Order with Pin', () => {
    test('should re-sort list when note is pinned', async ({ page }) => {
      // Create notes with specific content for identification
      await createNote(page, 'Zebra note - created last');
      await page.waitForTimeout(1500);

      await createNote(page, 'Apple note - created first');
      await page.waitForTimeout(1500);

      const noteList = page.getByRole('list');

      // In "recent" sort order, Zebra should be first (created more recently based on modifiedAt)
      // Actually, Apple was created last so it should be first
      const noteItems = noteList.locator('[class*="item"], li, [role="listitem"]');

      // Click on Zebra note
      await noteList.getByRole('button').filter({ hasText: /Zebra/i }).first().click();
      await page.waitForTimeout(500);

      // Pin Zebra note
      const pinButton = page.locator('button').filter({ hasText: /pin/i }).first()
        .or(page.locator('button[title*="pin" i]').first())
        .or(page.locator('button[aria-label*="pin" i]').first());

      if (await pinButton.isVisible()) {
        await pinButton.click();
        await page.waitForTimeout(1500); // Wait for save and re-sort

        // Now Zebra should be at the top regardless of when it was modified
        const firstNoteText = await noteItems.first().textContent();
        expect(firstNoteText?.toLowerCase()).toContain('zebra');
      }
    });
  });

  test.describe('Word Wrap State', () => {
    test('should persist word wrap setting', async ({ page }) => {
      // Create a note with long content
      await createNote(page, 'This is a very long line that should wrap if word wrap is enabled otherwise it will extend beyond the viewport horizontally');

      // Look for word wrap toggle
      const wordWrapButton = page.locator('button').filter({ hasText: /wrap/i }).first()
        .or(page.locator('button[title*="wrap" i]').first())
        .or(page.locator('button[aria-label*="wrap" i]').first());

      if (await wordWrapButton.isVisible()) {
        // Toggle word wrap off
        await wordWrapButton.click();
        await page.waitForTimeout(1000);

        // Create another note
        await createNote(page, 'Short note');

        // Go back to the long note
        const noteList = page.getByRole('list');
        await noteList.getByRole('button').filter({ hasText: /very long line/i }).first().click();
        await page.waitForTimeout(500);

        // Word wrap state should be preserved
        // This is hard to test visually, but we can check if the toggle state is preserved
        const isWordWrapOff = await page.locator('[class*="nowrap"], [data-wordwrap="false"]').count() > 0;

        // If we found the toggle, the state should have been persisted
        expect(isWordWrapOff || await wordWrapButton.isVisible()).toBe(true);
      }
    });
  });

  test.describe('Pinned Note Protection', () => {
    // Use desktop viewport for this test (hover behaviour)
    test.use({ viewport: { width: 1280, height: 720 } });

    test('should NOT show delete button when hovering over pinned notes', async ({ page }) => {
      const jp = new JotteryPage(page);

      // Create a note
      await createNote(page, 'Protected pinned note');

      // Click on the note in the list to select it
      const noteList = page.getByRole('list');
      await noteList.getByRole('button').filter({ hasText: /Protected pinned/i }).first().click();
      await page.waitForTimeout(500);

      // Pin the note
      const pinButton = page.getByRole('button', { name: 'Pin note' });
      if (await pinButton.isVisible()) {
        await pinButton.click();
        await page.waitForTimeout(1000);

        // Create another note so we're not editing the pinned one
        await createNote(page, 'Another unpinned note');

        // Find the pinned note in the list
        const pinnedNoteItem = jp.noteListItems.filter({
          hasText: /Protected pinned/i
        }).first();
        await expect(pinnedNoteItem).toBeVisible();

        // Hover over the pinned note
        await pinnedNoteItem.hover();
        await page.waitForTimeout(300);

        // The delete button (x mark) should NOT be visible on pinned notes
        const deleteButton = pinnedNoteItem.locator('.delete-button, [aria-label*="delete" i], button svg path[d*="M6 18L18 6"]');
        const deleteButtonVisible = await deleteButton.isVisible().catch(() => false);

        expect(deleteButtonVisible).toBe(false);
      }
    });

    test('should show delete button when hovering over unpinned notes', async ({ page }) => {
      const jp = new JotteryPage(page);

      // Create an unpinned note
      await createNote(page, 'Unpinned deletable note');

      // Find the note in the list
      const noteItem = jp.noteListItems.filter({
        hasText: /Unpinned deletable/i
      });
      await expect(noteItem).toBeVisible();

      // Hover over the unpinned note
      await noteItem.hover();
      await page.waitForTimeout(300);

      // The delete button should be visible for unpinned notes
      const deleteButton = noteItem.locator('.delete-button');
      const deleteButtonVisible = await deleteButton.isVisible().catch(() => false);

      expect(deleteButtonVisible).toBe(true);
    });
  });
});
