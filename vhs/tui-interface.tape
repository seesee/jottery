# Jottery TUI - Interactive Interface
Output public/screenshots/tui-interface.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Navigate to executable directory
Type "cd tui/target/release"
Enter
Sleep 500ms

# Show the TUI list command with formatting
Type "./jottery list -d demo.db -p demo123"
Enter
Sleep 5s

# Clear
Ctrl+L
Sleep 500ms

# Show search command
Type "./jottery search -d demo.db -p demo123 'meeting'"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show a specific note
Type "./jottery show -d demo.db -p demo123 --help"
Enter
Sleep 3s
