# THE COMPILED UNITS OF THE VERSION IN USE, held by a garbage-collector root —
# `ronoctua-keep-units`, run by `ronoctua-machine apply` after a build that
# worked and by `nix/kept-sources.nix` on a checkout.
#
# # Why they need one
#
# The Rust build is one derivation per compilation unit (`nix/rust/units.nix`),
# and a program references none of the units it was linked from (they are
# `disallowedReferences`): what a system carries is the programs. So after a
# build every library unit is garbage, and the update's own weekly collection
# — or Nix's floor, when the disk runs short (`nix/modules/store.nix`) — takes
# them. The next update then compiles every library again to change one line,
# which is the whole build the per-unit derivations exist to avoid. A machine
# keeps them instead (the owner's decision, 2026-10-12): not a cache shared
# between machines, the machine's own build of the version it runs.
#
# # Only what is there, and only the current set
#
# It is given the CANDIDATES — every output of every unit of the version just
# built (`rust.release.keep`, as paths: `out` and `metrics`) — and roots the
# ones the store holds.
# Rooting is never a reason to compile: a machine of the minimal profile never
# built the units only the full one needs, and they are not rooted, not built.
# Each run makes a new generation of the set (`<set>@<stamp>/unit`,
# `unit-2`, …, one indirect root each) and only then removes the older ones:
# never fewer roots than before, for an instant. A set equal to the last one
# only marks it fresh.
#
# `--check-validity` and not `-e`: a path half-deleted by a running collection
# still exists and is no longer valid.
{
  writeShellApplication,
  coreutils,
  findutils,
  gawk,
  gnugrep,
  util-linux,
  # The Nix whose store is held: the machine's own (`config.nix.package`)
  # on a machine, the flake's on a checkout.
  nix,
}:

writeShellApplication {
  name = "ronoctua-keep-units";
  runtimeInputs = [
    nix
    coreutils
    findutils
    gawk
    gnugrep
    util-linux
  ];
  text = ''
    usage() {
      echo "usage: ronoctua-keep-units --roots DIR --set NAME [--release-days N] PATHS-FILE"
      echo "roots, in a new generation of DIR/NAME@<stamp>/, every path of PATHS-FILE"
      echo "(one per line) the store holds, then removes the set's older generations;"
      echo "with --release-days, also every set of DIR nobody kept in N days"
    }
    roots="" set="" file="" days=""
    needs() {
      if [ "$1" -lt 2 ]; then
        echo "ronoctua-keep-units: $2 needs a value" >&2
        exit 2
      fi
    }
    while [ $# -gt 0 ]; do
      case "$1" in
        --roots) needs $# "$1"; roots=$2; shift ;;
        --set) needs $# "$1"; set=$2; shift ;;
        --release-days) needs $# "$1"; days=$2; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) echo "ronoctua-keep-units: unknown option $1" >&2; exit 2 ;;
        *) file=$1 ;;
      esac
      shift
    done
    if [ -z "$roots" ] || [ -z "$set" ] || [ -z "$file" ]; then
      usage >&2
      exit 2
    fi
    case "$set" in
      *[!A-Za-z0-9._-]* | .* | "") echo "ronoctua-keep-units: '$set' is not a set's name" >&2; exit 2 ;;
    esac
    if [ -n "$days" ] && ! [[ "$days" =~ ^[0-9]+$ ]]; then
      echo "ronoctua-keep-units: $days is not a count of days" >&2
      exit 2
    fi
    mapfile -t wanted < <(grep -v '^$' "$file" | sort -u)
    for p in "''${wanted[@]}"; do
      case "$p" in
        /nix/store/*) ;;
        *) echo "ronoctua-keep-units: $p is not a store path" >&2; exit 2 ;;
      esac
    done

    # WHAT THE STORE HOLDS: the candidates minus the invalid ones.
    valid=()
    if [ "''${#wanted[@]}" -gt 0 ]; then
      mapfile -t invalid < <(nix-store --check-validity --print-invalid "''${wanted[@]}")
      declare -A gone=()
      for p in "''${invalid[@]}"; do gone[$p]=1; done
      for p in "''${wanted[@]}"; do
        [ -n "''${gone[$p]:-}" ] || valid+=("$p")
      done
    fi

    mkdir -p "$roots"
    # ONE RUN OF A SET AT A TIME: two at once would each remove the other's
    # generation, and leave none.
    exec 9> "$roots/.$set.lock"
    flock 9
    # THE LAST GENERATION, and what it holds.
    last=$(find "$roots" -mindepth 1 -maxdepth 1 -type d -name "$set@*" -printf '%T@ %p\n' \
      | sort -n | tail -n 1 | cut -d' ' -f2-)
    held=""
    if [ -n "$last" ]; then
      held=$(find "$last" -mindepth 1 -maxdepth 1 -type l -printf '%l\n' | sort)
    fi
    now=$(printf '%s\n' "''${valid[@]}" | grep -v '^$' || true)

    if [ -n "$last" ] && [ "$held" = "$now" ]; then
      # THE SAME SET: marked fresh, nothing rooted twice.
      touch -- "$last"
      gen=$last
    else
      gen="$roots/$set@$(date -u +%Y%m%dT%H%M%S)-$$"
      mkdir -- "$gen"
      if [ "''${#valid[@]}" -gt 0 ]; then
        if ! nix-store --realise --add-root "$gen/unit" --indirect "''${valid[@]}" > /dev/null; then
          # A path collected between the question and the root: this set
          # is not made, and the last one keeps holding what it held.
          rm -rf -- "$gen"
          echo "ronoctua-keep-units: could not root the $set set; the last one stays" >&2
          exit 1
        fi
      fi
      # Only now the older generations go.
      find "$roots" -mindepth 1 -maxdepth 1 -type d -name "$set@*" ! -path "$gen" \
        -exec rm -rf -- {} +
    fi
    # EVERY SET NOBODY KEPT IN THE DAYS GIVEN — another checkout's, one that
    # is gone: a generation's own time is when it was last kept.
    if [ -n "$days" ]; then
      while IFS= read -r -d "" stale; do
        rm -rf -- "$stale"
        echo "released $(basename -- "$stale"): nobody kept it in $days days" >&2
      done < <(find "$roots" -mindepth 1 -maxdepth 1 -type d -name '*@*' ! -path "$gen" \
        -mmin +$(( days * 24 * 60 )) -print0)
    fi
    bytes=0
    if [ "''${#valid[@]}" -gt 0 ]; then
      bytes=$(nix-store --query --size "''${valid[@]}" | awk '{ s += $1 } END { print s + 0 }')
    fi
    printf 'kept %s of %s paths of the %s set (%s MiB)\n' \
      "''${#valid[@]}" "''${#wanted[@]}" "$set" "$(( bytes / 1048576 ))"
  '';

  meta.description = "roots the compiled units of the version in use that the store holds";
}
