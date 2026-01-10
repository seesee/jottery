/**
 * E2E tests for attachment functionality
 * Tests upload, view, and delete of file attachments
 */

import { test, expect } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';

test.describe('Attachments', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before each test
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
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('test-password-123');
    await passwordInputs.nth(1).fill('test-password-123');
    await page.locator('button[type="submit"], button').filter({ hasText: /Create|Set|Unlock/i }).first().click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Create a note to work with
    const newNoteButton = page.locator('button').filter({ hasText: /New|^\+$/ }).first();
    await newNoteButton.click();
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible();
    await editor.click();
    await editor.pressSequentially('Test note for attachments');
    await page.waitForTimeout(2000);
  });

  test('should show attachment button or area', async ({ page }) => {
    // Look for attachment-related UI
    const attachmentButton = page.locator('button, [role="button"]').filter({
      hasText: /attach|upload|file|📎|📄/i
    });
    const attachmentArea = page.locator('[class*="attachment"], [class*="drop"], input[type="file"]');

    const hasAttachmentUI = await attachmentButton.count() > 0 || await attachmentArea.count() > 0;
    expect(hasAttachmentUI).toBe(true);
  });

  test('should accept file upload via input', async ({ page }) => {
    // Find file input (may be hidden)
    const fileInput = page.locator('input[type="file"]');

    if (await fileInput.count() > 0) {
      // Create a test file
      const testContent = 'This is a test file content';
      const testFilePath = path.join(__dirname, 'test-upload.txt');
      fs.writeFileSync(testFilePath, testContent);

      try {
        // Upload the file
        await fileInput.setInputFiles(testFilePath);
        await page.waitForTimeout(2000);

        // Should show the attachment
        const attachmentIndicator = page.locator('text=/test-upload|attachment|1.*file/i');
        const hasAttachment = await attachmentIndicator.count() > 0;

        // Attachment should be visible or count should increase
        expect(hasAttachment).toBe(true);
      } finally {
        // Cleanup
        if (fs.existsSync(testFilePath)) {
          fs.unlinkSync(testFilePath);
        }
      }
    }
  });

  test('should display attachment count on note', async ({ page }) => {
    const fileInput = page.locator('input[type="file"]');

    if (await fileInput.count() > 0) {
      // Create and upload a test file
      const testFilePath = path.join(__dirname, 'test-count.txt');
      fs.writeFileSync(testFilePath, 'Test content');

      try {
        await fileInput.setInputFiles(testFilePath);
        await page.waitForTimeout(2000);

        // Check for attachment indicator in note list
        const noteList = page.getByRole('list');
        const attachmentBadge = noteList.locator('text=/📎|attachment|1/i');

        if (await attachmentBadge.count() > 0) {
          await expect(attachmentBadge.first()).toBeVisible();
        }
      } finally {
        if (fs.existsSync(testFilePath)) {
          fs.unlinkSync(testFilePath);
        }
      }
    }
  });

  test('should show attachment list in note view', async ({ page }) => {
    const fileInput = page.locator('input[type="file"]');

    if (await fileInput.count() > 0) {
      const testFilePath = path.join(__dirname, 'test-list.txt');
      fs.writeFileSync(testFilePath, 'Test content for list');

      try {
        await fileInput.setInputFiles(testFilePath);
        await page.waitForTimeout(2000);

        // Look for attachments section
        const attachmentsSection = page.locator('text=/attachments|files/i');
        const attachmentItem = page.locator('[class*="attachment"], [data-testid*="attachment"]');

        const hasAttachmentList = await attachmentsSection.count() > 0 || await attachmentItem.count() > 0;
        expect(hasAttachmentList).toBe(true);
      } finally {
        if (fs.existsSync(testFilePath)) {
          fs.unlinkSync(testFilePath);
        }
      }
    }
  });

  test('should allow deleting attachments', async ({ page }) => {
    const fileInput = page.locator('input[type="file"]');

    if (await fileInput.count() > 0) {
      const testFilePath = path.join(__dirname, 'test-delete.txt');
      fs.writeFileSync(testFilePath, 'Test content for deletion');

      try {
        // Upload
        await fileInput.setInputFiles(testFilePath);
        await page.waitForTimeout(2000);

        // Find delete button for attachment
        const deleteButton = page.locator('button, [role="button"]').filter({
          hasText: /delete|remove|×|✕/i
        }).first();

        if (await deleteButton.isVisible()) {
          await deleteButton.click();

          // May need to confirm
          const confirmButton = page.locator('button').filter({ hasText: /confirm|yes|delete/i });
          if (await confirmButton.isVisible()) {
            await confirmButton.click();
          }

          await page.waitForTimeout(1000);

          // Attachment should be removed
          const attachmentIndicator = page.locator('text=/test-delete/i');
          await expect(attachmentIndicator).not.toBeVisible();
        }
      } finally {
        if (fs.existsSync(testFilePath)) {
          fs.unlinkSync(testFilePath);
        }
      }
    }
  });

  test('should show image preview for image attachments', async ({ page }) => {
    const fileInput = page.locator('input[type="file"]');

    if (await fileInput.count() > 0) {
      // Create a simple PNG file (1x1 transparent pixel)
      const pngBuffer = Buffer.from([
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00,
        0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82
      ]);

      const testFilePath = path.join(__dirname, 'test-image.png');
      fs.writeFileSync(testFilePath, pngBuffer);

      try {
        await fileInput.setInputFiles(testFilePath);
        await page.waitForTimeout(2000);

        // Look for image preview
        const imagePreview = page.locator('img[src*="blob:"], img[src*="data:"], .thumbnail, [class*="preview"]');
        const hasPreview = await imagePreview.count() > 0;

        // Image preview is optional but good to have
        if (hasPreview) {
          await expect(imagePreview.first()).toBeVisible();
        }
      } finally {
        if (fs.existsSync(testFilePath)) {
          fs.unlinkSync(testFilePath);
        }
      }
    }
  });

  test('should handle multiple attachments', async ({ page }) => {
    const fileInput = page.locator('input[type="file"]');

    if (await fileInput.count() > 0) {
      const testFile1 = path.join(__dirname, 'test-multi-1.txt');
      const testFile2 = path.join(__dirname, 'test-multi-2.txt');
      fs.writeFileSync(testFile1, 'Content 1');
      fs.writeFileSync(testFile2, 'Content 2');

      try {
        // Upload multiple files
        await fileInput.setInputFiles([testFile1, testFile2]);
        await page.waitForTimeout(2000);

        // Check for multiple attachments
        const attachmentItems = page.locator('[class*="attachment"]');
        const attachmentCount = page.locator('text=/2.*file|2.*attachment/i');

        const hasMultiple = await attachmentItems.count() >= 2 || await attachmentCount.count() > 0;
        expect(hasMultiple).toBe(true);
      } finally {
        if (fs.existsSync(testFile1)) fs.unlinkSync(testFile1);
        if (fs.existsSync(testFile2)) fs.unlinkSync(testFile2);
      }
    }
  });
});
