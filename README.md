# nixy - Simple Declarative Nix Package Management

[日本語版はこちら](README_ja.md)

![nixy demo](demo.gif)

**Reproducible Nix packages, simple commands.** Install packages with a single command, sync them across all your machines.

```bash
nixy install ripgrep    # That's it. Nix made simple.
```

nixy manages your Nix packages through a declarative `flake.nix`. Unlike `nix profile` which lacks built-in reproducibility, nixy ensures the same packages and versions on every machine. Written in Rust for reliability and performance.

## Motivation

**For users frustrated with Homebrew, asdf, or similar tools** who want:
- Reproducible environments across machines (not just "it works on my machine")
- Atomic upgrades that never leave your system in a broken state
- A single lockfile for all packages (no more version drift)

**nixy is a package management layer on top of Nix.** It doesn't replace Nix's full capabilities (dev shells, builds, NixOS) - it focuses solely on managing globally installed packages, like Homebrew does.

### What nixy gives you:
- **Simple commands**: `nixy install`, `nixy uninstall`, `nixy upgrade`
- **True reproducibility**: `flake.nix` + `flake.lock` = identical environments everywhere
- **Multiple profiles**: Separate package sets for work, personal, projects
- **No lock-in**: Plain Nix underneath - eject anytime
- **Cross-platform**: Same workflow on macOS and Linux

### What nixy is NOT:
- A replacement for Home Manager or NixOS
- A development environment tool (use `nix develop` for that)
- A build system

If you want Homebrew's simplicity with Nix's reproducibility for your CLI tools, nixy is for you.

## How it works

nixy uses plain Nix features - no Home Manager, no NixOS, no complex setup. Your packages are defined in `flake.nix` at `~/.config/nixy/profiles/<name>/`, built with `nix build`.

nixy is **purely declarative** - your `packages.json` is the single source of truth, and `flake.nix` is fully regenerated from it on every operation. Unlike `nix profile` which maintains mutable state, nixy uses `nix build --out-link` to create a symlink (`~/.local/state/nixy/env`) pointing to your built environment. This means:
- No hidden profile state to get out of sync
- What's in `packages.json` is exactly what's installed
- Easy to understand, debug, and version control

The generated `flake.nix` is plain Nix - you can read it, inspect it, or use `nix` commands directly anytime. However, manual edits to `flake.nix` will be overwritten.

## nixy and nix profile

nixy is not a replacement for `nix profile` - it's a complement that adds reproducibility.

`nix profile` is great for quick, single-machine package management. nixy adds a declarative layer on top of Nix for when you need:

- **A unified lockfile**: All packages pinned to the same nixpkgs version
- **Easy sync**: Copy `packages.json` to a new machine, run `nixy sync`, done
- **Version-controlled config**: `packages.json` + `flake.lock` are designed for git

nixy and `nix profile` use separate paths (`~/.local/state/nixy/env` vs `~/.nix-profile`) and don't interfere with each other. Use `nix profile` for quick experiments, nixy for your reproducible base environment - or use both together.

## Comparison with Other Tools

### vs devbox

