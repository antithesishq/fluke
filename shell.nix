let
  pinned = import ./npins;
  pkgs = import pinned.nixpkgs { };
in
pkgs.mkShell {
  packages = with pkgs; [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer
    watchman
    cargo-nextest
    cargo-edit
    nixfmt
  ];

  WATCHMAN_PATH = "${pkgs.watchman}/bin";
  LIX_PATH = pkgs.lib.getExe' (import ./default.nix).passthru.lix "nix-instantiate";
}
