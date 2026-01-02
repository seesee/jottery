# Jottery

Jottery is a simple, searchable, and privacy-focused scratchpad/notes application. It is designed to be self-hosted and accessible from both a web browser and a terminal. The core idea is to provide a secure place for your notes, with all data being end-to-end encrypted. It runs as a web app or in a terminal, the sync server is optional. All data are stored locally unless explicitly configured to sync to an endpoint of your choosing.

## Why Jottery?

Jottery was created to solve a common problem: **finding a quick, reliable place to capture fleeting thoughts and temporary data**. Whether you're tweaking text before pasting it elsewhere, saving command output, or jotting down ideas that might be useful later (or might not), you need a scratchpad that's always available and works the way you do.

Most note-taking apps are either too heavy (designed for long-form content) or too limited (basic text files). Jottery strikes a balance: it's lightweight enough for quick captures but powerful enough to handle code snippets, searchable tags, and rich formatting. By providing both web and terminal clients that sync seamlessly, it meets you wherever you're working—whether that's a browser, SSH session, or local terminal.

## Features

*   **End-to-End Encryption**: All your notes are encrypted on your device before being stored/sent to the sync server. The server only stores encrypted blobs of data.
*   **Multi-User Support**: Multiple users can use the same server, each with their own encrypted notes and devices.
*   **Admin Approval Workflow**: New user registrations require admin approval for controlled access.
*   **Cross-Platform**: Access your notes from a web client or a Terminal User Interface (TUI).
*   **Self-Hostable**: You have full control over your data by hosting the sync server yourself.
*   **Device Management**: Register multiple devices per user, each with its own API key.
*   **Admin Dashboard**: Web-based interface for managing users, viewing statistics, and monitoring server activity.
*   **Search**: Quickly find your notes with a powerful full-text search.
*   **Tagging**: Organise your notes with tags.
*   **Attachments**: Add, preview, and download attachments to your notes.
*   **Code Snippets**: A rich text editor with support for various programming languages.
*   **Quick Delete**: Delete notes instantly from the list view (hover for delete button, or use configurable keyboard shortcut).
*   **Keyboard Shortcuts**: Fully customisable keyboard shortcuts for common actions.
*   **Many handy features**: Export notes, preview HTML, document info, basic versioning, and markdown documents in-editor.

## Caveats
*   **Sync**: The sync mechanism is quite robust but very basic. Updates are sent periodically and the last version to be received "wins". To mitigate against accidental deletions, the previous version is also stored.
*   **Security**: Use a strong password -- all data blobs (notes, tags, attachments) are encrypted using it. If you forget your password, there is no recovery process. You will lose all your notes and will need to start over. Because it's handy for development purposes, there is a mechanism to store the password in both web and tui app (this is NOT synced) so you don't need to constantly input your password -- but if you use this, your notes are basically plain text to anyone with access to your device/db files. I suggest using a password manager.
*   **Sync Server**: All sync accounts need to be approved by an admin. Admins can see "username" (email address is suggested but does not need to be "real"), the total number of notes and the combined size (in kb) of the notes held by that user. 

## Components

Jottery consists of three main components:

### 1. Web Client

A modern web application that provides a rich user experience for managing your notes.

### 2. TUI Client

A lightweight and fast terminal user interface for those who prefer to work in the terminal. The TUI includes powerful command-line tools for quick note operations:

**CLI Commands:**
- `jottery note` - Create a note with your `$EDITOR`, or pipe content directly:
  ```bash
  echo "Quick note" | jottery note
  ls -la | jottery note -t system,logs
  ```
- `jottery list` - List all notes with optional tag filtering
- `jottery search <query>` - Search notes by content or tags
- `jottery show <id>` - Display a specific note
- `jottery sync` - Manually trigger sync with server
- `jottery export` / `jottery import` - Backup and restore notes

