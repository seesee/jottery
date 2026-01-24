# Jottery TUI - Piping Content
Output public/screenshots/en-GB/tui-piping.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"


# Example 1: Echo a quick note
Type "echo 'Quick reminder - Update documentation' | jottery note -t reminder,demo"
Enter
Sleep 2s

# Example 2: Capture command output
Type "date | jottery note -t timestamp,demo"
Enter
Sleep 2s

# Example 3: Capture command output
Type "ls -la | jottery note -t demo,directory-listing"
Enter
Sleep 2s

# Show the created notes
Type "jottery note -t demo,editor"
Enter
Sleep 500ms
Type "a"
Type "Create a new Jottery note in $EDITOR without invoking the TUI"
Escape
Sleep 1s
Type ":wq"
Enter

Sleep 1s

Type "jottery list -t demo"
Enter
Sleep 5s
