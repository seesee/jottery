# Jottery TUI - Interactive Interface
Output public/screenshots/en-GB/tui-interface.gif

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

# Insert text
Type "i"
Enter
Sleep 200ms
Type "hello from $EDITOR!"
Enter
Up
Sleep 2s

# Delete the text
Escape
Sleep 300ms
Type "d2d"
Sleep 500ms

# Save and quit
Type ":wq"
Enter
Sleep 2s
