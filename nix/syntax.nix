# The syntax table, from `assets/syntax.nix` to a Rust table.
#
# Same machine as the update's `sequence.nix`, and for the same reasons: the declaration
# is the source, the Rust is derived, and a guard that has never refused
# anything has not been tested.
#
# WHY IT TAKES `mimeDb`
# ---------------------
# Because a language claims MIME TYPES and must not be allowed to invent one.
# The list of types that exist is published by `nix/mimegen.nix` as
# `types.json` — the same derivation that reads freedesktop's files for the
# Rust table. Reading `share/mime` a second time here would put the pinned
# version `2.4` in two places, and the second one is the one that goes stale.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  mimeDb,
  syntaxFile,
}:

let
  generator = ./scripts/gen-syntax.py;

  asJson = table: writeText "syntax.json" (builtins.toJSON table);

  declared = import syntaxFile;

  # THE NEGATIVE FIXTURE, and it plants TWO defects rather than one.
  #
  # One steals a type another language already claims; the other claims prose.
  # Two, because the two refusals are different in kind — one is about the
  # table disagreeing with itself, the other about the table disagreeing with
  # what colouring is for — and a fixture that only exercised one would leave
  # the other free to rot.
  fixture = {
    languages = declared.languages ++ [
      {
        id = "fixture-steals-json";
        label = "Fixture";
        kind = "code";
        mime = [ "application/json" ];
        tags = [ "fixture-steals" ];
        guessable = false;
        shebang = [ ];
        sniff = [ ];
        sniff-shape = "none";
        line-comment = [ "#" ];
        block-comment = null;
        strings = [ ];
        interpolation = [ ];
        fences = [ ];
        attribute = null;
        numbers = "none";
        property = "none";
        shapes = [ ];
        call = false;
        keywords = [ "fixture" ];
        types = [ ];
        constants = [ ];
      }
      {
        id = "fixture-claims-prose";
        label = "Fixture";
        kind = "code";
        mime = [ "text/plain" ];
        tags = [ "fixture-prose" ];
        guessable = false;
        shebang = [ ];
        sniff = [ ];
        sniff-shape = "none";
        line-comment = [ "#" ];
        block-comment = null;
        strings = [ ];
        interpolation = [ ];
        fences = [ ];
        attribute = null;
        numbers = "none";
        property = "none";
        shapes = [ ];
        call = false;
        keywords = [ "fixture" ];
        types = [ ];
        constants = [ ];
      }
    ];
  };

  mkTable =
    {
      name,
      table,
      expectFailure ? false,
    }:
    stdenvNoCC.mkDerivation {
      inherit name;

      nativeBuildInputs = [ python3 ];
      dontUnpack = true;
      dontConfigure = true;

      buildPhase =
        let
          # A shell FUNCTION and not a command string, for the reason
          # the update's `sequence.nix` and `nix/glyphgen.nix` both record: interpolating
          # a multi-line string and then appending `> file` puts the
          # redirection on a line of its own, where it applies to nothing.
          cmd = ''
            gen() {
              python3 ${generator} \
                --syntax ${asJson table} \
                --types ${mimeDb}/types.json \
                --out-rust "$1"/syntax.rs
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            echo "### negative fixture: the generator MUST fail ###"

            # NO PIPE. In `cmd | tee` the exit status is `tee`'s, which is
            # always zero — and the fixture would report success while the
            # guard was broken.
            ${cmd}
            set +e
            gen $out > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt

            if [ "$rc" = 0 ]; then
              echo "ERROR: the generator ACCEPTED a table in which one language steals" >&2
              echo "       another's type and another claims prose. The guard guards" >&2
              echo "       nothing." >&2
              exit 1
            fi
            # AND IT FAILED FOR THE RIGHT REASONS, both of them. A refusal for
            # the wrong reason is a refusal that will not survive the next edit.
            for owed in "application/json" "fixture-steals-json" "text/plain" "fixture-claims-prose"; do
              grep -q "$owed" $out/out_text.txt || {
                echo "ERROR: it refused without naming \"$owed\"" >&2
                exit 1
              }
            done
            echo "OK: refused, and named both defects"
            runHook postBuild
          ''
        else
          ''
            runHook preBuild
            mkdir -p $out
            ${cmd}
            gen $out
            runHook postBuild
          '';

      installPhase = "true";

      meta.description = "Syntax table generated from assets/syntax.nix";
    };
in
{
  table = mkTable {
    name = "ronoctua-syntax-rs";
    table = declared;
  };

  negativeFixture = mkTable {
    name = "ronoctua-syntax-negative";
    table = fixture;
    expectFailure = true;
  };
}
