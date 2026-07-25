# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**jottery** is a privacy-focused, self-hosted scratch pad application for capturing, organising, and searching notes with rich content, syntax highlighting, and encryption. The project is being developed in phases:

- **Phase 1**: Web application (single-page application) ✅ COMPLETE
- **Phase 2**: Enhanced features (attachments, themes, keyboard shortcuts) ✅ COMPLETE
- **Phase 3**: Sync & server capabilities ✅ COMPLETE
  - Multi-user authentication with admin approval workflow
  - Session-based admin dashboard
  - Device API key management
- **Phase 4**: Unix TUI (terminal user interface) 🔄 IN PROGRESS

**License**: MIT

## Technology Stack

### Web Application (Phase 1)
- **Framework**: Svelte (or Vue 3 as alternative)
- **Build Tool**: Vite
- **Language**: TypeScript
- **Storage**: IndexedDB (via `idb` wrapper)
- **Encryption**: Web Crypto API (AES-256-GCM with PBKDF2/Argon2id)
- **Editor**: CodeMirror 6
- **Search**: FlexSearch
- **Styling**: TailwindCSS (or UnoCSS)
- **i18n**: svelte-i18n or i18next
- **Icons**: Lucide Icons or Heroicons

### TUI Application (Phase 4)
- **Language**: Rust
- **Framework**: ratatui
- **Storage**: SQLite with SQLCipher
- **Editor**: System $EDITOR or built-in
- **Async Runtime**: Tokio

### Sync Server (Phase 3)
- **Language**: Rust
- **Framework**: Axum (async web framework)
- **Database**: SQLite with SQLx
- **Authentication**: Argon2id for passwords, SHA-256 for API keys
- **Sessions**: Token-based with 7-day expiry
- **Admin UI**: Svelte (separate SPA at `/admin`)
- **Async Runtime**: Tokio

## Architecture Principles

### UI/UX Guidelines

**CRITICAL: NEVER USE NATIVE BROWSER CONTROLS IN WEB APPLICATIONS**

- ❌ **NEVER** use `alert()`, `confirm()`, or `prompt()` - they block the UI thread and provide poor UX
- ❌ **NEVER** use native `<input type="file">` dialogs without custom styling
- ❌ **NEVER** use native `<select>` dropdowns without custom styling
- ✅ **ALWAYS** use inline messages, custom modals, or toast notifications
- ✅ **ALWAYS** style form controls with Tailwind/CSS to match the application design
- ✅ **ALWAYS** provide visual feedback through the UI, not through alerts

**Examples:**
- Instead of `alert('Success!')` → Show a success message in the UI or use a toast component
- Instead of `confirm('Delete?')` → Use the ConfirmModal component
- Instead of `prompt('Enter name')` → Create a custom modal with an input field

### Storage Architecture
- **Repository Pattern**: Abstract all storage logic behind repository interfaces to enable future backend changes
- **Separate Object Stores**: Notes metadata, encrypted content, encrypted attachments, user settings, encryption metadata
- **Lazy Loading**: Only decrypt and load data when needed (selected note, viewed attachment)
- **Lazy Decryption**: Decrypt content on-demand, not on app startup

### Security Design
- **Encryption Algorithm**: AES-256-GCM for all sensitive data
- **Key Derivation**: PBKDF2 (≥100,000 iterations) or Argon2id
- **Master Key**: Password-derived, kept in memory only, never persisted
- **Session-Based**: Key required on app load with auto-lock after timeout (default: 15 minutes)
- **No Key Recovery**: If password is lost, data cannot be recovered
- **Encrypted Fields**: Note content, tags, attachment data
- **Unencrypted Fields**: Note ID, timestamps, sync metadata (for sorting/filtering)

### Data Model

