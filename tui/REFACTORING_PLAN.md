# TUI Refactoring Plan

## Overview

This plan organizes 14 refactoring tasks in optimal order, ensuring:
1. Dependencies are resolved before dependent tasks
2. Tests are added/maintained at each step
3. Existing tests (243 total) continue to pass
4. Building blocks are created before larger refactors

**Total Estimated Time: 22-30 hours**

---

## Phase 1: Foundation & Cleanup (4 tasks, ~3 hours)

These tasks create building blocks used by later refactors.

### 1.1 `jottery-rb6l` - Audit and clean up dead code
**Time: 30 min | Dependencies: None**

```bash
# Verify dead code
grep -r "remove_tags_from_selected\|get_tags_from_selected\|handle_text_input_with_callback" tui/src/

# Tasks:
1. Check if functions are truly unused
2. Remove or document with reason for keeping
3. Remove #[allow(dead_code)] if functions are used
```

**Tests:** Run `cargo test` to ensure no regressions

---

### 1.2 `jottery-efj7` - Extract path completion reset helper
**Time: 30 min | Dependencies: None**

```rust
// Add to tui/src/ui/app.rs
impl App {
    pub fn reset_path_completions(&mut self) {
        self.path_completions.clear();
        self.path_completion_index = 0;
    }
}
```

**Files to modify:**
- `tui/src/ui/app.rs` - Add method
- `tui/src/ui/input/note_list.rs` - Replace 5 occurrences (lines 207-208, 217-218, 231-232, 275-276, 281-282)

**Tests to add:**
```rust
#[test]
fn test_reset_path_completions() {
    let mut app = create_test_app();
    app.path_completions = vec!["a".into(), "b".into()];
    app.path_completion_index = 1;
    app.reset_path_completions();
    assert!(app.path_completions.is_empty());
    assert_eq!(app.path_completion_index, 0);
}
```

---

### 1.3 `jottery-jmfk` - Extract note finding helper
**Time: 1 hour | Dependencies: None**

```rust
// Add to tui/src/ui/app.rs
impl App {
    pub fn get_note(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn get_note_mut(&mut self, id: &str) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }
}
```

**Files to modify:**
- `tui/src/ui/app.rs` - Add methods
- `tui/src/ui/operations/bulk.rs` - Lines 50, 91
- `tui/src/ui/operations/attachments.rs` - Line 324
- `tui/src/ui/input/note_list.rs` - Lines 584, 830, 884, 904, 935, 997, 1034, 1054

**Tests to add:**
```rust
#[test]
fn test_get_note_found() {
    let mut app = create_test_app_with_notes();
    assert!(app.get_note("existing-id").is_some());
}

#[test]
fn test_get_note_not_found() {
    let app = create_test_app();
    assert!(app.get_note("nonexistent").is_none());
}

#[test]
fn test_get_note_mut_modifies() {
    let mut app = create_test_app_with_notes();
    if let Some(note) = app.get_note_mut("existing-id") {
        note.pinned = true;
    }
    assert!(app.get_note("existing-id").unwrap().pinned);
}
```

---

### 1.4 `jottery-vt5g` - Consolidate centered_rect implementations
**Time: 1-2 hours | Dependencies: None**

```rust
// Add to tui/src/ui/rendering/modal.rs
/// Calculate centered rect with absolute dimensions
pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Calculate centered rect with percentage dimensions
pub fn centered_rect_percent(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let width = (area.width * width_percent) / 100;
    let height = (area.height * height_percent) / 100;
    centered_rect(area, width, height)
}
```

**Files to modify:**
- `tui/src/ui/rendering/modal.rs` - Add/update functions
- `tui/src/ui/rendering/inbox.rs` - Remove local function, use modal::centered_rect_percent
- `tui/src/ui/rendering/conflict.rs` - Replace inline calculation
- `tui/src/ui/rendering/note_list.rs` - Replace 6 inline calculations

