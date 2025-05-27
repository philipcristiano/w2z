{ pkgs ? import <nixpkgs> { } }:
let manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
in
pkgs.rustPlatform.buildRustPackage rec {
  pname = manifest.name;
  nativeBuildInputs = [ pkgs.tailwindcss ];
  version = manifest.version;
  cargoLock.lockFile = ./Cargo.lock;
  cargoLock.outputHashes = {};
  src = pkgs.lib.cleanSource ./.;

  # build environment variables
  SQLX_OFFLINE = true;
}

