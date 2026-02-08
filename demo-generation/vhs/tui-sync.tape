# Jottery TUI - Sync Commands
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets environment variables
#
# Output path is overridden by --output flag from wrapper script
Output tui-sync.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Set environment variables (hidden from recording)
Hide
Type "export LANG=$LANG LC_ALL=$LANG JOTTERY_DB_PATH=$JOTTERY_DB_PATH"
Enter
Type "clear"
Enter
Show
Sleep 500ms

# Show sync help
Type@50ms "$DEMO_SYNC_HELP"
Enter
Type@50ms "jottery sync --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show register-device help
Type@50ms "$DEMO_REGISTER"
Enter
Type@50ms "jottery register-device --help"
Enter
Sleep 3s

# Clear
Ctrl+L
Sleep 500ms

# Show example of sync command
Type@50ms "$DEMO_FORCE_SYNC"
Enter
Sleep 500ms
Type@50ms "jottery sync"
Enter
Sleep 3s
