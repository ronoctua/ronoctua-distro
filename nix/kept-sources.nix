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
# what `nix-collect-garbage` honours.
#
# # One root per SET, never one per name
#
# The directory is the person's, and every checkout of this repository — a
# worktree with another lockfile, an old branch — runs this same script into
# it. A root named `sources` or `layer-release` would be taken over by
# whichever checkout ran last: a worktree whose lockfile moved would release
# the main checkout's sources and its 483 compiled crates, and the next
# collection — the floor's, during that very build — would free them. So a
# root is named after the store path it holds (`sources-<hash>…`,
# `layer-<name>-<hash>…`), each checkout's run refreshes its own, and a root
# NO checkout has refreshed in `releaseDays` is released: that is how the set
# of an old lockfile dies, a while after the last build that used it.
#
# It is run by `.#dev` and by the gate and the batteries: the commands a person
# types before a build. It costs one `nix-store` call on paths that are already
# valid — the script's own closure is what made them valid.
#
# NOT the sources of `nix/prefetch.nix`: that list is the `full` profile's, and
# holding it from here would make every `.#dev` download 1.37 GB of Android
# Studio on a machine that never installs it.
#
# CHROME'S .deb IS HERE, although only the `full` profile installs Chrome:
# `kiosk-vm` runs the real one and is in `.#most`, so every battery builds from
# it — and its address is Google's pool, which drops a release once the next
# one is out (a hash with an expiry date, the defect above). It costs a
# machine that never ran a battery one download of 134 MiB.
#
# # And the dependency layers, the CURRENT ones only
#
# The two `buildDepsOnly` layers (`rust.cargoArtifacts`, `rust.loopArtifacts`)
# are what every edit of the workspace builds on: the 483 third-party crates,
# compiled, per profile. Nothing installed references them — they are build
# inputs — so a collection takes them, and the next build pays both again.
# The store now collects itself — the update's weekly collection, and Nix's
# own floor when the disk runs short (`nix/modules/store.nix`) — so without a
# root that would be most weeks.
#
# So each gets a root of its own, pointing at the layer THIS evaluation would
# build — named after that layer, for the reason above. A layer is rooted only
# if it is already in the store: rooting is never a reason to compile 483
# crates. The layer of an OLD lockfile (or an old toolchain) keeps its root
# until no checkout has asked for it in `releaseDays`, and then dies at the
# next collection like anything else unreferenced.
#
# `unsafeDiscardStringContext`, as in `nix/gate.nix`: the path is TEXT here.
# With its context the script would depend on both layers, and running it
# would build them.
{
  lib,
  linkFarm,
  writeShellApplication,
  nix,
  coreutils,
  findutils,
  inputs,
  # Every fixed-output download a derivation of this flake declares.
  downloads ? [ ],
  # The dependency layers to hold while they are the current ones, by name.
  layers ? { },
  # How long a root no checkout refreshed is kept (`assets/cache.nix`'s
  # `oldCompilerDays`, passed by `flake.nix`: a layer IS an old compiler's
  # build once nothing builds on it).
  releaseDays,
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
  runtimeInputs = [
    nix
    coreutils
    findutils
  ];
  text = ''
    roots="''${XDG_STATE_HOME:-$HOME/.local/state}/ronoctua/gcroots"
    mkdir -p "$roots"
    # ROOT IT, AND MARK IT FRESH: `--add-root` replaces the link, and `touch
    # -h` says so even where the link was already right — its own time is
    # what the release below reads.
    hold() {
      if nix-store --realise --add-root "$1" --indirect "$2" > /dev/null; then
        touch -h -- "$1"
        return 0
      fi
      return 1
    }
    # A SAFEGUARD, NEVER A GATE: where there is no store to talk to — a build
    # sandbox, a machine whose daemon is down — the session it guards still
    # has to start. Under `errexit` a bare failure here ended `.#dev` before
    # it had installed its own cleanup, so it says so and carries on.
    if ! hold "$roots/sources-$(basename ${kept})" ${kept}; then
      echo "warning: could not root the flake's sources; a garbage collection may take them" >&2
    fi
    ${lib.concatStrings (
      lib.mapAttrsToList (name: layer: ''
        layer=${builtins.unsafeDiscardStringContext layer.outPath}
        # `--check-validity` and not `-e`: a path half-deleted by a running
        # collection still exists and is no longer valid.
        if nix-store --check-validity "$layer" 2> /dev/null; then
          if ! hold "$roots/layer-${name}-$(basename "$layer")" "$layer"; then
            echo "warning: could not root the ${name} dependency layer" >&2
          fi
        fi
      '') layers
    )}
    # RELEASE WHAT NO CHECKOUT HAS ASKED FOR IN ${toString releaseDays} DAYS — the
    # names of before this rule (`sources`, `layer-<name>`) included. Only
    # links, only here, and only these names: the directory holds nothing
    # else of ours, and whatever else is in it is not this script's.
    while IFS= read -r -d "" stale; do
      rm -f -- "$stale"
      echo "released $(basename -- "$stale"): no checkout asked for it in ${toString releaseDays} days" >&2
    done < <(find "$roots" -mindepth 1 -maxdepth 1 -type l \( -name 'sources*' -o -name 'layer-*' \) \
      -mmin +${toString (releaseDays * 24 * 60)} -print0)
  '';
}
