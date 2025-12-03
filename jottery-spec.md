"Jottery" Notes Application - Technical Specification

1. Project Overview


Project Name: jottery

Purpose: A privacy-focused, self-hosted scratch pad application for capturing, organising, and searching notes with support for rich content, syntax highlighting, and encryption.

Target Platforms:


- Web (single-page application) - Phase 1

- Unix TUI (terminal user interface) - Phase 2

Licence: Open source (recommend MIT or GPL-3.0)


---

2. Core Requirements

2.1 Storage Architecture


Local Storage (Phase 1):


- IndexedDB for structured data storage

- Separate object stores for:
	- Notes metadata

	- Note content (encrypted)

	- Binary attachments (encrypted)

	- User settings

	- Encryption metadata


Sync Design (Phase 2):


- RESTful API or WebSocket-based sync protocol

- Sync state tracking per note (last_modified, sync_hash)

- Queue-based sync operations (create, update, delete)

- Conflict resolution: last-write-wins (modular interface for future strategies)

- Sync token/cursor for incremental synchronisation

Design Patterns to Avoid:


- Direct DOM manipulation of content (makes sync harder)

- Storing absolute timestamps without timezone information

- Tightly coupled storage logic (abstract behind repository pattern)

- Inline base64 encoding large files in note documents


---

2.2 Security & Encryption


Encryption Specification:


- Algorithm: AES-256-GCM for symmetric encryption

- Key Derivation: PBKDF2 with high iteration count (≥100,000) or Argon2id

- Salt: Unique per user, stored unencrypted

- Encrypted Fields: Note content, attachment data, tags

- Unencrypted Fields: Note ID, created timestamp, modified timestamp, sync metadata

- Implementation: Web Crypto API for browser, compatible library for TUI

Key Management:


- Password-derived master key

- Master key remains in memory only (never persisted)

- Session-based authentication (key required on app load)

- No key recovery mechanism

- Auto-lock after configurable timeout period (default: 15 minutes)

Security Considerations:


- Content Security Policy (CSP) headers

- No third-party analytics or tracking

- Local-first: all operations work offline

- Optional: TOTP 2FA for server authentication (Phase 2)


---

2.3 Data Model

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


---

2.4 User Interface (Web)


Framework Recommendation:


- Svelte or Vue 3 (Composition API)
	- Minimal boilerplate

	- Small bundle size

	- Reactive by default

	- Good component ecosystem

	- Easy to maintain


Layout:


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

Components:


1. SearchBar - Input with search syntax support

2. NoteList - Virtualized list (for performance with many notes)

3. NoteListItem - Preview with auto-generated title, tags, date

4. EditorPane - Code editor with syntax highlighting

5. TagInput - Tag editor with autocomplete

6. AttachmentList - File attachments with preview/download

7. SettingsModal - Application configuration

8. RecycleBin - View for deleted notes

UI Features:


- Keyboard shortcuts (Vim-inspired optional)

- Drag-and-drop file uploads

- Copy/paste image support

- Responsive design (mobile-friendly)

- Dark/light theme support

- Focus mode (hide note list)


---

2.5 Editor Functionality


Syntax Highlighting:


