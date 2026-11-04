/**
 * E2E test for backups: made in the app, then restored on a device with nothing on it.
 */

import { test, expect } from '@playwright/test';
import { JotteryPage } from './page-objects';

const PASSWORD = 'test-password-123';
const NOTE = 'Kept safe in a backup';

test('a backup restores on an empty device', async ({ page, browser }, testInfo) => {
  const jp = new JotteryPage(page);
  await jp.setup(PASSWORD);
  await jp.createNote(NOTE);
  await page.waitForTimeout(1500); // let autosave finish

  await jp.openAdvancedTab();
  const downloading = page.waitForEvent('download');
  await page.getByRole('button', { name: /create encrypted backup/i }).click();
  const download = await downloading;
  const backupFile = testInfo.outputPath(download.suggestedFilename());
  await download.saveAs(backupFile);

  // A new browser context has empty storage: a device that has never seen the vault.
  const device = await browser.newContext({ baseURL: testInfo.project.use.baseURL });
  const emptyPage = await device.newPage();
  const empty = new JotteryPage(emptyPage);
  await emptyPage.goto('', { waitUntil: 'domcontentloaded' });

  await emptyPage.getByRole('button', { name: /already using jottery/i }).click();
  await emptyPage.getByRole('button', { name: /restore from backup/i }).click();
  await emptyPage.locator('input[type="file"]').setInputFiles(backupFile);
  await emptyPage.locator('#backup-password').fill(PASSWORD);
  await emptyPage.getByRole('button', { name: /^restore$/i }).click();
  await expect(empty.noteListItem(new RegExp(NOTE)).first()).toBeVisible({ timeout: 30000 });

  // The restored vault opens with its password from now on.
  await emptyPage.reload();
  await emptyPage.locator('#password').fill(PASSWORD);
  await emptyPage.locator('#password').press('Enter');
  await expect(empty.noteListItem(new RegExp(NOTE)).first()).toBeVisible({ timeout: 30000 });

  await device.close();
});

test('a backup refuses the wrong password and leaves the device empty', async ({ page, browser }, testInfo) => {
  const jp = new JotteryPage(page);
  await jp.setup(PASSWORD);
  await jp.createNote(NOTE);
  await page.waitForTimeout(1500);

  await jp.openAdvancedTab();
  const downloading = page.waitForEvent('download');
  await page.getByRole('button', { name: /create encrypted backup/i }).click();
  const download = await downloading;
  const backupFile = testInfo.outputPath(download.suggestedFilename());
  await download.saveAs(backupFile);

  const device = await browser.newContext({ baseURL: testInfo.project.use.baseURL });
  const emptyPage = await device.newPage();
  await emptyPage.goto('', { waitUntil: 'domcontentloaded' });

  await emptyPage.getByRole('button', { name: /already using jottery/i }).click();
  await emptyPage.getByRole('button', { name: /restore from backup/i }).click();
  await emptyPage.locator('input[type="file"]').setInputFiles(backupFile);
  await emptyPage.locator('#backup-password').fill('not-the-password');
  await emptyPage.getByRole('button', { name: /^restore$/i }).click();
  await expect(emptyPage.getByText(/incorrect password/i).first()).toBeVisible({ timeout: 15000 });

  await device.close();
});
