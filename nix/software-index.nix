# The search index of the system's own nixpkgs (`RONOCTUA_NIX_INDEX`).
#
# Built from the SAME source the machine installs from, so what the software
# view finds is exactly what `nix profile add nixpkgs#…` resolves to — the
# registry is pinned to `inputs.nixpkgs` (`nix/modules/base.nix`). It rebuilds
# only when that input moves.
#
# It is run-time DATA for the shell and the CLI (the `atRunTime` side of
# `nix/rust.nix`), never `generated`: a nixpkgs bump must not rebuild the
# workspace.
#
# `nix-instantiate --eval` inside a build is the same move nixpkgs' own
# `make-tarball.nix` makes to publish `packages.json.br`: a private
# `NIX_STATE_DIR`, no `NIX_PATH`, evaluation only.
{
  pkgs,
  nixpkgs,
}:
pkgs.runCommand "ronoctua-software-nix-index"
  {
    nativeBuildInputs = [
      pkgs.nix
      pkgs.python3
    ];
  }
  ''
    export NIX_STATE_DIR=$TMPDIR/state HOME=$TMPDIR NIX_PATH=
    mkdir -p $out
    nix-instantiate --eval --strict --json \
      --option allow-import-from-derivation false \
      --expr "import ${./scripts/software-index.nix} { src = ${nixpkgs}; system = \"${pkgs.stdenv.hostPlatform.system}\"; }" \
      > raw.json
    python3 ${./scripts/software-index.py} raw.json $out/index.tsv
    echo ${nixpkgs.rev or "unknown"} > $out/rev
  ''
