# Jottery TUI - Sync Commands
Output public/screenshots/tui-sync.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Navigate to executable directory
Type "cd tui/target/release"
Enter
Sleep 500ms

# Show sync help
Type "./jottery sync --help"
Enter
Sleep 4s

# Clear
Ctrl+L
Sleep 500ms

# Show register-device help (shows cross-platform capabilities)
Type "./jottery register-device --help"
Enter
Sleep 3s

# Clear
Ctrl+L
Sleep 500ms

# Show example of checking sync (will fail but shows command)
Type "# Cross-platform sync example"
Enter
Sleep 500ms
Type "./jottery sync -d demo.db -p demo123"
Enter
Sleep 3s