**CLI Features:**
- **Stored password support** - No password prompts when you've saved your password
- **Pipe content directly** - Capture command output as notes instantly
- **Auto-sync** - Notes sync to server automatically after creation (if configured)
- **Tag support** - Add tags to piped content with `-t tag1,tag2`

### 3. Sync Server

A simple server that stores your encrypted notes and syncs them between your clients.

## Authentication & User Management

Jottery supports multi-user deployments with an admin approval workflow:

### User Registration

1. New users register with email and password (minimum 12 characters)
2. Account enters "pending approval" state
3. Admin approves the account via admin dashboard or CLI
4. Approved users can register devices for sync

### Device Registration

- Each user can register multiple devices (web, TUI, CLI)
- Each device receives a unique API key for sync operations
- Devices can be individually revoked by admins
- Separate from user accounts for better security

### Admin Dashboard

Access the web-based admin dashboard at `http://your-server:3030/admin`

**Default credentials** (⚠️ CHANGE IMMEDIATELY):
- Email: `admin@localhost`
- Password: `changeme`

**Admin Features:**
- Approve/reject user registrations
- View user statistics (notes, devices, storage)
- Deactivate/reactivate user accounts
- Manage device registrations
- View audit logs of sync operations
- Monitor server statistics

## Technology Stack

### Web Client (Svelte)

*   **Framework**: Svelte
*   **Language**: TypeScript
*   **Styling**: Tailwind CSS
*   **Editor**: CodeMirror
*   **Database**: IndexedDB
*   **Search**: FlexSearch

### TUI Client (Rust)

*   **Framework**: Ratatui
*   **Language**: Rust
*   **Database**: SQLite with SQLCipher
*   **Async Runtime**: Tokio

### Sync Server (Rust)

*   **Framework**: Axum
*   **Language**: Rust
*   **Database**: SQLite
*   **Async Runtime**: Tokio

## Getting Started

### With Docker

The easiest way to get started is with Docker. This will build and run the web client and sync server in a single container.

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/seesee/jottery.git
    cd jottery
    ```

2.  **Build and run with Docker Compose:**

    ```bash
    docker-compose up -d
    ```

    The web interface will be available at `http://localhost:8000`. The data will be stored in a `data` directory on your host machine.

3.  **Access the admin dashboard:**

    Navigate to `http://localhost:3030/admin` and login with the default credentials:
    - Email: `admin@localhost`
    - Password: `changeme`

    ⚠️ **IMPORTANT:** Change the default admin password immediately via the admin dashboard.

4.  **Create your first user account:**

    - Register a new user account via the web UI at `http://localhost:8000`
    - Login to the admin dashboard and approve the new user
    - The approved user can now register devices and start syncing notes

### Manual Installation

