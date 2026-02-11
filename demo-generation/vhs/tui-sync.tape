# Jottery TUI - Sync Commands
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets environment variables
#   3. Demo sync server must be running (start-demo-server.sh --bg)
#
# Environment variables used:
#   JOTTERY_DB_PATH - path to demo database
#   DEMO_PASSWORD - encryption password for database
#   DEMO_SYNC_HELP - localised "Show sync help" comment
#   DEMO_REGISTER - localised "Register device" comment
#   DEMO_FORCE_SYNC - localised "Force sync" comment
#
# Demo server credentials:
#   Email: demo@example.com
#   Password: demo-password-2026
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
Sleep 3s

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

# Actually register the device with the demo server
Type@50ms "# Register device with sync server"
Enter
Sleep 500ms

# Use non-interactive registration with all args provided (single line)
Type@40ms "jottery register-device -s http://localhost:3030 -e demo@example.com -p demo-password-2026 -n Demo -k demo-password-2026"
Enter
Sleep 5s

# Clear
Ctrl+L
Sleep 500ms

# Now actually run sync
Type@50ms "$DEMO_FORCE_SYNC"
Enter
Sleep 500ms
Type@50ms "jottery sync"
Enter
Sleep 4s