- Library: CodeMirror 6 or Monaco Editor (VSCode's editor)
	- Recommendation: CodeMirror 6 (smaller, more flexible)


- Supported Languages: Auto-detect or manual selection
	- JavaScript/TypeScript

	- Python

	- Shell/Bash

	- Markdown

	- JSON/YAML

	- SQL

	- HTML/CSS

	- C/C++/Rust

	- Go

	- And 50+ others via language packs


Editor Features:


- Line numbers

- Line wrapping (configurable)

- Find/replace within note

- Undo/redo history

- Tab/space configuration

- Bracket matching

- Auto-closing brackets/quotes


---

2.6 Search Functionality


Search Syntax:


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
	#work "meeting notes" #urgent  - Multiple criteria

Search Implementation:


- Full-text search using:
	- lunr.js (client-side search library)

	- Or flexsearch (faster, smaller)


- Search index rebuilt on note change

- Incremental search (results as you type)

- Search history (recent searches)

- Case-insensitive by default

- Optional regex search mode


---

2.7 Import/Export


Export Format (JSON):


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

Export Options:


- Export all notes (excluding deleted)

- Export filtered notes (current search results)

- Export individual note

- Markdown export for individual notes

- HTML export (rendered markdown with syntax highlighting)

- PDF export (via HTML → PDF library like jsPDF or Puppeteer)

Import:


- JSON format validation

- Duplicate detection (by content hash)

- Import strategies: merge, replace, skip duplicates

- Progress indication for large imports


---

2.8 Internationalisation (i18n)


Implementation:


- Library: i18next or vue-i18n/svelte-i18n

- Default Language: English (en-GB per user preference)

- Supported Languages (initial): en-GB, en-US

- Translation Files: JSON format

	locales/
	  en-GB.json
	  en-US.json
	  fr-FR.json



Translatable Strings:


- All UI labels and buttons

- Error messages

- Help text and tooltips

- Date/time formatting (use Intl API)

- Number formatting

Best Practices:


- No hardcoded strings in components

- Use translation keys: t('note.create')

- Support plural forms

- RTL language support (future)


---

3. Additional Features

3.1 Note Organisation


Pinned Notes:


- Pin icon in note list

- Pinned notes appear at top (regardless of sort order)

- Visual distinction (star icon, different background)

Soft Delete:


- Deleted notes moved to "Recycle Bin"

- Recycle bin view accessible from settings/menu

- Auto-purge after configurable period (default: 30 days)

- Manual permanent delete option

- Restore from recycle bin

Sort Options:


- Most recent modified (default)

- Oldest modified

- Alphabetical (by auto-generated title)

- Creation date


---

3.2 Attachments


Supported Formats:


- Images: PNG, JPG, GIF, WebP, SVG

- Documents: PDF, TXT

- Archives: ZIP (view contents)

- Any other file type (up to 100MB)

Attachment Features:


- Drag-and-drop upload

- Clipboard paste (images)

- Inline image preview (thumbnails)

- Download original file

- Remove attachment

- Storage quota indicator

Storage Strategy:


- Separate IndexedDB object store for blobs

- Attachments encrypted separately from note content

- Reference-based linking (attachment ID → blob)

- Lazy loading (don't load all attachments on app start)


---

3.3 Performance Optimisations


Lazy Loading:


- Virtual scrolling for note list (render visible items only)

- Lazy decrypt note content (only decrypt selected note)

- Lazy load attachments (only when viewed)

Caching:


- In-memory cache for recently accessed notes

- Search index cache

- Decrypted content cache (cleared on lock)

Database Indices:


- Index on modifiedAt for sorting

- Index on deleted for filtering

- Index on pinned for prioritisation

- Compound index on deleted + modifiedAt


---

3.4 Keyboard Shortcuts


Global:


- Ctrl/Cmd + K - Focus search

- Ctrl/Cmd + N - New note

- Ctrl/Cmd + S - Save note (auto-save, but manual trigger)

- Ctrl/Cmd + , - Settings

- Ctrl/Cmd + L - Lock application

- Ctrl/Cmd + / - Show keyboard shortcuts

Note List:


- ↑/↓ or J/K - Navigate notes

- Enter - Open selected note

- Delete - Delete selected note

- P - Pin/unpin selected note

Editor:


- Ctrl/Cmd + F - Find in note

- Ctrl/Cmd + H - Replace in note

- Esc - Close note (return to list)


---

4. Technical Stack Recommendations

4.1 Web Application


Core:


- Framework: Svelte (or Vue 3)

- Build Tool: Vite

- Language: TypeScript

- Storage: IndexedDB (via idb wrapper)

- Crypto: Web Crypto API

UI Libraries:


- Editor: CodeMirror 6

- Search: FlexSearch

- Icons: Lucide Icons or Heroicons

- Styling: TailwindCSS (or UnoCSS for smaller bundle)

- i18n: svelte-i18n or i18next

Utilities:


- Date/Time: date-fns (or native Intl API)

- UUID: uuid or crypto.randomUUID()

- PDF Export: jsPDF + html2canvas

- Markdown Processing: marked.js


---

4.2 TUI Application (Phase 2)


Core:


- Language: Rust or Go

- TUI Framework:
	- Rust: ratatui (formerly tui-rs)

	- Go: bubbletea


- Editor: Embedded text editor (use system $EDITOR or built-in)

- Storage: SQLite with SQLCipher for encryption

- Sync: HTTP client (reqwest in Rust, net/http in Go)

Architecture:


- Shared data format with web app (JSON protocol)

- Compatible encryption (same algorithms)

- Sync via same backend API

- CLI commands for scripting


---

5. Sync Protocol (Phase 2)

5.1 API Endpoints

	POST   /auth/login          - Authenticate and get token
	POST   /auth/logout         - Invalidate token
	GET    /sync/pull           - Pull changes since last sync
	POST   /sync/push           - Push local changes
	GET    /sync/status         - Get sync status

5.2 Sync Process

1. Authentication: Derive key from password, authenticate with server

2. Pull Phase:
	- Request changes since lastSyncTimestamp

	- Server returns encrypted note deltas

	- Apply changes locally (last-write-wins on conflict)


3. Push Phase:
	- Send local changes to server

	- Server validates and stores encrypted data


4. Conflict Resolution:
	- Compare syncHash of note versions

	- If mismatch, use modifiedAt to determine winner

	- Optionally save conflict to separate note


5.3 Sync Architecture


Modular Conflict Resolution:


	interface ConflictResolver {
	  resolve(local: Note, remote: Note): Note;
	}
	
	class LastWriteWinsResolver implements ConflictResolver {
	  resolve(local: Note, remote: Note): Note {
	    return local.modifiedAt > remote.modifiedAt ? local : remote;
	  }
	}
	
	// Future: implement other strategies
	// class ManualMergeResolver implements ConflictResolver { ... }
	// class CRDTResolver implements ConflictResolver { ... }


---

6. Development Phases

Phase 1: Core Web Application


Deliverables:


1. IndexedDB storage implementation

2. Encryption/decryption layer

3. Note CRUD operations

4. Basic UI (list + editor)

5. Syntax highlighting

6. Tag management

7. Search functionality

8. Import/export (JSON)

Phase 2: Enhanced Features


Deliverables:


1. Attachment support

2. Pin/star notes

3. Soft delete + recycle bin

4. Multiple sort options

5. Keyboard shortcuts

6. Dark/light theme

7. Settings panel

8. i18n support

Phase 3: Sync & Server


Deliverables:


1. Backend API server

2. Sync protocol implementation

3. Conflict resolution

4. Server authentication

5. Multi-device testing

Phase 4: TUI Application


Deliverables:


1. Terminal user interface

2. Sync compatibility

3. CLI commands

4. System editor integration


---

7. Testing Strategy


Unit Tests:


- Encryption/decryption functions

- Search query parsing

- Conflict resolution logic

- Data model validation

Integration Tests:


- IndexedDB operations

- Import/export workflows

- Sync process

- Encryption round-trips

E2E Tests:


- User workflows (create, edit, search, delete)

- Cross-browser compatibility

- Offline functionality

- Large dataset performance

Security Testing:


- Encryption strength

- Key derivation timing attacks

- XSS vulnerabilities

- CSP compliance


---

8. Documentation Requirements

1. README.md - Project overview, installation, basic usage

2. ARCHITECTURE.md - Technical architecture decisions

3. API.md - Sync API documentation (Phase 3)

4. CONTRIBUTING.md - Contribution guidelines

5. User Guide - End-user documentation

6. Keyboard Shortcuts - Built into app (help modal)


---

9. Future Enhancements (Backlog)

-  Collaborative notes (shared encryption key)

-  Note versioning/history

-  Nested tags or hierarchical organisation

-  Template system for common note types

-  Browser extension for quick capture

-  Mobile native apps (iOS/Android)

-  Vim keybindings mode

-  Note linking (wiki-style [[note]])

-  Fulltext search with regex

-  Export to Notion/Obsidian formats

-  Custom themes

-  Plugins/extensions system


---
End of Specification
