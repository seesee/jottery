/**
 * E2E tests for Remember Password and Persist Session: the two settings that
 * keep the password so the app can unlock itself.
 */

import { test, expect, type Page } from '@playwright/test';
import { JotteryPage } from './page-objects';

const PASSWORD = 'test-password-123';
const AUTOSAVE_DELAY_MS = 1000; // settings auto-save 500ms after a change
const AUTO_UNLOCK_MS = 2000; // long enough for an auto-unlock to finish, if one started

const options = [
  { name: 'Remember Password', heading: /remember password/i, input: '#remember-password-confirm', enable: /enable anyway/i },
  { name: 'Persist Session', heading: /persist session/i, input: '#persist-session-confirm', enable: /enable session persistence/i },
];

function optionSwitch(page: Page, heading: RegExp) {
  return page.locator('h4', { hasText: heading }).locator('xpath=ancestor::div[2]').locator('label');
}

function optionCheckbox(page: Page, heading: RegExp) {
  return optionSwitch(page, heading).locator('input[type="checkbox"]');
}

async function closeSettings(page: Page) {
  await page.keyboard.press('Escape');
  await expect(page.locator('#setting-autolock')).toHaveCount(0);
}

async function expectUnlockedByItself(page: Page, jp: JotteryPage) {
  await expect(jp.appLoaded.first()).toBeVisible({ timeout: 30000 });
  await expect(page.locator('#password')).toHaveCount(0);
}

/** The field also shows briefly during an auto-unlock, so check that it stays. */
async function expectPasswordPrompt(page: Page, jp: JotteryPage) {
  await expect(page.locator('#password')).toBeVisible({ timeout: 15000 });
  await page.waitForTimeout(AUTO_UNLOCK_MS);
  await expect(page.locator('#password')).toBeVisible();
  await expect(jp.appLoaded).toHaveCount(0);
}

async function unlock(page: Page, jp: JotteryPage) {
  await expect(page.locator('#password')).toBeVisible({ timeout: 15000 });
  await page.waitForTimeout(AUTO_UNLOCK_MS);
  await page.locator('#password').fill(PASSWORD);
  await page.locator('#password').press('Enter');
  await expect(jp.appLoaded.first()).toBeVisible({ timeout: 30000 });
}

test.describe('Password options', () => {
  let jp: JotteryPage;

  test.beforeEach(async ({ page }) => {
    await page.clock.install();
    jp = new JotteryPage(page);
    await jp.setup(PASSWORD);
    await jp.openSettings();
  });

  for (const option of options) {
    test.describe(option.name, () => {
      test('turns on with the right password and unlocks after a reload', async ({ page }) => {
        await optionSwitch(page, option.heading).click();
        await page.locator(option.input).fill(PASSWORD);
        await page.getByRole('button', { name: option.enable }).click();
        await expect(page.locator(option.input)).toHaveCount(0);
        await expect(optionCheckbox(page, option.heading)).toBeChecked();
        await closeSettings(page);

        await page.reload();
        await expectUnlockedByItself(page, jp);
      });

      test('refuses the wrong password', async ({ page }) => {
        await optionSwitch(page, option.heading).click();
        await page.locator(option.input).fill('not-the-password');
        await page.getByRole('button', { name: option.enable }).click();
        await expect(page.getByText('Incorrect password')).toBeVisible();

        await page.getByRole('button', { name: /cancel/i }).last().click();
        await expect(optionCheckbox(page, option.heading)).not.toBeChecked();
        await closeSettings(page);

        await page.reload();
        await expectPasswordPrompt(page, jp);
      });

      test('accepts the right password after a wrong one', async ({ page }) => {
        await optionSwitch(page, option.heading).click();
        await page.locator(option.input).fill('not-the-password');
        await page.getByRole('button', { name: option.enable }).click();
        await expect(page.getByText('Incorrect password')).toBeVisible();

        await page.locator(option.input).fill(PASSWORD);
        await page.getByRole('button', { name: option.enable }).click();
        await expect(page.locator(option.input)).toHaveCount(0);
        await closeSettings(page);

        await page.reload();
        await expectUnlockedByItself(page, jp);
      });

      test('does not lock the app while the password is being typed', async ({ page }) => {
        await optionSwitch(page, option.heading).click();
        await page.waitForTimeout(AUTOSAVE_DELAY_MS);
        await page.mouse.move(200, 200);
        await page.locator(option.input).pressSequentially(PASSWORD, { delay: 10 });

        await expect(page.locator(option.input)).toHaveValue(PASSWORD);
        await page.getByRole('button', { name: option.enable }).click();
        await expect(page.locator(option.input)).toHaveCount(0);
        await closeSettings(page);
        await expect(jp.appLoaded.first()).toBeVisible();
      });

      test('stays off when cancelled after a pause', async ({ page }) => {
        await optionSwitch(page, option.heading).click();
        await page.waitForTimeout(AUTOSAVE_DELAY_MS);
        await page.getByRole('button', { name: /cancel/i }).last().click();
        await page.waitForTimeout(AUTOSAVE_DELAY_MS);
        await closeSettings(page);

        await page.reload();
        await expectPasswordPrompt(page, jp);
      });

      test('stays off when the page is left before confirming', async ({ page }) => {
        await optionSwitch(page, option.heading).click();
        await page.waitForTimeout(AUTOSAVE_DELAY_MS);

        await page.reload();
        await unlock(page, jp);

        // An unlock must not quietly complete what was never confirmed.
        await page.reload();
        await expectPasswordPrompt(page, jp);
      });
    });
  }

  test('turning Persist Session off leaves the app unlocked', async ({ page }) => {
    const option = options[1];
    await optionSwitch(page, option.heading).click();
    await page.locator(option.input).fill(PASSWORD);
    await page.getByRole('button', { name: option.enable }).click();
    await expect(page.locator(option.input)).toHaveCount(0);
    await page.waitForTimeout(AUTOSAVE_DELAY_MS);

    await optionSwitch(page, option.heading).click();
    await page.mouse.move(300, 300);
    await page.mouse.move(310, 310);
    await page.waitForTimeout(AUTOSAVE_DELAY_MS);
    await expect(page.locator('#password')).toHaveCount(0);
    await closeSettings(page);
    await expect(jp.appLoaded.first()).toBeVisible();

    await page.reload();
    await expectPasswordPrompt(page, jp);
  });

  test('turning Remember Password off locks the app and forgets the password', async ({ page }) => {
    const option = options[0];
    await optionSwitch(page, option.heading).click();
    await page.locator(option.input).fill(PASSWORD);
    await page.getByRole('button', { name: option.enable }).click();
    await expect(page.locator(option.input)).toHaveCount(0);
    await page.waitForTimeout(AUTOSAVE_DELAY_MS);

    await optionSwitch(page, option.heading).click();
    await expectPasswordPrompt(page, jp);

    await page.reload();
    await expectPasswordPrompt(page, jp);
  });

  test('Persist Session ends when its time is up', async ({ page }) => {
    const option = options[1];
    await optionSwitch(page, option.heading).click();
    await page.locator(option.input).fill(PASSWORD);
    await page.getByRole('button', { name: option.enable }).click();
    await expect(page.locator(option.input)).toHaveCount(0);
    await closeSettings(page);

    await page.clock.runFor(31 * 60 * 1000);
    await expectPasswordPrompt(page, jp);

    await page.reload();
    await expectPasswordPrompt(page, jp);
  });
});