```typescript
interface Note {
  id: string;                    // UUID v4
  userId?: string;               // Owner user ID (multi-user support)
  createdAt: string;             // ISO 8601 with timezone
  modifiedAt: string;            // ISO 8601 with timezone
  syncedAt?: string;             // ISO 8601 with timezone
  content: string;               // Encrypted note content
  tags: string[];                // Encrypted array of tags
  attachments: Attachment[];     // Array of attachment references
  pinned: boolean;               // Pin status
  deleted: boolean;              // Soft delete flag
  deletedAt?: string;            // Deletion timestamp
  syncHash?: string;             // Hash for conflict detection
  version: number;               // Optimistic locking
  wordWrap?: boolean;            // Editor preference
  syntaxLanguage?: string;       // Syntax highlighting language
}

interface Attachment {
  id: string;                   // UUID v4
  filename: string;             // Original filename (encrypted)
  mimeType: string;             // MIME type
  size: number;                 // Size in bytes
  data: string;                 // Reference to encrypted blob store
  thumbnailData?: string;       // Optional thumbnail for images
}

interface UserSettings {
  language: string;             // i18n locale code
  theme: 'light' | 'dark' | 'auto';
  sortOrder: 'recent' | 'oldest' | 'alpha';
  autoLockTimeout: number;      // Minutes
  syncEnabled: boolean;
  syncEndpoint?: string;
  userEmail?: string;           // For multi-user systems
  userId?: string;              // User ID from server
  deviceName?: string;          // Registered device name
}
```

### Conflict Resolution (Phase 3)

Use modular conflict resolver pattern:

```typescript
interface ConflictResolver {
  resolve(local: Note, remote: Note): Note;
}

class LastWriteWinsResolver implements ConflictResolver {
  resolve(local: Note, remote: Note): Note {
    return local.modifiedAt > remote.modifiedAt ? local : remote;
  }
}
```

Default to last-write-wins, but design for future strategies (manual merge, CRDTs).

### Multi-User Architecture

**Authentication Hierarchy:**
```
User Account (email/password)
  ├── Device 1 (API key)
  ├── Device 2 (API key)
  └── Device N (API key)
```

**Separation of Concerns:**
- **User Registration:** Creates user account, requires admin approval
- **Device Registration:** Links device to approved user, issues API key
- **Admin Sessions:** Separate authentication for dashboard access
- **Sync Operations:** Device-level authentication with API keys

**Security Model:**
- User passwords: Argon2id hashed (memory=19456, time=2, parallelism=1)
- API keys: 32-byte random, SHA-256 hashed
- Session tokens: UUID v4, SHA-256 hashed
- All sensitive data encrypted client-side before sync

**User Isolation:**
- Notes belong to users (user_id foreign key)
- Devices belong to users (user_id foreign key)
- Sync operations filtered by user
- Admin can view metadata, not decrypted content

**Default Admin Account:**
- Email: `admin@localhost`
- Password: `changeme` (must be changed on first deployment)
- Created automatically by migration 003
- Pre-approved and active

## Design Patterns to Avoid

- **Direct DOM Manipulation**: Makes sync harder and breaks reactive patterns
- **Timestamps Without Timezone**: Always use ISO 8601 with timezone
- **Tightly Coupled Storage**: Abstract behind repository pattern
- **Inline Base64 for Large Files**: Use reference-based blob storage
- **Absolute Paths**: Use relative paths from project root

## Performance Optimizations

- **Virtual Scrolling**: Use virtualized list for note list (render visible items only)
- **Search Index Cache**: Rebuild incrementally on note changes
- **Database Indices**: Index on `modifiedAt`, `deleted`, `pinned`, compound index on `deleted + modifiedAt`
- **In-Memory Cache**: Cache recently accessed decrypted notes (clear on lock)
- **Lazy Load Attachments**: Only load when viewed, not on app start

## Search Syntax

The search functionality supports a powerful query language:

