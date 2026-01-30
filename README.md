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

nixy manages your Nix packages through a declarative `flake.nix` + `flake.lock`, ensuring the same packages and versions on every machine.

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
nixy install ripgrep    # Install a package
nixy install nodejs git # Install multiple
nixy list               # See installed packages
nixy search python      # Find packages
nixy uninstall nodejs   # Remove a package
nixy upgrade            # Upgrade all
```

## Commands

| Command | Description |
|---------|-------------|
| `nixy install <pkg>` | Install a package (alias: `add`) |
| `nixy install --from <flake> <pkg>` | Install from a flake URL |
| `nixy install --file <path>` | Install from a nix file |
| `nixy uninstall <pkg>` | Uninstall a package (alias: `remove`) |
| `nixy list` | List installed packages (alias: `ls`) |
| `nixy search <query>` | Search for packages |
| `nixy upgrade` | Upgrade all inputs |
| `nixy sync` | Rebuild from flake.nix |
| `nixy profile` | List profiles + interactive TUI selection |
| `nixy profile <name>` | Switch to profile |
| `nixy profile <name> -c` | Create and switch to profile |
| `nixy profile <name> -d` | Delete profile (with confirmation) |
| `nixy self-upgrade` | Upgrade nixy itself |

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

Each profile has its own `flake.nix` at `~/.config/nixy/profiles/<name>/`.

## How nixy works

nixy is **purely declarative** - `packages.json` is the source of truth, and `flake.nix` is regenerated from it on every operation.

```
┌─────────────────┐      ┌─────────────┐      ┌─────────────────────────────┐
│ packages.json   │ ──── │  flake.nix  │ ──── │ ~/.local/state/nixy/env/bin │
│ (source of truth)│ generate │ (+ flake.lock)│ nix build │      (symlink to /nix/store) │
└─────────────────┘      └─────────────┘      └─────────────────────────────┘
                                                            │
                                                            ▼
                                              eval "$(nixy config zsh)" adds
                                              this path to your $PATH
```

Unlike `nix profile` which maintains mutable state, nixy:
1. Regenerates `flake.nix` from `packages.json` on every operation
2. Runs `nix build` to create a combined environment in `/nix/store`
3. Creates a symlink at `~/.local/state/nixy/env` pointing to that environment
4. Your shell config just adds `~/.local/state/nixy/env/bin` to `$PATH`

This means syncing is simple: copy `packages.json` + `flake.lock` to another machine, run `nixy sync`, and you have the exact same environment.

## FAQ

**How do I find the right package name?**
Use `nixy search <keyword>`.

**Where are packages installed?**
In `/nix/store/`. nixy creates a symlink at `~/.local/state/nixy/env` pointing to your environment.

**Can I edit flake.nix manually?**
No, it's regenerated from `packages.json` on every operation. Use `--from` or `--file` for custom packages.

**How does nixy differ from nix profile?**
nixy adds reproducibility on top of Nix - your `packages.json` + `flake.lock` can be synced and version controlled across machines.

**How do I rollback?**
Version control your `packages.json` and `flake.lock` with git:
```bash
git checkout HEAD~1 -- packages.json flake.lock
nixy sync
```

---

## Advanced

<details>
<summary>Profile directory structure</summary>

```
~/.config/nixy/profiles/default/
├── packages.json    # Source of truth
├── flake.nix        # Generated (do not edit)
├── flake.lock       # Nix lockfile
└── packages/        # Custom package definitions
```

</details>

<details>
<summary>Custom package definitions</summary>

**From external flake:**
```bash
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
  inputs.nixy-packages.url = "path:~/.config/nixy/profiles/default";

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
| `~/.config/nixy/profiles/<name>/packages.json` | Package state |
| `~/.config/nixy/profiles/<name>/flake.nix` | Generated flake |
| `~/.config/nixy/profiles/<name>/flake.lock` | Nix lockfile |
| `~/.config/nixy/active` | Current profile |
| `~/.local/state/nixy/env` | Symlink to environment |

Environment variables: `NIXY_CONFIG_DIR`, `NIXY_ENV`

</details>

## License

MIT
