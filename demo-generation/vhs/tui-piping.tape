# Jottery TUI - Piping Content
# Use DEMO_LANG environment variable to set the output language (default: en-GB)
#
# Prerequisites:
#   1. Run setup-tui-demo.sh to create isolated demo databases
#   2. Set DEMO_LANG before running: DEMO_LANG=ja vhs tui-piping.tape
#
Env DEMO_LANG "en-GB"
Env DEMO_ROOT ""
Env DB_FLAG "--database ${DEMO_ROOT}demo-generation/tui-demo/${DEMO_LANG}/jottery.db"
Output ${DEMO_ROOT}public/screenshots/${DEMO_LANG}/tui-piping.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"


# Example 1: Echo a quick note
Type "echo 'Quick reminder - Update documentation' | jottery ${DB_FLAG} note -t reminder,demo"
Enter
Sleep 2s

# Example 2: Capture command output
Type "date | jottery ${DB_FLAG} note -t timestamp,demo"
Enter
Sleep 2s

# Example 3: Capture command output
Type "ls -la | jottery ${DB_FLAG} note -t demo,directory-listing"
Enter
Sleep 2s

# Show the created notes
Type "jottery ${DB_FLAG} note -t demo,editor"
Enter
Sleep 500ms
Type "a"
Type "Create a new Jottery note in $EDITOR without invoking the TUI"
Escape
Sleep 1s
Type ":wq"
Enter

Sleep 1s

Type "jottery ${DB_FLAG} list -t demo"
Enter
Sleep 5s
