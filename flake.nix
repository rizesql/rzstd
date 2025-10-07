{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    inputs:
    inputs.flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = inputs.nixpkgs.outputs.legacyPackages.${system};

        toolchain = inputs.fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-SJwZ8g0zF2WrKDVmHrVG3pD2RGoQeo24MEXnNx5FyuI=";
        };
      in
      {
        packages.rzstd = pkgs.callPackage ./rzstd.nix {
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        };
        packages.default = inputs.self.outputs.packages.${system}.rzstd;

        devShells.default = inputs.self.packages.${system}.default.overrideAttrs (super: {
          nativeBuildInputs = super.nativeBuildInputs ++ [
            pkgs.cargo-deny
            pkgs.clippy
            pkgs.rustfmt
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        });
      }
    )
    // {
      overlays.default = final: prev: {
        inherit (inputs.self.packages.${final.system}) rzstd;
      };
    };
}
