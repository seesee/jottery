/**
 * E2E tests for bulk operations (multi-select actions)
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

test.describe('Bulk Operations', () => {
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    jp = new JotteryPage(page);
    await jp.setup();
  });

  async function createNote(page: import('@playwright/test').Page, content: string) {
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await editor.click();
    await page.keyboard.press('Control+A');
    await editor.pressSequentially(content);
    await page.waitForTimeout(3000);
  }

  test('should combine two notes into one', async ({ page }) => {
    // Create first note (will be oldest)
    await createNote(page, 'First note content - should appear first');

    // Create second note (will be newest)
    await createNote(page, 'Second note content - should appear last');

    // Verify both notes exist
    const noteList = page.getByRole('list');
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/First note content/i).first()).toBeVisible();
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Second note content/i).first()).toBeVisible();

    // Enter multi-select mode with Ctrl+click on first note
    const firstNoteItem = jp.noteListItems.filter({ hasText: /First note content/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = jp.noteListItems.filter({ hasText: /Second note content/i });
    const secondCheckbox = secondNoteItem.locator('[role="checkbox"]');
    await secondCheckbox.click();

    // Bulk toolbar should appear with "2 selected"
    await expect(page.getByText(/2.*selected/i)).toBeVisible();

    // Click Combine button
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await expect(combineButton).toBeVisible();
    await combineButton.click();

    // Confirmation modal should appear
    await expect(page.getByText(/Combine.*notes/i)).toBeVisible();

    // Click confirm
    const confirmButton = page.locator('button').filter({ hasText: /Combine/i }).last();
    await confirmButton.click();

    // Wait for operation to complete
    await page.waitForTimeout(2000);

    // Should now have only one note containing content from both
    // The combined note should contain a horizontal rule separator
    const editor = jp.editorContent;
    await expect(editor).toContainText(/First note content/i);
    await expect(editor).toContainText(/Second note content/i);

    // Original notes should be gone (moved to recycle bin)
    // There should be only one note in the list now
    const noteItems = await jp.noteListItems.count();
    expect(noteItems).toBe(1);
  });

  test('should combine notes with tags merged by default', async ({ page }) => {
    // Create first note with tags
    await createNote(page, 'Note with tag A');
    const tagInput = jp.tagInput;
    await tagInput.click();
    await tagInput.fill('tag-a');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Create second note with different tags
    await createNote(page, 'Note with tag B');
    await tagInput.click();
    await tagInput.fill('tag-b');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Select both notes - Ctrl+click first to enter multi-select mode
    const firstNoteItem = jp.noteListItems.filter({ hasText: /Note with tag A/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = jp.noteListItems.filter({ hasText: /Note with tag B/i });
    await secondNoteItem.locator('[role="checkbox"]').click();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Confirm (merge tags checkbox is checked by default)
    const confirmButton = page.locator('button').filter({ hasText: /Combine/i }).last();
    await confirmButton.click();

    await page.waitForTimeout(2000);

    // Combined note should have both tags
    await expect(jp.tagPills.filter({ hasText: '#tag-a' })).toBeVisible();
    await expect(jp.tagPills.filter({ hasText: '#tag-b' })).toBeVisible();
  });

  test('should combine notes without merging tags when checkbox unchecked', async ({ page }) => {
    // Create first note (oldest) with tag-first
    await createNote(page, 'First note oldest');
    const tagInput = jp.tagInput;
    await tagInput.click();
    await tagInput.fill('tag-first');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Create second note (newest) with tag-second
    await createNote(page, 'Second note newest');
    await tagInput.click();
    await tagInput.fill('tag-second');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    // Select both notes - Ctrl+click first to enter multi-select mode
    const firstNoteItem = jp.noteListItems.filter({ hasText: /First note oldest/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = jp.noteListItems.filter({ hasText: /Second note newest/i });
    await secondNoteItem.locator('[role="checkbox"]').click();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Wait for modal
    const modal = jp.dialog;
    await expect(modal).toBeVisible();

    // Uncheck the merge tags checkbox
    const mergeTagsCheckbox = modal.locator('input[type="checkbox"]');
    await mergeTagsCheckbox.uncheck();

    // Confirm
    const confirmButton = modal.locator('button').filter({ hasText: /Combine/i });
    await confirmButton.click();

    await page.waitForTimeout(2000);

    // Combined note should only have tag from first (oldest) note
    await expect(jp.tagPills.filter({ hasText: '#tag-first' })).toBeVisible();
    // Second note's tag should NOT be present
    await expect(jp.tagPills.filter({ hasText: '#tag-second' })).not.toBeVisible();
  });

  test('should not show combine button with only one note selected', async ({ page }) => {
    // Create one note
    await createNote(page, 'Single note');

    // Select it with Ctrl+click
    const noteItem = jp.noteListItems.filter({ hasText: /Single note/i });
    await noteItem.click({ modifiers: ['ControlOrMeta'] });

    // Bulk toolbar should appear with "1 selected"
    await expect(page.getByText(/1.*selected/i)).toBeVisible();

    // Combine button should NOT be visible (requires 2+ notes)
    const combineButton = page.locator('button').filter({ hasText: /Combine/i });
    await expect(combineButton).not.toBeVisible();
  });

  test('should cancel combine operation', async ({ page }) => {
    // Create two notes
    await createNote(page, 'Note one');
    await createNote(page, 'Note two');

    // Select both notes - Ctrl+click first to enter multi-select mode
    const noteList = page.getByRole('list');
    const firstNoteItem = jp.noteListItems.filter({ hasText: /Note one/i });
    await firstNoteItem.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click second note's checkbox
    const secondNoteItem = jp.noteListItems.filter({ hasText: /Note two/i });
    await secondNoteItem.locator('[role="checkbox"]').click();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Confirmation modal should appear
    const modal = jp.dialog;
    await expect(modal).toBeVisible();
    await expect(modal.getByText(/Combine.*notes/i)).toBeVisible();

    // Click Cancel button inside the modal
    const cancelButton = modal.locator('button').filter({ hasText: /Cancel/i });
    await cancelButton.click();

    // Modal should close, both notes should still exist
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Note one/i).first()).toBeVisible();
    await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Note two/i).first()).toBeVisible();

    // Count should still be 2
    const noteItems = await jp.noteListItems.count();
    expect(noteItems).toBe(2);
  });

  test('should combine three notes in creation order', async ({ page }) => {
    // Create three notes with distinct content
    await createNote(page, 'AAA First created');
    await page.waitForTimeout(100); // Ensure different creation times
    await createNote(page, 'BBB Second created');
    await page.waitForTimeout(100);
    await createNote(page, 'CCC Third created');

    // Select all three notes - Ctrl+click first to enter multi-select mode
    const firstNote = jp.noteListItems.filter({ hasText: /AAA First/i });
    await firstNote.click({ modifiers: ['ControlOrMeta'] });

    // Now checkboxes are visible - click remaining notes' checkboxes
    const secondNote = jp.noteListItems.filter({ hasText: /BBB Second/i });
    await secondNote.locator('[role="checkbox"]').click();

    const thirdNote = jp.noteListItems.filter({ hasText: /CCC Third/i });
    await thirdNote.locator('[role="checkbox"]').click();

    // Should show 3 selected
    await expect(page.getByText(/3.*selected/i)).toBeVisible();

    // Click Combine
    const combineButton = page.locator('button').filter({ hasText: /Combine/i }).first();
    await combineButton.click();

    // Confirm
    const confirmButton = page.locator('button').filter({ hasText: /Combine/i }).last();
    await confirmButton.click();

    await page.waitForTimeout(2000);

    // Check combined note has content in creation order (oldest first)
    const editor = jp.editorContent;
    const content = await editor.textContent();

    // AAA should come before BBB, and BBB before CCC
    const posA = content?.indexOf('AAA') ?? -1;
    const posB = content?.indexOf('BBB') ?? -1;
    const posC = content?.indexOf('CCC') ?? -1;

    expect(posA).toBeLessThan(posB);
    expect(posB).toBeLessThan(posC);

    // Should only have 1 note now
    const noteItemCount = await jp.noteListItems.count();
    expect(noteItemCount).toBe(1);
  });
});
