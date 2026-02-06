# Jottery TUI - CLI Commands
# Use DEMO_LANG environment variable to set the output language (default: en-GB)
Env DEMO_LANG "en-GB"
Output public/screenshots/${DEMO_LANG}/tui-cli.gif

Set FontSize 18
Set Width 1400
Set Height 900
Set Theme "Catppuccin Mocha"
Set Shell "bash"

## Navigate to the executable directory
#Type "cd tui/target/release"
#Enter
#Sleep 500ms

# Run the help command
Type "jottery --help"
Enter
Sleep 2s

Type "jottery list"
Enter
Sleep 2s

Type "jottery search japan"
Enter
Sleep 2s

Type "jottery show d0e1f2a3 | less"
Enter
Sleep 4s
