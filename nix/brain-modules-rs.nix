# The brain's modules, from their manifests to Rust — brain.md §3.
#
# `nix/brain-modules.nix` reads the folders; this runs
# `nix/scripts/gen-brain-modules.py` over what it read and formats the result
# with the pinned `rustfmt`. ONE PART PER CRATE OF THE BRAIN (`part`), each a
# derivation of its own, so that what one reads never moves the other:
#
#   "library"  `RONOCTUA_BRAIN_MODULES_DIR` (`nix/rust.nix`, `generated`), for
#              `ronoctua-brain`, which the footer links: one `<id>/manifest.rs`
#              per DISTRO module, `distro.rs`, `ids.txt` and `shared.txt`. It
#              is handed the distro's records alone, so a person's module —
#              on a machine that has one — leaves this derivation as it was.
#   "daemon"   `RONOCTUA_BRAIN_REGISTRY_DIR`, for `ronoctua-brain-daemon`: the
#              person's manifests, `all.rs` over every module, and `ids.txt` —
#              which the daemon's `build.rs` compares with the folders it
#              sees, refusing to build on a stale environment instead of
#              compiling a module list that is not the tree's. The one a
#              machine overrides (`nix/person-brain.nix`).
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
  # Which crate's part: "library" or "daemon" (the header says what each holds).
  part,
}:

assert lib.assertOneOf "the brain's generated part" part [
  "library"
  "daemon"
];

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

  # THE LIBRARY'S PART READS THE DISTRO'S RECORDS ALONE: filtered here, by
  # whose each is, before anything is written — so the file it is handed, and
  # with it this derivation, is the same with or without a person's module.
  # The daemon's part writes no distro manifest (the library's are
  # re-exported), so a distro record reaches it without its key names and
  # tables: an edit of either is not an input of the daemon's registry.
  every = brainModules.forGenerator tablesOf;
  handed =
    if part == "library" then
      builtins.filter (m: m.origin == "distro") every
    else
      map (
        m:
        if m.origin == "distro" then
          removeAttrs m [
            "names"
            "tables"
          ]
        else
          m
      ) every;
  records = writeText "brain-${part}-modules.json" (builtins.toJSON handed);
in
stdenvNoCC.mkDerivation {
  pname = if part == "library" then "ronoctua-brain-manifests-rs" else "ronoctua-brain-modules-rs";
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
      --part ${part} \
      --modules ${records} \
      --language ${builtins.head languages} \
      --out $out
    # EVERY FILE THIS WROTE, and not the tables: those come out of other
    # generators, formatted or not, and are theirs.
    formatted $out/${if part == "library" then "distro.rs" else "all.rs"}
    for manifest in $out/*/manifest.rs; do
      if [ -e "$manifest" ]; then
        formatted "$manifest"
      fi
    done
    runHook postBuild
  '';

  dontInstall = true;

  # THE PERSON'S FOLDER TRAVELS WITH ITS TABLE: `nix/rust.nix` sets
  # `RONOCTUA_BRAIN_PERSON_MODULES_DIR` from the daemon's part, so the table
  # that lists a person's module and the folder its `module.rs` is read from
  # are one copy — never a table of one build and a folder of another. It is
  # the loader's copy with only the folders that load (`personRefused` are not
  # in it), or `null` for none — and always `null` on the library's part,
  # which compiles no person's module.
  passthru.personModulesDir =
    if part == "daemon" then brainModules.personModulesDir or null else null;

  meta = {
    description =
      if part == "library" then
        "The distro's brain modules' manifests as Rust, for the library the footer links"
      else
        "The brain's registry as Rust, the person's manifests included, for the daemon";
    platforms = lib.platforms.all;
  };
}
