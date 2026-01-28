# VHS tape file for nixy demo
# Run: rm -rf /tmp/nixy-demo-* && vhs demo.tape

Output demo.gif

Set FontSize 13
Set Width 800
Set Height 600
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set TypingSpeed 50ms

# Hidden setup
# Hide
# Type "export NIXY_CONFIG_DIR=/tmp/nixy-demo-config NIXY_ENV=/tmp/nixy-demo-env"
# Enter
# Show

Type "# Add packages like Homebrew"
Enter
Sleep 500ms

Type "nixy add ripgrep"
Enter
Wait

Type "nixy add fzf"
Enter
Wait

Type "# See what's added"
Enter
Sleep 500ms

Type "nixy list"
Enter
Wait

Type "# Search for packages"
Enter
Sleep 500ms

Type "nixy search vhs"
Enter
Wait

Type "# Create a new profile"
Enter
Sleep 500ms

Type "nixy profile work -c"
Enter
Wait

Type "# Switch to a profile"
Enter
Sleep 500ms

Type "nixy profile default"
Enter
Wait
