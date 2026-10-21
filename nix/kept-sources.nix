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
# The second is not only about the atlas. The four font inputs and the
# ambient sounds' originals point at releases of this repository, and a
# release can go: when the repository was made again (2026-10-21) they were
# published again from this store, byte for byte. A collection that took them
# would have left only `nix run .#fonts-release` from a folder on the owner's
# disk.
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
# # No dependency layers any more
#
# The two `buildDepsOnly` layers crane's whole-workspace builds stood on were
# rooted here, one root per layer (`layer-<name>-<hash>…`), because nothing
# installed references them and every edit built on them. They are gone with
# those builds (2026-10-12): the Rust build is one derivation per unit, the
# third-party crates included, and what a collection must not take is now the
# units below. A `layer-…` root an older checkout left is released like the
# rest, `releaseDays` after it was last refreshed.
#
# # And the compiled units, the CURRENT ones of THIS checkout
#
# With the Rust build in one derivation per unit (`nix/rust/units.nix`), a
# library unit — ours or a third-party crate's — is one nothing installed
# references, taken by the next collection, paid again by the next build.
# The sets (`units`: `rust.release.keep`, the release graph's units, and
# `rust.loop.keep`, the loop's third-party ones, which the test graph
# shares) are rooted by `ronoctua-keep-units`
# (`nix/keep-units.nix`) — only the members the store holds, so rooting
# compiles nothing.
#
# PER CHECKOUT, and replaced, never accumulated. A layer moved with the
# lockfile, so a root per hash was a root per lockfile; a set of units moves
# with nearly every edit, and a root per hash would hold every version for
# `releaseDays` — gigabytes an afternoon. So a set is named after the
# checkout that ran this (its top directory, hashed): each checkout holds the
# units of its own current tree, the last run replaces what the one before
# held, and a checkout nobody ran in `releaseDays` (a lane dropped, a branch
# forgotten) is released like the rest.
{
  lib,
  linkFarm,
  fetchurl,
  writeShellApplication,
  writeText,
  nix,
  coreutils,
  findutils,
  git,
  inputs,
  # Every fixed-output download a derivation of this flake declares.
  downloads ? [ ],
  # The sets of compiled units to hold while they are the current ones, by
  # name (each with its members' `paths`), and what roots them.
  units ? { },
  keepUnits ? null,
  # How long a root no checkout refreshed is kept (`assets/cache.nix`'s
  # `oldCompilerDays`, passed by `flake.nix`: a set of units IS an old
  # compiler's build once nothing builds on it).
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
    }) (downloads ++ lib.attrValues (import ./test-vectors.nix { inherit fetchurl; }))
  );
  # Each set's members, as text: naming them builds nothing.
  unitFiles = lib.mapAttrs (
    name: set: writeText "ronoctua-kept-${name}-unit-paths" (lib.concatLines set.paths)
  ) units;
in
assert lib.assertMsg (
  units == { } || keepUnits != null
) "nix/kept-sources.nix: units to keep, and nothing to keep them with";
writeShellApplication {
  name = "ronoctua-keep-sources";
  runtimeInputs = [
    nix
    coreutils
    findutils
    git
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
    ${lib.optionalString (units != { }) ''
      # THE UNITS OF THIS CHECKOUT'S TREE, by the checkout's own name.
      checkout=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
      key=$(printf '%s' "$checkout" | sha256sum | cut -c1-12)
      ${lib.concatStrings (
        lib.mapAttrsToList (name: file: ''
          if ! ${keepUnits}/bin/ronoctua-keep-units --roots "$roots" --set "units-${name}-$key" \
               --release-days ${toString releaseDays} ${file} >&2; then
            echo "warning: could not root the ${name} units; a garbage collection may take them" >&2
          fi
        '') unitFiles
      )}
    ''}
    # RELEASE WHAT NO CHECKOUT HAS ASKED FOR IN ${toString releaseDays} DAYS — the
    # names of before this rule (`sources`, `layer-<name>`) included. Only
    # links, only here, and only these names: the directory holds nothing
    # else of ours but the unit sets, which `ronoctua-keep-units` releases
    # itself, and whatever else is in it is not this script's.
    while IFS= read -r -d "" stale; do
      rm -f -- "$stale"
      echo "released $(basename -- "$stale"): no checkout asked for it in ${toString releaseDays} days" >&2
    done < <(find "$roots" -mindepth 1 -maxdepth 1 -type l \( -name 'sources*' -o -name 'layer-*' \) \
      -mmin +${toString (releaseDays * 24 * 60)} -print0)
  '';
}
