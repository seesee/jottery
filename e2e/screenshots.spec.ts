/**
 * Screenshot generation for landing page placeholders
 * Run with: npx playwright test screenshots --project=chromium
 * Screenshots will be saved to: screenshots/
 */

import { test, expect } from '@playwright/test';
import { readFile } from 'fs/promises';
import { join } from 'path';

test.describe('Landing Page Screenshots', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
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

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);
  });

  test('01. Main Interface - Note List and Editor', async ({ page }) => {
    // Import demo notes via browser API
    const demoNotesPath = join(process.cwd(), 'demo', 'jottery-demo-notes.json');
    const demoNotesJson = await readFile(demoNotesPath, 'utf-8');

    await page.evaluate((jsonData) => {
      const data = JSON.parse(jsonData);
      // Store in localStorage for import
      localStorage.setItem('jottery_import_data', JSON.stringify(data));
      // Trigger a page reload to import
      window.location.reload();
    }, demoNotesJson);

    // Wait for reload and app to initialize
    await page.waitForTimeout(3000);

    // Look for the import functionality in the UI
    // First, try to find settings or menu button
    const settingsButton = page.locator('button').filter({ hasText: /Settings|⚙️/i }).first();
    if (await settingsButton.isVisible().catch(() => false)) {
      await settingsButton.click();
      await page.waitForTimeout(500);

      // Look for import button in settings
      const importButton = page.locator('button').filter({ hasText: /Import/i }).first();
      if (await importButton.isVisible().catch(() => false)) {
        await importButton.click();
        await page.waitForTimeout(500);

        // Paste JSON
        const importTextarea = page.locator('textarea').first();
        if (await importTextarea.isVisible().catch(() => false)) {
          await importTextarea.fill(demoNotesJson);
          await page.locator('button').filter({ hasText: /Import|Confirm/i }).first().click();
          await page.waitForTimeout(2000);
        }
      }

      // Close settings
      await page.keyboard.press('Escape');
      await page.waitForTimeout(500);
    }

    // Click on the first note to show editor
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    if (await firstNote.isVisible().catch(() => false)) {
      await firstNote.click();
      await page.waitForTimeout(1000);
    }

    // Take full page screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface.png');
  });

  test('02. Rich Editor - Syntax Highlighting', async ({ page }) => {
    // Create a note with code
    const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newButton.click();
    await page.waitForTimeout(500);

    // Type Python code
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await editor.pressSequentially(`# Python QuickSort Example

def quicksort(arr):
    if len(arr) <= 1:
        return arr

    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]

    return quicksort(left) + middle + quicksort(right)

# Test it
numbers = [64, 34, 25, 12, 22, 11, 90]
sorted_numbers = quicksort(numbers)
print(f"Sorted: {sorted_numbers}")`, { delay: 10 });

    await page.waitForTimeout(1000);

    // Set syntax to Python via settings menu
    const moreMenu = page.locator('button[aria-label*="More"], button').filter({ hasText: /⋮|More/i }).first();
    if (await moreMenu.isVisible().catch(() => false)) {
      await moreMenu.click();
      await page.waitForTimeout(300);

      const syntaxOption = page.locator('button, [role="menuitem"]').filter({ hasText: /Syntax|Language/i }).first();
      if (await syntaxOption.isVisible().catch(() => false)) {
        await syntaxOption.click();
        await page.waitForTimeout(300);

        const pythonOption = page.locator('button, option').filter({ hasText: /Python/i }).first();
        if (await pythonOption.isVisible().catch(() => false)) {
          await pythonOption.click();
          await page.waitForTimeout(500);
        }
      }
    }

    await page.screenshot({
      path: 'screenshots/02-rich-editor.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/02-rich-editor.png');
  });

  test('03. Multi-Select - Bulk Operations', async ({ page }) => {
    // Create several notes
    for (let i = 1; i <= 5; i++) {
      const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
      await newButton.click();
      await page.waitForTimeout(300);

      const editor = page.locator('.cm-content').first();
      await editor.click();
      await editor.pressSequentially(`Test note ${i} for multi-select demo`, { delay: 10 });
      await page.waitForTimeout(1000);
    }

    // Multi-select notes using Ctrl+Click or checkboxes
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');

    // Try to click checkboxes if they exist
    const checkboxes = page.locator('input[type="checkbox"]');
    const checkboxCount = await checkboxes.count();

    if (checkboxCount > 0) {
      // Click first 3 checkboxes
      for (let i = 0; i < Math.min(3, checkboxCount); i++) {
        await checkboxes.nth(i).click();
        await page.waitForTimeout(200);
      }
    } else {
      // Try Ctrl+Click on notes
      const noteCount = await noteListItems.count();
      for (let i = 0; i < Math.min(3, noteCount); i++) {
        await noteListItems.nth(i).click({ modifiers: ['Control'] });
        await page.waitForTimeout(200);
      }
    }

    await page.waitForTimeout(1000);

    // Bulk operations toolbar should be visible
    await page.screenshot({
      path: 'screenshots/03-multi-select.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/03-multi-select.png');
  });

  test('04. Advanced Search - With Results', async ({ page }) => {
    // Create notes with different tags
    const notesData = [
      { content: 'Meeting notes for Q1 planning #work #urgent', tags: ['work', 'urgent'] },
      { content: 'Recipe for chocolate cake #personal #cooking', tags: ['personal', 'cooking'] },
      { content: 'Bug fix for login issue #work #development', tags: ['work', 'development'] },
    ];

    for (const note of notesData) {
      const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
      await newButton.click();
      await page.waitForTimeout(300);

      const editor = page.locator('.cm-content').first();
      await editor.click();
      await editor.pressSequentially(note.content, { delay: 10 });
      await page.waitForTimeout(1500);
    }

    // Search with modifier
    const searchInput = page.locator('input[type="search"], input[placeholder*="Search"]').first();
    await searchInput.click();
    await searchInput.fill('#work');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/04-search-results.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/04-search-results.png');
  });

  test('05. REPL Calculator', async ({ page }) => {
    // Try to open calculator
    // Look for calculator button or use keyboard shortcut
    await page.keyboard.press('Control+Shift+C');
    await page.waitForTimeout(500);

    // If modal didn't open, try finding button
    const calcButton = page.locator('button').filter({ hasText: /Calculator|🧮|Calc/i }).first();
    if (await calcButton.isVisible().catch(() => false)) {
      await calcButton.click();
      await page.waitForTimeout(500);
    }

    // Type some calculations
    const calcInput = page.locator('input[type="text"]').last();
    if (await calcInput.isVisible().catch(() => false)) {
      await calcInput.click();
      await calcInput.fill('2 + 2');
      await page.keyboard.press('Enter');
      await page.waitForTimeout(300);

      await calcInput.fill('sqrt(144)');
      await page.keyboard.press('Enter');
      await page.waitForTimeout(300);

      await calcInput.fill('pi * 10');
      await page.keyboard.press('Enter');
      await page.waitForTimeout(500);
    }

    await page.screenshot({
      path: 'screenshots/05-calculator.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/05-calculator.png');
  });

  test('06. Settings - Categories/Colors', async ({ page }) => {
    // Open settings
    const settingsButton = page.locator('button').filter({ hasText: /Settings|⚙️/i }).first();
    await settingsButton.click();
    await page.waitForTimeout(500);

    // Navigate to Colors/Categories tab
    const categoriesTab = page.locator('button, [role="tab"]').filter({ hasText: /Categories|Colors|Colours/i }).first();
    if (await categoriesTab.isVisible().catch(() => false)) {
      await categoriesTab.click();
      await page.waitForTimeout(500);
    }

    await page.screenshot({
      path: 'screenshots/06-categories-settings.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/06-categories-settings.png');
  });

  test('07. Mobile View - Responsive Design', async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 }); // iPhone SE

    // Create a note
    const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newButton.click();
    await page.waitForTimeout(500);

    const editor = page.locator('.cm-content').first();
    await editor.click();
    await editor.pressSequentially('Mobile view demo note\n\nThis shows how Jottery looks on mobile devices.', { delay: 10 });
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/07-mobile-view.png',
      fullPage: true,
    });

    console.log('✓ Screenshot saved: screenshots/07-mobile-view.png');
  });

  test('08. Dark Mode', async ({ page }) => {
    // Try to toggle dark mode
    const settingsButton = page.locator('button').filter({ hasText: /Settings|⚙️/i }).first();
    await settingsButton.click();
    await page.waitForTimeout(500);

    // Look for theme setting
    const themeSelect = page.locator('select').filter({ hasText: /Light|Dark|Auto/i }).first();
    if (await themeSelect.isVisible().catch(() => false)) {
      await themeSelect.selectOption('dark');
      await page.waitForTimeout(500);
    } else {
      // Try toggle button
      const darkModeToggle = page.locator('button, [role="switch"]').filter({ hasText: /Dark|Theme/i }).first();
      if (await darkModeToggle.isVisible().catch(() => false)) {
        await darkModeToggle.click();
        await page.waitForTimeout(500);
      }
    }

    // Close settings
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);

    // Create a note in dark mode
    const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newButton.click();
    await page.waitForTimeout(300);

    const editor = page.locator('.cm-content').first();
    await editor.click();
    await editor.pressSequentially('Dark mode example\n\nJottery looks great in dark mode too!', { delay: 10 });
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/08-dark-mode.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/08-dark-mode.png');
  });
});
