# Jottery TUI - Sync Commands
# Use DEMO_LANG environment variable to set the output language (default: en-GB)
Env DEMO_LANG "en-GB"
Output public/screenshots/${DEMO_LANG}/tui-sync.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Show sync help
Type "# Show Jottery Sync help"
Enter
Type "jottery sync --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show register-device help (shows cross-platform capabilities)
Type "# Register your client from the CLI"
Enter
Type "jottery register-device --help"
Enter
Sleep 3s

# Clear
Ctrl+L
Sleep 500ms

# Show example of checking sync (will fail but shows command)
Type "# Force a sync from the CLI"
Enter
Sleep 500ms
Type "jottery sync"
Enter
Sleep 3s