**Tests to add:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_basic() {
        let area = Rect::new(0, 0, 100, 50);
        let result = centered_rect(area, 40, 20);
        assert_eq!(result.x, 30);
        assert_eq!(result.y, 15);
        assert_eq!(result.width, 40);
        assert_eq!(result.height, 20);
    }

    #[test]
    fn test_centered_rect_larger_than_area() {
        let area = Rect::new(0, 0, 50, 30);
        let result = centered_rect(area, 100, 60);
        assert_eq!(result.width, 50); // Clamped to area
        assert_eq!(result.height, 30);
    }

    #[test]
    fn test_centered_rect_percent() {
        let area = Rect::new(0, 0, 100, 100);
        let result = centered_rect_percent(area, 50, 50);
        assert_eq!(result.width, 50);
        assert_eq!(result.height, 50);
        assert_eq!(result.x, 25);
        assert_eq!(result.y, 25);
    }
}
```

---

## Phase 2: Consolidation (3 tasks, ~4 hours)

These tasks reduce code duplication by consolidating similar functions.

### 2.1 `jottery-9me2` - Consolidate note sorting logic
**Time: 1 hour | Dependencies: Phase 1 complete**

```rust
// Add to tui/src/ui/app.rs or new tui/src/ui/utils.rs
pub fn sort_notes_by_pinned_and_date<'a>(notes: &mut Vec<&'a Note>) {
    notes.sort_by(|a, b| {
        match (a.pinned, b.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.modified_at.cmp(&a.modified_at),
        }
    });
}
```

**Files to modify:**
- `tui/src/ui/app.rs` - Lines 622-628, 643-649
- `tui/src/ui/operations/notes.rs` - Lines 24-30, 43-49

**Tests to add:**
```rust
#[test]
fn test_sort_notes_pinned_first() {
    let notes = create_test_notes(); // 3 notes, middle one pinned
    let mut refs: Vec<&Note> = notes.iter().collect();
    sort_notes_by_pinned_and_date(&mut refs);
    assert!(refs[0].pinned);
}

#[test]
fn test_sort_notes_by_date_within_pinned() {
    let notes = create_test_notes_same_pin_status();
    let mut refs: Vec<&Note> = notes.iter().collect();
    sort_notes_by_pinned_and_date(&mut refs);
    assert!(refs[0].modified_at > refs[1].modified_at);
}
```

---

### 2.2 `jottery-oqna` - Consolidate color cycling functions
**Time: 1 hour | Dependencies: Phase 1 complete**

```rust
// In tui/src/ui/input/note_list.rs
fn cycle_color(app: &App, current: Option<&String>, direction: i32) -> Option<String> {
    let color_names: Vec<&String> = app.settings.get_color_palette().keys().collect();
    if color_names.is_empty() {
        return None;
    }

    let current_idx = current
        .and_then(|c| color_names.iter().position(|n| *n == c))
        .map(|i| i as i32);

    let new_idx = match current_idx {
        Some(idx) => (idx + direction).rem_euclid(color_names.len() as i32) as usize,
        None => if direction > 0 { 0 } else { color_names.len() - 1 },
    };

    Some(color_names[new_idx].clone())
}

pub fn cycle_color_forward(app: &App) -> Option<String> {
    let current = app.current_note().and_then(|n| n.color.as_ref());
    cycle_color(app, current, 1)
}

pub fn cycle_color_backward(app: &App) -> Option<String> {
    let current = app.current_note().and_then(|n| n.color.as_ref());
    cycle_color(app, current, -1)
}
```

**Tests to add:**
```rust
#[test]
fn test_cycle_color_forward_from_none() {
    let app = create_test_app_with_colors();
    let result = cycle_color(&app, None, 1);
    assert!(result.is_some());
}

#[test]
fn test_cycle_color_wraps_around() {
    let app = create_test_app_with_colors(); // 3 colors
    let last_color = app.settings.get_color_palette().keys().last().unwrap();
    let result = cycle_color(&app, Some(last_color), 1);
    let first_color = app.settings.get_color_palette().keys().next().unwrap();
    assert_eq!(result.as_ref(), Some(first_color));
}
```

---

### 2.3 `jottery-vmun` - Extract search filter logic to shared function
**Time: 2-3 hours | Dependencies: 2.1 complete**

```rust
// Add to tui/src/ui/app.rs
pub struct SearchContext<'a> {
    pub modifiers: &'a SearchModifiers,
    pub query_parts: Vec<&'a str>,
    pub settings: &'a UserSettings,
    pub archive_mode: bool,
}

