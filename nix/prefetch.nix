# The sources no cache has, fetched with a downloader that RESUMES.
#
# # The defect
#
# `nix run .#gate` could not finish on this network, and the reason had nothing
# to do with this repository. One source in the `full` profile — Android Studio,
# 1.37 GB — comes from a single upstream URL, and the path to it resets the
# connection partway through:
#
#   13  1.37G  13 186.5M  ...  15.43M
#   curl: (56) Recv failure: Connection reset by peer
#
# Reproduced outside Nix, with plain `curl` and no sandbox: 104 MB at 13.6 MB/s
# and then the same reset. It is not the store, not the sandbox, and not a
# package that disappeared upstream.
#
# What turns a flaky transfer into a wall is that `fetchurl` retries WITHOUT
# resuming. Four attempts, each starting from zero, each dying around 100-190 MB
# of 1370: the file is never finished, and the message is
# `cannot download ... from any mirror`, which reads like the file is gone.
#
# # Why no cache saves it
#
# Android Studio is unfree, so Hydra never builds it and `cache.nixos.org` has
# no copy — measured: `path-info` answers `is not valid`. The hashed mirror
# (`tarballs.nixos.org`) answers 404 for the same hash. Every machine that
# installs the `full` profile downloads this file from Google, once, over
# whatever path it has.
#
# # What this does
#
# `curl --continue-at -` in a loop, then `nix hash file` against the hash the
# derivation already declares, then `nix-store --add-fixed`. After that the
# fixed-output derivation is already valid and Nix never fetches it.
#
# NOTHING HERE IS HAND-WRITTEN. The name, the URL and the hash come from
# `pkgs.<attr>.unwrapped.src`, so they cannot drift from what the build wants;
# what a person maintains is one attribute name.
{
  writeShellApplication,
  coreutils,
  curl,
  nix,
  lib,
  # Each entry is `{ name, url, hash, path }`, derived and not written.
  sources ? [ ],
}:

let
  # One line per source, and the shell reads them as a table. Built here so the
  # script body stays a single string with no nesting.
  table = lib.concatMapStringsSep "\n" (s: "${s.path}\t${s.name}\t${s.hash}\t${s.url}") sources;
in
writeShellApplication {
  name = "ronoctua-prefetch";
  runtimeInputs = [
    coreutils
    curl
    nix
  ];
  text = ''
    # HOW MANY TIMES TO RESUME BEFORE GIVING UP.
    #
    # Each round makes progress, because `--continue-at -` starts where the last
    # one stopped. Measured on the failing path: the first round reached 104 MB
    # and the resumed one carried on past 400 MB without a reset. The cap is
    # here so a genuinely dead URL still ends, rather than looping forever.
    rounds=40

    # THE VALIDITY QUERY IS TESTED BEFORE IT IS TRUSTED.
    #
    # The whole "already in the store" branch is one command, and if it ever
    # stops meaning what it means the failure is SILENT: every path looks
    # invalid, everything is downloaded again, and the only symptom is a gate
    # that got slower. It happened while this was being written — the first
    # spelling was `nix-store --query --valid`, which is not a flag, and the
    # skip branch never ran once.
    #
    # So it is asked about a path that is valid by construction: this script's
    # own. If the answer is no, the query is broken, not the store.
    self="${placeholder "out"}"
    if ! nix-store --check-validity "$self" 2>/dev/null; then
      echo "ERROR: the validity query does not work in this version of nix." >&2
      echo "      It answered that $self is not in the store, and it is —" >&2
      echo "      and without it everything would be downloaded again, in silence." >&2
      exit 1
    fi

    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT

    fetched=0
    already=0

    while IFS=$'\t' read -r path name want url; do
      [ -n "$path" ] || continue
      if nix-store --check-validity "$path" 2>/dev/null; then
        already=$((already + 1))
        continue
      fi

      echo "== $name"
      echo "   $url"
      # THE FILE IS NAMED AFTER THE SOURCE, and that is not cosmetic:
      # `nix-store --add-fixed` takes the store path's name from the FILE's
      # basename and has no flag to override it. A temp name would produce a
      # valid path with the wrong name, and the build would fetch anyway.
      file="$work/$name"
      ok=0
      for _ in $(seq "$rounds"); do
        curl --fail --location --silent --show-error \
          --continue-at - --output "$file" "$url" || true
        # THE HASH DECIDES, NOT curl's EXIT CODE. A resumed request against an
        # already-complete file answers 416, which `--fail` reports as an
        # error — and the file in hand is the right one.
        if [ "$(nix hash file --type sha256 --sri "$file" 2>/dev/null)" = "$want" ]; then
          ok=1
          break
        fi
        echo "   retomando em $(stat -c%s "$file" 2>/dev/null || echo 0) bytes"
      done

      if [ "$ok" != 1 ]; then
        echo "ERROR: $name did not come whole after $rounds resumptions." >&2
        echo "      Last size: $(stat -c%s "$file" 2>/dev/null || echo 0) bytes." >&2
        echo "      Expected: $want" >&2
        exit 1
      fi

      nix-store --add-fixed sha256 "$file" >/dev/null
      rm -f "$file"
      fetched=$((fetched + 1))
      echo "   no store"
    done <<'TABLE'
    ${table}
    TABLE

    echo "prefetch: $fetched baixados, $already ja estavam no store"
  '';
}
