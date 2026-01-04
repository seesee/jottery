# Admin App Internationalisation TODO

## Completed

- ✅ Installed `svelte-i18n` dependency
- ✅ Created i18n service (`src/lib/services/i18nService.ts`)
- ✅ Created locale structure (`src/locales/`)
- ✅ Extracted all admin strings to `en-GB.json` (comprehensive locale file)
- ✅ Initialised i18n in `main.ts`
- ✅ Translated `App.svelte` (navigation, logout confirmation)
- ✅ Translated `Login.svelte` (complete)

## Remaining Work

### 1. Complete Component Translations

The following components still need translation integration:

- **Dashboard.svelte**: Stats, server information, refresh button
- **Users.svelte**: User table, filters, actions, confirmation messages
- **Settings.svelte**: Password change form, validation messages
- **ConfirmModal.svelte**: Already accepts translated props, no changes needed
- **Toast.svelte**: Toast messages (handled by calling code)

### 2. Create Multi-Language Translations

All 15 languages need translation files created in `admin/src/locales/`:

Supported languages (from main app):
- en-US.json (English US variant)
- es.json (Spanish)
- fr.json (French)
- de.json (German)
- it.json (Italian)
- pt.json (Portuguese)
- ja.json (Japanese)
- zh.json (Chinese Simplified)
- ko.json (Korean)
- ru.json (Russian)
- nl.json (Dutch)
- pl.json (Polish)
- tr.json (Turkish)
- el.json (Greek)

### 3. Translation Approach

**Option A: Manual Translation**
- Copy `en-GB.json` to each language file
- Translate strings manually or with AI assistance
- Ensure placeholders like `{email}`, `{count}`, `{action}` are preserved

**Option B: Auto-Translate Script**
- Adapt the main app's `scripts/auto-translate.js` for admin locales
- Create `scripts/admin-auto-translate.js` targeting `admin/src/locales/`
- Use DeepL or LibreTranslate API

**Option C: Share with Main App Script**
- Modify `scripts/auto-translate.js` to accept a `--target` parameter
- Run: `node scripts/auto-translate.js --target=admin`

### 4. Language Selector (Optional Enhancement)

Consider adding a language selector to the Settings page:
- Dropdown using `AVAILABLE_LOCALES` from `i18nService.ts`
- Store selection in localStorage
- Apply on app initialisation

## Translation Keys Structure

The `en-GB.json` file is organised as follows:

```
app.*          - Application name
common.*       - Shared UI strings (buttons, labels)
nav.*          - Navigation menu items
login.*        - Login page
logout.*       - Logout confirmation
dashboard.*    - Dashboard statistics and info
users.*        - User management (table, actions, confirmations)
settings.*     - Settings page (password change)
```

## Technical Notes

- All components using i18n should import: `import { _ } from 'svelte-i18n';`
- Use `$_('key.path')` for reactive translations
- Placeholder syntax: `$_('key', { values: { name: 'Value' } })`
- Browser language auto-detection is enabled
- Falls back to `en-GB` if locale not found

## Estimated Effort

- Complete component translations: ~2-3 hours
- Create 14 language translations (manual): ~6-8 hours
- Create 14 language translations (automated): ~1-2 hours (script setup + review)
- Testing all languages: ~1 hour

**Total: 9-14 hours** (depending on approach)
