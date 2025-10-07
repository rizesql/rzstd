{
  lib,
  rustPlatform,
  installShellFiles,
}:
rustPlatform.buildRustPackage {
  name = "rzstd";

  src = lib.cleanSource ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  nativeBuildInputs = [ installShellFiles ];
}
