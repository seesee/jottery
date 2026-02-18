/**
 * E2E tests for Calculator (REPL) mode
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

test.describe('Calculator Mode', () => {
	test.beforeEach(async ({ page }) => {
		const jp = new JotteryPage(page);
		await jp.setup();
	});

	test('should create calc note and show basic arithmetic result', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type calculation
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('2 + 2');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Check for result
		const result = page.locator('.cm-calc-result');
		await expect(result).toBeVisible();
		await expect(result).toContainText('4');
	});

	test('should handle variables across lines', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type multi-line with variables
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('x = 10');
		await page.keyboard.press('Enter');
		await editor.pressSequentially('y = 20');
		await page.keyboard.press('Enter');
		await editor.pressSequentially('x + y');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Check that last line shows result
		const results = page.locator('.cm-calc-result');
		await expect(results.last()).toBeVisible();
		await expect(results.last()).toContainText('30');
	});

	test('should evaluate math functions', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type math function
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('sqrt(16)');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Check for result
		const result = page.locator('.cm-calc-result');
		await expect(result).toBeVisible();
		await expect(result).toContainText('4');
	});

	test('should handle comments', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type comment and calculation
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('# This is a comment');
		await page.keyboard.press('Enter');
		await editor.pressSequentially('5 * 3');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Comment should not show result
		const results = page.locator('.cm-calc-result');
		await expect(results).toHaveCount(1); // Only one result (for 5 * 3)
		await expect(results.first()).toContainText('15');
	});

	test('should display errors inline', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type invalid expression
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('undefined_var');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Check for error - errors now show as red line numbers, not inline text
		const errorLine = page.locator('.cm-calc-error-line');
		await expect(errorLine).toBeVisible();
		// No result should be shown for error lines
		const result = page.locator('.cm-calc-result');
		await expect(result).toHaveCount(0);
	});

	test('should handle unit conversions', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type unit conversion
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('5 miles to km');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Check for result with units
		const result = page.locator('.cm-calc-result');
		await expect(result).toBeVisible();
		await expect(result).toContainText('8.04'); // Approximate value
		await expect(result).toContainText('km');
	});

	test('should update results when editing', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type calculation
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('10 + 5');
		await page.waitForTimeout(500);

		// Check initial result
		let result = page.locator('.cm-calc-result');
		await expect(result).toContainText('15');

		// Edit the calculation - clear and type new value
		await editor.click();
		// Triple-click to select all content
		await editor.click({ clickCount: 3 });
		// Type new value (this will replace selected text)
		await page.keyboard.type('10 + 10');
		await page.waitForTimeout(500);

		// Check updated result
		result = page.locator('.cm-calc-result');
		await expect(result).toContainText('20');
	});

	test('should persist calc language when note is reopened', async ({ page }) => {
		const jp = new JotteryPage(page);

		// Create new note
		await jp.newNoteButton.click();

		// Switch to calc language
		await jp.languageSelect.selectOption('calc');

		// Type calculation
		const editor = jp.editorContent;
		await editor.click();
		await editor.pressSequentially('2 + 2');
		await page.waitForTimeout(3000); // Wait for save

		// Create another note
		await jp.newNoteButton.click();
		await page.waitForTimeout(1000);

		// Clear any multi-select mode first
		await page.keyboard.press('Escape');
		await page.waitForTimeout(500);

		// Go back to first note - click on the note list item button
		const noteList = page.getByRole('list');
		const firstNoteItem = noteList.locator('button').filter({ hasText: /2 \+ 2/i }).first();
		await firstNoteItem.click();
		await page.waitForTimeout(1000);

		// Check that calc language is still selected
		const selectedLanguage = await jp.languageSelect.inputValue();
		expect(selectedLanguage).toBe('calc');

		// Check that result is still displayed
		const result = page.locator('.cm-calc-result');
		await expect(result).toBeVisible();
		await expect(result).toContainText('4');
	});
});
