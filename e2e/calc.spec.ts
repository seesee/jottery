/**
 * E2E tests for Calculator (REPL) mode
 */

import { test, expect } from '@playwright/test';

test.describe('Calculator Mode', () => {
	test.beforeEach(async ({ page }) => {
		// Clear all storage before each test
		await page.goto('/');
		await page.evaluate(() => {
			localStorage.clear();
			sessionStorage.clear();
			// Clear IndexedDB
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
		await page
			.locator('button[type="submit"], button')
			.filter({ hasText: /Create|Set|Unlock/i })
			.first()
			.click();

		// Wait for app to load
		const appVisible = page
			.getByText(/No notes yet|Create your first note/i)
			.or(page.getByRole('list'));
		await expect(appVisible.first()).toBeVisible({ timeout: 5000 });
	});

	test('should create calc note and show basic arithmetic result', async ({ page }) => {
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type calculation
		const editor = page.locator('.cm-content').first();
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
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type multi-line with variables
		const editor = page.locator('.cm-content').first();
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
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type math function
		const editor = page.locator('.cm-content').first();
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
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type comment and calculation
		const editor = page.locator('.cm-content').first();
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
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type invalid expression
		const editor = page.locator('.cm-content').first();
		await editor.click();
		await editor.pressSequentially('undefined_var');

		// Wait for debounced evaluation
		await page.waitForTimeout(500);

		// Check for error display
		const error = page.locator('.cm-calc-error');
		await expect(error).toBeVisible();
		await expect(error).toContainText('Error');
	});

	test('should handle unit conversions', async ({ page }) => {
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type unit conversion
		const editor = page.locator('.cm-content').first();
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
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type calculation
		const editor = page.locator('.cm-content').first();
		await editor.click();
		await editor.pressSequentially('10 + 5');
		await page.waitForTimeout(500);

		// Check initial result
		let result = page.locator('.cm-calc-result');
		await expect(result).toContainText('15');

		// Edit the calculation
		await editor.click();
		await page.keyboard.press('Control+A'); // Select all
		await editor.pressSequentially('10 + 10');
		await page.waitForTimeout(500);

		// Check updated result
		result = page.locator('.cm-calc-result');
		await expect(result).toContainText('20');
	});

	test('should persist calc language when note is reopened', async ({ page }) => {
		// Create new note
		const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
		await newNoteButton.click();

		// Switch to calc language
		const languageSelect = page.locator('select').filter({ has: page.locator('option[value="calc"]') });
		await languageSelect.selectOption('calc');

		// Type calculation
		const editor = page.locator('.cm-content').first();
		await editor.click();
		await editor.pressSequentially('2 + 2');
		await page.waitForTimeout(3000); // Wait for save

		// Create another note
		await newNoteButton.click();
		await page.waitForTimeout(1000);

		// Go back to first note (click in note list)
		const noteList = page.getByRole('list');
		await noteList.getByText(/2 \+ 2/i).click();
		await page.waitForTimeout(1000);

		// Check that calc language is still selected
		const selectedLanguage = await languageSelect.inputValue();
		expect(selectedLanguage).toBe('calc');

		// Check that result is still displayed
		const result = page.locator('.cm-calc-result');
		await expect(result).toBeVisible();
		await expect(result).toContainText('4');
	});
});
