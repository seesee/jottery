/**
 * Page Object Model for Jottery e2e tests
 *
 * Centralises all selectors so that UI changes only need updating in one place.
 * Prefers data-testid attributes where available, falling back to resilient
 * role-based or structural selectors.
 */

import { type Page, type Locator, expect } from '@playwright/test';

export class JotteryPage {
  readonly page: Page;

  constructor(page: Page) {
    this.page = page;
  }

  // ── Setup & Navigation ─────────────────────────────────────────────

  /** Navigate to the app and complete password setup */
  async setup(password = 'test-password-123') {
    await this.page.goto('', { waitUntil: 'domcontentloaded' });
    await this.page.waitForSelector('#app', { state: 'attached', timeout: 30000 });

    const passwordField = this.page.locator('#password');
    const confirmField = this.page.locator('#confirm');

    await expect(confirmField).toBeVisible({ timeout: 30000 });
    await expect(passwordField).toBeVisible({ timeout: 5000 });
    await expect(passwordField).toBeEnabled({ timeout: 5000 });
    await expect(confirmField).toBeEnabled({ timeout: 5000 });

    await passwordField.click();
    await passwordField.press('Control+a');
    await passwordField.pressSequentially(password, { delay: 10 });

    await confirmField.click();
    await confirmField.press('Control+a');
    await confirmField.pressSequentially(password, { delay: 10 });

    await expect(passwordField).toHaveValue(password, { timeout: 5000 });
    await expect(confirmField).toHaveValue(password, { timeout: 5000 });

    const submitButton = this.page.locator('button[type="submit"]').or(
      this.page.locator('button').filter({ hasText: /Create Password|Set Password|Unlock/i })
    ).first();

    await expect(submitButton).toBeEnabled({ timeout: 5000 });
    await submitButton.click();

    await expect(this.appLoaded.first()).toBeVisible({ timeout: 30000 });
  }

  // ── Element Locators ───────────────────────────────────────────────

  /** Locator that resolves once the main app is loaded */
  get appLoaded(): Locator {
    return this.newNoteButton
      .or(this.noteList)
      .or(this.page.getByText(/No notes yet|Create your first note/i))
      .or(this.searchInput);
  }

  // -- Header / Global --

  get newNoteButton(): Locator {
    return this.page.locator('[data-testid="btn-new-note"]')
      .or(this.page.locator('[data-testid="btn-new-note-mobile"]'))
      .first();
  }

  get newNoteButtonDesktop(): Locator {
    return this.page.locator('[data-testid="btn-new-note"]');
  }

  get newNoteButtonMobile(): Locator {
    return this.page.locator('[data-testid="btn-new-note-mobile"]');
  }

  get searchInput(): Locator {
    return this.page.locator('[data-testid="search-input"]')
      .or(this.page.locator('[data-testid="search-input-mobile"]'))
      .first();
  }

  get searchInputDesktop(): Locator {
    return this.page.locator('[data-testid="search-input"]');
  }

  get searchInputMobile(): Locator {
    return this.page.locator('[data-testid="search-input-mobile"]');
  }

  get settingsButton(): Locator {
    return this.page.locator('[data-testid="btn-settings"]');
  }

  get recycleBinButton(): Locator {
    return this.page.locator('[data-testid="btn-recycle-bin"]');
  }

  get archiveButton(): Locator {
    return this.page.locator('[data-testid="btn-archive"]');
  }

  get lockButton(): Locator {
    return this.page.locator('[data-testid="btn-lock"]');
  }

  get hamburgerMenu(): Locator {
    return this.page.locator('[data-testid="btn-hamburger-menu"]');
  }

  // -- Note List --

  get noteList(): Locator {
    return this.page.locator('[data-testid="note-list"]')
      .or(this.page.getByRole('list'))
      .first();
  }

  get noteListItems(): Locator {
    return this.page.locator('[data-testid="note-list-item"]')
      .or(this.page.locator('.note-list-item'));
  }

