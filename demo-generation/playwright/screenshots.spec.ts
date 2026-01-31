/**
 * Screenshot generation for landing page placeholders
 * Run with: npx playwright test screenshots --project=firefox
 * Screenshots will be saved to: screenshots/en-GB/
 */

import { test, expect } from '@playwright/test';
import { readFile } from 'fs/promises';
import { join } from 'path';

const LANG = process.env.LANG || 'en-GB';
const SCREENSHOT_DIR = `screenshots/${LANG}`;

// Helper function to import demo notes via Settings UI
async function importDemoNotes(page: any) {
  const demoNotesPath = join(process.cwd(), 'demo-generation', 'jottery-demo-notes.json');

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
      path: `${SCREENSHOT_DIR}/01-main-interface-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-light.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-light-preview.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-light-preview.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-light-japan-preview.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-light-japan-preview.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-light-japan.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-light-japan.png');
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
      path: `${SCREENSHOT_DIR}/02-rich-editor-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/02-rich-editor-light.png');
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
      path: `${SCREENSHOT_DIR}/03-multi-select-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/03-multi-select-light.png');
  });

  test('04-light. Version History', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Create version history by making edits and navigating away/back to trigger sync
    const editor = page.locator('.cm-content').first();
    const welcomeNote = noteListItems.filter({ hasText: /Welcome to Jottery/i }).first();

    // Edit 1: Add packing list start
    await editor.click();
    await page.keyboard.press('End'); // Go to end of document
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('## Packing List');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Passport');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Camera');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 1)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Edit 2: Add more items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Travel adapter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Guidebook');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 2)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Edit 3: Add final items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Comfortable shoes');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Rain jacket');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 3)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Open the more menu to access version history
    const moreButton = page.locator('button[aria-label="More actions"]').first();
    await moreButton.waitFor({ state: 'visible', timeout: 5000 });
    await moreButton.click();
    await page.waitForTimeout(500);

    // Click Version History option (look for 📜 emoji)
    const versionHistoryButton = page.locator('button').filter({ hasText: /📜/ }).first();
    await versionHistoryButton.waitFor({ state: 'visible', timeout: 5000 });
    await versionHistoryButton.click();
    await page.waitForTimeout(1000);

    // Wait for modal to appear
    const modal = page.locator('[role="dialog"]').first();
    await modal.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {
      console.warn('⚠️ Version history modal did not appear');
    });

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/04-version-history-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/04-version-history-light.png');
  });

  test('05-light. REPL Calculator', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Create a new note
    const newButton = page.locator('button').filter({ hasText: /\+ New|New Note/i }).first();
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples (results auto-evaluate in gray)
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('# Compound interest');
    await page.keyboard.press('Enter');
    await page.keyboard.type('principal = 1000');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('rate = 0.05');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('years = 10');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('principal * (1 + rate)^years');
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/05-calculator-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/05-calculator-light.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-dark.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-dark-preview.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-dark-preview.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-dark-japan-preview.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-dark-japan-preview.png');
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
      path: `${SCREENSHOT_DIR}/01-main-interface-dark-japan.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/01-main-interface-dark-japan.png');
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
      path: `${SCREENSHOT_DIR}/02-rich-editor-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/02-rich-editor-dark.png');
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
      path: `${SCREENSHOT_DIR}/03-multi-select-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/03-multi-select-dark.png');
  });

  test('04-dark. Version History', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Create version history by making edits and navigating away/back to trigger sync
    const editor = page.locator('.cm-content').first();
    const welcomeNote = noteListItems.filter({ hasText: /Welcome to Jottery/i }).first();

    // Edit 1: Add packing list start
    await editor.click();
    await page.keyboard.press('End'); // Go to end of document
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('## Packing List');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Passport');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Camera');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 1)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Edit 2: Add more items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Travel adapter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Guidebook');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 2)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Edit 3: Add final items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Comfortable shoes');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Rain jacket');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 3)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Open the more menu to access version history
    const moreButton = page.locator('button[aria-label="More actions"]').first();
    await moreButton.waitFor({ state: 'visible', timeout: 5000 });
    await moreButton.click();
    await page.waitForTimeout(500);

    // Click Version History option (look for 📜 emoji)
    const versionHistoryButton = page.locator('button').filter({ hasText: /📜/ }).first();
    await versionHistoryButton.waitFor({ state: 'visible', timeout: 5000 });
    await versionHistoryButton.click();
    await page.waitForTimeout(1000);

    // Wait for modal to appear
    const modal = page.locator('[role="dialog"]').first();
    await modal.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {
      console.warn('⚠️ Version history modal did not appear');
    });

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/04-version-history-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/04-version-history-dark.png');
  });

  test('05-dark. REPL Calculator', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Create a new note
    const newButton = page.locator('button').filter({ hasText: /\+ New|New Note/i }).first();
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples (results auto-evaluate in gray)
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('# Compound interest');
    await page.keyboard.press('Enter');
    await page.keyboard.type('principal = 1000');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('rate = 0.05');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('years = 10');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('principal * (1 + rate)^years');
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/05-calculator-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/05-calculator-dark.png');
  });
});

