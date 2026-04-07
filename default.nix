let
  pinned = import ./npins;
  pkgs = import pinned.nixpkgs { };
  inherit (pkgs) lib;
  craneLib = import pinned.crane { inherit pkgs; };

  patchedLix = pkgs.lixPackageSets.latest.lix.overrideAttrs (old: {
    patches = (old.patches or [ ]) ++ [ ./lix.patch ];
  });

  commonArgs = {
    src = craneLib.cleanCargoSource ./.;
    strictDeps = true;
  };
in
craneLib.buildPackage (
  commonArgs
  // {
    nativeBuildInputs = [ pkgs.makeWrapper ];
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;

    doCheck = false;

    postInstall = ''
      wrapProgram "$out/bin/fluke" --prefix PATH : ${
        lib.escapeShellArg (
          lib.makeBinPath [
            patchedLix
            pkgs.watchman
          ]
        )
      }
      wrapProgram "$out/bin/fluke-nix-build" --prefix PATH : "$out/bin"
    '';

    # TODO: rustfmt and clippy

    passthru.lix = patchedLix;
  }
)
