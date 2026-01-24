# Jottery TUI - Interactive Interface
Output public/screenshots/en-GB/tui-interface.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Change to TUI directory
Type "cd tui/target/release"
Enter
Sleep 300ms
Type "pwd"
Enter
Sleep 300ms

# Start jottery TUI with default database
Type "./jottery"
Enter
Sleep 2s

# Navigate down 2 notes
Down
Sleep 400ms
Down
Sleep 400ms

# Open note in editor (vim)
Enter
Sleep 2s

# Navigate down 2 lines in vim
Down
Down
Sleep 500ms

# Insert text
Type "i"
Enter
Sleep 200ms
Type "hello from $EDITOR!"
Enter
Up
Sleep 1s

# Delete the text
Escape
Sleep 300ms
Type "dd"
Sleep 500ms

# Save and quit
Type ":wq"
Enter
Sleep 2s
