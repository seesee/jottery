# Jottery TUI - Interactive Interface
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets environment variables
#
# Output path is overridden by --output flag from wrapper script
Output tui-interface.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Start jottery TUI with locale set inline
Type@50ms "LANG=$LANG jottery --database $JOTTERY_DB_PATH"
Sleep 2s
Enter
Sleep 2s

# Navigate down 2 notes
Down
Sleep 500ms
Down
Sleep 500ms

# Open note in editor (vim)
Enter
Sleep 2s

# Navigate down 2 lines in vim
Down
Down
Down
Down
Down

# Insert text (DEMO_TEXT is set by generate script)
Type "i"
Enter
Sleep 200ms
Type "- [ ] $DEMO_TEXT"
Enter
Up
Sleep 2s

# Delete the text
Escape
Sleep 300ms
Type "d"
Up
Sleep 500ms

# Save and quit
Type ":q!"
Enter
Sleep 2s

# Search for travel note (DEMO_SEARCH is set by generate script)
Type "/"
Sleep 500ms
Type "$DEMO_SEARCH"
Sleep 500ms
Down
Down
Sleep 2s
Escape
Type "y"
Sleep 1s
Type "q"
