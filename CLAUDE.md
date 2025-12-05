# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**jottery** is a privacy-focused, self-hosted scratch pad application for capturing, organizing, and searching notes with rich content, syntax highlighting, and encryption. The project is being developed in phases:

- **Phase 1**: Web application (single-page application)
- **Phase 2**: Enhanced features (attachments, themes, keyboard shortcuts)
- **Phase 3**: Sync & server capabilities
- **Phase 4**: Unix TUI (terminal user interface)

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
- **Language**: Rust or Go
- **Framework**: ratatui (Rust) or bubbletea (Go)
- **Storage**: SQLite with SQLCipher
- **Editor**: System $EDITOR or built-in

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
  createdAt: string;             // ISO 8601 with timezone
  modifiedAt: string;            // ISO 8601 with timezone
  syncedAt?: string;             // ISO 8601 with timezone
  content: string;               // Encrypted note content
  tags: string[];                // Encrypted array of tags
  attachments: Attachment[];     // Array of attachment references
  pinned: boolean;              // Pin status
  deleted: boolean;             // Soft delete flag
  deletedAt?: string;           // Deletion timestamp
  syncHash?: string;            // Hash for conflict detection
  version: number;              // Optimistic locking
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

# Combined
#animals dog -cat          - Tagged #animals, contains "dog", not "cat"
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
1. **SearchBar** - Input with search syntax support
2. **NoteList** - Virtualized list for performance
3. **NoteListItem** - Preview with auto-generated title, tags, date
4. **EditorPane** - CodeMirror 6 editor with syntax highlighting
5. **TagInput** - Tag editor with autocomplete
6. **AttachmentList** - File attachments with preview/download
7. **SettingsModal** - Application configuration
8. **RecycleBin** - View for soft-deleted notes

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

### Editor
- `Ctrl/Cmd + F` - Find in note
- `Ctrl/Cmd + H` - Replace in note
- `Esc` - Close note

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
