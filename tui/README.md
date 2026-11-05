# Jottery TUI

Terminal User Interface for Jottery - A privacy-focused, encrypted note-taking application.

## Features

- 🔐 **End-to-End Encryption**: AES-256-GCM encryption with a random master key, wrapped using PBKDF2 (600k iterations)
- 📝 **Full Note Management**: Create, edit, delete, search, and tag notes
- 🔄 **Bidirectional Sync**: Full sync with Jottery server including auto-sync
- 💾 **Local Storage**: SQLite database. Note content and tags are always encrypted, and the whole file is too when built with SQLCipher
- ⌨️ **Keyboard-Driven**: Efficient Vim-style keyboard shortcuts for all operations
- 🎨 **Syntax Highlighting**: Support for JavaScript, Python, Markdown, JSON, HTML, CSS, SQL, Bash, Perl
- 📋 **Markdown Rendering**: Beautiful markdown preview with tables, lists, code blocks, and inline formatting
- 🗑️ **Recycle Bin**: Recover accidentally deleted notes
- 🎨 **Color Schemes**: 11 built-in themes (Default Dark/Light, Monokai, Solarized, Nord, Dracula, Gruvbox, Tokyo Night, Catppuccin)
- 🔒 **Auto-lock**: Configurable timeout with optional password storage
- 🔍 **Advanced Search**: Powerful search with tag filtering and boolean operators
- 📊 **Split-pane View**: Note list + preview pane for quick navigation
- 📜 **Version History**: Browse and restore previous versions of any note
- ✅ **Multi-Select**: Bulk operations on multiple notes (tag, delete, export)
- 📌 **Note Pinning**: Pin important notes to the top of the list
- 🌍 **15 Languages**: Full i18n support with locale-aware date formatting

## Installation

### Prerequisites

Notes and tags are encrypted in every build. To encrypt the rest of the database file as well, such as your email, the server address, attachment names and dates, build with **SQLCipher**. The default build doesn't include it, and says so on the unlock screen. Either way the file is readable only by your user.

**macOS:**
```bash
brew install sqlcipher
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install libsqlcipher-dev
```

**Linux (Fedora):**
```bash
sudo dnf install sqlcipher-devel
```

### From Source

```bash
cd tui
cargo build --release                      # or: cargo build --release --features sqlcipher
sudo cp target/release/jottery /usr/local/bin/
```

**Note:** On macOS, the build is configured to automatically find SQLCipher installed via Homebrew. On Linux, SQLCipher should be in standard system paths.

## Usage

### Interactive Mode

```bash
# Start the TUI (creates database in ~/.config/jottery/)
jottery

# Use custom database location
jottery --database /path/to/notes.db

# Enable debug logging
jottery --debug
```

### CLI Commands

#### Registration (for sync setup)

```bash
# Register a new user account on the server
jottery register-user --server https://your-server.com --email your@email.com
# Optional: provide password via flag (or will prompt)
jottery register-user -s https://your-server.com -e your@email.com -p yourpassword

# Register this device with an approved user account
jottery register-device --server https://your-server.com --email your@email.com
# Optional: provide device name (default: "TUI")
jottery register-device -s https://your-server.com -e your@email.com -n "My Laptop"
```

#### Sync

```bash
# Manually trigger sync (if already configured)
jottery sync --password yourpassword
```

#### Export/Import

```bash
# Export notes to JSON (decrypted for backup)
jottery export --output backup.json --password yourpassword

# Import notes from JSON
jottery import --input backup.json --password yourpassword
```

#### Note Management via CLI

```bash
# Create a note via pipe
echo "Note content here" | jottery note --password yourpassword

# Create a note with tags
echo "Task item" | jottery note -p yourpassword --tags todo,work

# Open editor for new note
jottery note --password yourpassword
```

#### Attachments and the Archive via CLI

