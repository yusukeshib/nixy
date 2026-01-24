# VHS tape file for nixy demo
# Run: rm -rf /tmp/nixy-demo-* && vhs demo.tape

Output demo.gif

Set FontSize 16
Set Width 800
Set Height 500
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set TypingSpeed 50ms

# Hidden setup
Hide
Type "export NIXY_CONFIG_DIR=/tmp/nixy-demo-config NIXY_PROFILE=/tmp/nixy-demo-profile"
Enter
Type "alias nixy='./nixy'"
Enter
Sleep 500ms
Show

Type "# Install packages like Homebrew"
Enter
Sleep 500ms

Type "nixy install -g ripgrep"
Enter
Sleep 6s

Type "nixy install -g fzf"
Enter
Sleep 5s

Type "# See what's installed"
Enter
Sleep 500ms

Type "nixy list -g"
Enter
Sleep 2s

Type "# Search for packages"
Enter
Sleep 500ms

Type "nixy search nodejs"
Enter
Sleep 3s

Type "# Check the generated flake.nix"
Enter
Sleep 500ms

Type "head -30 $NIXY_CONFIG_DIR/flake.nix"
Enter
Sleep 3s