[devbox](https://github.com/jetify-com/devbox) is a **development environment tool** - think of it as a replacement for asdf, nvm, or pyenv. It manages per-project dependencies and isolated shells.

nixy is a **package manager** - think of it as a replacement for Homebrew. It manages your globally installed CLI tools.

Different tools for different jobs.

### vs home-manager

[home-manager](https://github.com/nix-community/home-manager) manages your entire home directory - dotfiles, services, and packages. It's powerful but requires learning Nix.

nixy only manages packages. If you want full home configuration, use home-manager. If you just want Homebrew-style package management with Nix's reproducibility, use nixy.

## Quick Start

nixy uses **profiles** to organize packages. A "default" profile is created automatically on first use. You can create additional profiles later for different contexts (work, personal, projects).

### 1. Install Nix (if you haven't)

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

### 2. Install nixy

**Quick install (recommended):**

```bash
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash
```

This will try (in order): pre-built binary or nix build.

**With cargo (from crates.io):**

```bash
cargo install nixy
```

**With nix:**

```bash
nix profile install github:yusukeshib/nixy
```

**From source:**

```bash
git clone https://github.com/yusukeshib/nixy.git
cd nixy
cargo build --release
cp target/release/nixy ~/.local/bin/
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

### Package Management

| Command | Alias | Description |
|---------|-------|-------------|
| `nixy install <pkg>` | `add` | Install a package from nixpkgs |
| `nixy install --from <flake> <pkg>` | | Install from a flake (registry name or URL) |
| `nixy install --file <path>` | | Install from a custom nix file |
| `nixy uninstall <pkg>` | `remove` | Uninstall a package |
| `nixy list` | `ls` | List installed packages with source info |
| `nixy search <query>` | | Search for packages |
| `nixy upgrade [input...]` | | Upgrade all inputs or specific ones |
| `nixy sync` | | Build environment from flake.nix (for new machines) |

### Profile Management

| Command | Alias | Description |
|---------|-------|-------------|
| `nixy profile` | | Show current profile |
| `nixy profile switch <name>` | `use` | Switch to a different profile |
| `nixy profile switch -c <name>` | | Create and switch to a new profile |
| `nixy profile list` | `ls` | List all profiles |
| `nixy profile delete <name>` | `rm` | Delete a profile (requires `--force`) |

### Utilities

| Command | Description |
|---------|-------------|
| `nixy config <shell>` | Output shell config (for PATH setup) |
| `nixy version` | Show nixy version |
| `nixy self-upgrade` | Upgrade nixy to the latest version |
| `nixy self-upgrade --force` | Force reinstall even if already at latest |

### Install Options

The `install` command supports several options:

```bash
nixy install ripgrep              # Install from nixpkgs (default)
nixy install --from <flake> <pkg> # Install from external flake
nixy install --file my-pkg.nix    # Install from custom nix file
```

## Multiple Profiles

Maintain separate package sets for different contexts (work, personal, projects):

```bash
nixy profile switch -c work   # Create and switch to a new profile
nixy install slack terraform  # Install work-specific packages

nixy profile switch -c personal  # Create another profile
nixy install spotify games    # Different packages here

nixy profile list             # See all profiles
nixy profile                  # Show current profile
```

Each profile has its own `flake.nix` at `~/.config/nixy/profiles/<name>/`. Switching profiles rebuilds the environment symlink to point to that profile's packages.

**Use cases:**
- **Work vs Personal**: Keep work tools separate from personal apps
- **Client projects**: Different toolchains for different clients
- **Experimentation**: Try new packages without affecting your main setup

**Managing profiles with dotfiles:**

```bash
# Back up all profiles to dotfiles
cp -r ~/.config/nixy/profiles ~/dotfiles/nixy-profiles

# On a new machine, restore and sync
cp -r ~/dotfiles/nixy-profiles ~/.config/nixy/profiles
nixy profile switch work      # Switch to desired profile
nixy sync                     # Build the environment
```

## Sync Across Machines

Your package state is stored in `packages.json`. Back it up, version control it, or sync it with dotfiles:

```bash
# Back up your package state (default profile)
cp ~/.config/nixy/profiles/default/packages.json ~/dotfiles/
cp ~/.config/nixy/profiles/default/flake.lock ~/dotfiles/  # For exact versions
cp -r ~/.config/nixy/profiles/default/packages ~/dotfiles/ # If you have custom packages

# On a new machine:
mkdir -p ~/.config/nixy/profiles/default
cp ~/dotfiles/packages.json ~/.config/nixy/profiles/default/
cp ~/dotfiles/flake.lock ~/.config/nixy/profiles/default/   # Optional
cp -r ~/dotfiles/packages ~/.config/nixy/profiles/default/  # If applicable
nixy sync    # Regenerates flake.nix and installs everything
```

Same packages, same versions, on every machine.

---

## FAQ

**How do I find the right package name?**
Use `nixy search <keyword>`. Package names sometimes differ from what you expect (e.g., `ripgrep` not `rg`).

**Where are packages actually installed?**
In the Nix store (`/nix/store/`). nixy builds a combined environment and creates a symlink at `~/.local/state/nixy/env` pointing to it. The `nixy config` command sets up your PATH to include this location.

**Can I edit the flake.nix manually?**
The `flake.nix` is fully regenerated from nixy's state file (`packages.json`) on every operation. Manual edits will be overwritten.

For custom packages, use the supported methods instead:
- `nixy install --from <flake> <pkg>` for external flakes
- `nixy install --file <path>` for custom nix definitions
- Place files in `packages/` directory for auto-discovery

See "Custom Package Definitions" in the Appendix for details.

**How do I update nixy?**
Run `nixy self-upgrade` to automatically update to the latest version. Alternatively, use `cargo install nixy` or re-run the install script.

**How do I uninstall nixy?**
Delete the `nixy` binary (typically `~/.local/bin/nixy` or `~/.cargo/bin/nixy`). Your flake.nix files remain and work with standard `nix` commands.

**Why not use `nix profile` directly?**
`nix profile` lacks built-in reproducibility - there's no official way to export your packages and recreate the same environment on another machine. nixy uses `packages.json` as the source of truth and generates a reproducible `flake.nix`, which can be copied, version-controlled, and shared.

**How do I rollback to a previous state?**
Since nixy is declarative, your `packages.json` and `flake.lock` files *are* the state. If you version control them with git (recommended), rollback is simple:

```bash
git checkout HEAD~1 -- packages.json flake.lock  # Revert to previous commit
nixy sync                                         # Regenerate flake.nix and apply
```

This is more powerful than `nix profile rollback` - you can go back to any point in history, see why changes were made via commit messages, and experiment with branches.

**How do I install unfree packages?**
Packages with non-free licenses (e.g., `graphite-cli`, `slack`) are allowed by default in nixy. Just install them normally:

```bash
nixy install slack
```

**How do I clean up old Nix store paths?**
nixy doesn't provide a garbage collection command because it uses `nix build --out-link` instead of `nix profile`. To clean up unused Nix store paths, use the standard Nix command directly:

```bash
nix-collect-garbage -d
```

Note: This will clean up ALL unused Nix profiles and store paths on your system, not just nixy-related ones.

---

## Appendix

### How nixy manages state

nixy uses a `packages.json` file in each profile directory as the source of truth. The `flake.nix` is fully regenerated from this state on every operation.

```
~/.config/nixy/profiles/default/
├── packages.json    # Source of truth (managed by nixy)
├── flake.nix        # Generated (do not edit manually)
├── flake.lock       # Nix lockfile
└── packages/        # Custom package definitions
    ├── my-tool.nix
    └── my-flake/
        └── flake.nix
```

This design ensures:
- No marker-based editing that can get out of sync
- Clean separation between state and generated output
- Easy backup (just copy `packages.json` and `packages/` directory)

### For Existing Nix Users

If you already manage your own `flake.nix` and want to use nixy's package list, you can import it:

```nix
{
  inputs.nixy-packages.url = "path:~/.config/nixy/profiles/default";

  outputs = { self, nixpkgs, nixy-packages }: {
    # nixy-packages.packages.<system>.default is a buildEnv with all nixy packages
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

### Installing from External Flakes

Install packages from any flake using `--from`:

```bash
# Direct flake URL
nixy install --from github:nix-community/neovim-nightly-overlay neovim

# Or use nix registry names
nixy install --from nixpkgs hello
```

The flake is added as a custom input to your `flake.nix`, and the full URL is stored for reproducibility. This works with any flake that exports packages.

### Custom Package Definitions

Install packages from custom nix files:

```bash
nixy install --file my-package.nix
```

The file is copied to the `packages/` directory and automatically discovered during flake generation.

**Format for simple packages** (`my-package.nix`):
```nix
{
  pname = "my-package";  # or "name"
  overlay = "overlay-name.overlays.default";
  packageExpr = "pkgs.my-package";
  # Optional: custom inputs
  input.overlay-name.url = "github:user/repo";
}
```

**Format for flake-based packages**:

Place a directory with `flake.nix` in `packages/`:
```
packages/my-tool/flake.nix
```

nixy will automatically add it as a path input and include its default package.

**Auto-discovery**:

Any files in the `packages/` directory are automatically included:
- `packages/*.nix` - Single file packages
- `packages/*/flake.nix` - Flake-based packages

You can also manually place files in `packages/` without using `nixy install --file`.

### Config Locations

| Path | Description |
|------|-------------|
| `~/.config/nixy/profiles/<name>/packages.json` | Package state (source of truth) |
| `~/.config/nixy/profiles/<name>/flake.nix` | Generated flake (do not edit) |
| `~/.config/nixy/profiles/<name>/flake.lock` | Nix lockfile (version control this) |
| `~/.config/nixy/profiles/<name>/packages/` | Custom package definitions (auto-discovered) |
| `~/.config/nixy/active` | Current active profile name |
| `~/.local/state/nixy/env` | Symlink to built environment (add `bin/` to PATH) |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NIXY_CONFIG_DIR` | `~/.config/nixy` | Location of global flake.nix |
| `NIXY_ENV` | `~/.local/state/nixy/env` | Symlink to built environment |

### Limitations

- Package names use Nix naming (search with `nixy search` to find exact names)
- No GUI app support (like Homebrew Cask) yet
- Requires Nix with flakes enabled (the Determinate installer enables this by default)

## Development

```bash
# Build
cargo build --release

# Run tests
cargo test

# Run with debug output
RUST_LOG=debug cargo run -- install hello
```

## License

MIT