// Helper function to import demo notes on mobile (via hamburger menu)
async function importDemoNotesMobile(page: any) {
  const demoNotesPath = join(process.cwd(), 'demo-generation', 'jottery-demo-notes.json');

  // On mobile, settings is behind the hamburger menu
  const hamburgerMenu = page.locator('button[aria-label="Menu"], button.hamburger-menu, [aria-label="Toggle menu"]').first();
  const isHamburgerVisible = await hamburgerMenu.isVisible().catch(() => false);

  if (isHamburgerVisible) {
    await hamburgerMenu.click();
    await page.waitForTimeout(500);
  }

  // Now find and click Settings
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

test.describe('Landing Page Screenshots - Mobile Light', () => {
  // iPhone 12 viewport
  const MOBILE_VIEWPORT = { width: 390, height: 844 };

  test.beforeEach(async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize(MOBILE_VIEWPORT);

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

    // Reload with light theme
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

    // Import demo notes (mobile version - handles hamburger menu)
    await importDemoNotesMobile(page);

    // Set tag colors for demo, recipe, and notes tags
    await setTagColors(page, {
      demo: 'blue',
      recipe: 'orange',
      notes: 'purple',
    });

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);
  });

  test('06-mobile-light. Note List View', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Take screenshot of the note list (mobile shows list by default)
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/06-mobile-list-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/06-mobile-list-light.png');
  });

  test('07-mobile-light. Japan Itinerary Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Find and click the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot of the note view
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/07-mobile-japan-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/07-mobile-japan-light.png');
  });

  test('08-mobile-light. Calculator Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Create a new note - on mobile this is a "+" icon button
    // Try multiple selectors for the new note button
    const newButtonSelectors = [
      'button[aria-label="New note"]',
      'button[aria-label="Add note"]',
      'button:has-text("+")',
      'button.new-note-button',
    ];

    let newButton = null;
    for (const selector of newButtonSelectors) {
      const btn = page.locator(selector).first();
      if (await btn.isVisible().catch(() => false)) {
        newButton = btn;
        break;
      }
    }

    // Fallback: find blue button with + or the text version
    if (!newButton) {
      newButton = page.locator('button').filter({ hasText: /^\+$|New Note|\+ New/i }).first();
    }

    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('# Budget calc');
    await page.keyboard.press('Enter');
    await page.keyboard.type('income = 5000');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('rent = 1500');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('income - rent');
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/08-mobile-calculator-light.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/08-mobile-calculator-light.png');
  });
});

test.describe('Landing Page Screenshots - Mobile Dark', () => {
  // iPhone 12 viewport
  const MOBILE_VIEWPORT = { width: 390, height: 844 };

  test.beforeEach(async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize(MOBILE_VIEWPORT);

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

    // Import demo notes (mobile version - handles hamburger menu)
    await importDemoNotesMobile(page);

    // Set tag colors for demo, recipe, and notes tags
    await setTagColors(page, {
      demo: 'blue',
      recipe: 'orange',
      notes: 'purple',
    });

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);

    // Apply dark theme
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500);
  });

  test('06-mobile-dark. Note List View', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Take screenshot of the note list (mobile shows list by default)
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/06-mobile-list-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/06-mobile-list-dark.png');
  });

  test('07-mobile-dark. Japan Itinerary Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Find and click the Japan Trip note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const japanNote = noteListItems.filter({ hasText: /Japan Trip/i }).first();
    await japanNote.waitFor({ state: 'visible' });
    await japanNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot of the note view
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/07-mobile-japan-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/07-mobile-japan-dark.png');
  });

  test('08-mobile-dark. Calculator Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Create a new note - on mobile this is a "+" icon button
    const newButtonSelectors = [
      'button[aria-label="New note"]',
      'button[aria-label="Add note"]',
      'button:has-text("+")',
      'button.new-note-button',
    ];

    let newButton = null;
    for (const selector of newButtonSelectors) {
      const btn = page.locator(selector).first();
      if (await btn.isVisible().catch(() => false)) {
        newButton = btn;
        break;
      }
    }

    // Fallback: find blue button with + or the text version
    if (!newButton) {
      newButton = page.locator('button').filter({ hasText: /^\+$|New Note|\+ New/i }).first();
    }

    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('# Budget calc');
    await page.keyboard.press('Enter');
    await page.keyboard.type('income = 5000');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('rent = 1500');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('income - rent');
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/08-mobile-calculator-dark.png`,
      fullPage: false,
    });

    console.log('✓ Screenshot saved: ${SCREENSHOT_DIR}/08-mobile-calculator-dark.png');
  });
});