  noteListItem(textPattern: RegExp): Locator {
    return this.noteListItems.filter({ hasText: textPattern });
  }

  noteListItemTitle(textPattern: RegExp): Locator {
    return this.noteList.locator('[data-testid="note-list-item"] h3').getByText(textPattern).first();
  }

  // -- Editor --

  get editor(): Locator {
    return this.page.locator('[data-testid="editor-content"]')
      .or(this.page.locator('.cm-content, [contenteditable="true"], textarea'))
      .first();
  }

  get editorContent(): Locator {
    return this.page.locator('.cm-content, [contenteditable="true"], textarea').first();
  }

  get previewButton(): Locator {
    return this.page.locator('[data-testid="btn-preview"]');
  }

  get versionHistoryButton(): Locator {
    return this.page.locator('[data-testid="btn-version-history"]');
  }

  get moreActionsButton(): Locator {
    return this.page.locator('[data-testid="btn-more-actions"]');
  }

  get languageSelect(): Locator {
    return this.page.locator('select').filter({ has: this.page.locator('option[value="calc"]') });
  }

  // -- Tags --

  get tagInput(): Locator {
    return this.page.locator('[data-testid="tag-input"]')
      .or(this.page.locator('.tag-input-container input[type="text"]'))
      .first();
  }

  get tagPills(): Locator {
    return this.page.locator('[data-testid="tag-pill"]')
      .or(this.page.locator('.tag-pill'));
  }

  tagPill(tagName: string): Locator {
    return this.tagPills.filter({ hasText: `#${tagName}` });
  }

  // -- Bulk Operations --

  get bulkToolbar(): Locator {
    return this.page.locator('[data-testid="bulk-toolbar"]');
  }

  get bulkAddTagsButton(): Locator {
    return this.page.locator('[data-testid="bulk-add-tags"]');
  }

  get bulkRemoveTagsButton(): Locator {
    return this.page.locator('[data-testid="bulk-remove-tags"]');
  }

  get bulkSetColorButton(): Locator {
    return this.page.locator('[data-testid="bulk-set-color"]');
  }

  get bulkExportButton(): Locator {
    return this.page.locator('[data-testid="bulk-export"]');
  }

  get bulkCombineButton(): Locator {
    return this.page.locator('[data-testid="bulk-combine"]');
  }

  get bulkArchiveButton(): Locator {
    return this.page.locator('[data-testid="bulk-archive"]');
  }

  get bulkDeleteButton(): Locator {
    return this.page.locator('[data-testid="bulk-delete"]');
  }

  get bulkCancelButton(): Locator {
    return this.page.locator('[data-testid="bulk-cancel"]');
  }

  // -- Modals / Dialogs --

  get dialog(): Locator {
    return this.page.locator('[role="dialog"]');
  }

  get settingsModal(): Locator {
    return this.page.locator('[role="dialog"]').first();
  }

  get tabs(): Locator {
    return this.page.locator('[role="tab"]');
  }

  get tabPanel(): Locator {
    return this.page.locator('[role="tabpanel"]');
  }

  // -- Attachments --

  get attachmentsButton(): Locator {
    return this.page.locator('button').filter({ hasText: '📎' });
  }

  // ── Common Actions ─────────────────────────────────────────────────

  /** Create a note with content and optional tags, waits for auto-save */
  async createNote(content: string, tags: string[] = []) {
    await this.newNoteButton.click();

    const editor = this.editorContent;
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();

    // Brief pause for the new-note action to settle (note creation + editor switch).
    // Then select-all + delete to guarantee a clean slate regardless of whether
    // the app created a fresh note or reused an existing blank one.
    await this.page.waitForTimeout(300);
    await this.page.keyboard.press('ControlOrMeta+a');
    await this.page.keyboard.press('Backspace');
    await editor.pressSequentially(content);

    if (tags.length > 0) {
      const tagInput = this.tagInput;
      if (await tagInput.isVisible()) {
        for (const tag of tags) {
          await tagInput.fill(tag);
          await tagInput.press('Enter');
          await expect(this.tagPill(tag)).toBeVisible({ timeout: 3000 });
        }
      }
    }

    // Wait for auto-save to flush to the store by checking the note list
    // contains an item with a unique portion of the content. Using 25+ chars
    // prevents false-positive matches when notes share the same first word
    // (e.g., "Foxtrot report about elephants" vs "Foxtrot report about giraffes").
    const uniqueText = content.slice(0, 25).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    await expect(
      this.page.locator('[data-testid="note-list-item"]').filter({ hasText: new RegExp(uniqueText, 'i') }).first()
    ).toBeVisible({ timeout: 5000 });
  }