impl App {
    fn note_matches_search(&self, note: &Note, ctx: &SearchContext) -> bool {
        // Extract the ~80 lines of search logic here
        // Used by both filtered_notes() and count_opposite_mode_matches()
    }
}
```

**Files to modify:**
- `tui/src/ui/app.rs` - Extract from lines 720-866 and 869-973

**Tests to add:**
```rust
#[test]
fn test_note_matches_search_by_content() {
    let app = create_test_app();
    let note = create_note_with_content("hello world");
    let ctx = SearchContext { query_parts: vec!["hello"], .. };
    assert!(app.note_matches_search(&note, &ctx));
}

#[test]
fn test_note_matches_search_by_tag() {
    let app = create_test_app();
    let note = create_note_with_tags(vec!["rust"]);
    let ctx = SearchContext { modifiers: SearchModifiers { tags: vec!["rust".into()], .. }, .. };
    assert!(app.note_matches_search(&note, &ctx));
}

#[test]
fn test_note_matches_search_archived_filter() {
    let app = create_test_app();
    let archived_note = create_archived_note();
    let ctx = SearchContext { archive_mode: false, .. };
    assert!(!app.note_matches_search(&archived_note, &ctx));
}
```

---

## Phase 3: Performance & Caching (3 tasks, ~5 hours)

These tasks improve performance through caching.

### 3.1 `jottery-a6zt` - Cache color palette in App
**Time: 1 hour | Dependencies: 2.2 complete**

```rust
// In tui/src/ui/app.rs
pub struct App {
    // ... existing fields
    color_palette_cache: Vec<String>,
}

impl App {
    pub fn refresh_color_palette_cache(&mut self) {
        self.color_palette_cache = self.settings
            .get_color_palette()
            .keys()
            .cloned()
            .collect();
    }

    pub fn get_cached_color_names(&self) -> &[String] {
        &self.color_palette_cache
    }
}
```

**Call `refresh_color_palette_cache()` when:**
- App is created
- Settings are loaded/changed

**Tests to add:**
```rust
#[test]
fn test_color_palette_cache_populated() {
    let app = create_test_app();
    assert!(!app.get_cached_color_names().is_empty());
}

#[test]
fn test_color_palette_cache_refreshes() {
    let mut app = create_test_app();
    let initial_count = app.get_cached_color_names().len();
    // Modify settings to add color
    app.refresh_color_palette_cache();
    // Assert count changed
}
```

---

### 3.2 `jottery-oxdz` - Cache tag completions
**Time: 2 hours | Dependencies: Phase 2 complete**

```rust
// In tui/src/ui/app.rs
pub struct App {
    // ... existing fields
    all_tags_cache: Vec<String>,
    tags_cache_dirty: bool,
}

impl App {
    pub fn invalidate_tags_cache(&mut self) {
        self.tags_cache_dirty = true;
    }

    pub fn get_all_tags(&mut self) -> &[String] {
        if self.tags_cache_dirty {
            let mut tags: HashSet<String> = HashSet::new();
            for note in &self.notes {
                for tag in &note.tags {
                    tags.insert(tag.clone());
                }
            }
            self.all_tags_cache = tags.into_iter().collect();
            self.all_tags_cache.sort();
            self.tags_cache_dirty = false;
        }
        &self.all_tags_cache
    }

    pub fn get_tag_completions(&mut self, partial: &str) -> Vec<String> {
        self.get_all_tags()
            .iter()
            .filter(|t| t.starts_with(partial))
            .cloned()
            .collect()
    }
}
```

**Invalidate cache when:**
- Note is saved with changed tags
- Note is deleted
- Notes are loaded

**Tests to add:**
```rust
#[test]
fn test_tag_cache_returns_all_tags() {
    let mut app = create_test_app_with_tagged_notes();
    let tags = app.get_all_tags();
    assert!(tags.contains(&"rust".to_string()));
}

