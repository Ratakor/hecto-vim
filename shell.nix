{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  packages = with pkgs; [
    cargo
    clippy
    clang-analyzer
    rustfmt

    nil
  ];
}
