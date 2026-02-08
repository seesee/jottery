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

# Show sync help
Type@50ms "$DEMO_SYNC_HELP"
Enter
Type@50ms "LANG=$LANG jottery sync --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show register-device help
Type@50ms "$DEMO_REGISTER"
Enter
Type@50ms "LANG=$LANG jottery register-device --help"
Enter
Sleep 3s

# Clear
Ctrl+L
Sleep 500ms

# Show example of sync command
Type@50ms "$DEMO_FORCE_SYNC"
Enter
Sleep 500ms
Type@50ms "LANG=$LANG jottery --database $JOTTERY_DB_PATH sync"
Enter
Sleep 3s
