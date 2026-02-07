# Jottery TUI - CLI Commands
# Use DEMO_LANG environment variable to set the output language (default: en-GB)
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Set DEMO_LANG before running: DEMO_LANG=ja vhs tui-cli.tape
#
Env DEMO_LANG "en-GB"
Env DEMO_ROOT ""
Env DB_FLAG "--database ${DEMO_ROOT}demo-generation/tui-demo/${DEMO_LANG}/jottery.db"
Output ${DEMO_ROOT}public/screenshots/${DEMO_LANG}/tui-cli.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Run the help command
Type "jottery --help"
Enter
Sleep 2s

# List notes (uses isolated demo database)
Type "jottery ${DB_FLAG} list"
Enter
Sleep 2s

# Search notes
Type "jottery ${DB_FLAG} search japan"
Enter
Sleep 2s

# Show a specific note
Type "jottery ${DB_FLAG} show d0e1f2a3 | less"
Enter
Sleep 4s
