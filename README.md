# nixy - Simple Declarative Nix Package Management

[日本語版はこちら](README_ja.md)

![nixy demo](demo.gif)

**Reproducible Nix packages, simple commands.** Install packages with a single command, sync them across all your machines.

```bash
nixy install ripgrep    # That's it. Nix made simple.
```

nixy manages your Nix packages through a declarative `flake.nix`. Unlike `nix profile` which lacks built-in reproducibility, nixy ensures the same packages and versions on every machine. It's just a thin bash wrapper - no lock-in, no magic.

## Motivation

Nix is powerful, but managing packages shouldn't be complicated. You want the benefits of Nix without writing flake files by hand.

nixy gives you:
- **Simple commands**: `nixy install`, `nixy uninstall`, `nixy upgrade` - that's it
- **Reproducibility**: Same packages, same versions, on every machine
- **No hidden state**: Your `flake.nix` is the single source of truth
- **Atomic upgrades & rollbacks**: Updates either fully succeed or nothing changes
- **Cross-platform**: Same workflow on macOS and Linux
- **Multiple profiles**: Separate package sets for work, personal, projects

Unlike `nix profile`, nixy uses `flake.nix` + `flake.lock` for full reproducibility. Copy your config to a new machine, run `nixy sync`, done.

## How it works

nixy uses plain Nix features - no Home Manager, no NixOS, no complex setup. Your packages are defined in `flake.nix` at `~/.config/nixy/profiles/<name>/`, built with `nix build`.

nixy is **purely declarative** - your `flake.nix` is the single source of truth. Unlike `nix profile` which maintains mutable state, nixy uses `nix build --out-link` to create a symlink (`~/.local/state/nixy/env`) pointing to your built environment. This means:
- No hidden profile state to get out of sync
- What's in `flake.nix` is exactly what's installed
- Easy to understand, debug, and version control

nixy edits the flake.nix and runs standard `nix` commands. The flake.nix it generates is plain Nix - you can read it, edit it manually, or use `nix` commands directly anytime.

## Why not nix profile?

`nix profile` is the standard Nix tool for imperative package management. It works well for single-machine use, but falls short for reproducibility:

| | nix profile | nixy |
|---|-------------|------|
| Package list | Hidden in `manifest.json` | Readable `flake.nix` |
| Version locking | Per-package only, no unified lock | Single `flake.lock` for all packages |
| Sync to new machine | Manual re-installation | `nixy sync` |
| Rollback | Profile generations only | Git + `flake.lock` |

If you only use one machine and don't need reproducibility, `nix profile` is simpler. If you want the same environment everywhere, use nixy.

## Quick Start

### 1. Install Nix (if you haven't)

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

### 2. Install nixy

```bash
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash
```

### 3. Set up your shell

Add to your shell config (`.bashrc`, `.zshrc`, etc.):

```bash
eval "$(nixy config zsh)"
```

For fish, add to `~/.config/fish/config.fish`:

```fish
nixy config fish | source
```

### 4. Start installing packages

```bash
nixy install ripgrep    # First run auto-creates the default profile
nixy install nodejs
nixy install git

nixy list               # See what's installed
nixy search python      # Find packages
nixy uninstall nodejs   # Remove a package
nixy upgrade            # Upgrade all inputs
nixy upgrade nixpkgs    # Upgrade only nixpkgs
```

Packages are installed globally and available in all terminal sessions.

## Commands

| Command | Description |
|---------|-------------|
| `nixy install <pkg>` | Install a package globally |
| `nixy uninstall <pkg>` | Uninstall a package |
| `nixy list` | List packages in flake.nix |
| `nixy search <query>` | Search for packages |
| `nixy upgrade [input...]` | Upgrade all inputs or specific ones |
| `nixy sync` | Build environment from flake.nix (for new machines) |
| `nixy gc` | Clean up old package versions |
| `nixy config <shell>` | Output shell config (for PATH setup) |
| `nixy version` | Show nixy version |
| `nixy self-upgrade` | Upgrade nixy to the latest version |
| `nixy profile` | Show current profile |
| `nixy profile create <name>` | Create a new profile |
| `nixy profile switch <name>` | Switch to a different profile |
| `nixy profile list` | List all profiles |
| `nixy profile delete <name>` | Delete a profile (requires `--force`) |

## Sync Across Machines

Your package list is just a text file. Back it up, version control it, or sync it with dotfiles:

```bash
# Back up your package list (default profile)
cp ~/.config/nixy/profiles/default/flake.nix ~/dotfiles/

# On a new machine:
mkdir -p ~/.config/nixy/profiles/default
cp ~/dotfiles/flake.nix ~/.config/nixy/profiles/default/
nixy sync    # Installs everything from flake.nix
```

