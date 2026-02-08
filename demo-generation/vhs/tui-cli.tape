# Jottery TUI - CLI Commands
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets environment variables
#
# Output path is overridden by --output flag from wrapper script
Output tui-cli.gif

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

# Run the help command
Type@50ms "jottery --help"
Enter
Sleep 2s

# List notes
Type@50ms "jottery list"
Enter
Sleep 2s

# Search notes
Type@50ms "jottery search $DEMO_SEARCH"
Enter
Sleep 2s

# Show a specific note
Type@50ms "jottery show d0e1f2a3 | less"
Enter
Sleep 4s
