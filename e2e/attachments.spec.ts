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
import { JotteryPage } from './page-objects';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Use mobile viewport to get the attachment button in toolbar
test.use({ viewport: { width: 375, height: 667 } });

/**
 * Helper to open the attachments modal via the toolbar button
 */
async function openAttachmentsModal(page: Page, jp: JotteryPage): Promise<void> {
  // Find and click the attachments button in toolbar (has 📎 emoji)
  const attachmentsButton = jp.attachmentsButton;
  await expect(attachmentsButton).toBeVisible({ timeout: 10000 });
  await attachmentsButton.click();

  // Wait for the modal to open (modal has role="dialog")
  const modal = jp.dialog;
  await expect(modal).toBeVisible({ timeout: 5000 });

  // Wait for modal content to be fully rendered (header should be visible)
  const modalHeader = modal.locator('h2');
  await expect(modalHeader).toBeVisible({ timeout: 5000 });
}

/**
 * Helper to close the attachments modal
 */
async function closeAttachmentsModal(page: Page, jp: JotteryPage): Promise<void> {
  // Click the close button (X) in the modal header
  const closeButton = jp.dialog.locator('button').filter({
    has: page.locator('svg')
  }).first();
  await closeButton.click();
  await expect(jp.dialog).not.toBeVisible({ timeout: 5000 });

  // Wait a moment for any state updates to propagate to the toolbar
  // (attachment count badge update happens asynchronously)
  await page.waitForTimeout(500);
}

/**
 * Helper to upload a file and wait for it to appear
 */