Same packages, same versions, on every machine.

## Multiple Profiles

Maintain separate package sets for different contexts (work, personal, projects):

```bash
nixy profile create work      # Create a new profile
nixy profile switch work      # Switch to it
nixy install slack terraform  # Install work-specific packages

nixy profile create personal  # Create another profile
nixy profile switch personal
nixy install spotify games    # Different packages here

nixy profile list             # See all profiles
nixy profile                  # Show current profile
```

Each profile has its own `flake.nix` at `~/.config/nixy/profiles/<name>/`. Switching profiles rebuilds the environment symlink to point to that profile's packages.

**Use cases:**
- **Work vs Personal**: Keep work tools separate from personal apps
- **Client projects**: Different toolchains for different clients
- **Experimentation**: Try new packages without affecting your main setup

---

## FAQ

**How do I find the right package name?**
Use `nixy search <keyword>`. Package names sometimes differ from what you expect (e.g., `ripgrep` not `rg`).

**Where are packages actually installed?**
In the Nix store (`/nix/store/`). nixy builds a combined environment and creates a symlink at `~/.local/state/nixy/env` pointing to it. The `nixy config` command sets up your PATH to include this location.

**Can I edit the flake.nix manually?**
Yes! nixy provides custom markers where you can add your own inputs, packages, and paths that will be preserved during regeneration:

```nix
# [nixy:custom-inputs]
my-overlay.url = "github:user/my-overlay";
# [/nixy:custom-inputs]
```

Any content outside these markers will be overwritten when nixy regenerates the flake. For heavy customization, see "Customizing flake.nix" in the Appendix.

**How do I update nixy?**
Run `nixy self-upgrade`. It checks for updates, downloads the latest version, and replaces itself. Use `--force` to reinstall even if already up to date.

**How do I uninstall nixy?**
Just delete the `nixy` script. Your flake.nix files remain and work with standard `nix` commands.

**Why not use `nix profile` directly?**
`nix profile` lacks built-in reproducibility - there's no official way to export your packages and recreate the same environment on another machine. nixy uses `flake.nix` as the source of truth, which can be copied, version-controlled, and shared.

---

## Appendix

### Customizing flake.nix

nixy provides custom markers where you can add your own content that will be preserved when nixy regenerates the flake:

**Custom inputs** - Add your own flake inputs:
```nix
# [nixy:custom-inputs]
my-overlay.url = "github:user/my-overlay";
home-manager.url = "github:nix-community/home-manager";
# [/nixy:custom-inputs]
```

**Custom packages** - Add custom package definitions:
```nix
# [nixy:custom-packages]
my-tool = pkgs.writeShellScriptBin "my-tool" ''echo "Hello"'';
patched-app = pkgs.app.overrideAttrs { ... };
# [/nixy:custom-packages]
```

**Custom paths** - Add extra paths to the buildEnv:
```nix
# [nixy:custom-paths]
my-tool
patched-app
# [/nixy:custom-paths]
```

If you edit content **outside** these markers, nixy will warn you before overwriting:
```
Warning: flake.nix has modifications outside nixy markers.
Use --force to proceed (custom changes will be lost).
```

### For Existing Nix Users

If you already manage your own `flake.nix` and want to use nixy's package list, you can import it:

```nix
{
  inputs.nixy.url = "path:~/.config/nixy";

  outputs = { self, nixpkgs, nixy }: {
    # nixy.packages.<system>.default is a buildEnv with all nixy packages
    # You can use it as a dependency or merge it with your own environment
  };
}
```

This way, nixy manages your package list while you maintain full control of your flake.

### Coexistence with nix profile

nixy and `nix profile` use separate paths and don't conflict:
- nixy: `~/.local/state/nixy/env/bin`
- nix profile: `~/.nix-profile/bin`

If you have both in your PATH, the one listed first takes precedence for packages installed in both. You can use both tools for different purposes.

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
| `~/.config/nixy/profiles/<name>/flake.nix` | Profile packages |
| `~/.config/nixy/active` | Current active profile name |
| `~/.config/nixy/profiles/<name>/packages/` | Custom package definitions for profile |
| `~/.local/state/nixy/env` | Symlink to built environment (add `bin/` to PATH) |
| `~/.config/nixy/flake.nix` | Legacy location (auto-migrated to default profile) |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NIXY_CONFIG_DIR` | `~/.config/nixy` | Location of global flake.nix |
| `NIXY_ENV` | `~/.local/state/nixy/env` | Symlink to built environment |

### Limitations

- Package names use Nix naming (search with `nixy search` to find exact names)
- No GUI app support (like Homebrew Cask) yet
- Requires Nix with flakes enabled (the Determinate installer enables this by default)

## License

MIT
