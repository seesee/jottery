/**
 * E2E tests for attachment functionality
 * Tests upload, view, and delete of file attachments
 *
 * Uses mobile viewport (375x667) because:
 * - Desktop relies on drag-and-drop only for file uploads
 * - Mobile provides a visible attachment button (📎) in the toolbar
 * - The mobile modal makes testing easier and more reliable
 */

import { test, expect, Page } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';
import { setupFreshEnvironment } from './test-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Use mobile viewport to get the attachment button in toolbar
test.use({ viewport: { width: 375, height: 667 } });

/**
 * Helper to open the attachments modal via the toolbar button
 */
async function openAttachmentsModal(page: Page): Promise<void> {
  // Find and click the attachments button in toolbar (has 📎 emoji)
  const attachmentsButton = page.locator('button').filter({ hasText: '📎' });
  await expect(attachmentsButton).toBeVisible({ timeout: 10000 });
  await attachmentsButton.click();

  // Wait for the modal to open (modal has role="dialog")
  const modal = page.locator('[role="dialog"]');
  await expect(modal).toBeVisible({ timeout: 5000 });
}

/**
 * Helper to close the attachments modal
 */
async function closeAttachmentsModal(page: Page): Promise<void> {
  // Click the close button (X) in the modal header
  const closeButton = page.locator('[role="dialog"]').locator('button').filter({
    has: page.locator('svg')
  }).first();
  await closeButton.click();
  await expect(page.locator('[role="dialog"]')).not.toBeVisible({ timeout: 5000 });
}

/**
 * Helper to upload a file and wait for it to appear
 */
async function uploadFileAndWait(page: Page, filePath: string, expectedName: string): Promise<void> {
  // The file input is inside the FileUpload component in the modal
  const fileInput = page.locator('[role="dialog"] input[type="file"]');
  await fileInput.setInputFiles(filePath);

  // Wait for the attachment to appear in the list
  const attachmentItem = page.locator('[role="dialog"]').locator('.attachment-item, [class*="attachment"]').filter({
    hasText: new RegExp(expectedName, 'i')
  });
  await expect(attachmentItem).toBeVisible({ timeout: 10000 });
}

/**
 * Helper to create a test file
 */
function createTestFile(name: string, content: string = 'Test content'): string {
  const filePath = path.join(__dirname, name);
  fs.writeFileSync(filePath, content);
  return filePath;
}

function cleanupTestFile(filePath: string): void {
  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
  }
}

