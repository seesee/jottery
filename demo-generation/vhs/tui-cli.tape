# Jottery TUI - CLI Commands
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets paths correctly
#
# Output path is overridden by --output flag from wrapper script
Output tui-cli.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Run the help command
Type@50ms "jottery --help"
Enter
Sleep 2s

# List notes (uses isolated demo database via DB_PATH env var)
Type@50ms "jottery --database $DB_PATH list"
Enter
Sleep 2s

# Search notes
Type@50ms "jottery --database $DB_PATH search japan"
Enter
Sleep 2s

# Show a specific note
Type@50ms "jottery --database $DB_PATH show d0e1f2a3 | less"
Enter
Sleep 4s
