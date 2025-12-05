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

### From Source

```bash
cd tui
cargo build --release
sudo cp target/release/jottery /usr/local/bin/
```

## Usage

```bash
# Start the TUI
jottery

# Show help
jottery --help

# Enable debug logging
RUST_LOG=jottery_tui=debug jottery
```

## Configuration

Configuration is stored in:
- **Linux**: `~/.config/jottery/config.toml`
- **macOS**: `~/Library/Application Support/jottery/config.toml`
- **Windows**: `%APPDATA%\jottery\config.toml`

Database is stored in the same directory as `jottery.db`.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `?` | Show help |
| `n` | New note |
| `e` | Edit note |
| `d` | Delete note |
| `/` | Search |
| `t` | Edit tags |
| `p` | Toggle pin |
| `s` | Sync now |
| `,` | Settings |
| `q` | Quit |
| `Esc` | Cancel/Go back |

## Development Status

This is a work in progress. Feature parity with the web application is the goal.

### Completed
- [x] Project setup

### In Progress
- [ ] Database layer
- [ ] Encryption layer
- [ ] TUI framework
- [ ] Note management
- [ ] Sync client

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
