/**
 * E2E tests for Recycle Bin functionality
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

test.describe('Recycle Bin', () => {
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
		await page.waitForTimeout(3000); // Wait for auto-save
	}

	async function deleteNoteViaBulk(page: import('@playwright/test').Page, noteContent: string) {
		// Use bulk selection to delete - Ctrl/Cmd+click to enter multi-select mode
		const noteItem = jp.noteListItems.filter({ hasText: new RegExp(noteContent, 'i') });
		await noteItem.click({ modifiers: ['ControlOrMeta'] });

		// Click bulk delete
		const bulkDeleteButton = page.locator('button').filter({ hasText: /^Delete$/i }).first();
		await bulkDeleteButton.click();

		// Confirm bulk delete
		const confirmBulkDelete = jp.dialog.locator('button').filter({ hasText: /Delete/i }).first();
		await confirmBulkDelete.click();
		await page.waitForTimeout(1000);
	}

	async function openRecycleBin(page: import('@playwright/test').Page) {
		const recycleBinButton = page.locator('button').filter({ hasText: /Recycle Bin/i });
		await recycleBinButton.click();
		await page.waitForTimeout(500);
	}

	test('should restore note and immediately show in notes list', async ({ page }) => {
		// Create a note
		await createNote(page, 'Note to restore');

		// Verify it exists in the list
		const noteList = page.getByRole('list');
		await expect(noteList.locator('[data-testid="note-list-item"] h3').getByText(/Note to restore/i).first()).toBeVisible();

		// Delete the note using bulk selection
		await deleteNoteViaBulk(page, 'Note to restore');

		// Note should no longer be in the main list
		const noteInMainList = jp.noteListItems.filter({ hasText: /Note to restore/i });
		await expect(noteInMainList).toHaveCount(0);

		// Open recycle bin
		await openRecycleBin(page);

		// Verify note is in recycle bin
		const recycleBinModal = jp.dialog;
		await expect(recycleBinModal).toBeVisible();
		await expect(recycleBinModal.getByText(/Note to restore/i)).toBeVisible();

		// Click restore button
		const restoreButton = recycleBinModal.locator('button').filter({ hasText: /Restore/i }).first();
		await restoreButton.click();

		// Wait for restore to process
		await page.waitForTimeout(1000);

		// Note should no longer be in recycle bin
		await expect(recycleBinModal.getByText(/Note to restore/i)).not.toBeVisible();

		// Close recycle bin
		const closeButton = recycleBinModal.locator('button[aria-label*="Close"], button').filter({ hasText: /✕/ }).first();
		await closeButton.click();
		await page.waitForTimeout(500);

		// Note should immediately appear in the main notes list (without refresh)
		await expect(jp.noteListItems.filter({ hasText: /Note to restore/i })).toBeVisible();
	});

	test('should restore note and make it searchable', async ({ page }) => {
		// Create a note with unique content
		await createNote(page, 'UniqueSearchableContent12345');

		// Delete the note using bulk selection
		await deleteNoteViaBulk(page, 'UniqueSearchableContent12345');

		// Open recycle bin and restore
		await openRecycleBin(page);
		const recycleBinModal = jp.dialog;
		const restoreButton = recycleBinModal.locator('button').filter({ hasText: /Restore/i }).first();
		await restoreButton.click();
		await page.waitForTimeout(1000);

		// Close recycle bin
		const closeButton = recycleBinModal.locator('button[aria-label*="Close"], button').filter({ hasText: /✕/ }).first();
		await closeButton.click();
		await page.waitForTimeout(500);

		// Search for the restored note
		const searchInput = jp.searchInput;
		await searchInput.fill('UniqueSearchableContent12345');
		await page.waitForTimeout(500);

		// Note should appear in search results
		await expect(jp.noteListItems.filter({ hasText: /UniqueSearchableContent12345/i })).toBeVisible();
	});

	test('should show empty recycle bin message', async ({ page }) => {
		// Open recycle bin without any deleted notes
		await openRecycleBin(page);

		// Should show empty message
		const recycleBinModal = jp.dialog;
		await expect(recycleBinModal).toBeVisible();
		await expect(recycleBinModal.getByText(/Recycle bin is empty|empty/i)).toBeVisible();
	});

	test('should permanently delete note from recycle bin', async ({ page }) => {
		// Create a note
		await createNote(page, 'Note to permanently delete');

		// Delete the note using bulk selection
		await deleteNoteViaBulk(page, 'Note to permanently delete');

		// Open recycle bin
		await openRecycleBin(page);
		const recycleBinModal = jp.dialog;

		// Verify note is in recycle bin
		await expect(recycleBinModal.getByText(/Note to permanently delete/i)).toBeVisible();

		// Click permanent delete button
		const deleteForeverButton = recycleBinModal.locator('button').filter({ hasText: /Delete Forever/i }).first();
		await deleteForeverButton.click();

		// Confirm permanent deletion in the second modal
		await page.waitForTimeout(500);
		const confirmModal = jp.dialog.last();
		const confirmButton = confirmModal.locator('button').filter({ hasText: /Delete Forever/i });
		await confirmButton.click();
		await page.waitForTimeout(1000);

		// Note should be gone from recycle bin (show empty message)
		await expect(recycleBinModal.getByText(/empty/i)).toBeVisible();
	});
});
