# THE KEYS AND THE TEXTS FOR `cargo` IN A CHECKOUT — `ronoctua-keys`, which
# the devshell runs on entry and `.#dev` at its start and whenever a fragment
# changes.
#
# A build gets each crate's `key` file from the evaluation
# (`nix/text-keys.nix`): one directory per unit, holding its own crate's file.
# A `cargo` in a checkout cannot be handed a store path per crate, and one store
# path for all of them would be a NEW path at every key added anywhere — cargo
# sees `RONOCTUA_KEYS_DIR` change and recompiles every crate that speaks. So a
# checkout keeps a directory of its own, OUTSIDE the tree (`lean-tree`), named
# by the checkout's path so two checkouts never share it, and
# `nix/scripts/gen-keys.py` keeps it in step, rewriting only the files whose
# bytes changed: cargo, which tracks the file each crate's `src/key.rs`
# includes, recompiles only the crates whose keys moved. The bytes are the
# evaluation's (`keys-agree`).
#
# AND THE TEXTS OF THE SAME TREE, beside it (`<dir>.texts`): a program cargo
# built from those keys asks its texts for every one of them at its start, and
# the evaluation's texts (`RONOCTUA_TEXTS` of the devshell, of the loop) are
# the tree's as it was when the shell was entered — a key added since would be
# missing from them, and every program refused with 78 until the shell was
# entered again. So the texts are made again here first, from every fragment
# of the tree, by the build's own generator with the build's own arguments
# (`nix/i18n.nix`): a fragment it refuses stops this before any key is
# written, said by the generator. Each language's file is written whole under
# another name and renamed over the old one, so a program starting meanwhile
# reads the old file or the new, never half of one.
#
#   ronoctua-keys [--tree <checkout>] [--dir | --texts-dir] [--written-exit <n>]
#
# `--dir` prints the keys' directory and `--texts-dir` the texts', and neither
# writes anything; `--written-exit` is the status when a key file was written
# or removed (the loop recompiles then). The checkout is git's top level from
# here, or `--tree`.
{
  lib,
  writeShellApplication,
  python3,
  coreutils,
  git,
  languages,
  # THE MODULES' TEXT NAMESPACES, `[ { id; prefixes; } ]`: each module's own
  # folder is read from the tree (`crates/ronoctua-brain/modules/<id>/i18n`),
  # with the prefixes its keys may begin with, as the `texts` derivation hands
  # them to the generator.
  textModules,
}:

writeShellApplication {
  name = "ronoctua-keys";
  runtimeInputs = [
    python3
    coreutils
    git
  ];
  text = ''
    tree=""
    if [ "''${1:-}" = --tree ]; then
      tree=$2
      shift 2
    fi
    if [ -z "$tree" ]; then
      tree=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    fi
    tree=$(cd "$tree" && pwd -P)
    dir="''${XDG_CACHE_HOME:-$HOME/.cache}/ronoctua-distro/keys/$(printf %s "$tree" | sha256sum | cut -c1-16)"
    texts="$dir.texts"
    case "''${1:-}" in
      --dir)
        echo "$dir"
        exit 0
        ;;
      --texts-dir)
        echo "$texts"
        exit 0
        ;;
    esac

    # NEVER INSIDE THE TREE (`lean-tree`), asked before anything is written —
    # the keys' own refusal (`gen-keys.py`) would come after the texts.
    case "$(realpath -m "$dir")/" in
      "$tree"/*)
        echo "ERROR: $dir is inside $tree: the keys and the texts are written outside the tree" >&2
        exit 2
        ;;
    esac

    # THE TEXTS FIRST: every fragment of the tree, as `nix/i18n.nix` hands
    # them — each crate's `i18n/` (a fragment in a crate that does not speak
    # is refused by the keys, below, by name), the extension's, and each
    # module's with its prefixes.
    fragments=()
    for folder in "$tree"/crates/*/i18n "$tree"/extension/i18n; do
      if [ -d "$folder" ]; then
        fragments+=(--fragment "$(basename "$(dirname "$folder")")=$folder")
      fi
    done
    modules=(${
      lib.concatMapStringsSep " " (
        m:
        "--module ${lib.escapeShellArg m.id}=\"$tree/crates/ronoctua-brain/modules/\"${lib.escapeShellArg m.id}/i18n --prefixes ${lib.escapeShellArg "${m.id}=${lib.concatStringsSep "," m.prefixes}"}"
      ) textModules
    })
    rm -rf "$texts.new"
    python3 ${./scripts/gen-i18n.py} "$texts.new" ${lib.escapeShellArgs languages} \
      "''${fragments[@]}" "''${modules[@]}" > /dev/null
    mkdir -p "$texts"
    for language in ${lib.escapeShellArgs languages}; do
      mv -f "$texts.new/$language.ftl" "$texts/$language.ftl"
    done
    rm -rf "$texts.new"

    # THEN THE KEYS.
    exec python3 ${./scripts/gen-keys.py} --tree "$tree" --base ${lib.escapeShellArg (builtins.head languages)} --into "$dir" "$@"
  '';
  meta.description = "Writes a checkout's text key files for cargo, rewriting only what changed, and its texts";
}