```bash
# Attach a file to a note, then save it somewhere else
jottery attach 1a2b3c4d ~/Documents/report.pdf
jottery show 1a2b3c4d
jottery save-attachment 1a2b3c4d -n 1 -o ~/Downloads/report.pdf

# Archive a note, list the archive, and bring the note back
jottery archive 1a2b3c4d
jottery list --archived
jottery unarchive 1a2b3c4d
```

These sync afterwards if this device is connected. `$EDITOR` and `$PAGER` can include arguments, such as `code -w`.

#### Other Commands

```bash
# List all notes
jottery list --password yourpassword
# Show note content
jottery list --password yourpassword --show-content

# Show a specific note
jottery show <note-id> --password yourpassword

# Delete a note
jottery delete <note-id> --password yourpassword

# Reset database (deletes all data)
jottery --reset        # asks first; --yes skips that
```

Other commands:

```bash
# Run a saved search
jottery search --saved "Work"

# See what the server holds, or delete it (the destructive ones ask for your email)
jottery account info -e you@example.com
jottery account delete-notes -e you@example.com
jottery account delete -e you@example.com

# Name a colour, and give a tag one
jottery colours name red Urgent
jottery colours tag work Urgent
jottery colours list

# Check the vault. Exits non-zero if something's wrong
jottery doctor
jottery doctor --repair     # also fetches missing attachments from the server
```

Exports are decrypted, so they're readable by you alone, and `export` won't replace a
file unless you add `--force`.

### First Run

1. Run `jottery` to start
2. Enter a password to create encrypted database
3. Press `n` to create your first note
4. Use your system's default `$EDITOR` to edit the note
5. Save and close the editor to return to the list

### Color Schemes

Press `s` to open settings and cycle through 11 color schemes:
- **Default Dark** - Clean dark theme
- **Default Light** - Clean light theme
- **Monokai** - Popular syntax highlighting theme
- **Solarized Dark** - Low-contrast dark theme
- **Solarized Light** - Low-contrast light theme
- **Nord** - Arctic-inspired color palette
- **Dracula** - Dark theme with purple accents
- **Gruvbox Dark** - Retro groove dark theme
- **Gruvbox Light** - Retro groove light theme
- **Tokyo Night** - Dark theme inspired by Tokyo's night
- **Catppuccin** - Pastel color scheme

Use `Enter`/`i`/`Space` to cycle forward, `I`/`Shift+Enter` to cycle backward.

## Configuration

Configuration is stored in:
- **Linux**: `~/.config/jottery/config.toml`
- **macOS**: `~/Library/Application Support/jottery/config.toml`
- **Windows**: `%APPDATA%\jottery\config.toml`

Database is stored in the same directory as `jottery.db`.

### Settings

