# Jottery TUI

Terminal User Interface for Jottery - A privacy-focused, encrypted note-taking application.

## Features

- 🔐 **End-to-End Encryption**: AES-256-GCM encryption with PBKDF2 key derivation (256k iterations)
- 📝 **Full Note Management**: Create, edit, delete, search, and tag notes
- 🔄 **Bidirectional Sync**: Full sync with Jottery server including auto-sync
- 💾 **Local Storage**: SQLite database with SQLCipher encryption
- ⌨️ **Keyboard-Driven**: Efficient Vim-style keyboard shortcuts for all operations
- 🎨 **Syntax Highlighting**: Support for JavaScript, Python, Markdown, JSON, HTML, CSS, SQL, Bash, Perl
- 📋 **Markdown Rendering**: Beautiful markdown preview with tables, lists, code blocks, and inline formatting
- 🗑️ **Recycle Bin**: Recover accidentally deleted notes
- 🎨 **Color Schemes**: 11 built-in themes (Default Dark/Light, Monokai, Solarized, Nord, Dracula, Gruvbox, Tokyo Night, Catppuccin)
- 🔒 **Auto-lock**: Configurable timeout with optional password storage
- 🔍 **Advanced Search**: Powerful search with tag filtering and boolean operators
- 📊 **Split-pane View**: Note list + preview pane for quick navigation

## Installation

### Prerequisites

**SQLCipher** is required for database encryption:

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
cargo build --release
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
jottery --reset
```

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
- **Local database password**: Separate password for encrypting your local database
  - This can be different from your server password
  - Used to encrypt your local notes database
  - Required each time you open the TUI (unless "Remember Password" is enabled)

The device registration will:
- Authenticate with your approved user account
- Receive an API key from the server
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

## Search Syntax

The search bar supports powerful query syntax:

### Tag Filtering
```
#tagname                    # Notes with this tag
#tag1 #tag2                 # Notes with both tags (AND)
#tag1 | #tag2               # Notes with either tag (OR)
```

### Text Search
```
dog                         # Contains "dog"
"exact phrase"              # Exact phrase match
dog cat                     # Contains both words (AND)
dog | cat                   # Contains either word (OR)
-cat                        # Does NOT contain "cat"
```

### Wildcards
```
dog*                        # Starts with "dog"
*dog                        # Ends with "dog"
*dog*                       # Contains "dog" anywhere
```

### Combined Queries
```
#work project -meeting      # Tagged #work, contains "project", not "meeting"
#personal "family vacation" # Tagged #personal with exact phrase
```

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
| `Enter` | Open selected note in external editor |
| `Space` | Toggle preview pane |
| `d` | Delete selected note (moves to recycle bin) |
| `r` | Restore note from recycle bin |
| `D` | Permanently delete note |
| `j`/`↓` | Move down |
| `k`/`↑` | Move up |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `l` | Cycle syntax language forward (Plain → JavaScript → Python → Markdown → ...) |
| `L` | Cycle syntax language backward |
| `/` | Focus search input |
| `t` | Focus tag input |
| `Esc` | Clear search/tag filters |
| `s` | Open settings |
| `?` | Show keyboard shortcuts help |
| `Ctrl+l` | Lock database |
| `Ctrl+q` | Quit application |

### Preview Pane (when open)
| Key | Action |
|-----|--------|
| `j`/`↓` | Scroll down |
| `k`/`↑` | Scroll up |
| `Ctrl+d` | Scroll down half page |
| `Ctrl+u` | Scroll up half page |
| `g` | Jump to top |
| `G` | Jump to bottom |

### Settings Screen
| Key | Action |
|-----|--------|
| `j`/`↓` | Move to next setting |
| `k`/`↑` | Move to previous setting |
| `Enter`/`i`/`Space` | Edit setting (cycle for theme/sort order) |
| `I`/`Shift+Enter` | Cycle backward (theme/sort order) |
| `p` | Paste sync credentials from clipboard |
| `Esc`/`q` | Close settings |

### Recycle Bin
| Key | Action |
|-----|--------|
| `r` | Restore selected note |
| `D` | Permanently delete |
| `Esc`/`q` | Return to note list |

## Development Status

### Completed ✓
- [x] Project setup and build configuration
- [x] Database layer (SQLite + SQLCipher)
- [x] Data models (Rust structs)
- [x] Encryption layer (AES-256-GCM + PBKDF2 with 256k iterations)
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

### Future Enhancements
- [ ] Attachment support (view and download)
- [ ] Nested tags
- [ ] Note linking
- [ ] Full-text search indexing
- [ ] Multi-select operations
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
