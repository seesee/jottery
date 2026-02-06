# Jottery TUI - Interactive Interface
# Use DEMO_LANG environment variable to set the output language (default: en-GB)
Env DEMO_LANG "en-GB"
Output public/screenshots/${DEMO_LANG}/tui-interface.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Start jottery TUI with default database
Type "jottery"
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

# Insert text
Type "i"
Enter
Sleep 200ms
Type "- [ ] Hello from $EDITOR!"
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

Type "/"
Sleep 500ms
Type "japan"
Sleep 500ms
Down
Down
Sleep 2s
Escape
Type "y"
Sleep 1s
Type "q"
