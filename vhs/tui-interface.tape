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

# Show list command help first
Type "./jottery list --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show search command help
Type "./jottery search --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show show command help
Type "./jottery show --help"
Enter
Sleep 3s