If you prefer to run the components manually, you can follow these steps:

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/seesee/jottery.git
    cd jottery
    ```

2.  **Web Client:**

    ```bash
    npm install
    npm run dev
    ```

3.  **Sync Server:**

    ```bash
    cd server
    cargo run
    ```

4.  **TUI Client:**

    You can download a pre-compiled binary for your platform from the releases section in the web app, or build it from source:

    ```bash
    cd tui
    cargo run
    ```

## TUI Client Downloads

You can download pre-compiled binaries for the TUI client for Linux, macOS, and Windows from the "Releases" section of the web application.

## Import/Export Format

Jottery uses a JSON-based format for importing and exporting notes. All data is **decrypted** during export and **re-encrypted** during import using your master password.

### Overview

- **Format**: JSON with UTF-8 encoding
- **Version**: `1.0` (specified in `version` field)
- **Security**: Exported data contains decrypted note content, tags, and attachments
- **Compatibility**: Import works across different Jottery clients (Web, TUI)

### Export Format Structure

```json
{
  "version": "1.0",
  "exportDate": "2025-12-30T18:30:00.000Z",
  "notes": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "createdAt": "2025-12-25T10:00:00.000Z",
      "modifiedAt": "2025-12-29T15:30:00.000Z",
      "content": "# Meeting Notes\n\nDiscussed project timeline...",
      "tags": ["work", "meetings", "project-alpha"],
      "attachments": [
        {
          "filename": "diagram.png",
          "mimeType": "image/png",
          "data": "iVBORw0KGgoAAAANSUhEUgAAAAUA..."
        }
      ],
      "pinned": false,
      "wordWrap": true,
      "syntaxLanguage": "markdown"
    }
  ]
}
```

### Field Descriptions

#### Root Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | string | Yes | Export format version (currently `"1.0"`) |
| `exportDate` | string | Yes | ISO 8601 timestamp when export was created |
| `notes` | array | Yes | Array of exported notes (see Note Object below) |

#### Note Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | UUID v4 identifier for the note |
| `createdAt` | string | Yes | ISO 8601 timestamp when note was created |
| `modifiedAt` | string | Yes | ISO 8601 timestamp of last modification |
| `content` | string | Yes | **Decrypted** note content (plain text or markdown) |
| `tags` | array | Yes | Array of **decrypted** tag strings |
| `attachments` | array | Yes | Array of attachment objects (can be empty) |
| `pinned` | boolean | Yes | Whether note is pinned to top of list |
| `wordWrap` | boolean | No | Word wrap setting for editor (default: false) |
| `syntaxLanguage` | string | No | Syntax highlighting language ID (e.g., `"javascript"`, `"markdown"`) |

#### Attachment Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `filename` | string | Yes | **Decrypted** original filename |
| `mimeType` | string | Yes | MIME type (e.g., `"image/png"`, `"application/pdf"`) |
| `data` | string | Yes | Base64-encoded file data (**decrypted**) |

### Import Strategies

When importing notes, you can choose one of three strategies:

1. **`merge`** (default): Import all notes with new UUIDs, preserving existing notes
2. **`skip`**: Skip notes with IDs that already exist in your database
3. **`replace`**: Delete existing notes with matching IDs and import new versions

### Example: Minimal Valid Export

```json
{
  "version": "1.0",
  "exportDate": "2025-12-30T20:00:00.000Z",
  "notes": [
    {
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "createdAt": "2025-12-30T19:00:00.000Z",
      "modifiedAt": "2025-12-30T19:00:00.000Z",
      "content": "Hello World",
      "tags": [],
      "attachments": [],
      "pinned": false
    }
  ]
}
```

### Example: Note with Attachments and Tags

```json
{
  "version": "1.0",
  "exportDate": "2025-12-30T20:15:00.000Z",
  "notes": [
    {
      "id": "a3b2c1d0-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "createdAt": "2025-12-28T14:22:00.000Z",
      "modifiedAt": "2025-12-30T16:45:00.000Z",
      "content": "```javascript\nconst hello = () => console.log('Hello!');\n```",
      "tags": ["code", "javascript", "snippets"],
      "attachments": [
        {
          "filename": "screenshot.png",
          "mimeType": "image/png",
          "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
        },
        {
          "filename": "README.txt",
          "mimeType": "text/plain",
          "data": "VGhpcyBpcyBhIHRlc3QgZmlsZQ=="
        }
      ],
      "pinned": true,
      "wordWrap": false,
      "syntaxLanguage": "javascript"
    }
  ]
}
```

### Supported Syntax Languages

The `syntaxLanguage` field supports all languages available in CodeMirror 6, including:

- Programming: `javascript`, `typescript`, `python`, `rust`, `go`, `java`, `cpp`, `c`, `csharp`, `php`, `ruby`, `swift`, `kotlin`
- Web: `html`, `css`, `scss`, `less`, `vue`, `svelte`
- Markup: `markdown`, `xml`, `yaml`, `json`, `toml`
- Shell: `shell`, `bash`, `powershell`
- Query: `sql`, `mysql`, `postgresql`
- And many more...

### Important Notes

**Security:**
- Exported files contain **unencrypted, plain-text data**
- Store export files securely (encrypt with tools like GPG or use encrypted volumes)
- Delete export files after transferring if they contain sensitive information

**Compatibility:**
- Export format version `1.0` is the current standard
- Future versions may add fields but will maintain backward compatibility
- Timestamps must be valid ISO 8601 format with timezone (e.g., `2025-12-30T20:00:00.000Z`)
- UUIDs must be valid UUID v4 format

**File Size:**
- Attachments are Base64-encoded, which increases size by ~33%
- Large exports with many attachments may take time to process
- Consider exporting in batches for very large note collections

**Automation:**
- The JSON format is designed to be easily parsed and generated by scripts
- Perfect for backup automation, migration tools, or external integrations
- Can be processed with `jq`, Python, Node.js, or any JSON-compatible tool

### Programmatic Usage Examples

**Using `jq` to extract all tags:**
```bash
jq '.notes[].tags[]' jottery-export-2025-12-30.json | sort -u
```

**Using Python to filter notes by tag:**
```python
import json

