# nixy - Use Nix Like Homebrew

[日本語版はこちら](README_ja.md)

**Start using Nix today, learn as you go.** Install packages with a single command, just like Homebrew.

```bash
nixy install -g ripgrep    # That's it. Nix made simple.
```

nixy gives you the power of Nix (reproducible builds, rollbacks, no dependency conflicts) with the simplicity of Homebrew. It's just a thin bash wrapper - no lock-in, no magic.

## Motivation

Nix is powerful, but the learning curve is steep. You want to learn Nix, but writing flake.nix just to install a package feels overwhelming at first.

nixy makes the transition easier. Start with simple commands, and the generated `flake.nix` teaches you Nix patterns along the way. You get all the benefits of Nix:
- **Reproducibility**: Same packages, same versions, everywhere
- **No conflicts**: Different projects can use different versions of the same tool
- **Atomic upgrades**: Updates either fully succeed or nothing changes
- **Rollbacks**: Instantly revert if an upgrade breaks something
- **Cross-platform**: Same workflow on macOS and Linux

Start with `nixy install -g <package>`, then read the generated flake.nix when you're ready to learn more.

## How it works

nixy uses plain Nix features - no Home Manager, no NixOS, no complex setup:

- **Global packages (`-g`)**: `flake.nix` + `nix profile` at `~/.config/nix/`
- **Project packages**: Just a `flake.nix` in your project directory

nixy edits the flake.nix and runs standard `nix` commands. The flake.nix it generates is plain Nix - you can read it, edit it manually, or use `nix` commands directly anytime.

## Homebrew vs nixy

| Homebrew | nixy |
|----------|------|
| `brew install ripgrep` | `nixy install -g ripgrep` |
| `brew uninstall ripgrep` | `nixy uninstall -g ripgrep` |
| `brew list` | `nixy list -g` |
| `brew search git` | `nixy search git` |
| `brew upgrade` | `nixy upgrade -g` |

Same simplicity, but with Nix's reliability underneath. No lock-in - it's just standard Nix.

## Quick Start

### 1. Install Nix (if you haven't)

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

### 2. Install nixy

```bash
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash
```

### 3. Start using it like Homebrew

```bash
nixy install -g ripgrep    # First run auto-creates ~/.config/nix/flake.nix
nixy install -g nodejs
nixy install -g git

nixy list -g               # See what's installed
nixy search python         # Find packages
nixy uninstall -g nodejs   # Remove a package
nixy upgrade -g            # Upgrade all packages
```

The `-g` (or `--global`) flag works like Homebrew - packages are installed globally and available in all terminal sessions.

## Commands

| Command | Description |
|---------|-------------|
| `nixy install -g <pkg>` | Install a package globally |
| `nixy uninstall -g <pkg>` | Uninstall a package |
| `nixy list -g` | List installed packages |
| `nixy search <query>` | Search for packages |
| `nixy upgrade -g` | Upgrade all packages |
| `nixy gc` | Clean up old package versions |

## Sync Across Machines

Your package list is just a text file (`~/.config/nix/flake.nix`). Back it up, version control it, or sync it with dotfiles:

```bash
# Back up your package list
cp ~/.config/nix/flake.nix ~/dotfiles/

# On a new machine:
mkdir -p ~/.config/nix
cp ~/dotfiles/flake.nix ~/.config/nix/
nixy sync -g    # Installs everything from flake.nix
```

Same packages, same versions, on every machine.

---

## Advanced: Per-Project Packages

For developers who want project-specific dependencies (like a `package.json` but for any tools):

```bash
cd my-project
nixy init              # Create a flake.nix in this directory
nixy install nodejs    # Install packages for this project only
nixy install postgres

nixy shell             # Enter a shell with these packages available
```

Without `-g`, nixy automatically finds and uses the nearest `flake.nix` in parent directories (similar to how git finds `.git`).

### Sharing Project Environment

```bash
# Commit flake.nix to your repo
git add flake.nix flake.lock

# Teammates can get the same environment:
git clone my-project && cd my-project
nixy sync              # Install all project packages
```

### Additional Commands for Projects

| Command | Description |
|---------|-------------|
| `nixy init` | Create a flake.nix in current directory |
| `nixy shell` | Enter dev shell with project packages |
| `nixy sync` | Install packages from existing flake.nix |

All commands support `-g` or `--global` to use global packages instead of project-local.

---

## FAQ

**Can I use Homebrew and nixy together?**
Yes. They don't conflict. You can migrate gradually or use both.

**How do I find the right package name?**
Use `nixy search <keyword>`. Package names sometimes differ from what you expect (e.g., `ripgrep` not `rg`).

**Where are packages actually installed?**
In the Nix store (`/nix/store/`). nixy just manages which packages are available in your PATH.

**Can I edit the flake.nix manually?**
Yes. It's standard Nix. nixy will preserve your manual changes outside the `# [nixy:...]` markers.

**How do I uninstall nixy?**
Just delete the `nixy` script. Your flake.nix files remain and work with standard `nix` commands.

---

## Appendix

### Custom Package Definitions

Install packages from custom nix files:

```bash
nixy install --file my-package.nix
```

Format for `my-package.nix`:
```nix
{
  name = "my-package";
  inputs = { overlay-name.url = "github:user/repo"; };
  overlay = "overlay-name.overlays.default";
  packageExpr = "pkgs.my-package";
}
```

### Config Locations

| Path | Description |
|------|-------------|
| `~/.config/nix/flake.nix` | Global packages (used with `-g`) |
| `./flake.nix` | Project-local packages |
| `~/.config/nix/packages/` | Custom package definitions |

### Environment Variables

| Variable | Default |
|----------|---------|
| `NIXY_CONFIG_DIR` | `~/.config/nix` |

### Limitations

- Package names use Nix naming (search with `nixy search` to find exact names)
- No GUI app support (like Homebrew Cask) yet
- Requires Nix with flakes enabled (the Determinate installer enables this by default)

## License

MIT