```
# Tag filtering
#tagname                    - Notes with this tag
#tag1 #tag2                - Notes with both tags (AND)
#tag1 | #tag2              - Notes with either tag (OR)

# Text search
dog                        - Contains "dog"
"exact phrase"             - Exact phrase match
dog cat                    - Contains both words (AND)
dog | cat                  - Contains either word (OR)
-cat                       - Does NOT contain "cat"

# Wildcards
dog*                       - Starts with "dog"
*dog                       - Ends with "dog"
*dog*                      - Contains "dog" anywhere

# Advanced modifiers
has:attachment             - Notes with attachments
created:>2024-01-01        - Created after date
created:<2024-06-30        - Created before date
created:2024-01-01..2024-06-30  - Created in date range
modified:>2024-01-01       - Modified after date
words:>100                 - More than 100 words
words:<50                  - Less than 50 words
words:50..200              - Between 50 and 200 words

# Combined
#animals dog -cat          - Tagged #animals, contains "dog", not "cat"
has:attachment created:>2024-01-01  - With attachments, created this year
```

## UI Architecture

```
┌─────────────────────────────────────────────────┐
│  [Search Bar]  [+ New]  [Sort ▼]  [☰ Settings] │
├──────────────┬──────────────────────────────────┤
│              │                                   │
│  Note List   │      Editor Pane                 │
│              │                                   │
│  □ Note 1    │  [Tags: #tag1 #tag2]             │
│  ★ Note 2    │                                   │
│  □ Note 3    │  Content here...                 │
│              │                                   │
│              │                                   │
│              │                                   │
│              │  [Attachments: file.png]         │
│              │                                   │
│              │  Created: 2025-01-01 14:30       │
│              │  Modified: 2025-01-02 09:15      │
└──────────────┴──────────────────────────────────┘
```

### Key Components
1. **SearchBar** - Input with search syntax support and result count
2. **NoteList** - Virtualized list for performance with multi-select support
3. **NoteListItem** - Preview with auto-generated title, tags, date, selection checkbox
4. **EditorPane** - CodeMirror 6 editor with syntax highlighting
5. **TagInput** - Tag editor with autocomplete
6. **AttachmentList** - File attachments with preview/download
7. **SettingsModal** - Application configuration
8. **RecycleBin** - View for soft-deleted notes
9. **BulkOperationsToolbar** - Fixed bottom toolbar for bulk operations on selected notes

## Keyboard Shortcuts

### Global
- `Ctrl/Cmd + K` - Focus search
- `Ctrl/Cmd + N` - New note
- `Ctrl/Cmd + S` - Save note
- `Ctrl/Cmd + ,` - Settings
- `Ctrl/Cmd + L` - Lock application
- `Ctrl/Cmd + /` - Show keyboard shortcuts

### Note List
- `↑/↓` or `J/K` - Navigate notes
- `Enter` - Open selected note
- `Delete` - Delete selected note
- `P` - Pin/unpin selected note

### Multi-Select (Web)
- `Ctrl/Cmd + Click` - Toggle note selection
- `Shift + Click` - Select range from last selected
- `Ctrl/Cmd + A` - Select all filtered notes
- `Esc` - Clear selection

### Multi-Select (TUI)
- `Space` - Toggle current note selection
- `Shift + V` - Select range from last selected
- `Ctrl + A` - Select all filtered notes
- `Esc` - Clear selection
- `t` (in multi-select) - Add tags to selected
- `d` (in multi-select) - Delete selected (with confirmation)
- `e` (in multi-select) - Export selected to JSON

### Editor
- `Ctrl/Cmd + F` - Find in note
- `Ctrl/Cmd + H` - Replace in note
- `Esc` - Close note

## Multi-Select & Bulk Operations

Both web and TUI clients support multi-select for bulk operations on notes.

### Selection Methods
- **Toggle**: Ctrl/Cmd+click (web) or Space (TUI)
- **Range**: Shift+click (web) or Shift+V (TUI)
- **Select All**: Ctrl/Cmd+A selects all filtered notes
- **Clear**: Escape clears the selection

### Bulk Operations
When notes are selected, a toolbar appears with these actions:
- **Add Tags**: Add comma-separated tags to all selected notes
- **Remove Tags**: Select tags to remove from selected notes
- **Export**: Download selected notes as JSON file
- **Delete**: Move selected notes to recycle bin (with confirmation)
- **Select All**: Link to select all currently filtered notes

### Search Result Count
When searching, the UI displays `matches/total` (e.g., "87/456") to show how many notes match the current search.

## Import/Export Format