Press `s` to open the settings panel:
- **Language** - Interface language (default: en-GB)
- **Color Scheme** - Choose from 11 themes
- **Sort Order** - Recent, Oldest, Alphabetical, or Created date
- **Auto-lock Timeout** - Minutes before auto-lock (1-1440)
- **Sync Enabled** - Enable/disable server sync
- **Sync Endpoint** - Server URL for sync
- **Auto-sync Interval** - Minutes between auto-sync (default: 5)
- **Remember Password** - Auto-unlock on startup (see [Password Storage](#password-storage) below)

### Password Storage

The "Remember Password" feature stores your database password locally for auto-unlock. The storage method depends on your platform:

| Platform | Storage Method | Security Level |
|----------|---------------|----------------|
| **Windows** | Credential Manager | ✅ Secure |
| **Linux** | Secret Service (GNOME Keyring/KDE Wallet) | ✅ Secure |
| **macOS (signed app)** | Keychain | ✅ Secure |
| **macOS (unsigned/dev build)** | File-based obfuscation | ⚠️ Obfuscated only |

**macOS Note:** The Keychain requires a properly code-signed binary to maintain access across app rebuilds. Development builds use ad-hoc signatures which change on each compile, breaking Keychain access. For development builds, the app falls back to file-based storage with obfuscation (not cryptographically secure).

To enable Keychain support on macOS during development, sign the binary with your Apple Developer certificate:
```bash
# Find your signing identity
security find-identity -v -p codesigning

# Sign the binary
codesign -s "Apple Development: your@email.com (XXXXXXXXXX)" target/debug/jottery
```

### Sync Setup

Jottery now requires user account registration and admin approval before syncing.

#### Step 1: Register User Account

```bash
jottery register-user --server https://your-server.com --email your@email.com
```

You'll be prompted to:
- Enter a password (min 12 characters)
- This creates your user account on the server
- Account will be **pending admin approval** initially

#### Step 2: Wait for Admin Approval

The server administrator must approve your account before you can register devices.
- Contact your administrator after registration
- They can approve via the admin web dashboard or CLI tool

#### Step 3: Register Your Device

Once approved, register this TUI as a device:

```bash
jottery register-device --server https://your-server.com --email your@email.com
```

You'll be prompted for:
- **Server password**: Your account password from Step 1
- **Notes password**: The password that protects your notes
  - It must be different from your server password. The server sees your server password, so that one can't also open your notes
  - If this account already has notes, use the notes password from your other devices. A new notes password needs at least 8 characters
  - Required each time you open the TUI (unless "Remember Password" is enabled)

The device registration will:
- Authenticate with your approved user account
- Receive an API key from the server
- Unlock the account's master key with your notes password, and move any local notes onto it. If the notes password can't open it, the key may have been saved by an older version: unlock Jottery on a device that's already connected, which fixes it, then try again
- Store encrypted credentials in your local database
- Enable sync automatically

#### Step 4: Use the TUI

After registration, simply run:

```bash
jottery
```

- Sync will happen automatically every 5 minutes (configurable)
- Manual sync: Press `s` → navigate to "Sync Now" → press `Enter`
- Sync settings available in the settings panel (`s` key)

Sync runs in the background, so a slow server doesn't hold up the screen. Automatic
syncs wait while a note is open, and a sync that's already running won't replace it:
if the note changed elsewhere, that shows as a conflict.

If your server uses a private certificate authority, such as Caddy's internal CA or
mkcert, install it in your system's trust store, or point `SSL_CERT_FILE` at it. The
bundled roots are trusted too.

## Passwords on the Command Line

Commands prompt for a password when they need one. For scripts, pipe it in with
`--password-stdin` (one password per line, ahead of anything else on stdin), or set
`JOTTERY_PASSWORD`, `JOTTERY_ACCOUNT_PASSWORD` or `JOTTERY_KEY_PASSWORD`.

`--password` still works, but it's visible in `ps` and your shell history, so it's
deprecated and says so.

## Temporary Files

An editor, a pager or a viewer can only work on a file, so a note or attachment leaves the
vault while it's open. It goes in a private folder (`0700`), under `$XDG_RUNTIME_DIR` or
`/dev/shm` where there is one, so that it stays in memory. Elsewhere, including macOS, it's
in the system temp folder on disk, and Jottery says so the first time.

The folder is emptied when the file closes, and if Jottery gets SIGHUP, SIGTERM or SIGINT,
as it does when an SSH connection drops. If it's killed outright, or the power goes, the
next run clears up.

vim and neovim are started with swap, backup, undo and viminfo turned off. Other editors
keep their own copies, and those are up to you. Side files beside the note go with the
folder, such as nano's `.save` and emacs's `file~` and `#file#`, but anything an editor
keeps elsewhere doesn't. Check your editor's settings if that matters, for example
`backup-directory-alist` and `auto-save-file-name-transforms` in emacs.

## Search Syntax

The search bar supports powerful query syntax:

### Tag Filtering
```
#tagname                    # Notes with this tag
#tag1 #tag2                 # Notes with both tags (AND)
#tag1 | #tag2               # Notes with either tag (OR)
-#tagname                   # Notes without this tag
```

### Text Search
```
dog                         # Contains "dog"
"exact phrase"              # Exact phrase match
dog cat                     # Contains both words (AND)
dog | cat                   # Contains either word (OR)
-cat                        # Does NOT contain "cat"
-"exact phrase"             # Does NOT contain this phrase
```

### Wildcards
```
dog*                        # A word starts with "dog"
*dog                        # A word ends with "dog"
*dog*                       # Contains "dog" anywhere
```

### Modifiers
```
has:attachment              # Notes with attachments
archived                    # Search the archive (archived:false for active notes)
created:>2024-01-01         # Created after a date
created:<2024-06-30         # Created before a date
created:2024-01-01..2024-06-30  # Created in a date range
modified:>2024-01-01        # The same for the modified date
words:>100                  # More than 100 words (also words:<50, words:50..200)
color:red                   # This colour on the note or a tag (add note or tag to narrow it)
category:important          # The same, by a colour's name
```

### Combined Queries
```
#work project -meeting      # Tagged #work, contains "project", not "meeting"
#personal "family vacation" # Tagged #personal with exact phrase
```

`jottery search` takes the same queries. The rules are shared with the other clients, and
written down with test cases in `docs/search-query-fixture.json`.

## Markdown Rendering

The preview pane renders markdown with full formatting support:

### Supported Elements
- **Headers** (H1-H6) - Styled in cyan + bold
- **Bold text** (`**bold**` or `__bold__`) - White + bold
- **Italic text** (`*italic*` or `_italic_`) - Cyan color
- **Inline code** (`` `code` ``) - Yellow
- **Links** (`[text](url)`) - Blue + underlined
- **Code blocks** with syntax highlighting (9 languages)
- **Tables** with aligned columns and header formatting
- **Lists** (bullet points with `•` marker)
- **Task lists** (`- [ ]` and `- [x]`)
- **Horizontal rules** (`---` or `***`)

### Example
```markdown
## Tasks

- [x] Make coffee
- [ ] Open notes app
- [ ] Do actual work

| Item     | Priority | Notes                       |
|----------|----------|-----------------------------|
| Biscuits | High     | Essential for productivity  |
| Exercise | Medium   | Allegedly good for you      |
| Email    | Low      | Ignored on purpose          |
```

## Keyboard Shortcuts

### Unlock Screen
| Key | Action |
|-----|--------|
| Type | Enter password |
| `Enter` | Unlock database |
| `Backspace` | Delete character |
| `Ctrl+r` | Toggle remember password checkbox |
| `q`/`Esc` | Quit |

### Note List (Normal Mode)
| Key | Action |
|-----|--------|
| `n` | Create new note |
| `Enter`/`i` | Open selected note in external editor |
| `p` | Pin/unpin selected note |
| `L` | Lock/unlock selected note against changes |
| `t` | Edit tags |
| `d` | Delete selected note (moves to recycle bin) |
| `z` | Archive selected note (or bring it back in archive mode) |
| `A` | Show or leave the archive |
| `r` | Open the recycle bin |
| `v` | Open version history for selected note |
| `I` | Show note info |
| `x` | Resolve a sync conflict on the selected note |
| `b` | Open the inbox |
| `g` | Open your saved searches |
| `Shift+D` | Duplicate selected note |
| `o` | Copy the note's text to the clipboard (through the terminal, so it also works over SSH) |
| `w` | Turn word wrap on or off for selected note |
| `l` | Move through the links in the preview (`Enter` follows one) |
| `j`/`↓` | Move down |
| `k`/`↑` | Move up |
| `]`/`[` | Cycle syntax language forward/backward |
| `C`/`c` | Cycle note colour forward/backward |
| `/` | Search (`Ctrl+S` saves the query, `Ctrl+P`/`Ctrl+N` go through earlier searches) |
| `y` | Sync now |
| `Y` | Force a full resync |
| `s` | Open settings |
| `?` | Show keyboard shortcuts help |
| `Ctrl+l` | Lock Jottery |
| `q` | Quit |
| `Ctrl+q` | Quit application |

### Multi-Select
| Key | Action |
|-----|--------|
| `Space` | Toggle selection of the current note |
| `V` | Select a range |
| `Ctrl+a` | Select all listed notes |
| `t` | Add tags to selected notes |
| `d` | Delete selected notes |
| `z` | Archive selected notes |
| `c` | Combine selected notes |
| `e` | Export selected notes to JSON |
| `Esc` | Clear selection |

### Attachments
| Key | Action |
|-----|--------|
| `+` | Attach a file to the selected note |
| `a` | Open the attachment list |
| `Tab` | Focus the attachments of the selected note |
| `j`/`k` | Move between attachments |
| `Enter` | View the selected attachment |
| `S` | Save the selected attachment to a file |
| `d` | Delete the selected attachment |
| `X` | Remove the selected attachment |

Images, PDFs and plain text open in a viewer. Anything else could run code from whoever sent it, so Jottery offers to save it instead.

### Preview
| Key | Action |
|-----|--------|
| `Ctrl+d`/`Ctrl+u` | Scroll down/up half a page |
| `Ctrl+f`/`Ctrl+b` | Scroll down/up a page |
| `PageDown`/`PageUp` | Scroll down/up a page |

### Settings Screen
| Key | Action |
|-----|--------|
| `j`/`↓` | Move to next setting |
| `k`/`↑` | Move to previous setting |
| `Enter`/`i`/`Space` | Edit setting (cycle for theme/sort order) |
| `I`/`Shift+Enter` | Cycle backward (theme/sort order) |
| `Esc`/`q` | Close settings |

### Version History
| Key | Action |
|-----|--------|
| `j`/`↓` | Move to next version |
| `k`/`↑` | Move to previous version |
| `Enter` | Restore selected version |
| `Esc`/`q` | Close version history |

### Recycle Bin
| Key | Action |
|-----|--------|
| `r` | Restore selected note |
| `E` | Empty the bin, after asking |
| `Esc`/`q` | Return to note list |

## Development Status

### Completed ✓
- [x] Project setup and build configuration
- [x] Database layer (SQLite + SQLCipher)
- [x] Data models (Rust structs)
- [x] Encryption layer (AES-256-GCM + PBKDF2 with 600k iterations)
- [x] Repository pattern for data access
- [x] TUI framework (ratatui)
- [x] Note management (create, edit, delete, list)
- [x] Password-based unlocking with auto-lock
- [x] Remember password feature (optional auto-unlock)
- [x] Import/export (JSON)
- [x] Tag management (add, remove, filter)
- [x] Advanced search (boolean operators, tag filters, wildcards)
- [x] Recycle bin (soft delete with restore)
- [x] Settings panel (theme, sort order, auto-lock, sync config)
- [x] Bidirectional sync client
- [x] Auto-sync on schedule
- [x] Syntax highlighting in preview (9 languages)
- [x] Markdown rendering with proper formatting
- [x] External editor integration ($EDITOR)
- [x] Split-pane view (list + preview)
- [x] Keyboard shortcuts help screen
- [x] Multiple color schemes (11 themes)
- [x] Vim-style navigation (hjkl, gg, G, Ctrl+d/u)
- [x] Version history with restore functionality
- [x] Multi-select operations (bulk tag, delete, export)
- [x] Note pinning
- [x] Internationalisation (15 languages)
- [x] Locale-aware date formatting

### Future Enhancements
- [ ] Attachment support (view and download)
- [ ] Nested tags
- [ ] Note linking
- [ ] Full-text search indexing
- [ ] Note templates

## Architecture

```
tui/
├── src/
│   ├── main.rs           # Entry point
│   ├── ui/              # TUI components
│   ├── db/              # Database layer
│   ├── crypto/          # Encryption/decryption
│   ├── sync/            # Sync client
│   ├── models/          # Data models
│   ├── services/        # Business logic
│   └── error/           # Error types
└── Cargo.toml
```

## License

MIT
