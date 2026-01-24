# Jottery TUI - Piping Content
Output public/screenshots/en-GB/tui-piping.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Navigate to executable directory
Type "cd tui/target/release"
Enter
Sleep 500ms

# Example 1: Echo a quick note
Type "echo 'Quick reminder - Update documentation' | ./jottery note -d demo.db -p demo123"
Enter
Sleep 2s

# Example 2: List directory and capture as note
Type "ls -la | ./jottery note -d demo.db -p demo123 -t system,logs"
Enter
Sleep 2s

# Example 3: Capture command output
Type "date | ./jottery note -d demo.db -p demo123 -t timestamp"
Enter
Sleep 2s

# Show the created notes
Type "./jottery list -d demo.db -p demo123"
Enter
Sleep 3s