Export format is JSON with decrypted content:

```json
{
  "version": "1.0",
  "exportDate": "2025-03-12T17:00:00Z",
  "notes": [
    {
      "id": "uuid",
      "createdAt": "2025-01-01T10:00:00Z",
      "modifiedAt": "2025-01-02T15:30:00Z",
      "content": "Note content here",
      "tags": ["tag1", "tag2"],
      "attachments": [
        {
          "filename": "image.png",
          "mimeType": "image/png",
          "data": "base64_encoded_data"
        }
      ],
      "pinned": false
    }
  ]
}
```

## Internationalization

- Default language: English (en-GB)
- **IMPORTANT**: Always use British English spelling and grammar in all documentation, code comments, and user-facing text (e.g., "organise" not "organize", "customisable" not "customizable", "colour" not "color")
- Use translation keys: `t('note.create')`
- No hardcoded strings in components
- Translation files in `locales/` directory (e.g., `en-GB.json`, `en-US.json`)
- Use Intl API for date/time and number formatting

## Security Considerations

- **CSP Headers**: Implement Content Security Policy
- **No Third-Party Tracking**: Zero analytics or tracking
- **Local-First**: All operations work offline
- **XSS Prevention**: Sanitize all user input, especially in editor
- **No Key Recovery**: Users must understand data is unrecoverable without password

## Development Phases

### Phase 1: Core Web Application
1. IndexedDB storage implementation
2. Encryption/decryption layer
3. Note CRUD operations
4. Basic UI (list + editor)
5. Syntax highlighting
6. Tag management
7. Search functionality
8. Import/export (JSON)

### Phase 2: Enhanced Features
1. Attachment support
2. Pin/star notes
3. Soft delete + recycle bin
4. Multiple sort options
5. Keyboard shortcuts
6. Dark/light theme
7. Settings panel
8. i18n support

### Phase 3: Sync & Server
1. Backend API server
2. Sync protocol implementation
3. Conflict resolution
4. Server authentication
5. Multi-device testing

### Phase 4: TUI Application
1. Terminal user interface
2. Sync compatibility
3. CLI commands
4. System editor integration

## Testing Strategy

- **Unit Tests**: Encryption/decryption, search query parsing, conflict resolution, data validation
- **Integration Tests**: IndexedDB operations, import/export, sync process, encryption round-trips
- **E2E Tests**: User workflows, cross-browser compatibility, offline functionality, large datasets
- **Security Tests**: Encryption strength, key derivation timing, XSS vulnerabilities, CSP compliance

## Git Workflow and Commit Practices

**CRITICAL: NEVER LOSE WORK - COMMIT EARLY AND OFTEN**

### Mandatory Commit Rules

1. **Commit After Each Logical Change**
   - ✅ Fixed a bug? Commit immediately.
   - ✅ Added a feature? Commit immediately.
   - ✅ Updated translation strings? Commit immediately.
   - ✅ Modified multiple related files? Commit them together as one logical unit.

2. **NEVER Use `git reset --hard` on Uncommitted Work**
   - ❌ **FORBIDDEN**: `git reset --hard` destroys uncommitted changes permanently
   - ✅ Use `git reset --soft` or `git reset --mixed` to preserve working directory
   - ✅ Use `git stash` to temporarily save uncommitted work
   - ✅ Always commit work before any destructive git operations

3. **Before Switching Branches**
   - ✅ **ALWAYS** commit all work on current branch first
   - ✅ Or use `git stash` to save uncommitted changes
   - ❌ **NEVER** switch branches with uncommitted changes unless they're trivial

4. **Commit Message Guidelines**
   - Use clear, descriptive commit messages
   - Format: `<type>: <description>` (e.g., "fix: decrypt notes before adding to store")
   - Include context and rationale when needed
   - Add attribution footer for AI-assisted commits

5. **Incremental Development Pattern**
   ```bash
   # Bad: Make 10 changes, then try to commit everything
   # Risk: Lose all work if something goes wrong

   # Good: Make change, commit, repeat
   git add <files>
   git commit -m "feat: add translation keys to PdfViewer component"

   # Make next change
   git add <files>
   git commit -m "feat: add translation keys to VersionHistoryModal"

   # Continue iterating...
   ```