#[test]
fn test_tag_completions_filter() {
    let mut app = create_test_app_with_tagged_notes(); // tags: rust, ratatui, python
    let completions = app.get_tag_completions("ru");
    assert!(completions.contains(&"rust".to_string()));
    assert!(!completions.contains(&"python".to_string()));
}
```

---

### 3.3 `jottery-4l82` - Cache filtered_notes() result
**Time: 2-3 hours | Dependencies: 2.3 complete**

```rust
// In tui/src/ui/app.rs
pub struct App {
    // ... existing fields
    filtered_notes_cache: Option<Vec<String>>, // Store IDs, not full notes
    filter_cache_dirty: bool,
}

impl App {
    pub fn invalidate_filter_cache(&mut self) {
        self.filter_cache_dirty = true;
        self.filtered_notes_cache = None;
    }

    pub fn filtered_note_ids(&mut self) -> Vec<String> {
        if self.filter_cache_dirty || self.filtered_notes_cache.is_none() {
            let ids = self.filtered_notes()
                .iter()
                .map(|n| n.id.clone())
                .collect();
            self.filtered_notes_cache = Some(ids);
            self.filter_cache_dirty = false;
        }
        self.filtered_notes_cache.clone().unwrap_or_default()
    }
}
```

**Invalidate when:**
- Search query changes
- Archive mode toggles
- Notes change (add/delete/modify)
- Settings change

**Tests to add:**
```rust
#[test]
fn test_filter_cache_returns_consistent_results() {
    let mut app = create_test_app_with_notes();
    let first = app.filtered_note_ids();
    let second = app.filtered_note_ids();
    assert_eq!(first, second);
}

#[test]
fn test_filter_cache_invalidates_on_search_change() {
    let mut app = create_test_app_with_notes();
    let _ = app.filtered_note_ids();
    app.search_input = "new search".to_string();
    app.invalidate_filter_cache();
    // Cache should be recomputed
}
```

---

## Phase 4: Builders & Extraction (2 tasks, ~3 hours)

### 4.1 `jottery-u0b2` - Create input modal builder
**Time: 1 hour | Dependencies: 1.4 complete**

```rust
// Add to tui/src/ui/rendering/modal.rs
pub struct InputModalConfig<'a> {
    pub title: &'a str,
    pub input_text: &'a str,
    pub help_text: &'a str,
    pub width: u16,
    pub height: u16,
}

pub fn render_input_modal(frame: &mut Frame, area: Rect, config: InputModalConfig) {
    let modal_area = centered_rect(area, config.width, config.height);

    // Clear background
    frame.render_widget(Clear, modal_area);

    // Create bordered block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(config.title);

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    // Render input line
    let input = Paragraph::new(config.input_text)
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(input, Rect::new(inner.x, inner.y + 1, inner.width, 1));

    // Render help text
    let help = Paragraph::new(config.help_text)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, Rect::new(inner.x, inner.y + 3, inner.width, 1));
}
```

**Files to modify:**
- `tui/src/ui/rendering/modal.rs` - Add builder
- `tui/src/ui/rendering/note_list.rs` - Replace bulk add tags modal (1119-1158) and export path modal (1160-1199)

**Tests:** Visual inspection, ensure modals render correctly

---

### 4.2 `jottery-pkqn` - Extract pager operations module
**Time: 1-2 hours | Dependencies: Phase 1 complete**

Create new file `tui/src/ui/operations/pager.rs`:

```rust
//! Pager operations for viewing notes in external programs

use std::process::{Command, Stdio};
use std::io::Write;

/// Check if a command is available in PATH
pub fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run content through a pager (less, bat, etc.)
pub fn run_pager(pager: &str, content: &str) -> std::io::Result<()> {
    let mut child = Command::new(pager)
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}

/// View note content in readonly mode using system pager
pub fn view_with_pager(content: &str, syntax: Option<&str>) -> std::io::Result<()> {
    // Try bat first (syntax highlighting), fall back to less
    if is_command_available("bat") {
        let lang_arg = syntax.map(|s| format!("--language={}", s));
        // ... bat logic
    } else if is_command_available("less") {
        run_pager("less", content)
    } else {
        // Fallback: just print
        println!("{}", content);
        Ok(())
    }
}
```

**Files to modify:**
- Create `tui/src/ui/operations/pager.rs`
- `tui/src/ui/operations/mod.rs` - Add `pub mod pager;`
- `tui/src/ui/operations/notes.rs` - Remove lines 217-323, import from pager

**Tests to add:**
```rust
#[test]
fn test_is_command_available_ls() {
    assert!(is_command_available("ls"));
}

