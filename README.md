# Jottery

Jottery is a simple, searchable, and privacy-focused scratchpad/notes application. It is designed to be self-hosted and accessible from both a web browser and a terminal. The core idea is to provide a secure place for your notes, with all data being end-to-end encrypted. It runs as a web app, the sync server is optional.

## Why Jottery?

Jottery was created to solve a common problem: **finding a quick, reliable place to capture fleeting thoughts and temporary data**. Whether you're tweaking text before pasting it elsewhere, saving command output, or jotting down ideas that might be useful later (or might not), you need a scratchpad that's always available and works the way you do.

Most note-taking apps are either too heavy (designed for long-form content) or too limited (basic text files). Jottery strikes a balance: it's lightweight enough for quick captures but powerful enough to handle code snippets, searchable tags, and rich formatting. By providing both web and terminal clients that sync seamlessly, it meets you wherever you're working—whether that's a browser, SSH session, or local terminal.

## Features

*   **End-to-End Encryption**: All your notes are encrypted on your device before being sent to the server. The server only stores encrypted blobs of data.
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

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
