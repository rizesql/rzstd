{
  inputs,
  system,
  pkgs,
}:
let
  toolchain = inputs.fenix.packages.${system}.fromToolchainFile {
    file = ../rust-toolchain.toml;
    sha256 = "sha256-SJwZ8g0zF2WrKDVmHrVG3pD2RGoQeo24MEXnNx5FyuI=";
  };

  rustPlatform = pkgs.makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  };

  rzstd = pkgs.callPackage ./rzstd.nix {
    inherit rustPlatform;
  };
in
{
  package = rzstd;

  devShell = rzstd.overrideAttrs (super: {
    nativeBuildInputs = super.nativeBuildInputs ++ [
      pkgs.cargo-deny
      pkgs.clippy
      pkgs.rustfmt
    ];
    RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
  });
}
