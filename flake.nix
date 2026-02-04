{
  description = "nixy - Homebrew-style wrapper for Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        nixy = pkgs.rustPlatform.buildRustPackage {
          pname = "nixy";
          version = "0.2.6";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          doCheck = false; # Integration tests require Nix runtime

          meta = with pkgs.lib; {
            description = "Homebrew-style wrapper for Nix using flake.nix";
            homepage = "https://github.com/yusukeshib/nixy";
            license = licenses.mit;
            maintainers = [];
            mainProgram = "nixy";
          };
        };
      in
      {
        packages = {
          default = nixy;
          nixy = nixy;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = nixy;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
          ];
        };
      }
    );
}