#[test]
fn test_is_command_available_nonexistent() {
    assert!(!is_command_available("definitely_not_a_real_command_12345"));
}
```

---

## Phase 5: Large File Splits (2 tasks, ~7-10 hours)

These are the largest refactors and depend on all previous phases.

### 5.1 `jottery-2wbq` - Split rendering/note_list.rs (1,366 lines)
**Time: 3-4 hours | Dependencies: 4.1 complete**

**New file structure:**
```
tui/src/ui/rendering/
├── mod.rs
├── note_list.rs      (reduced to ~400 lines - main list rendering)
├── version_history.rs (new - ~475 lines)
├── modal.rs          (expanded with modal helpers)
├── inbox.rs
├── conflict.rs
└── help.rs
```

**Extract to `version_history.rs`:**
- Lines 600-1075 from note_list.rs
- `render_version_history_view()`
- Version diff rendering
- Version list rendering

**Move to `modal.rs`:**
- Confirmation modal rendering (currently inline)
- Use the builder from 4.1

**Tests:** Ensure all views still render correctly

---

### 5.2 `jottery-1uxz` - Split input/note_list.rs (1,856 lines)
**Time: 4-6 hours | Dependencies: All previous phases**

**New file structure:**
```
tui/src/ui/input/
├── mod.rs
├── note_list.rs      (reduced to ~800 lines - core navigation)
├── attachment.rs     (new - ~100 lines)
├── search.rs         (new - ~150 lines)
├── bulk.rs           (new - ~100 lines)
└── text_input.rs     (existing)
```

**Extract to `attachment.rs`:**
- Lines 201-286 from note_list.rs
- Path input handling
- Tab completion for paths

**Extract to `search.rs`:**
- Lines 290-400+ from note_list.rs
- Search input handling
- Tag completion logic

**Extract to `bulk.rs`:**
- Bulk add tags input
- Bulk export path input

**Tests:** Ensure all input modes still work correctly

---

## Execution Checklist

### Before Each Task
- [ ] `cargo test` - All 243 tests pass
- [ ] `cargo clippy` - No warnings
- [ ] Create git commit checkpoint

### After Each Task
- [ ] `cargo test` - All tests still pass
- [ ] `cargo clippy` - No new warnings
- [ ] New tests added for new code
- [ ] Git commit with descriptive message
- [ ] Update beads issue status

### Git Commit Convention
```
refactor(tui): <description>

- Bullet points of changes
- Tests added: X

Closes jottery-XXXX
```

---

## Summary Table

| Phase | Task ID | Description | Time | Dependencies |
|-------|---------|-------------|------|--------------|
| 1.1 | jottery-rb6l | Dead code audit | 30m | None |
| 1.2 | jottery-efj7 | Path completion reset | 30m | None |
| 1.3 | jottery-jmfk | Note finding helper | 1h | None |
| 1.4 | jottery-vt5g | Centered rect | 1-2h | None |
| 2.1 | jottery-9me2 | Note sorting | 1h | Phase 1 |
| 2.2 | jottery-oqna | Color cycling | 1h | Phase 1 |
| 2.3 | jottery-vmun | Search filter | 2-3h | 2.1 |
| 3.1 | jottery-a6zt | Color palette cache | 1h | 2.2 |
| 3.2 | jottery-oxdz | Tag cache | 2h | Phase 2 |
| 3.3 | jottery-4l82 | Filtered notes cache | 2-3h | 2.3 |
| 4.1 | jottery-u0b2 | Modal builder | 1h | 1.4 |
| 4.2 | jottery-pkqn | Pager extraction | 1-2h | Phase 1 |
| 5.1 | jottery-2wbq | Split rendering | 3-4h | 4.1 |
| 5.2 | jottery-1uxz | Split input | 4-6h | All |

**Total: 22-30 hours**
