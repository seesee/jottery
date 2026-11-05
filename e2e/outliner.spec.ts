/**
 * E2E tests for Outliner document type
 */

import { test, expect, Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

// Helper to type into a contenteditable element reliably
async function typeInContentEditable(page: Page, locator: ReturnType<Page['locator']>, text: string) {
	await locator.click();
	// Select all existing content
	await page.keyboard.press('Control+a');
	// Type the new content (this replaces selected content)
	await page.keyboard.insertText(text);
}

test.describe('Outliner Mode', () => {
	test.beforeEach(async ({ page }) => {
		const jp = new JotteryPage(page);
		await jp.setup();
	});

	test('should create outliner note and add items', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Wait for outliner editor to appear
		await expect(page.locator('.outliner-editor')).toBeVisible();

		// Type first item
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'First item');

		// Check content is there
		await expect(firstContent).toContainText('First item');
	});

	test('should create new sibling on Enter', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Type first item
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'Item 1');
		await page.keyboard.press('Enter');

		// Verify two items exist
		const contents = page.locator('.outliner-editor .content');
		await expect(contents).toHaveCount(2);
		await expect(contents.first()).toContainText('Item 1');

		// Type in second item
		const secondContent = contents.nth(1);
		await typeInContentEditable(page, secondContent, 'Item 2');
		await expect(secondContent).toContainText('Item 2');
	});

	test('should indent item with Tab', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Create parent
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'Parent');
		await page.keyboard.press('Enter');
		await page.waitForTimeout(200);

		// Type child and indent
		const secondContent = page.locator('.outliner-editor .content').nth(1);
		await typeInContentEditable(page, secondContent, 'Child');
		await page.keyboard.press('Tab');

		// Verify nested structure exists
		const childrenContainer = page.locator('.outliner-editor .outliner-node .children');
		await expect(childrenContainer).toBeVisible();
	});

	test('should outdent item with Shift+Tab', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Create parent with indented child
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'Parent');
		await page.keyboard.press('Enter');
		await page.waitForTimeout(200);

		const secondContent = page.locator('.outliner-editor .content').nth(1);
		await typeInContentEditable(page, secondContent, 'Child');
		await page.keyboard.press('Tab');

		// Verify it's nested
		await expect(page.locator('.outliner-editor .children .content')).toHaveCount(1);

		// Outdent
		await page.keyboard.press('Shift+Tab');

		// After outdent, both should be at root level
		const rootNodes = page.locator('.outliner-editor > .outliner-node');
		await expect(rootNodes).toHaveCount(2);
	});

	test('should collapse and expand nodes with children', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Create parent with child
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'Parent');
		await page.keyboard.press('Enter');
		await page.waitForTimeout(200);

		const secondContent = page.locator('.outliner-editor .content').nth(1);
		await typeInContentEditable(page, secondContent, 'Child');
		await page.keyboard.press('Tab');

		// Verify child is visible in children container
		const childContent = page.locator('.outliner-editor .children .content');
		await expect(childContent).toBeVisible();

		// Click collapse button on parent
		const collapseButton = page.locator('.outliner-editor > .outliner-node > .node-row .bullet.has-children').first();
		await collapseButton.click();
		await page.waitForTimeout(300);

		// Children container should now be hidden
		const childrenContainer = page.locator('.outliner-editor > .outliner-node > .children');
		await expect(childrenContainer).not.toBeVisible();

		// Click expand button
		await collapseButton.click();

		// Children container should be visible again
		await expect(childrenContainer).toBeVisible();
	});

	test('should persist outliner language when note is reopened', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Add some content
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'Test content');

		// Wait for save
		await page.waitForTimeout(3000);

		// Create another note
		await jp.newNoteButton.click();
		await page.waitForTimeout(1000);

		// Clear any multi-select mode
		await page.keyboard.press('Escape');
		await page.waitForTimeout(500);

		// Go back to first note
		const noteList = page.getByRole('list');
		const firstNoteItem = noteList.locator('button').filter({ hasText: /Test content/i }).first();
		await firstNoteItem.click();
		await page.waitForTimeout(1000);

		// Check that outliner language is still selected
		const selectedLanguage = await languageSelect.inputValue();
		expect(selectedLanguage).toBe('outliner');

		// Check that outliner editor is visible
		await expect(page.locator('.outliner-editor')).toBeVisible();
	});

	test('should merge a line with text into the one before it on Backspace', async ({ page }) => {
		const jp = new JotteryPage(page);
		await jp.newNoteButton.click();
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		const firstNode = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstNode, 'first line');
		await page.keyboard.press('Enter');
		await page.waitForTimeout(200);
		await page.keyboard.insertText('second line');
		await page.keyboard.press('Home');

		await page.keyboard.press('Backspace');

		const nodes = page.locator('.outliner-editor .content');
		await expect(nodes).toHaveCount(1);
		await expect(nodes.first()).toHaveText('first linesecond line');
	});

	test('should delete empty node with Backspace', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to outliner language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
		await languageSelect.selectOption('outliner');

		// Create first item
		const firstContent = page.locator('.outliner-editor .content').first();
		await typeInContentEditable(page, firstContent, 'Item 1');
		await page.keyboard.press('Enter');
		await page.waitForTimeout(200);

		// Verify two items exist
		let contents = page.locator('.outliner-editor .content');
		await expect(contents).toHaveCount(2);

		// Delete empty second item with Backspace
		await page.keyboard.press('Backspace');
		await page.waitForTimeout(200);

		// Should be back to one item
		contents = page.locator('.outliner-editor .content');
		await expect(contents).toHaveCount(1);
		await expect(contents.first()).toContainText('Item 1');
	});

	test.describe('shortcuts set in Settings', () => {
		async function setShortcuts(page: Page, shortcuts: Record<string, unknown>) {
			await page.evaluate(async (changes) => {
				const { settings } = await import('/src/lib/stores/appStore.ts' as string);
				settings.update((s: Record<string, unknown>) => ({
					...s,
					keyboardShortcuts: { ...(s.keyboardShortcuts as object), ...changes },
				}));
			}, shortcuts);
		}

		async function twoItems(page: Page) {
			const jp = new JotteryPage(page);
			await jp.newNoteButton.click();
			const languageSelect = page.locator('select').filter({ has: page.locator('option[value="outliner"]') });
			await languageSelect.selectOption('outliner');

			await typeInContentEditable(page, page.locator('.outliner-editor .content').first(), 'First');
			await page.keyboard.press('Enter');
			await expect(page.locator('.outliner-editor .content')).toHaveCount(2);
			await typeInContentEditable(page, page.locator('.outliner-editor .content').nth(1), 'Second');
		}

		test('work on a key that means something else in the outliner', async ({ page }) => {
			await setShortcuts(page, { outlinerMoveUp: { key: 'ArrowUp', alt: true } });
			await twoItems(page);

			// At the start of an item, a plain Up moves to the item above
			await page.keyboard.press('Home');
			await page.keyboard.press('Alt+ArrowUp');

			await expect(page.locator('.outliner-editor .content').first()).toContainText('Second');
		});

		test('replace the old key, so Tab is free to leave the item', async ({ page }) => {
			await setShortcuts(page, { outlinerIndent: { key: 'i', alt: true } });
			await twoItems(page);

			await page.keyboard.press('Tab');
			await expect(page.locator('.outliner-editor .children')).toHaveCount(0);

			await page.locator('.outliner-editor .content').nth(1).click();
			await page.keyboard.press('Alt+i');
			await expect(page.locator('.outliner-editor .children .content')).toHaveCount(1);
		});

		test('still work from settings saved before they existed', async ({ page }) => {
			await page.evaluate(async () => {
				const { settings } = await import('/src/lib/stores/appStore.ts' as string);
				settings.update((s: Record<string, Record<string, unknown>>) => {
					const shortcuts = { ...s.keyboardShortcuts };
					for (const name of Object.keys(shortcuts).filter(key => key.startsWith('outliner'))) {
						delete shortcuts[name];
					}
					return { ...s, keyboardShortcuts: shortcuts };
				});
			});
			await twoItems(page);

			await page.keyboard.press('Tab');

			await expect(page.locator('.outliner-editor .children .content')).toHaveCount(1);
		});

		test('can be recorded, and Tab is allowed here though it is refused elsewhere', async ({ page }) => {
			const jp = new JotteryPage(page);
			await jp.openSettings();
			await jp.dialog.first().locator('button, [role="tab"]').filter({ hasText: /Keyboard|Shortcuts/ }).first().click();
			const indent = page.locator('div', { hasText: /^Indent/ }).last().locator('button').first();
			await expect(indent).toHaveText('TAB');

			await indent.click();
			await page.keyboard.press('Alt+i');
			await expect(indent).toContainText('I');
			await expect(indent).not.toHaveText('TAB');

			await indent.click();
			await page.keyboard.press('Tab');
			await expect(indent).toHaveText('TAB');
			await expect(page.getByRole('alert')).toHaveCount(0);
		});
	});
});