with open('jottery-export.json', 'r') as f:
    data = json.load(f)

work_notes = [n for n in data['notes'] if 'work' in n['tags']]
print(f"Found {len(work_notes)} work-related notes")
```

**Using Node.js to create a backup:**
```javascript
const fs = require('fs');
const exportData = {
  version: '1.0',
  exportDate: new Date().toISOString(),
  notes: [
    {
      id: crypto.randomUUID(),
      createdAt: new Date().toISOString(),
      modifiedAt: new Date().toISOString(),
      content: 'Automated backup note',
      tags: ['backup', 'automated'],
      attachments: [],
      pinned: false
    }
  ]
};
fs.writeFileSync('backup.json', JSON.stringify(exportData, null, 2));
```

## Using Attachments in Markdown

Jottery supports embedding attachments directly in your markdown notes. This allows you to create rich documents with inline images, PDFs, and other file types.

### Attachment Syntax

Use markdown link syntax to reference attachments by their filename:

**Images** (will be displayed inline when previewing markdown):
```markdown
![description](attachment:filename.png)
```

**Other files** (PDFs, documents, etc. - will show as clickable links):
```markdown
[Attachment: filename.pdf](attachment:filename.pdf)
```

### How to Get the Markdown Link

1. **Upload an attachment** to your note using the attachment area at the bottom
2. **Click the 📋 (clipboard) button** next to the attachment
3. **Paste the markdown link** into your note content

The correct syntax will be copied automatically:
- Images use `![filename](attachment:filename)` syntax
- Other files use `[Attachment: filename](attachment:filename)` syntax

### Preview Behaviour

When you preview a markdown note with attachment links:

- **Images** are rendered inline (displayed directly in the preview)
- **PDFs, audio, and video** show as clickable cards that open a full preview modal
- **Text files** can be previewed in the modal as well
- **Other file types** show a download link

### Supported Previews

- **Images**: PNG, JPG, GIF, SVG, WebP
- **PDFs**: Full page-by-page viewer with zoom controls
- **Audio**: MP3, WAV, OGG, etc.
- **Video**: MP4, WebM, etc.
- **Text**: TXT, JSON, XML, source code files
- **Unsupported types**: Download button to view externally

### Example

```markdown
# Project Documentation

Here's the architecture diagram:

![System architecture](attachment:architecture.png)

And here's the detailed specification:

[Attachment: specifications.pdf](attachment:specifications.pdf)

## Audio Notes

Meeting recording:

[Attachment: meeting-2025-01-02.mp3](attachment:meeting-2025-01-02.mp3)
```

### Notes

- Attachment filenames are **encrypted** in the database
- The markdown preview decrypts and resolves filenames automatically
- Renaming attachments will break existing markdown links (references are by filename)
- You can reference the same attachment multiple times in one note

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
