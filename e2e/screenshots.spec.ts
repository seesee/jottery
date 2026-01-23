/**
 * Screenshot generation for landing page placeholders
 * Run with: npx playwright test screenshots --project=firefox
 * Note: Firefox is used instead of Chromium due to rendering issues with theme switching
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

// Helper to set tag colors directly in IndexedDB
async function setTagColors(page: any, tagColors: Record<string, string>) {
  await page.evaluate(async (colors: Record<string, string>) => {
    // Open IndexedDB
    const dbRequest = indexedDB.open('jottery');
    await new Promise<void>((resolve, reject) => {
      dbRequest.onsuccess = async () => {
        const db = dbRequest.result;
        const tx = db.transaction('settings', 'readwrite');
        const store = tx.objectStore('settings');

        // Get existing settings
        const getRequest = store.get('user_settings');
        getRequest.onsuccess = () => {
          const settings = getRequest.result || {};
          settings.tagColors = colors;

          // Save updated settings
          store.put(settings, 'user_settings');
          tx.oncomplete = () => resolve();
        };
        getRequest.onerror = () => reject(getRequest.error);
      };
      dbRequest.onerror = () => reject(dbRequest.error);
    });
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
    // Ensure light theme is applied
    await page.evaluate(() => {
      document.documentElement.classList.remove('dark');
    });
    await page.waitForTimeout(500);

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

  test('02-light. Rich Editor - Python Syntax Highlighting', async ({ page }) => {
    // Ensure light theme is applied
    await page.evaluate(() => {
      document.documentElement.classList.remove('dark');
    });
    await page.waitForTimeout(500);

    // Create a new note with Python code
    const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newButton.waitFor({ state: 'visible', timeout: 10000 });
    await newButton.click();
    await page.waitForTimeout(1000);

    // Type Python code
    const editor = page.locator('.cm-content').first();
    await editor.waitFor({ state: 'visible' });
    await editor.click();
    await editor.pressSequentially(`def quicksort(arr):
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
print(f"Sorted: {sorted_numbers}")`, { delay: 5 });

    await page.waitForTimeout(1500);

    // Set syntax to Python via the language dropdown (visible in editor toolbar)
    const languageSelect = page.locator('select[aria-label*="language"], select[aria-label*="syntax"], select').first();
    if (await languageSelect.isVisible().catch(() => false)) {
      await languageSelect.selectOption('python');
      await page.waitForTimeout(1500);
    }

    await page.screenshot({
      path: 'screenshots/02-rich-editor-light.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/02-rich-editor-light.png');
  });

  test('03-light. Multi-Select - Bulk Operations', async ({ page }) => {
    // Ensure light theme is applied
    await page.evaluate(() => {
      document.documentElement.classList.remove('dark');
    });
    await page.waitForTimeout(500);

    // Select first 3 notes using checkboxes or Ctrl+Click
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const noteCount = await noteListItems.count();

    // Try checkboxes first
    const checkboxes = page.locator('.note-list-item input[type="checkbox"], [role="listitem"] input[type="checkbox"]');
    const hasCheckboxes = await checkboxes.count() > 0;

    if (hasCheckboxes) {
      // Click first 3 checkboxes
      for (let i = 0; i < Math.min(3, await checkboxes.count()); i++) {
        await checkboxes.nth(i).check();
        await page.waitForTimeout(200);
      }
    } else {
      // Use Ctrl+Click
      for (let i = 0; i < Math.min(3, noteCount); i++) {
        await noteListItems.nth(i).click({ modifiers: ['Control'] });
        await page.waitForTimeout(200);
      }
    }

    await page.waitForTimeout(1000);

    await page.screenshot({
      path: 'screenshots/03-multi-select-light.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/03-multi-select-light.png');
  });

  test('05-light. REPL Calculator', async ({ page }) => {
    // Ensure light theme is applied
    await page.evaluate(() => {
      document.documentElement.classList.remove('dark');
    });
    await page.waitForTimeout(500);

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

    // Reload with theme parameter and set up password
    await page.goto('/?theme=dark');

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

  test('01-dark. Main Interface - Note List and Editor', async ({ page }) => {
    // Ensure dark theme is applied
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500);

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

  test('02-dark. Rich Editor - Python Syntax Highlighting', async ({ page }) => {
    // Ensure dark theme is applied
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500);

    // Create a new note with Python code
    const newButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newButton.waitFor({ state: 'visible', timeout: 10000 });
    await newButton.click();
    await page.waitForTimeout(1000);

    // Type Python code
    const editor = page.locator('.cm-content').first();
    await editor.waitFor({ state: 'visible' });
    await editor.click();
    await editor.pressSequentially(`def quicksort(arr):
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
print(f"Sorted: {sorted_numbers}")`, { delay: 5 });

    await page.waitForTimeout(1500);

    // Set syntax to Python via the language dropdown
    const languageSelect = page.locator('select[aria-label*="language"], select[aria-label*="syntax"], select').first();
    if (await languageSelect.isVisible().catch(() => false)) {
      await languageSelect.selectOption('python');
      await page.waitForTimeout(1500);
    }

    await page.screenshot({
      path: 'screenshots/02-rich-editor-dark.png',
      fullPage: false,
    });

    console.log('✓ Screenshot saved: screenshots/02-rich-editor-dark.png');
  });
});
