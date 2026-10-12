# The brain's modules, from their manifests to Rust — brain.md §3.
#
# `nix/brain-modules.nix` reads the folders; this runs
# `nix/scripts/gen-brain-modules.py` over what it read and formats the result
# with the pinned `rustfmt`. The output is `RONOCTUA_BRAIN_MODULES_DIR`
# (`nix/rust.nix`, `generated`): one `<id>/manifest.rs` per module, `all.rs`,
# and `ids.txt` — which the brain crate's `build.rs` compares with the folders
# it sees, refusing to build on a stale environment instead of compiling a
# module list that is not the tree's.
#
# FORMATTED HERE, and by the toolchain's own `rustfmt`: the generator writes
# plain Rust, and what the crate includes is what `cargo fmt` would leave — so
# the hand-written golden sample can be compared with it byte for byte.
{
  lib,
  pkgs,
  stdenvNoCC,
  python3,
  writeText,
  rustToolchain,
  languages,
  # The loader's view, `import ./brain-modules.nix { … }`. The flake hands in
  # the real one; `brain-modules-match-the-golden` the fixture's.
  brainModules,
}:

let
  # A MODULE'S `tables.nix`, called with what a generator needs. It answers
  # `{ "<name>.rs" = <file>; }` and each file lands beside the manifest.
  tablesOf =
    m:
    lib.mapAttrs (_: file: "${file}") (
      import (m.dir + "/tables.nix") {
        inherit pkgs lib;
      }
    );

  records = writeText "brain-modules.json" (builtins.toJSON (brainModules.forGenerator tablesOf));
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-brain-modules-rs";
  version = "1";

  nativeBuildInputs = [
    python3
    rustToolchain
  ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    # THE FORMATTER, with the edition the manifest would have told it. A
    # function defined BEFORE the generator's line, so its flags are not read
    # as the generator's (`generator-flags-agree` reads the lines after it).
    formatted() {
      rustfmt --edition 2024 "$1"
    }
    python3 ${./scripts/gen-brain-modules.py} \
      --modules ${records} \
      --parser ${./scripts/gen-i18n.py} \
      --language ${builtins.head languages} \
      --out $out
    # EVERY FILE THIS WROTE, and not the tables: those come out of other
    # generators, formatted or not, and are theirs.
    formatted $out/all.rs
    for manifest in $out/*/manifest.rs; do
      if [ -e "$manifest" ]; then
        formatted "$manifest"
      fi
    done
    runHook postBuild
  '';

  dontInstall = true;

  # THE PERSON'S FOLDER TRAVELS WITH ITS TABLE: `nix/rust.nix` sets
  # `RONOCTUA_BRAIN_PERSON_MODULES_DIR` from here, so the table that lists a
  # person's module and the folder its `module.rs` is read from are one copy
  # — never a table of one build and a folder of another. It is the loader's
  # copy with only the folders that load (`personRefused` are not in it), or
  # `null` for none.
  passthru.personModulesDir = brainModules.personModulesDir or null;

  meta = {
    description = "The brain's modules as Rust, generated from their manifests";
    platforms = lib.platforms.all;
  };
}
