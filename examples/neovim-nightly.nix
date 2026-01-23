{
  name = "neovim-nightly";
  inputs = {
    neovim-nightly-overlay.url = "github:nix-community/neovim-nightly-overlay";
  };
  overlay = "neovim-nightly-overlay.overlays.default";
  packageExpr = "pkgs.neovim";
}
