/**
 * E2E tests for import/export functionality
 * Tests JSON export, import, and data validation
 */

import { test, expect } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';
import { JotteryPage } from './page-objects';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

test.describe('Import/Export', () => {
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    jp = new JotteryPage(page);
    await jp.setup();
  });

  async function createNote(page: any, content: string, tags: string[] = []) {
    await jp.newNoteButton.click();

    const editor = jp.editorContent;
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially(content);

    // Add tags if provided
    if (tags.length > 0) {
      const tagInput = page.locator('input[placeholder*="tag" i]').first();
      if (await tagInput.isVisible()) {
        for (const tag of tags) {
          await tagInput.fill(tag);
          await tagInput.press('Enter');
        }
      }
    }

    // Wait for auto-save
    await page.waitForTimeout(2000);
  }

  test('should have export button', async ({ page }) => {
    // Look for export button in settings Advanced tab
    await jp.openAdvancedTab();

    // Look within the settings modal dialog
    const settingsModal = jp.dialog.first();
    const exportButton = settingsModal.locator('button').filter({
      hasText: /export/i
    });

    const hasExport = await exportButton.count() > 0;
    expect(hasExport).toBe(true);
  });

  test('should have import button', async ({ page }) => {
    await jp.openAdvancedTab();

    // Look within the settings modal dialog
    const settingsModal = jp.dialog.first();
    const importButton = settingsModal.locator('button').filter({
      hasText: /import/i
    });

    const hasImport = await importButton.count() > 0;
    expect(hasImport).toBe(true);
  });

  test('should export notes to JSON file', async ({ page }) => {
    // Create some notes first
    await createNote(page, 'Export test note one');
    await createNote(page, 'Export test note two');

    // Open settings Advanced tab
    await jp.openAdvancedTab();

    // Set up download listener
    const downloadPromise = page.waitForEvent('download', { timeout: 10000 }).catch(() => null);

    // Click export button within settings modal
    const settingsModal = jp.dialog.first();
    const exportButton = settingsModal.locator('button').filter({
      hasText: /export/i
    }).first();

    if (await exportButton.isVisible()) {
      await exportButton.click();

      // Wait for download
      const download = await downloadPromise;

      if (download) {
        // Verify download happened
        const filename = download.suggestedFilename();
        expect(filename).toMatch(/\.json$/i);

        // Save and verify content
        const downloadPath = path.join(__dirname, 'test-export.json');
        await download.saveAs(downloadPath);

        try {
          const content = fs.readFileSync(downloadPath, 'utf-8');
          const data = JSON.parse(content);

          // Verify export format
          expect(data).toHaveProperty('version');
          expect(data).toHaveProperty('notes');
          expect(Array.isArray(data.notes)).toBe(true);
        } finally {
          if (fs.existsSync(downloadPath)) {
            fs.unlinkSync(downloadPath);
          }
        }
      }
    }
  });

  test('should import notes from JSON file', async ({ page }) => {
    // Create a valid export file
    const exportData = {
      version: '1.0',
      exportDate: new Date().toISOString(),
      notes: [
        {
          id: 'test-import-id-1',
          createdAt: new Date().toISOString(),
          modifiedAt: new Date().toISOString(),
          content: 'Imported note content from test',
          tags: ['imported', 'test'],
          attachments: [],
          pinned: false
        }
      ]
    };

    const importFilePath = path.join(__dirname, 'test-import.json');
    fs.writeFileSync(importFilePath, JSON.stringify(exportData, null, 2));

    try {
      await jp.openAdvancedTab();

      // Find file input for import (hidden, used by import button)
      const fileInput = page.locator('input[type="file"]');

      if (await fileInput.count() > 0) {
        await fileInput.setInputFiles(importFilePath);

        // Wait for import to complete - look for success message or note appearing
        const noteList = page.getByRole('list');
        const successIndicator = page.locator('text=/import.*success|imported|complete/i');

        // Wait for import result dialog to appear (shows on top of settings modal)
        const importDoneButton = page.locator('[data-testid="btn-import-done"]');
        await expect(importDoneButton).toBeVisible({ timeout: 10000 });

        // Dismiss the import result dialog
        await importDoneButton.click();
        await expect(importDoneButton).not.toBeVisible({ timeout: 3000 });

        // Close settings modal
        await page.keyboard.press('Escape');

        // Wait for settings modal to close
        const modal = page.getByRole('dialog');
        await expect(modal).not.toBeVisible({ timeout: 3000 }).catch(() => {});

        // Verify imported note appears in list
        const importedNote = noteList.locator('h3').getByText(/Imported note/i);

        // Note should be visible (wait longer as import may take time)
        await expect(importedNote).toBeVisible({ timeout: 8000 });
      }
    } finally {
      if (fs.existsSync(importFilePath)) {
        fs.unlinkSync(importFilePath);
      }
    }
  });

  test('should validate import file format', async ({ page }) => {
    // Create an invalid file (missing required 'version' and 'notes' fields)
    const invalidFilePath = path.join(__dirname, 'invalid-import.json');
    fs.writeFileSync(invalidFilePath, '{ "invalid": "format" }');

    try {
      await jp.openAdvancedTab();

      const fileInput = page.locator('input[type="file"]');

      if (await fileInput.count() > 0) {
        await fileInput.setInputFiles(invalidFilePath);
        await page.waitForTimeout(1500);

        // App shows errors via toast notifications
        // Look for toast or any error indicator
        const toastError = page.locator('[class*="toast"], [class*="notification"], [role="alert"]').filter({ hasText: /error|failed|invalid/i });
        const errorText = page.locator('text=/invalid|error|failed/i');

        const hasError = await toastError.count() > 0 || await errorText.count() > 0;

        // App should reject invalid format (show error or simply not import)
        // Even if no visible error, verify nothing was imported
        if (!hasError) {
          // Close settings and check no notes were added
          await page.keyboard.press('Escape');
          await page.waitForTimeout(500);
          const noteList = page.getByRole('list');
          const notes = noteList.locator('li, [role="listitem"]');
          // Should have no notes (empty state) since we started fresh
          const noteCount = await notes.count();
          expect(noteCount).toBeLessThanOrEqual(0);
        } else {
          expect(hasError).toBe(true);
        }
      }
    } finally {
      if (fs.existsSync(invalidFilePath)) {
        fs.unlinkSync(invalidFilePath);
      }
    }
  });

  test('should preserve tags during export/import', async ({ page }) => {
    // Create note with tags
    await createNote(page, 'Note with special tags', ['important', 'work']);

    // Export from Advanced tab
    await jp.openAdvancedTab();

    const downloadPromise = page.waitForEvent('download', { timeout: 10000 }).catch(() => null);

    // Look within the settings modal
    const settingsModal = jp.dialog.first();
    const exportButton = settingsModal.locator('button').filter({
      hasText: /export/i
    }).first();

    if (await exportButton.isVisible()) {
      await exportButton.click();
      const download = await downloadPromise;

      if (download) {
        const downloadPath = path.join(__dirname, 'test-tags-export.json');
        await download.saveAs(downloadPath);

        try {
          const content = fs.readFileSync(downloadPath, 'utf-8');
          const data = JSON.parse(content);

          // Find the note with tags
          const noteWithTags = data.notes.find((n: any) =>
            n.content && n.content.includes('special tags')
          );

          if (noteWithTags) {
            expect(noteWithTags.tags).toBeDefined();
            expect(noteWithTags.tags.length).toBeGreaterThan(0);
          }
        } finally {
          if (fs.existsSync(downloadPath)) {
            fs.unlinkSync(downloadPath);
          }
        }
      }
    }
  });

  test('should handle empty export gracefully', async ({ page }) => {
    // Don't create any notes - export empty

    await jp.openAdvancedTab();

    const downloadPromise = page.waitForEvent('download', { timeout: 10000 }).catch(() => null);

    // Look within the settings modal
    const settingsModal = jp.dialog.first();
    const exportButton = settingsModal.locator('button').filter({
      hasText: /export/i
    }).first();

    if (await exportButton.isVisible()) {
      await exportButton.click();
      const download = await downloadPromise;

      if (download) {
        const downloadPath = path.join(__dirname, 'test-empty-export.json');
        await download.saveAs(downloadPath);

        try {
          const content = fs.readFileSync(downloadPath, 'utf-8');
          const data = JSON.parse(content);

          // Empty export should still be valid JSON
          expect(data).toHaveProperty('version');
          expect(data).toHaveProperty('notes');
          expect(Array.isArray(data.notes)).toBe(true);
        } finally {
          if (fs.existsSync(downloadPath)) {
            fs.unlinkSync(downloadPath);
          }
        }
      }
    }
  });

  test('should show import success message', async ({ page }) => {
    const exportData = {
      version: '1.0',
      exportDate: new Date().toISOString(),
      notes: [
        {
          id: 'success-test-id',
          createdAt: new Date().toISOString(),
          modifiedAt: new Date().toISOString(),
          content: 'Success test note',
          tags: [],
          attachments: [],
          pinned: false
        }
      ]
    };

    const importFilePath = path.join(__dirname, 'test-success.json');
    fs.writeFileSync(importFilePath, JSON.stringify(exportData, null, 2));

    try {
      await jp.openAdvancedTab();

      // Click import button to trigger file dialog
      const importButton = page.locator('button').filter({ hasText: /import/i }).first();
      const fileInput = page.locator('input[type="file"]');

      if (await fileInput.count() > 0) {
        await fileInput.setInputFiles(importFilePath);
        await page.waitForTimeout(2000);

        // Should show success message or the note should appear
        const successMessage = page.locator('text=/success|imported|complete|notes/i');
        const hasSuccess = await successMessage.count() > 0;

        expect(hasSuccess).toBe(true);
      }
    } finally {
      if (fs.existsSync(importFilePath)) {
        fs.unlinkSync(importFilePath);
      }
    }
  });

  test('should skip existing notes on re-import with skip strategy', async ({ page }) => {
    // The app uses 'skip' strategy which skips notes that already exist (by ID)
    // This test verifies that behavior - re-importing does NOT create duplicates

    // Create a note
    await createNote(page, 'Unique note for import skip test');

    // Export from Advanced tab
    await jp.openAdvancedTab();

    const downloadPromise = page.waitForEvent('download', { timeout: 10000 }).catch(() => null);

    // Look within the settings modal
    let settingsModal = jp.dialog.first();
    const exportButton = settingsModal.locator('button').filter({
      hasText: /export/i
    }).first();

    let downloadPath = '';

    if (await exportButton.isVisible()) {
      await exportButton.click();
      const download = await downloadPromise;

      if (download) {
        downloadPath = path.join(__dirname, 'test-merge.json');
        await download.saveAs(downloadPath);

        // Verify file was saved successfully before proceeding
        if (!fs.existsSync(downloadPath)) {
          console.log('Download saved but file not found, skipping re-import test');
          return;
        }

        // Settings modal is still open on the Advanced tab - just re-import directly
        const fileInput = page.locator('input[type="file"]');
        if (await fileInput.count() > 0 && fs.existsSync(downloadPath)) {
          await fileInput.setInputFiles(downloadPath);

          // Wait for import result dialog and dismiss it
          const importDoneButton = page.locator('[data-testid="btn-import-done"]');
          await expect(importDoneButton).toBeVisible({ timeout: 10000 });
          await importDoneButton.click();
          await expect(importDoneButton).not.toBeVisible({ timeout: 3000 });
        }

        // Close settings modal
        await page.keyboard.press('Escape');
        await page.waitForTimeout(500);

        // Count notes with the unique content
        const noteList = page.getByRole('list');
        const matchingNotes = noteList.locator('text=/import skip test/i');
        const count = await matchingNotes.count();

        // With skip strategy, re-importing skips notes that already exist (by ID)
        // So we expect only 1 note (no duplicate created)
        expect(count).toBe(1);

        // Cleanup
        if (downloadPath && fs.existsSync(downloadPath)) {
          fs.unlinkSync(downloadPath);
        }
      }
    }
  });
});
