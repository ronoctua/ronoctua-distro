# The MIME database, from freedesktop's files to a Rust table.
#
# WHY THE PACKAGE IS A BUILD INPUT AND NOTHING ELSE
# -------------------------------------------------
# `shared-mime-info`'s closure is 75 MiB, and 66 of those are glib and libxml2.
# Those two belong to `update-mime-database`, the program that COMPILES the
# database — and it has already run, upstream, at package build time. What is
# left in `share/mime` is 125 KB of plain data: `globs2`, `magic`, `subclasses`,
# `aliases`, `generic-icons` and `types`.
#
# So this derivation reads those files and emits Rust. Nothing of the package
# survives into what runs, and `files-closure-has-no-mime-db` asserts it —
# without that check the 75 MiB would come back the first time somebody reached
# for the library instead of the table.
#
# AND IT PUBLISHES WHAT IT READ, in `types.json`
# ----------------------------------------------
# `assets/syntax.nix` claims MIME types, and its generator has to refuse one the
# database does not have. Letting it read `share/mime` for itself would put the
# pinned version in two files, and the second is the one that goes stale — the
# very argument the paragraph below makes about the version. So this derivation
# stays the only reader and writes down the type list, the aliases and the
# subclass pairs beside the Rust.

# THE VERSION IS WRITTEN BY HAND, on purpose
# ------------------------------------------
# Same argument as `nix/lucidegen.nix`: a version that FOLLOWS the package lets
# `nix flake update` swap the whole database silently. Written here, a bump is a
# line somebody had to type, and the generator refuses to run when the package
# disagrees with it.
{
  stdenvNoCC,
  lib,
  python3,
  writeText,
  shared-mime-info,
  glyphdb,
  mimeTableFile,
}:

let
  # It has to match what `share/mime/version` says. The generator checks.
  version = "2.4";

  # The tokens live in Nix (invariant 9) and the generator is Python. The
  # bridge is JSON, generated here — there is no Nix parser in Python, and
  # writing one would be reimplementing Nix logic (invariant 5).
  tableJson = writeText "mime-glyphs.json" (builtins.toJSON (import mimeTableFile));

  base = stdenvNoCC.mkDerivation {
    pname = "ronoctua-mimedb";
    inherit version;

    nativeBuildInputs = [ python3 ];
    dontUnpack = true;
    dontConfigure = true;

    buildPhase = ''
      runHook preBuild
      mkdir -p $out
      python3 ${./scripts/gen-mimedb.py} \
        --mime-dir ${shared-mime-info}/share/mime \
        --glyphs ${tableJson} \
        --icons ${glyphdb.db}/icons.json \
        --out $out/mimedb.rs \
        --out-types $out/types.json \
        --version ${version}
      runHook postBuild
    '';

    installPhase = "true";

    meta.description = "MIME table generated from shared-mime-info";
  };

  # THE NEGATIVE FIXTURE. A guard that has never refused anything has not been
  # tested — and this one guards the case that would otherwise show as a listing
  # of identical squares: a freedesktop release adding a generic icon name that
  # `mime-glyphs.nix` does not answer for.
  #
  # It removes one row that upstream DOES use, and the build has to fail.
  negativeFixture =
    let
      broken = writeText "mime-glyphs-broken.json" (
        builtins.toJSON (
          let
            t = import mimeTableFile;
          in
          t // { generic = builtins.filter (g: g.name != "x-office-spreadsheet") t.generic; }
        )
      );
    in
    stdenvNoCC.mkDerivation {
      name = "ronoctua-mimedb-negative";
      nativeBuildInputs = [ python3 ];
      dontUnpack = true;
      dontConfigure = true;
      buildPhase = ''
        runHook preBuild
        # NO PIPE. In `cmd | tee` the exit status is `tee`'s, which is always 0 —
        # the trap `nix/glyphgen.nix` already records, and paid for once.
        set +e
        python3 ${./scripts/gen-mimedb.py} \
          --mime-dir ${shared-mime-info}/share/mime \
          --glyphs ${broken} \
          --icons ${glyphdb.db}/icons.json \
          --out /dev/null \
          --version ${version} > output.txt 2>&1
        rc=$?
        set -e
        cat output.txt
        if [ "$rc" = 0 ]; then
          echo "ERROR: the generator ACCEPTED a table with no answer for a generic" >&2
          echo "      icon that freedesktop uses. The guard guards nothing." >&2
          exit 1
        fi
        # And it failed for the RIGHT reason, not for a typo in this fixture.
        grep -q "x-office-spreadsheet" output.txt || {
          echo "ERROR: it failed for another reason; the message does not name the icon" >&2
          exit 1
        }
        mkdir -p $out
        echo "OK: it refused, and said which icon was missing" > $out/verdict
        runHook postBuild
      '';
      installPhase = "true";
    };
in
base
// {
  inherit negativeFixture;
  # What the check compares against `share/mime/version`.
  pinnedVersion = version;
}