async function uploadFileAndWait(page: Page, jp: JotteryPage, filePath: string, expectedName: string): Promise<void> {
  // The file input is inside the FileUpload component in the modal
  const fileInput = jp.dialog.locator('input[type="file"]');
  await expect(fileInput).toBeAttached({ timeout: 5000 });
  await fileInput.setInputFiles(filePath);

  // Wait for the attachment to appear in the list
  const attachmentItem = jp.dialog.locator('.attachment-item, [class*="attachment"]').filter({
    hasText: new RegExp(expectedName, 'i')
  });
  await expect(attachmentItem).toBeVisible({ timeout: 15000 });

  // Wait for upload to complete - the item should have a size displayed
  await expect(attachmentItem).toContainText(/\d+\s*[BKMG]?B/i, { timeout: 10000 });
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
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    jp = new JotteryPage(page);
    await jp.setup();

    // On mobile, find and click the "New note" button by aria-label
    const newNoteButton = page.locator('button[aria-label="New note"]');
    await expect(newNoteButton).toBeVisible({ timeout: 10000 });
    await newNoteButton.click();

    // Wait for editor to be visible (mobile switches to editor view)
    const editor = jp.editorContent;
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially('Test note for attachments', { delay: 20 });

    // Wait for the attachment button to be visible (editor toolbar is rendered)
    const toolbarReady = jp.attachmentsButton;
    await expect(toolbarReady).toBeVisible({ timeout: 10000 });

    // CRITICAL: Wait for auto-save to complete before running attachment tests.
    // Auto-save has a 1000ms debounce + save time. The note must be saved before
    // we can reliably attach files to it. Wait for the "Modified" timestamp
    // indicator which confirms the note has been persisted.
    const savedIndicator = page.locator('text=/Modified|Saved|ago/i');
    await expect(savedIndicator).toBeVisible({ timeout: 5000 });
  });

  test('should show attachment button in toolbar', async ({ page }) => {
    // The attachments button (📎) should be visible in the toolbar
    const attachmentsButton = jp.attachmentsButton;
    await expect(attachmentsButton).toBeVisible({ timeout: 10000 });
  });

  test('should open attachments modal when clicking button', async ({ page }) => {
    await openAttachmentsModal(page, jp);

    // Modal should show "Attachments" header with count
    const modalHeader = jp.dialog.locator('h2');
    await expect(modalHeader).toBeVisible();
    await expect(modalHeader).toContainText(/attachments/i);

    // Should show "No attachments" message or file upload area
    const modal = jp.dialog;
    const noAttachmentsOrUpload = modal.locator('text=/no attachments/i').or(
      modal.locator('[role="button"]').filter({ hasText: /drag|drop|click/i })
    );
    await expect(noAttachmentsOrUpload.first()).toBeVisible();
  });

  test('should accept file upload via input', async ({ page }) => {
    await openAttachmentsModal(page, jp);

    const testFilePath = createTestFile('test-upload.txt', 'This is test content');

    try {
      await uploadFileAndWait(page, jp, testFilePath, 'test-upload');

      // Modal header should now show count of 1
      const modalHeader = jp.dialog.locator('h2');
      await expect(modalHeader).toContainText('1', { timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should display attachment count on toolbar button', async ({ page }) => {
    await openAttachmentsModal(page, jp);

    const testFilePath = createTestFile('test-count.txt');

    try {
      await uploadFileAndWait(page, jp, testFilePath, 'test-count');

      // Close modal
      await closeAttachmentsModal(page, jp);

      // The toolbar button should show a badge with count
      // Use longer timeout as the count updates asynchronously after modal close
      const attachmentsButton = jp.attachmentsButton;
      await expect(attachmentsButton).toContainText('1', { timeout: 10000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should show attachment list in modal', async ({ page }) => {
    await openAttachmentsModal(page, jp);

    const testFilePath = createTestFile('test-list.txt', 'Test content for list');

    try {
      await uploadFileAndWait(page, jp, testFilePath, 'test-list');

      // The attachment should be visible with filename
      const attachmentItem = jp.dialog.locator('.attachment-item, [class*="attachment"]').filter({
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
    await openAttachmentsModal(page, jp);

    const testFilePath = createTestFile('test-delete.txt', 'Test content for deletion');

    try {
      await uploadFileAndWait(page, jp, testFilePath, 'test-delete');

      // Find the attachment item
      const attachmentItem = jp.dialog.locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-delete/i
      });
      await expect(attachmentItem).toBeVisible();

      // Find and click the delete button (has 🗑️ emoji)
      const deleteButton = attachmentItem.locator('button').filter({ hasText: '🗑️' });
      await expect(deleteButton).toBeVisible();

      // Cancelling keeps it
      await deleteButton.click();
      await page.getByRole('dialog').getByRole('button', { name: /^Cancel$/ }).click();
      await page.waitForTimeout(1500); // Longer than the autosave delay
      await expect(attachmentItem).toBeVisible();

      // Confirming removes it
      await deleteButton.click();
      await page.getByRole('dialog').getByRole('button', { name: /^Remove$/ }).click();

      // Wait for the attachment to be removed
      await expect(attachmentItem).not.toBeVisible({ timeout: 5000 });

      // Modal header should show 0
      const modalHeader = jp.dialog.locator('h2');
      await expect(modalHeader).toContainText('0', { timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should show thumbnail for image attachments', async ({ page }) => {
    await openAttachmentsModal(page, jp);

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
      await uploadFileAndWait(page, jp, testFilePath, 'test-image');

      // Find the attachment item
      const attachmentItem = jp.dialog.locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-image/i
      });

      // Image attachments should have a thumbnail area
      const thumbnail = attachmentItem.locator('.attachment-thumbnail, button, img').first();
      await expect(thumbnail).toBeVisible({ timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });

  test('should play an audio attachment under the shipped policy', async ({ page }) => {
    // default-src alone doesn't match blob:, which left an inert player and no error
    const samples = Buffer.alloc(1600);
    const header = Buffer.alloc(44);
    header.write('RIFF', 0);
    header.writeUInt32LE(36 + samples.length, 4);
    header.write('WAVEfmt ', 8);
    header.writeUInt32LE(16, 16);
    header.writeUInt16LE(1, 20); // PCM
    header.writeUInt16LE(1, 22); // mono
    header.writeUInt32LE(8000, 24);
    header.writeUInt32LE(8000, 28);
    header.writeUInt16LE(1, 32);
    header.writeUInt16LE(8, 34);
    header.write('data', 36);
    header.writeUInt32LE(samples.length, 40);
    const filePath = path.join(__dirname, 'test-audio.wav');
    fs.writeFileSync(filePath, Buffer.concat([header, samples]));

    await page.evaluate(() => {
      const violations: string[] = [];
      (window as unknown as { __violations: string[] }).__violations = violations;
      document.addEventListener('securitypolicyviolation', event => violations.push(event.effectiveDirective));
    });

    try {
      await openAttachmentsModal(page, jp);
      await uploadFileAndWait(page, jp, filePath, 'test-audio.wav');
      await jp.dialog.locator('.attachment-item').first().locator('button').nth(1).click();

      const audio = page.locator('audio');
      await expect(audio).toBeAttached({ timeout: 10000 });
      await expect.poll(() => audio.evaluate((el: HTMLAudioElement) => el.readyState), { timeout: 10000 }).toBeGreaterThan(0);

      const violations = await page.evaluate(() => (window as unknown as { __violations: string[] }).__violations);
      expect(violations.filter(directive => directive === 'media-src')).toEqual([]);
    } finally {
      cleanupTestFile(filePath);
    }
  });

  test('should not let a locked note lose or gain an attachment', async ({ page }) => {
    const filePath = createTestFile('locked-note.txt');
    try {
      await openAttachmentsModal(page, jp);
      await uploadFileAndWait(page, jp, filePath, 'locked-note.txt');
      await closeAttachmentsModal(page, jp);

      await page.evaluate(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, shiftKey: true, bubbles: true }));
      });
      const palette = page.getByRole('dialog').locator('input').first();
      await palette.fill('Lock/Unlock');
      await palette.press('Enter');
      await expect(page.getByText(/this note is locked/i)).toBeVisible({ timeout: 5000 });

      await openAttachmentsModal(page, jp);
      await expect(jp.dialog.locator('.attachment-item')).toHaveCount(1);
      await expect(jp.dialog.locator('input[type="file"]')).toHaveCount(0);
      await expect(jp.dialog.getByRole('button', { name: /delete/i })).toHaveCount(0);
    } finally {
      cleanupTestFile(filePath);
    }
  });

  test('should handle multiple attachments', async ({ page }) => {
    await openAttachmentsModal(page, jp);

    const testFile1 = createTestFile('test-multi-1.txt', 'Content 1');
    const testFile2 = createTestFile('test-multi-2.txt', 'Content 2');

    try {
      // Upload first file
      await uploadFileAndWait(page, jp, testFile1, 'test-multi-1');

      // Upload second file
      await uploadFileAndWait(page, jp, testFile2, 'test-multi-2');

      // Verify both attachments are visible
      const modal = jp.dialog;
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
    await openAttachmentsModal(page, jp);

    const testFilePath = createTestFile('test-persist.txt', 'Persistent content');

    try {
      await uploadFileAndWait(page, jp, testFilePath, 'test-persist');

      // Close modal
      await closeAttachmentsModal(page, jp);

      // Reopen the modal
      await openAttachmentsModal(page, jp);

      // Attachment should still be there
      const attachmentItem = jp.dialog.locator('.attachment-item, [class*="attachment"]').filter({
        hasText: /test-persist/i
      });
      await expect(attachmentItem).toBeVisible({ timeout: 5000 });
    } finally {
      cleanupTestFile(testFilePath);
    }
  });
  test.describe('an SVG, which can hold script', () => {
    const SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><script>window.top.ran = true</script><rect width="40" height="30" fill="red"/></svg>';

    /** What a browser would serve if this image were opened in a tab of its own */
    const servedType = (page: Page, src: string) => page.evaluate(async url => (await (await fetch(url)).blob()).type, src);

    test('is shown as a picture, so opening it in a tab runs nothing', async ({ page }) => {
      await openAttachmentsModal(page, jp);
      const filePath = createTestFile('test-drawing.svg', SVG);

      try {
        await uploadFileAndWait(page, jp, filePath, 'test-drawing.svg');
        await jp.dialog.locator('.attachment-item').first().locator('button').nth(1).click();

        const image = page.locator('img[alt="test-drawing.svg"]');
        await expect(image).toBeVisible();
        const src = await image.getAttribute('src');

        expect(await servedType(page, src as string)).toBe('image/png');
        await expect.poll(() => image.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBe(40);
      } finally {
        cleanupTestFile(filePath);
      }
    });

    test('is drawn the same way wherever a note shows it', async ({ page }) => {
      const types = await page.evaluate(async svg => {
        const { imageObjectUrl } = await import('/src/lib/utils/safeObjectUrl.ts' as string);
        const typeOf = async (blob: Blob) => {
          const url = await imageObjectUrl(blob);
          return (await (await fetch(url)).blob()).type;
        };
        return {
          svg: await typeOf(new Blob([svg], { type: 'image/svg+xml' })),
          withParameter: await typeOf(new Blob([svg], { type: 'image/svg+xml; charset=utf-8' })),
          png: await typeOf(new Blob([new Uint8Array(8)], { type: 'image/png' })),
        };
      }, SVG);

      expect(types).toEqual({ svg: 'image/png', withParameter: 'image/png', png: 'image/png' });
    });

    test('refuses anything else that would run as a page, whatever an attachment says it is', async ({ page }) => {
      const refused = await page.evaluate(async () => {
        const { imageObjectUrl } = await import('/src/lib/utils/safeObjectUrl.ts' as string);
        try {
          await imageObjectUrl(new Blob(['<script>window.top.ran = true</script>'], { type: 'text/html' }));
          return false;
        } catch {
          return true;
        }
      });

      expect(refused).toBe(true);
    });
  });
  test.describe('decrypted attachments in memory', () => {
    /** Keeps count of blob URLs still alive, from before the app starts */
    async function trackObjectUrls(page: Page) {
      await page.evaluate(() => {
        const live = new Set<string>();
        (window as unknown as { liveUrls: Set<string> }).liveUrls = live;
        const create = URL.createObjectURL.bind(URL);
        const revoke = URL.revokeObjectURL.bind(URL);
        URL.createObjectURL = (blob: Blob) => { const url = create(blob); live.add(url); return url; };
        URL.revokeObjectURL = (url: string) => { live.delete(url); revoke(url); };
      });
    }

    const liveUrls = (page: Page) => page.evaluate(() => (window as unknown as { liveUrls: Set<string> }).liveUrls.size);

    test('are released when another preview opens before the first has finished loading', async ({ page }) => {
      const first = createTestFile('test-slow-1.png', 'not really a png');
      const second = createTestFile('test-slow-2.png', 'not one either');
      try {
        await openAttachmentsModal(page, jp);
        await uploadFileAndWait(page, jp, first, 'test-slow-1');
        await uploadFileAndWait(page, jp, second, 'test-slow-2');
        await trackObjectUrls(page);
        await page.evaluate(async () => {
          const { attachmentService } = await import('/src/lib/services/attachmentService.ts' as string);
          const state = window as unknown as { finished: number };
          state.finished = 0;
          const read = attachmentService.getAttachmentData.bind(attachmentService);
          attachmentService.getAttachmentData = async (attachment: unknown) => {
            await new Promise(resolve => setTimeout(resolve, 500));
            const data = await read(attachment);
            setTimeout(() => { state.finished++; }, 100);
            return data;
          };
        });
        const before = await liveUrls(page);
        const closePreview = page.getByRole('button', { name: /close/i }).last();

        await jp.dialog.locator('.attachment-item').nth(0).locator('button').nth(1).click();
        await closePreview.click();
        await jp.dialog.locator('.attachment-item').nth(1).locator('button').nth(1).click();
        await page.waitForFunction(() => (window as unknown as { finished: number }).finished === 2);
        await closePreview.click();

        expect(await liveUrls(page)).toBe(before);
      } finally {
        cleanupTestFile(first);
        cleanupTestFile(second);
      }
    });

    test('are all released when a list of thumbnails goes away', async ({ page }) => {
      const pixel = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==', 'base64');
      const filePath = path.join(__dirname, 'test-thumbnail.png');
      fs.writeFileSync(filePath, pixel);
      await trackObjectUrls(page);
      try {
        await openAttachmentsModal(page, jp);
        await uploadFileAndWait(page, jp, filePath, 'test-thumbnail');
        await closeAttachmentsModal(page, jp);
        await expect.poll(() => liveUrls(page)).toBe(0);
      } finally {
        cleanupTestFile(filePath);
      }
    });
  });
  test('a PDF that cannot be read says so, not "1 of 0"', async ({ page }) => {
    const filePath = createTestFile('test-broken.pdf', 'this is not a pdf');
    try {
      await openAttachmentsModal(page, jp);
      await uploadFileAndWait(page, jp, filePath, 'test-broken');
      await jp.dialog.locator('.attachment-item').first().locator('button').nth(1).click();

      await expect(page.getByText(/couldn't be opened/)).toBeVisible({ timeout: 15000 });
      await expect(page.getByText(/1 (of|\/) 0/)).toHaveCount(0);
    } finally {
      cleanupTestFile(filePath);
    }
  });
});
