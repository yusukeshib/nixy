# Plan: nbrew - Homebrew-style Wrapper for Nix

## Goal
Create a shell wrapper that makes Nix behave like Homebrew, with a `bundle` command for declarative, reproducible environment configuration.

## Configuration File: `Nixfile`

A simple, human-readable format (like Brewfile):

```bash
# ~/.config/nbrew/Nixfile
channel nixpkgs/nixos-unstable

# Packages
pkg git
pkg htop
pkg ripgrep
pkg nodejs
pkg python3

# Development tools (for `nbrew shell`)
dev cargo
dev rustc
```

## Commands to Implement

### Basic Commands
| Command | Nix Equivalent |
|---------|----------------|
| `nbrew install <pkg>` | `nix profile install nixpkgs#<pkg>` |
| `nbrew uninstall <pkg>` | `nix profile remove nixpkgs#<pkg>` |
| `nbrew search <query>` | `nix search nixpkgs <query>` |
| `nbrew list` | `nix profile list` |
| `nbrew upgrade` | `nix profile upgrade '.*'` |
| `nbrew update` | `nix-channel --update` |

### Bundle Commands (for reproducibility)
| Command | Description |
|---------|-------------|
| `nbrew bundle` | Install all packages from Nixfile |
| `nbrew bundle dump` | Export installed packages to Nixfile |
| `nbrew bundle cleanup` | Remove packages not in Nixfile |
| `nbrew bundle check` | Verify all Nixfile packages are installed |

### Environment Commands
| Command | Description |
|---------|-------------|
| `nbrew shell` | Enter dev shell with `dev` packages |
| `nbrew gc` | Garbage collect old generations |

## Files to Create

```
~/.local/bin/nbrew          # Main CLI script (single file)
~/.config/nbrew/
├── Nixfile                 # Global package declarations
└── Nixfile.lock            # Auto-generated locked versions (JSON)
```

## Implementation Structure

Single bash script (`~/.local/bin/nbrew`) with:

1. **Package parsing**: Read Nixfile line-by-line, extract `pkg` and `dev` entries
2. **State detection**: Use `nix profile list --json` to get installed packages
3. **Bundle install**: Compare Nixfile vs installed, install missing packages
4. **Bundle dump**: Parse `nix profile list --json`, extract package names, write Nixfile
5. **Bundle cleanup**: Remove packages in profile but not in Nixfile
6. **Shell generation**: Auto-generate `flake.nix` for `dev` packages, run `nix develop`

## Key Implementation Details

- Use `jq` for JSON parsing (common dependency)
- Store state in `~/.config/nbrew/`
- Support both global (`~/.config/nbrew/Nixfile`) and local (`./Nixfile`) configs
- Generate `Nixfile.lock` with nixpkgs revision for true reproducibility

## Limitations

- Package names must be Nix names (may differ from Homebrew)
- No cask equivalent for macOS GUI apps
- Requires `experimental-features = nix-command flakes` in `~/.config/nix/nix.conf`

## Verification

1. Run `nbrew install ripgrep` - should install via nix profile
2. Run `nbrew list` - should show installed packages
3. Run `nbrew bundle dump` - should create Nixfile with current packages
4. Edit Nixfile, run `nbrew bundle` - should install new packages
5. Run `nbrew bundle cleanup` - should remove unlisted packages
