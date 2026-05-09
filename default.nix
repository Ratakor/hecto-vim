{
  pkgs ? import <nixpkgs> { },
}:
let
  inherit (pkgs) lib;

  fs = lib.fileset;
in
pkgs.rustPlatform.buildRustPackage (finalAttrs: {
  pname = "hecto-vim";
  version = "0.1.0";

  src = fs.toSource {
    root = ./.;
    fileset = fs.unions [
      ./src
      ./Cargo.lock
      ./Cargo.toml
    ];
  };

  cargoLock = {
    allowBuiltinFetchGit = true;
    lockFile = ./Cargo.lock;
  };

  meta.mainProgram = finalAttrs.pname;
})