6. **Safety Checks Before Destructive Operations**
   ```bash
   # Before git reset --hard, git clean, or other destructive commands:
   git status                    # Check for uncommitted changes
   git stash                     # Save changes if needed
   # Then proceed with destructive operation
   ```

7. **Work-in-Progress Commits Are OK**
   - ✅ Commit unfinished work with "WIP: " prefix
   - ✅ Better to have WIP commits than lose work
   - ✅ Can squash or amend commits later during cleanup

### Recovery from Lost Work

If work is accidentally lost:
1. Check `git reflog` for recent HEAD positions
2. Check `git stash list` for stashed changes
3. Check editor auto-save/backup files
4. Redo the work (last resort)

### Example Workflow for Multi-File Feature

```bash
# Working on i18n feature affecting multiple files

# Step 1: Update translation file
# Edit: src/locales/en-GB.json
git add src/locales/en-GB.json
git commit -m "i18n: add PDF viewer translation keys"

# Step 2: Update first component
# Edit: src/lib/components/PdfViewer.svelte
git add src/lib/components/PdfViewer.svelte
git commit -m "i18n: translate PdfViewer component"

# Step 3: Update second component
# Edit: src/lib/components/VersionHistoryModal.svelte
git add src/lib/components/VersionHistoryModal.svelte
git commit -m "i18n: translate VersionHistoryModal component"

# Continue for each logical unit...
# Each commit is a checkpoint you can return to
```

### Branch Management

**IMPORTANT: Always use feature branches for development. Never commit directly to main.**

- **main**: Production-ready code, always deployable. Only receives merged PRs.
- **feature/***: Feature branches for all development work
- **fix/***: Bug fix branches
- **refactor/***: Refactoring branches

**Workflow:**
1. Create feature branch: `git checkout -b feature/my-feature`
2. Commit frequently to your branch
3. Push branch and open PR to main
4. CI runs automatically on PRs
5. Merge after review/CI passes
6. Delete feature branch after merge

**CI Triggers:**
- PRs to main → CI runs (validates before merge)
- Direct pushes to main → No CI (should not happen)
- Tags (`v*`) → CI runs (release validation)
- Manual trigger available via GitHub Actions UI

**Remember: Commits are cheap. Lost work is expensive. When in doubt, commit.**

### Beads: newly created issues are silently deleted unless you act

**After creating any issue with `bd create`, get it onto `main`'s manifest in the
same session — otherwise the next `bd sync` deletes it.**

`.beads/config.yaml` sets `sync-branch: beads-sync`, so `bd sync` commits
`.beads/issues.jsonl` to that branch. But `.beads/issues.jsonl` is *also* tracked
on `main`, and bd's `git-history-backfill` prunes any issue in the database that
has no history in the **checked-out branch's** manifest, writing a tombstone to
`.beads/deletions.jsonl`. An issue created on `main` and synced only to
`beads-sync` therefore vanishes on the next sync — and a later `bd import`
silently skips it as "in deletions manifest".

This bit five times in one session (2026-07-25) before the cause was found; the
issue documenting it was itself deleted twice. `beads-sync` had accumulated 112
commits since February without ever being merged back.

Either of these prevents it:

```bash
bd sync --merge                      # merge beads-sync back into main, or
git add .beads/issues.jsonl && git commit   # commit the manifest yourself
```

To recover a tombstoned issue: delete its line from `.beads/deletions.jsonl`,
then `git show beads-sync:.beads/issues.jsonl | bd import`, then commit the
manifest to `main` so it stays.

**Do not "fix" this by setting `sync-branch: main`.** `bd sync` commits through a
temporary git worktree, and git refuses to check out a branch already checked out
elsewhere, so sync fails outright while you are on main:
`fatal: 'main' is already used by worktree`. Note also that `bd sync` rewrites
`.beads/config.yaml` and strips comments from it, so notes like this one belong
here rather than in that file. Tracked in `jottery-anyy`.