  /** Create a note and return to the list (mobile flow) */
  async createNoteAndReturnToList(content: string) {
    await this.newNoteButton.click();

    const editor = this.editorContent;
    await expect(editor).toBeVisible({ timeout: 5000 });
    await editor.click();
    await editor.pressSequentially(content);
    await this.page.waitForTimeout(2000);

    // Go back to list
    const backButton = this.page.locator('button svg').first()
      .or(this.page.locator('button').filter({ hasText: /←/ }))
      .or(this.page.locator('[aria-label*="back" i]'));

    if (await backButton.count() > 0) {
      await backButton.first().click();
      await this.page.waitForTimeout(1000);
    }
  }

  /** Open settings modal */
  async openSettings(): Promise<boolean> {
    if (await this.settingsButton.isVisible({ timeout: 2000 }).catch(() => false)) {
      await this.settingsButton.click();
      await expect(this.dialog.first()).toBeVisible({ timeout: 5000 });
      return true;
    }

    // Fall back to keyboard shortcut
    await this.page.keyboard.press('Control+,');
    try {
      await expect(this.dialog.first()).toBeVisible({ timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  /** Open the Advanced tab in settings */
  async openAdvancedTab() {
    await this.openSettings();
    const settingsModal = this.dialog.first();
    await expect(settingsModal).toBeVisible({ timeout: 3000 });

    const advancedTab = settingsModal.locator('button, [role="tab"]').filter({
      hasText: /advanced/i
    }).first();

    if (await advancedTab.isVisible()) {
      await advancedTab.click();
      await this.page.waitForTimeout(500);
    }
  }

  /** Open the "More actions" (three dots) menu in the editor toolbar */
  async openMoreMenu() {
    await expect(this.moreActionsButton).toBeVisible({ timeout: 5000 });
    await this.moreActionsButton.click();
  }

  /** Open version history via the more actions menu */
  async openVersionHistory(): Promise<Locator> {
    await this.openMoreMenu();
    const vhButton = this.page.locator('button').filter({ hasText: '📜' });
    await expect(vhButton).toBeVisible();
    await vhButton.click();
    const modal = this.dialog;
    await expect(modal).toBeVisible({ timeout: 5000 });
    return modal;
  }

  /** Open recycle bin */
  async openRecycleBin() {
    const btn = this.page.locator('button').filter({ hasText: /Recycle Bin/i });
    await btn.click();
    await this.page.waitForTimeout(500);
  }

  /** Select a note from the list via Ctrl+Click (enter multi-select) */
  async ctrlClickNote(textPattern: RegExp) {
    const noteItem = this.noteListItem(textPattern);
    await noteItem.click({ modifiers: ['ControlOrMeta'] });
  }

  /** Click a note's checkbox in multi-select mode */
  async clickNoteCheckbox(textPattern: RegExp) {
    const noteItem = this.noteListItem(textPattern);
    const checkbox = noteItem.locator('[role="checkbox"]');
    await checkbox.click();
  }

  /** Blur the editor by clicking on the note list area */
  async blurEditor() {
    const noteList = this.noteList;
    if (await noteList.isVisible()) {
      await noteList.click({ position: { x: 10, y: 10 } });
      await this.page.waitForTimeout(200);
    }
  }
}
