# Jottery TUI - Sync Commands
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets paths correctly
#
# Output path is overridden by --output flag from wrapper script
Output tui-sync.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Show sync help
Type@50ms "# Show Jottery Sync help"
Enter
Type@50ms "jottery sync --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show register-device help (shows cross-platform capabilities)
Type@50ms "# Register your client from the CLI"
Enter
Type@50ms "jottery register-device --help"
Enter
Sleep 3s

# Clear
Ctrl+L
Sleep 500ms

# Show example of checking sync (will fail but shows command)
Type@50ms "# Force a sync from the CLI"
Enter
Sleep 500ms
Type@50ms "jottery --database $DB_PATH sync"
Enter
Sleep 3s
