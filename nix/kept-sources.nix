# The downloads this flake builds from, held by a garbage-collector ROOT.
#
# # The defect
#
# On 2026-09-22 a garbage collection on the host (a system update script ran
# it) emptied the store of everything nothing was holding, and the next
# `nix run .#dev` died on `hash mismatch in fixed-output derivation`. Nothing
# in this repository had changed. Nix downloaded again a file it had held for
# weeks, and the address now served different bytes.
#
# Two things had to be true for that, and each one has its own fix:
#
#   1. the address was not a version — `nix/atlas.nix` now fetches captures,
#      and `downloads-are-pinned` refuses a URL that names no version, commit
#      or capture;
#   2. the store was the only copy anyone could count on — which is this file.
#
# The second is not only about the atlas. The four font inputs point at a
# release of this repository that does not exist on GitHub yet (measured: 404),
# so the store is where they live, full stop. A collection that took them would
# take them for good, and the only way back would be `nix run .#fonts-release`
# from a folder on the owner's disk.
#
# # What this does
#
# One derivation that REFERENCES every flake input (bar `self`, which is this
# tree and changes with every edit) and every download a derivation here
# declares — so rooting it keeps all of them. `ronoctua-keep-sources` registers
# it as an indirect root under `$XDG_STATE_HOME/ronoctua/gcroots`, which is
# what `nix-collect-garbage` honours. The root is REPLACED on every run, so an
# old set of sources is released once nothing that builds from it is left.
#
# It is run by `.#dev` and by the gate and the batteries: the commands a person
# types before a build. It costs one `nix-store` call on paths that are already
# valid — the script's own closure is what made them valid.
#
# NOT the sources of `nix/prefetch.nix`: that list is the `full` profile's, and
# holding it from here would make every `.#dev` download 1.37 GB of Android
# Studio on a machine that never installs it.
{
  lib,
  linkFarm,
  writeShellApplication,
  nix,
  inputs,
  # Every fixed-output download a derivation of this flake declares.
  downloads ? [ ],
}:

let
  kept = linkFarm "ronoctua-kept-sources" (
    lib.mapAttrsToList (name: input: {
      name = "input-${name}";
      path = input.outPath;
    }) (removeAttrs inputs [ "self" ])
    ++ map (d: {
      name = "download-${d.name}";
      path = d;
    }) downloads
  );
in
writeShellApplication {
  name = "ronoctua-keep-sources";
  runtimeInputs = [ nix ];
  text = ''
    roots="''${XDG_STATE_HOME:-$HOME/.local/state}/ronoctua/gcroots"
    mkdir -p "$roots"
    nix-store --realise --add-root "$roots/sources" --indirect ${kept} > /dev/null
  '';
}
