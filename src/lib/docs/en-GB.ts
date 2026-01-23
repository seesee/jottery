// English (British) documentation
export const documentation = `# Jottery Documentation

## Table of Contents

- [Getting Started](#getting-started)
- [Creating & Editing Notes](#creating-editing-notes)
- [Syntax Highlighting](#syntax-highlighting)
- [Calculator Mode](#calculator-mode)
- [Search](#search)
  - [Basic Search](#basic-search)
  - [Tag Search](#tag-search)
  - [Advanced Search Modifiers](#advanced-search-modifiers)
- [Multi-Select & Bulk Operations](#multi-select-bulk-operations)
- [Archive Mode](#archive-mode)
- [Categories](#categories)
- [Version History](#version-history)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Sync](#sync)
- [Security & Privacy](#security-privacy)
- [Import & Export](#import-export)

---

## Getting Started

Jottery is a privacy-focused, encrypted note-taking application. All your notes are encrypted locally using **AES-256-GCM** encryption before being stored.

> **Important:** Your password is the encryption key. If you lose it, your notes cannot be recovered. There is no password reset functionality.

---

## Creating & Editing Notes

| Action | How to do it |
|--------|--------------|
| **Create a note** | Click "+ New Note" or press \`Alt+N\` |
| **Edit a note** | Click on a note in the list to open it |
| **Auto-save** | Changes are automatically saved as you type |
| **Close a note** | Press \`Escape\` or click another note |
| **Pin a note** | Click the pin icon to keep it at the top |
| **Delete a note** | Click the menu (⋮) and select "Delete" |

---

## Syntax Highlighting

Use the language dropdown in the editor to enable syntax highlighting. Supported languages include:

- **Markdown** - with live preview and code block highlighting
- **JavaScript/TypeScript** - ES6+ syntax support
- **Python** - including f-strings and decorators
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - interactive math expressions

---

## Calculator Mode

Set the syntax language to **Calc** to use the interactive calculator. Each line is evaluated as a mathematical expression, with results shown inline.

### Features

- **Basic arithmetic:** \`2 + 3 * 4\` → \`14\`
- **Variables:** \`x = 10\` then \`x * 2\` → \`20\`
- **Constants:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Functions:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Power:** \`2^10\` or \`2**10\` → \`1024\`
- **Factorial:** \`5!\` → \`120\`
- **Comments:** Lines starting with \`#\` are ignored

### Available Functions

| Category | Functions |
|----------|-----------|
| **Basic** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Powers** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometry** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hyperbolic** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Example

\`\`\`
# Calculate compound interest
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Search

### Basic Search

Type in the search box to find notes. The search looks at both note content and tags.

| Syntax | Description |
|--------|-------------|
| \`word\` | Notes containing "word" |
| \`word1 word2\` | Notes containing both words (AND) |
| \`"exact phrase"\` | Notes containing the exact phrase |
| \`-word\` | Exclude notes containing "word" |

### Tag Search

| Syntax | Description |
|--------|-------------|
| \`#tagname\` | Notes with this tag |
| \`#tag1 #tag2\` | Notes with both tags (AND) |
| \`#tag1 \\| #tag2\` | Notes with either tag (OR) |

### Advanced Search Modifiers

| Modifier | Description | Example |
|----------|-------------|---------|
| \`has:attachment\` | Notes with attachments | \`has:attachment\` |
| \`created:>DATE\` | Created after date | \`created:>2024-01-01\` |
| \`created:<DATE\` | Created before date | \`created:<2024-06-30\` |
| \`created:DATE..DATE\` | Created in date range | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATE\` | Modified after date | \`modified:>2024-01-01\` |
| \`modified:<DATE\` | Modified before date | \`modified:<2024-06-30\` |
| \`words:>N\` | More than N words | \`words:>100\` |
| \`words:<N\` | Fewer than N words | \`words:<50\` |
| \`words:N..M\` | Word count in range | \`words:50..200\` |
| \`category:NAME\` | Notes or tags with category | \`category:red\` |
| \`category:NAME note\` | Notes with category | \`category:blue note\` |
| \`category:NAME tag\` | Tags with category | \`category:green tag\` |

**Combining modifiers:** \`#project category:blue has:attachment modified:>2024-01-01 words:>100\`

---

## Multi-Select & Bulk Operations

Select multiple notes to perform bulk actions.

### Selecting Notes

| Action | How to do it |
|--------|--------------|
| **Toggle selection** | \`Ctrl/Cmd + Click\` on a note |
| **Range selection** | \`Shift + Click\` to select from last selected |
| **Select all visible** | Click "Select All" in the toolbar |
| **Clear selection** | Press \`Escape\` or click "Cancel" |

### Bulk Actions

When notes are selected, a toolbar appears at the bottom with these options:

- **Add Tags** - Add tags to all selected notes
- **Remove Tags** - Remove specific tags from selected notes
- **Export** - Export selected notes as JSON
- **Combine** - Merge selected notes into one (ordered by creation date)
- **Delete** - Move selected notes to recycle bin

---

## Archive Mode

Archive notes to declutter your main view whilst keeping them accessible. Archived notes are hidden from the default view but remain searchable and fully functional.

### Using Archive

| Action | How to do it |
|--------|--------------|
| **Archive a note** | Click ⋮ menu → "Archive" or press \`Shift + A\` |
| **Toggle archive view** | Click "Archive" button in header or press \`Shift + A\` again |
| **Search archived notes** | Use archive mode or search normally (archived results shown separately) |
| **Unarchive a note** | In archive mode, click ⋮ menu → "Unarchive" |

### Archive vs Delete

- **Archived notes** remain fully functional - you can edit, tag, and search them
- **Deleted notes** go to the recycle bin and are excluded from searches
- Archive is for notes you want to keep but don't need to see regularly
- Delete is for notes you're considering removing permanently

### Cross-Mode Search

When searching with ≤10 results, Jottery shows a count of matches in the opposite mode (active/archived). Click to switch modes and see those results.

---

## Categories

Organise notes and tags visually with categories. Each category has a distinct colour that helps you quickly identify related content. Note categories affect the note list background, whilst tag categories affect tag pills.

### Note Categories

| Action | How to do it |
|--------|--------------|
| **Set note category** | Click ⋮ menu → "Set Colour" and choose from 8 categories |
| **Remove category** | Select "No Colour" from the picker |
| **Bulk categorise** | Select multiple notes → "Set Colour" in bottom toolbar |

Categories available: **red**, **orange**, **yellow**, **green**, **blue**, **purple**, **pink**, **gray**

### Tag Categories

| Action | How to do it |
|--------|--------------|
| **Configure** | Settings → Colours → Tag Colours section |
| **Assign category** | Select a tag and choose a category colour |
| **Global mapping** | All notes with the same tag show the same category colour |

### Customising Category Colours

In Settings → Colours, you can customise the colour for each category:

- Each category has separate colour values for light and dark modes
- Colours are shown as hex codes (e.g., \`#FFE5E5\`)
- Click "Reset to Defaults" to restore the original palette
- Changes apply immediately across all notes and tags

### Category Search

Search for notes by category using the category name:

| Syntax | Description |
|--------|-------------|
| \`category:red\` | Notes OR tags in the red category |
| \`category:red note\` | Only notes in the red category |
| \`category:red tag\` | Only tags in the red category |

You can search by any category name: \`red\`, \`orange\`, \`yellow\`, \`green\`, \`blue\`, \`purple\`, \`pink\`, \`gray\`

Combine with other search terms: \`#project category:blue has:attachment\`

---

## Version History

Jottery automatically creates version snapshots when syncing notes.

| Action | How to do it |
|--------|--------------|
| **Open history** | Click ⋮ menu → "Version History" or press \`Alt+H\` |
| **View version** | Click on a version to see its content |
| **Compare** | Differences are highlighted automatically |
| **Restore** | Click "Restore" to revert to a previous version |

---

## Keyboard Shortcuts

All keyboard shortcuts are customisable in Settings → Keyboard Shortcuts.

### Default Shortcuts

| Shortcut | Action |
|----------|--------|
| \`Ctrl/Cmd + K\` | Focus search |
| \`Alt + N\` | Create new note |
| \`Ctrl/Cmd + Z\` | Undo |
| \`Ctrl/Cmd + Shift + Z\` | Redo |
| \`Alt + H\` | Version history |
| \`Alt + I\` | Note info |
| \`Escape\` | Close note / Clear selection |
| \`Ctrl/Cmd + ,\` | Open settings |

### Multi-Select Shortcuts

| Shortcut | Action |
|----------|--------|
| \`Ctrl/Cmd + Click\` | Toggle note selection |
| \`Shift + Click\` | Range select |
| \`Ctrl/Cmd + A\` | Select all filtered notes |

---

## Sync

Jottery supports self-hosted sync across devices.

### Setup

1. Go to **Settings → Sync**
2. Enter your self-hosted server URL
3. **First device:** Click "Register Device" to create sync credentials
4. **Other devices:** Use "Use Existing Credentials" with your sync credentials

> **Important:** All devices must use the **same password** to decrypt notes. The password is never sent to the server.

### How It Works

- Notes are encrypted **before** leaving your device
- The server only stores encrypted data
- Sync happens automatically when online
- Conflicts are resolved using last-write-wins

---

## Security & Privacy

| Feature | Description |
|---------|-------------|
| **Encryption** | AES-256-GCM for all note content and tags |
| **Local encryption** | All encryption happens in your browser |
| **Password** | Never stored or transmitted |
| **Auto-lock** | Protects notes when idle (default: 15 minutes) |
| **No tracking** | Zero analytics or third-party scripts |
| **Open source** | Full source code available on GitHub |

> **Tip:** Use a password manager to generate and store a strong, unique password for Jottery. Since there's no password recovery, losing your password means losing access to your notes permanently.

### Changing Your Password

Since your password is the encryption key, there's no direct way to change it. However, you can effectively change your password by:

1. **Export** all your notes (Settings → Import/Export → Export)
2. **Clear** your local data or use a new browser/device
3. **Set up** Jottery with your new password
4. **Import** your exported notes

Your notes will be re-encrypted with the new password.

---

## Import & Export

### Export

1. Go to **Settings → Import/Export**
2. Click "Export All Notes"
3. Choose a location to save the JSON file

> **Warning:** Exports are **unencrypted**. Store them securely!

### Import

1. Go to **Settings → Import/Export**
2. Click "Import Notes"
3. Select a previously exported JSON file
4. Notes will be merged with existing data (duplicates are skipped)

### Bulk Export

Select multiple notes and click "Export" to export only selected notes.
`;
