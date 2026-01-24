/**
 * Screenshot generation for landing page placeholders
 * Run with: npx playwright test screenshots --project=firefox
 * Screenshots will be saved to: screenshots/
 */

import { test, expect } from '@playwright/test';
import { readFile } from 'fs/promises';
import { join } from 'path';

// Helper function to import demo notes via Settings UI
async function importDemoNotes(page: any) {
  const demoNotesPath = join(process.cwd(), 'demo', 'jottery-demo-notes.json');

  // Open settings
  const settingsButton = page.locator('button').filter({ hasText: /Settings|⚙️/i }).first();
  await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
  await settingsButton.click();
  await page.waitForTimeout(500);

  // Navigate to Advanced tab
  const advancedTab = page.locator('button, [role="tab"]').filter({ hasText: /Advanced/i }).first();
  await advancedTab.waitFor({ state: 'visible' });
  await advancedTab.click();
  await page.waitForTimeout(500);

  // Find and click the Import button (which triggers file input)
  const importButton = page.locator('button').filter({ hasText: /📥.*Import/i }).first();
  await importButton.waitFor({ state: 'visible' });

  // Set up file chooser handler before clicking
  const fileChooserPromise = page.waitForEvent('filechooser');
  await importButton.click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(demoNotesPath);

  // Wait for import to complete - look for the Done button
  const doneButton = page.locator('button').filter({ hasText: /Done|Close/i }).last();
  await doneButton.waitFor({ state: 'visible', timeout: 15000 });
  await doneButton.click();
  await page.waitForTimeout(500);

  // Close settings modal
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);

  // Verify modal is closed
  const modalCheck = page.locator('[role="dialog"]').first();
  const isModalOpen = await modalCheck.isVisible().catch(() => false);

  if (isModalOpen) {
    // Try one more escape
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);
  }
}

// Helper to set tag colors directly in IndexedDB and update the settings store
async function setTagColors(page: any, tagColors: Record<string, string>) {
  await page.evaluate(async (colors: Record<string, string>) => {
    // Import settingsRepository and settings store from the global app context
    // @ts-ignore - These are available in the browser context
    const { settingsRepository, settings, DEFAULT_SETTINGS } = window.__appContext || {};

    if (!settingsRepository || !settings) {
      throw new Error('App context not available - settingsRepository or settings store not found');
    }

    // Update settings via the repository (which writes to IndexedDB)
    const currentSettings = await settingsRepository.get();
    const updatedSettings = {
      ...currentSettings,
      tagColors: colors,
    };
    await settingsRepository.update(updatedSettings);

    // Update the settings store to reflect the change immediately
    settings.set({ ...DEFAULT_SETTINGS, ...updatedSettings });
  }, tagColors);

  await page.waitForTimeout(500);
}

test.describe('Landing Page Screenshots - Light Mode', () => {
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

    // Reload with theme parameter and set up password
    await page.goto('/?theme=light');

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

    // Import demo notes
    await importDemoNotes(page);

    // Set tag colors for demo, recipe, and notes tags
    await setTagColors(page, {
      demo: 'blue',
      recipe: 'orange',
      notes: 'purple',
    });

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);
  });

  test('01-light. Main Interface - Note List and Editor', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note (should be pinned at top)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-light.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-light.png');
  });

  test('01a-light. Main Interface - Preview Mode', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button
    const previewButton = page.locator('button').filter({ hasText: /Preview/i }).first();
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-light-preview.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-light-preview.png');
  });

  test('01b-light. Main Interface - Japan Preview', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button
    const previewButton = page.locator('button').filter({ hasText: /Preview/i }).first();
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-light-japan-preview.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-light-japan-preview.png');
  });

  test('01c-light. Main Interface - Japan Note', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-light-japan.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-light-japan.png');
  });

  test('02-light. Rich Editor - Python Syntax Highlighting', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the Python QuickSort note from demo
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const pythonNote = noteListItems.filter({ hasText: /Python Quick Sort/i }).first();
    await pythonNote.waitFor({ state: 'visible' });
    await pythonNote.click();
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/02-rich-editor-light.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/02-rich-editor-light.png');
  });

  test('03-light. Multi-Select - Bulk Operations', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Select 3 notes using Cmd/Ctrl+Click (skip first note as it's pinned)
    const noteListItems = page.locator('button.note-list-item');
    const noteCount = await noteListItems.count();

    // Use Meta (Cmd) on Mac, Control on other platforms
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';

    // Click notes 1-3 (skip index 0 as Welcome note is pinned)
    for (let i = 1; i <= Math.min(3, noteCount - 1); i++) {
      await noteListItems.nth(i).click({ modifiers: [modifier] });
      await page.waitForTimeout(400);
    }

    // Wait for toolbar to appear and settle
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/03-multi-select-light.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/03-multi-select-light.png');
  });

  test('05-light. REPL Calculator', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Try keyboard shortcut
    await page.keyboard.press('Control+Shift+C');
    await page.waitForTimeout(500);

    // Check if calculator opened
    let calcVisible = await page.locator('[role="dialog"]').filter({ hasText: /Calculator|Calc/i }).isVisible().catch(() => false);

    if (!calcVisible) {
      // Try finding calculator button
      const calcButton = page.locator('button').filter({ hasText: /Calculator|🧮|Calc/i }).first();
      if (await calcButton.isVisible().catch(() => false)) {
        await calcButton.click();
        await page.waitForTimeout(500);
      }
    }

    // Type calculations
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
      path: 'screenshots/05-calculator-light.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/05-calculator-light.png');
  });
});

test.describe('Landing Page Screenshots - Dark Mode', () => {
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

    // Import demo notes
    await importDemoNotes(page);

    // Set tag colors for demo, recipe, and notes tags
    await setTagColors(page, {
      demo: 'blue',
      recipe: 'orange',
      notes: 'purple',
    });

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);

    // Apply dark theme manually
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500); // Wait for colors to update
  });

  test('01-dark. Main Interface - Note List and Editor', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/01-main-interface-dark.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-dark.png');
  });

  test('01a-dark. Main Interface - Preview Mode', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button
    const previewButton = page.locator('button').filter({ hasText: /Preview/i }).first();
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-dark-preview.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-dark-preview.png');
  });

  test('01b-dark. Main Interface - Japan Preview', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button
    const previewButton = page.locator('button').filter({ hasText: /Preview/i }).first();
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-dark-japan-preview.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-dark-japan-preview.png');
  });

  test('01c-dark. Main Interface - Japan Note', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: 'screenshots/01-main-interface-dark-japan.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/01-main-interface-dark-japan.png');
  });

  test('02-dark. Rich Editor - Python Syntax Highlighting', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the Python QuickSort note from demo
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const pythonNote = noteListItems.filter({ hasText: /Python Quick Sort/i }).first();
    await pythonNote.waitFor({ state: 'visible' });
    await pythonNote.click();
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/02-rich-editor-dark.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/02-rich-editor-dark.png');
  });

  test('03-dark. Multi-Select - Bulk Operations', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Select 3 notes using Cmd/Ctrl+Click (skip first note as it's pinned)
    const noteListItems = page.locator('button.note-list-item');
    const noteCount = await noteListItems.count();

    // Use Meta (Cmd) on Mac, Control on other platforms
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';

    // Click notes 1-3 (skip index 0 as Welcome note is pinned)
    for (let i = 1; i <= Math.min(3, noteCount - 1); i++) {
      await noteListItems.nth(i).click({ modifiers: [modifier] });
      await page.waitForTimeout(400);
    }

    // Wait for toolbar to appear and settle
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/03-multi-select-dark.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/03-multi-select-dark.png');
  });
});
