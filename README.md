# nixy - Simple Declarative Nix Package Management

[日本語版はこちら](README_ja.md)

## Why nixy?

I was frustrated with asdf and Homebrew while working, and tried to learn Nix several times—but the steep learning curve made me give up every time. What I really wanted was a simple asdf/Homebrew alternative that uses Nix's massive package repository and reproducibility under the hood.

So I built nixy—a simple Rust wrapper with profile support. It runs smoothly and I love it.

![nixy demo](demo.gif)

**Reproducible Nix packages, simple commands.** Install packages with a single command, sync them across all your machines.

```bash
nixy install ripgrep    # That's it. Nix made simple.
```

nixy manages your Nix packages through a declarative `nixy.json` configuration file, ensuring the same packages and versions on every machine.

## Prerequisites

nixy requires Nix. Install with:

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

## Quick Start

### 1. Install nixy

```bash
# Quick install (recommended)
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash

# Or with cargo
cargo install nixy-rs

# Or with nix
nix profile install github:yusukeshib/nixy
```

### 2. Set up your shell

Add to `.bashrc`, `.zshrc`, etc.:

```bash
eval "$(nixy config zsh)"
```

For fish (`~/.config/fish/config.fish`):

```fish
nixy config fish | source
```

### 3. Start using

```bash
nixy install ripgrep        # Install latest version
nixy install nodejs@20      # Install specific major version
nixy install python@3.11.5  # Install exact version
nixy list                   # See installed packages with versions
nixy search python          # Find packages + available versions
nixy uninstall nodejs       # Remove a package
nixy upgrade                # Upgrade all within version constraints
nixy upgrade nodejs         # Upgrade specific package
```

## Commands

| Command | Description |
|---------|-------------|
| `nixy install <pkg>[@version]` | Install a package with optional version (alias: `add`) |
| `nixy install <flake-ref>` | Install from a flake reference (e.g., `github:user/repo`) |
| `nixy install --from <flake> <pkg>` | Install a specific package from a flake URL |
| `nixy install --file <path>` | Install from a nix file |
| `nixy install <pkg> --platform <platform>` | Install only for specific platform(s) |
| `nixy uninstall <pkg>` | Uninstall a package (alias: `remove`) |
| `nixy list` | List installed packages with versions (alias: `ls`) |
| `nixy search <query>` | Search for packages with version info |
| `nixy upgrade [pkg...]` | Upgrade packages within version constraints |
| `nixy sync` | Rebuild from flake.nix |
| `nixy profile` | List profiles + interactive TUI selection |
| `nixy profile <name>` | Switch to profile |
| `nixy profile <name> -c` | Create and switch to profile |
| `nixy profile <name> -d` | Delete profile (with confirmation) |
| `nixy file <pkg>` | Show path to package source file in Nix store |
| `nixy self-upgrade` | Upgrade nixy itself |

### Version Specification

