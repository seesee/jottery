# TUI Internationalisation TODO

## Overview

The Jottery TUI is a Rust-based terminal application that requires a different internationalisation approach than the JavaScript-based web and admin apps.

## Recommended Approach

### Option 1: `rust-i18n` Crate (Recommended)

**Advantages:**
- Simple compile-time i18n
- YAML-based translation files
- Automatic fallback to default locale
- Lightweight and fast

**Implementation Steps:**

1. Add dependency to `tui/Cargo.toml`:
```toml
[dependencies]
rust-i18n = "3"
```

2. Create locale files in `tui/locales/`:
```
tui/locales/
├── en-GB.yml
├── en-US.yml
├── es.yml
├── fr.yml
├── de.yml
├── it.yml
├── pt.yml
├── ja.yml
├── zh.yml
├── ko.yml
├── ru.yml
├── nl.yml
├── pl.yml
├── tr.yml
└── el.yml
```

3. Initialize in `tui/src/main.rs` or `tui/src/lib.rs`:
```rust
rust_i18n::i18n!("locales");
```

4. Use in code:
```rust
use rust_i18n::t;

println!("{}", t!("welcome.message"));
println!("{}", t!("error.not_found", name = "note.txt"));
```

5. Example locale file (`en-GB.yml`):
```yaml
welcome:
  message: "Welcome to Jottery TUI"
error:
  not_found: "Error: %{name} not found"
  permission_denied: "Permission denied"
menu:
  notes: "Notes"
  settings: "Settings"
  quit: "Quit"
```

### Option 2: `fluent` Crate (More Powerful)

**Advantages:**
- Mozilla's Fluent localisation system
- Advanced pluralisation and gender support
- Industry standard

**Disadvantages:**
- More complex setup
- Larger dependency

### Option 3: `gettext` Crate (Traditional)

**Advantages:**
- Mature, widely used in Unix tools
- Excellent tooling (POEdit, etc.)

**Disadvantages:**
- More traditional workflow
- Requires separate compilation step

## Strings to Translate

### UI Elements

**Main Menu:**
- Notes list
- Create new note
- Settings
- Help
- Quit

**Note Editor:**
- Save note
- Discard changes
- Add tags
- Attach file
- Back to list

**Settings:**
- Change password
- Sync settings
- Auto-lock timeout
- Theme selection
- Language selection

**Error Messages:**
- Database errors
- File not found
- Permission denied
- Encryption/decryption failures
- Sync errors

**Status Messages:**
- Note saved
- Note deleted
- Syncing...
- Locked

### Estimated String Count

Based on TUI functionality:
- Main UI: ~30-40 strings
- Error messages: ~20-30 strings
- Help text: ~40-50 strings
- **Total: ~100-120 strings** (significantly fewer than web apps)

## Implementation Plan

### Phase 1: Setup (1-2 hours)

1. Choose i18n crate (`rust-i18n` recommended)
2. Add to `Cargo.toml`
3. Create `locales/` directory structure
4. Set up build integration

### Phase 2: Extract Strings (2-3 hours)

1. Audit TUI codebase for hardcoded strings
2. Create `en-GB.yml` master locale file
3. Replace hardcoded strings with `t!()` macro calls
4. Test with English locale

### Phase 3: Translations (variable)

**Option A: Manual Translation**
- Copy `en-GB.yml` for each language
- Translate manually or with AI
- Estimated: 4-6 hours for all 14 languages

**Option B: Automated**
- Create script to convert YAML to JSON
- Use DeepL/LibreTranslate API
- Convert back to YAML
- Estimated: 2-3 hours (including script creation)

### Phase 4: Language Selection (1-2 hours)

1. Add language setting to TUI settings
2. Detect system locale on first run
3. Store preference in settings database
4. Apply locale on TUI start

## Technical Considerations

### System Locale Detection

Use `sys-locale` crate to detect system language:

```rust
use sys_locale::get_locale;

fn get_default_locale() -> String {
    get_locale()
        .unwrap_or_else(|| "en-GB".to_string())
        .replace('-', "_") // Convert en-GB to en_GB for some i18n systems
}
```

### Settings Integration

Add to `src/models/settings.rs` or equivalent:

```rust
pub struct Settings {
    // ...existing fields...
    pub language: String,  // e.g., "en-GB", "fr", "ja"
}
```

### Build-Time vs Runtime

- `rust-i18n`: Compile-time (locales embedded in binary)
- `fluent`: Runtime (locales loaded from files)

**Recommendation:** Use `rust-i18n` for simplicity and smaller binary size.

## Example: Before and After

### Before (Hardcoded):
```rust
println!("Error: Failed to decrypt note");
println!("Note saved successfully");
```

### After (Internationalised):
```rust
use rust_i18n::t;

println!("{}", t!("error.decrypt_failed"));
println!("{}", t!("success.note_saved"));
```

## File Structure

```
tui/
├── Cargo.toml (add rust-i18n dependency)
├── locales/
│   ├── en-GB.yml
│   ├── fr.yml
│   ├── de.yml
│   └── ... (14 languages)
├── src/
│   ├── main.rs (initialize i18n)
│   ├── ui/
│   │   ├── app.rs (use t!() for UI strings)
│   │   └── ...
│   └── ...
└── I18N-TODO.md (this file)
```

## Estimated Total Effort

- Setup and infrastructure: ~2 hours
- Extract and refactor strings: ~3 hours
- Create 14 language translations: ~2-6 hours (depending on approach)
- Testing: ~2 hours

**Total: 9-13 hours**

## Resources

- [rust-i18n documentation](https://docs.rs/rust-i18n/)
- [Fluent documentation](https://projectfluent.org/)
- [sys-locale crate](https://docs.rs/sys-locale/)

## Notes

- TUI has significantly fewer strings than web apps (~100 vs ~440)
- Rust i18n at compile-time means faster runtime performance
- Consider providing English-only build option for minimal binary size
- Test with both LTR (Latin) and RTL (Arabic) languages if supporting Arabic
