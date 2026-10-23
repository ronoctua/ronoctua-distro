# A BATTERY DOES NOT START OVER A BUILD INSIDE THE TREE.
#
# Cargo writes next to the manifest when nothing tells it otherwise, and a
# day of `cargo test` there is tens of gigabytes: this tree once held 18 GB of
# `target/`, and before that a build inside a flake's tree cost 793 GB
# (`lean-tree`). The flake is evaluated as `git+file` and never copies an
# untracked file, so nothing in a check SEES such a directory — it only fills
# the disk the battery is about to need, until the store's floor starts
# collecting what the battery just built. So `.#gate`, `.#most` and `.#quick`
# ask first, and refuse to start, naming what to move.
#
# WHAT A BUILD IS: a directory holding `CACHEDIR.TAG` — the mark cargo writes
# at the root of every target and build directory (and other tools at theirs),
# which says "this is a cache, rebuildable". Builds belong under the cache
# (`nix/cargo-dirs.nix`).
{
  writeShellApplication,
  coreutils,
  findutils,
  git,
}:

writeShellApplication {
  name = "ronoctua-tree-holds-no-build";
  runtimeInputs = [
    coreutils
    findutils
    git
  ];
  text = ''
    root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    built=$(find "$root" -name .git -prune -o -name CACHEDIR.TAG -printf '%h\n' | sort | head -n 20)
    if [ -n "$built" ]; then
      echo "ERROR: a build is inside the tree, and a battery will not start over it:" >&2
      while IFS= read -r dir; do
        echo "       ''${dir#"$root"/} ($(du -sh --apparent-size -- "$dir" 2>/dev/null | cut -f1))" >&2
      done <<< "$built"
      echo "       Builds go under the cache (nix/cargo-dirs.nix); remove these, or move" >&2
      echo "       them there, and run it again." >&2
      exit 1
    fi
  '';
}