nixy supports flexible version constraints via [Nixhub](https://nixhub.io):

```bash
nixy install nodejs           # Latest version
nixy install nodejs@20        # Latest 20.x.x (semver range)
nixy install nodejs@20.11     # Latest 20.11.x
nixy install nodejs@20.11.0   # Exact version
```

When you run `nixy upgrade nodejs`, it respects your version constraint:
- `nodejs` (no version) → upgrades to absolute latest
- `nodejs@20` → upgrades to latest 20.x.x

### Platform-Specific Installation

Install packages only for specific platforms:

```bash
nixy install terminal-notifier --platform darwin   # macOS only
nixy install linux-tool --platform linux           # Linux only
nixy install specific --platform aarch64-darwin    # Apple Silicon only
```

Valid platform names:
- `darwin` or `macos` → both `x86_64-darwin` and `aarch64-darwin`
- `linux` → both `x86_64-linux` and `aarch64-linux`
- Full names: `x86_64-darwin`, `aarch64-darwin`, `x86_64-linux`, `aarch64-linux`

Platform-specific packages are shown with their restriction in `nixy list`:
```
terminal-notifier@2.0.0  (nixpkgs) [darwin]
```

## Profiles

Maintain separate package sets for different contexts:

```bash
nixy profile work -c            # Create and switch to new profile
nixy install slack terraform    # Install work packages

nixy profile personal -c        # Another profile
nixy install spotify            # Different packages

nixy profile                    # Interactive profile selector
nixy profile work               # Switch to existing profile
nixy profile old -d             # Delete a profile (with confirmation)
```

All profiles are stored in `~/.config/nixy/nixy.json`, with generated flakes in `~/.local/state/nixy/profiles/<name>/`.

## How nixy works

nixy is **purely declarative** - `nixy.json` is the source of truth, and `flake.nix` is regenerated from it on every operation.

```
┌─────────────────┐      ┌─────────────┐      ┌─────────────────────────────┐
│   nixy.json     │ ──── │  flake.nix  │ ──── │ ~/.local/state/nixy/env/bin │
│ (source of truth)│ generate │ (+ flake.lock)│ nix build │      (symlink to /nix/store) │
└─────────────────┘      └─────────────┘      └─────────────────────────────┘
                                                            │
                                                            ▼
                                              eval "$(nixy config zsh)" adds
                                              this path to your $PATH
```

Unlike `nix profile` which maintains mutable state, nixy:
1. Regenerates `flake.nix` from `nixy.json` on every operation
2. Runs `nix build` to create a combined environment in `/nix/store`
3. Creates a symlink at `~/.local/state/nixy/env` pointing to that environment
4. Your shell config just adds `~/.local/state/nixy/env/bin` to `$PATH`

This means syncing is simple: copy `nixy.json` and your profile's `flake.lock` (e.g., `~/.local/state/nixy/profiles/<profile>/flake.lock`) to another machine, run `nixy sync`, and you have the exact same environment.

## FAQ

**How do I find the right package name?**
Use `nixy search <keyword>`.

**Where are packages installed?**
In `/nix/store/`. nixy creates a symlink at `~/.local/state/nixy/env` pointing to your environment.

**Can I edit flake.nix manually?**
No, it's regenerated from `nixy.json` on every operation. Use `--from` or `--file` for custom packages.

**How does nixy differ from nix profile?**
nixy adds reproducibility on top of Nix - your `nixy.json` + `flake.lock` can be synced and version controlled across machines.

**How do I rollback?**
Version control your `nixy.json` and `flake.lock` with git:
```bash
cd ~/.config/nixy
git checkout HEAD~1 -- nixy.json
nixy sync
```

---

## Advanced

<details>
<summary>Directory structure</summary>

```
~/.config/nixy/
├── nixy.json        # Source of truth (all profiles)
└── packages/        # Global custom package definitions

~/.local/state/nixy/
├── env              # Symlink to active profile's build
└── profiles/
    ├── default/
    │   ├── flake.nix    # Generated (do not edit)
    │   └── flake.lock   # Nix lockfile
    └── work/
        └── ...
```

</details>

<details>
<summary>Custom package definitions</summary>

**From GitHub flake (default package):**
```bash
nixy install github:nix-community/neovim-nightly-overlay
```

**From GitHub flake (specific package):**
```bash
nixy install github:nix-community/neovim-nightly-overlay#neovim
# or equivalently:
nixy install --from github:nix-community/neovim-nightly-overlay neovim
```

**From nix file:**
```bash
nixy install --file my-package.nix
```

Files in `packages/` directory are auto-discovered.

</details>

<details>
<summary>For existing Nix users</summary>

You can import nixy's package list into your own flake:

```nix
{
  inputs.nixy-packages.url = "path:~/.local/state/nixy/profiles/default";

  outputs = { self, nixpkgs, nixy-packages }: {
    # nixy-packages.packages.<system>.default is a buildEnv with all packages
  };
}
```

nixy and `nix profile` use separate paths and don't conflict.

</details>

<details>
<summary>Config locations</summary>

| Path | Description |
|------|-------------|
| `~/.config/nixy/nixy.json` | Configuration (all profiles) |
| `~/.config/nixy/packages/` | Global custom package definitions |
| `~/.local/state/nixy/profiles/<name>/flake.nix` | Generated flake |
| `~/.local/state/nixy/profiles/<name>/flake.lock` | Nix lockfile |
| `~/.local/state/nixy/env` | Symlink to environment |

Environment variables: `NIXY_CONFIG_DIR`, `NIXY_STATE_DIR`, `NIXY_ENV`

</details>

## License

MIT
