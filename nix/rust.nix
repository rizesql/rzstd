{
  inputs,
  system,
  pkgs,
}:
let
  toolchain = inputs.fenix.packages.${system}.fromToolchainFile {
    file = ../rust-toolchain.toml;
    sha256 = "sha256-sqSWJDUxc+zaz1nBWMAJKTAGBuGWP25GCftIOlCEAtA=";
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
      pkgs.cargo-mutants
      pkgs.cargo-nextest
      pkgs.cargo-show-asm
      pkgs.clippy
      pkgs.rustfmt
    ];
    RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
  });
}