test.describe('Attachments', () => {
  test.beforeEach(async ({ page }) => {
    await setupFreshEnvironment(page);

    // On mobile, find and click the "New note" button by aria-label
    const newNoteButton = page.locator('button[aria-label="New note"]');
    await expect(newNoteButton).toBeVisible({ timeout: 10000 });
    await newNoteButton.click();

    // Wait for editor to be visible (mobile switches to editor view)
    const editor = page.locator('.cm-content, [contenteditable="true"], textarea').first();
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially('Test note for attachments', { delay: 20 });

    // Wait for auto-save to complete: the debounce is 1000ms after last keystroke,
    // then the save itself runs. Rather than guessing a timeout, wait for the
    // editor toolbar's attachment button — it confirms the note is active and
    // the editor is fully rendered.
    const toolbarReady = page.locator('button').filter({ hasText: '📎' });
    await expect(toolbarReady).toBeVisible({ timeout: 10000 });
  });

  test('should show attachment button in toolbar', async ({ page }) => {
    // The attachments button (📎) should be visible in the toolbar
    const attachmentsButton = page.locator('button').filter({ hasText: '📎' });
    await expect(attachmentsButton).toBeVisible({ timeout: 10000 });
  });

  test('should open attachments modal when clicking button', async ({ page }) => {
    await openAttachmentsModal(page);

    // Modal should show "Attachments" header with count
    const modalHeader = page.locator('[role="dialog"]').locator('h2');
    await expect(modalHeader).toBeVisible();
    await expect(modalHeader).toContainText(/attachments/i);

    // Should show "No attachments" message or file upload area
    const modal = page.locator('[role="dialog"]');
    const noAttachmentsOrUpload = modal.locator('text=/no attachments/i').or(
      modal.locator('[role="button"]').filter({ hasText: /drag|drop|click/i })
    );
    await expect(noAttachmentsOrUpload.first()).toBeVisible();
  });

  test('should accept file upload via input', async ({ page }) => {
    await openAttachmentsModal(page);

    const testFilePath = createTestFile('test-upload.txt', 'This is test content');

    try {
      await uploadFileAndWait(page, testFilePath, 'test-upload');

      // Modal header should now show count of 1
      const modalHeader = page.locator('[role="dialog"]').locator('h2');
      await expect(modalHeader).toContainText('1', { timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should display attachment count on toolbar button', async ({ page }) => {
    await openAttachmentsModal(page);

    const testFilePath = createTestFile('test-count.txt');

    try {
      await uploadFileAndWait(page, testFilePath, 'test-count');

      // Close modal
      await closeAttachmentsModal(page);

      // The toolbar button should show a badge with count
      const attachmentsButton = page.locator('button').filter({ hasText: '📎' });
      await expect(attachmentsButton).toContainText('1', { timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should show attachment list in modal', async ({ page }) => {
    await openAttachmentsModal(page);

    const testFilePath = createTestFile('test-list.txt', 'Test content for list');

    try {
      await uploadFileAndWait(page, testFilePath, 'test-list');

      // The attachment should be visible with filename
      const attachmentItem = page.locator('[role="dialog"]').locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-list/i
      });
      await expect(attachmentItem).toBeVisible();

      // Should show file size (B, KB, MB, etc.)
      await expect(attachmentItem).toContainText(/\d+\s*[BKMG]B?/i);
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should allow deleting attachments', async ({ page }) => {
    await openAttachmentsModal(page);

    const testFilePath = createTestFile('test-delete.txt', 'Test content for deletion');

    try {
      await uploadFileAndWait(page, testFilePath, 'test-delete');

      // Find the attachment item
      const attachmentItem = page.locator('[role="dialog"]').locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-delete/i
      });
      await expect(attachmentItem).toBeVisible();

      // Find and click the delete button (has 🗑️ emoji)
      const deleteButton = attachmentItem.locator('button').filter({ hasText: '🗑️' });
      await expect(deleteButton).toBeVisible();
      await deleteButton.click();

      // Wait for the attachment to be removed
      await expect(attachmentItem).not.toBeVisible({ timeout: 5000 });

      // Modal header should show 0
      const modalHeader = page.locator('[role="dialog"]').locator('h2');
      await expect(modalHeader).toContainText('0', { timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should show thumbnail for image attachments', async ({ page }) => {
    await openAttachmentsModal(page);

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
      await uploadFileAndWait(page, testFilePath, 'test-image');

      // Find the attachment item
      const attachmentItem = page.locator('[role="dialog"]').locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-image/i
      });

      // Image attachments should have a thumbnail area
      const thumbnail = attachmentItem.locator('.attachment-thumbnail, button, img').first();
      await expect(thumbnail).toBeVisible({ timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should handle multiple attachments', async ({ page }) => {
    await openAttachmentsModal(page);

    const testFile1 = createTestFile('test-multi-1.txt', 'Content 1');
    const testFile2 = createTestFile('test-multi-2.txt', 'Content 2');

    try {
      // Upload first file
      await uploadFileAndWait(page, testFile1, 'test-multi-1');

      // Upload second file
      await uploadFileAndWait(page, testFile2, 'test-multi-2');

      // Verify both attachments are visible
      const modal = page.locator('[role="dialog"]');
      const attachment1 = modal.locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-multi-1/i
      });
      const attachment2 = modal.locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-multi-2/i
      });

      await expect(attachment1).toBeVisible();
      await expect(attachment2).toBeVisible();

      // Modal header should show count of 2
      const modalHeader = modal.locator('h2');
      await expect(modalHeader).toContainText('2', { timeout: 5000 });
    } finally {
      cleanupTestFile(testFile1);
      cleanupTestFile(testFile2);
    }
  });

  test('should persist attachments after closing modal', async ({ page }) => {
    await openAttachmentsModal(page);

    const testFilePath = createTestFile('test-persist.txt', 'Persistent content');

    try {
      await uploadFileAndWait(page, testFilePath, 'test-persist');

      // Close modal
      await closeAttachmentsModal(page);

      // Reopen the modal
      await openAttachmentsModal(page);

      // Attachment should still be there
      const attachmentItem = page.locator('[role="dialog"]').locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-persist/i
      });
      await expect(attachmentItem).toBeVisible({ timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });
});
