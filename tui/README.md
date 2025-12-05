# Jottery TUI

Terminal User Interface for Jottery - A privacy-focused, encrypted note-taking application.

## Features (Planned)

- 🔐 **End-to-End Encryption**: AES-256-GCM encryption with PBKDF2 key derivation
- 📝 **Full Note Management**: Create, edit, delete, search, and tag notes
- 🔄 **Sync**: Bidirectional sync with Jottery server
- 💾 **Local Storage**: SQLite database with SQLCipher encryption
- ⌨️ **Keyboard-Driven**: Efficient keyboard shortcuts for all operations
- 🎨 **Syntax Highlighting**: Support for multiple programming languages
- 📎 **Attachments**: View and manage file attachments
- 🗑️ **Recycle Bin**: Recover accidentally deleted notes

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

### Export/Import

```bash
# Export notes to JSON (decrypted for backup)
jottery export --output backup.json --password yourpassword

# Import notes from JSON
jottery import --input backup.json --password yourpassword
```

### First Run

1. Run `jottery` to start
2. Enter a password to create encrypted database
3. Press `n` to create your first note
4. Press `i` to enter insert mode and start typing
5. Press `Esc` then `q` to save and return to list

## Configuration

Configuration is stored in:
- **Linux**: `~/.config/jottery/config.toml`
- **macOS**: `~/Library/Application Support/jottery/config.toml`
- **Windows**: `%APPDATA%\jottery\config.toml`

Database is stored in the same directory as `jottery.db`.

## Keyboard Shortcuts

### Unlock Screen
| Key | Action |
|-----|--------|
| Type | Enter password |
| `Enter` | Unlock database |
| `Backspace` | Delete character |
| `q`/`Esc` | Quit |

### Note List
| Key | Action |
|-----|--------|
| `n` | Create new note |
| `Enter` | Open selected note |
| `d` | Delete selected note |
| `j`/`↓` | Move down |
| `k`/`↑` | Move up |
| `Ctrl+q` | Quit application |

### Note Editor
| Key | Action |
|-----|--------|
| `i` | Enter insert mode (start typing) |
| `Esc` | Exit insert mode |
| `q` (normal mode) | Save and return to list |
| Type | Edit note content (insert mode) |
| `Enter` | New line (insert mode) |
| `Backspace` | Delete character (insert mode) |

## Development Status

### Completed ✓
- [x] Project setup
- [x] Database layer (SQLite + SQLCipher)
- [x] Data models (Rust structs)
- [x] Encryption layer (AES-256-GCM + PBKDF2)
- [x] Repository pattern
- [x] TUI framework (ratatui)
- [x] Note management (create, edit, delete, list)
- [x] Password-based unlocking
- [x] Import/export (JSON)

### In Progress / Future
- [ ] Tag management
- [ ] Search functionality
- [ ] Recycle bin (soft delete recovery)
- [ ] Settings panel
- [ ] Sync client
- [ ] Attachment support
- [ ] Syntax highlighting in editor
- [ ] Auto-sync
- [ ] Keyboard shortcuts help screen

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
