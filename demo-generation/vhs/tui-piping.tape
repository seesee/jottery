# Jottery TUI - Piping Content
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Run via generate-tui-demo.sh which sets environment variables
#
# Output path is overridden by --output flag from wrapper script
Output tui-piping.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

# Set environment variables (hidden from recording)
Hide
Type "export LANG=$LANG LC_ALL=$LANG JOTTERY_DB_PATH=$JOTTERY_DB_PATH"
Enter
Type "clear"
Enter
Show
Sleep 500ms

# Example 1: Echo a quick note
Type@50ms "echo '$DEMO_NOTE' | jottery note -t reminder,demo"
Enter
Sleep 2s

# Example 2: Capture command output
Type@50ms "date | jottery note -t timestamp,demo"
Enter
Sleep 2s

# Example 3: Capture command output
Type@50ms "ls -la | jottery note -t demo,directory-listing"
Enter
Sleep 2s

# Show the created notes
Type@50ms "jottery note -t demo,editor"
Enter
Sleep 500ms
Type "a"
Type "$DEMO_TEXT"
Escape
Sleep 1s
Type ":wq"
Enter

Sleep 1s

Type@50ms "jottery list -t demo"
Enter
Sleep 5s
