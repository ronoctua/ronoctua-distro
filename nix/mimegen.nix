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
  # THE SECOND NEGATIVE FIXTURE, for the `prefer` block.
  #
  # That block settles which of two types wins a name they both claim, and it
  # does so by REMOVING the loser's glob — so when it works, nothing about the
  # table says it is there. The only way to know the guard still guards is to
  # break the row on purpose and watch the build refuse.
  #
  # This one names a loser that does not claim the pattern, which is what
  # happens the day upstream drops the ambiguity: the row would silently stop
  # applying, and a row that looks deliberate and does nothing is the failure
  # this repository refuses everywhere else.
  preferenceFixture =
    let
      broken = writeText "mime-glyphs-stale-prefer.json" (
        builtins.toJSON (
          let
            t = import mimeTableFile;
          in
          t
          // {
            prefer = map (r: r // { over = "application/pdf"; }) t.prefer;
          }
        )
      );
    in
    stdenvNoCC.mkDerivation {
      name = "ronoctua-mimedb-stale-prefer";
      nativeBuildInputs = [ python3 ];
      dontUnpack = true;
      dontConfigure = true;
      buildPhase = ''
        runHook preBuild
        # NO PIPE, for the reason the fixture above gives.
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
          echo "ERROR: the generator ACCEPTED a `prefer` row whose loser does not" >&2
          echo "      claim the pattern. The row would apply to nothing and say so" >&2
          echo "      to nobody." >&2
          exit 1
        fi
        grep -q "prefer" output.txt || {
          echo "ERROR: it failed for another reason; the message does not name `prefer`" >&2
          exit 1
        }
        mkdir -p $out
        echo "OK: it refused a stale preference, and said which pattern" > $out/verdict
        runHook postBuild
      '';
      installPhase = "true";
    };

  # THE THIRD, for the `drop` block, and for the same reason as the second:
  # a row that removes a glob leaves nothing in the table to say it is there.
  # This one drops a glob from a type that does not claim it — the day
  # upstream stops giving `*.fasl` to Lisp source — and the build has to
  # refuse rather than keep a row that removes nothing.
  dropFixture =
    let
      broken = writeText "mime-glyphs-stale-drop.json" (
        builtins.toJSON (
          let
            t = import mimeTableFile;
          in
          t
          // {
            drop = map (r: r // { mime = "application/pdf"; }) t.drop;
          }
        )
      );
    in
    stdenvNoCC.mkDerivation {
      name = "ronoctua-mimedb-stale-drop";
      nativeBuildInputs = [ python3 ];
      dontUnpack = true;
      dontConfigure = true;
      buildPhase = ''
        runHook preBuild
        # NO PIPE, for the reason the first fixture gives.
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
          echo "ERROR: the generator ACCEPTED a `drop` row whose type does not" >&2
          echo "      claim the pattern. The row would remove nothing and say so" >&2
          echo "      to nobody." >&2
          exit 1
        fi
        grep -q "drop" output.txt || {
          echo "ERROR: it failed for another reason; the message does not name `drop`" >&2
          exit 1
        }
        mkdir -p $out
        echo "OK: it refused a stale drop, and said which pattern" > $out/verdict
        runHook postBuild
      '';
      installPhase = "true";
    };

  # AND THE `claims` REFUSALS, each built broken on its own: a row whose
  # `shares` names a type that does not claim the glob (a tie that is not
  # one), a row that shares a glob upstream claims at another weight (it
  # would not tie), and a row with `content` and no `shares` (nothing to
  # settle). Each must fail, and say the reason it was planted for.
  refusedClaim =
    name: row: owed:
    let
      broken = writeText "mime-glyphs-${name}.json" (
        builtins.toJSON ((import mimeTableFile) // { claims = [ row ]; })
      );
    in
    stdenvNoCC.mkDerivation {
      name = "ronoctua-mimedb-${name}";
      nativeBuildInputs = [ python3 ];
      dontUnpack = true;
      dontConfigure = true;
      buildPhase = ''
        runHook preBuild
        # NO PIPE, for the reason the first fixture gives.
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
          echo "ERROR: the generator ACCEPTED the claim planted as ${name}." >&2
          exit 1
        fi
        grep -qF ${lib.escapeShellArg owed} output.txt || {
          echo "ERROR: it failed for another reason; the message does not say: ${owed}" >&2
          exit 1
        }
        mkdir -p $out
        echo "OK: it refused ${name}, and said why" > $out/verdict
        runHook postBuild
      '';
      installPhase = "true";
    };
  claimsFixtures = {
    not-a-tie = refusedClaim "claim-not-a-tie" {
      glob = "*.lsp";
      mime = "text/x-common-lisp";
      shares = "text/x-opencl-src";
      content = [ "(in-package" ];
    } "the tie is not the one written here";
    wrong-weight = refusedClaim "claim-wrong-weight" {
      glob = "*.sage";
      mime = "text/x-common-lisp";
      shares = "text/x-sagemath";
      content = [ "(in-package" ];
    } "it would not tie";
    content-alone = refusedClaim "claim-content-alone" {
      glob = "*.lsp";
      mime = "text/x-common-lisp";
      content = [ "(in-package" ];
    } "has `content` and no `shares`";
  };
in
base
// {
  inherit
    negativeFixture
    preferenceFixture
    dropFixture
    claimsFixtures
    ;
  # What the check compares against `share/mime/version`.
  pinnedVersion = version;
}
