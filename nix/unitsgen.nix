# The calculator's unit table, from Nix to Rust.
#
# `assets/units.nix` is where the units are declared; this turns it into
# `units.rs`, which `ronoctua-calc` includes through `RONOCTUA_UNITS_RS` (a
# `generated` input of `nix/rust.nix`: it is CODE — a table the engine matches
# on — and small), and into `units.json`, the same table normalised for the
# checks. The script is `nix/scripts/gen-units.py`, and what it refuses is
# listed at its top.
#
# THE NEGATIVE FIXTURE proves the refusals the design leans on: a symbol and a
# word that name two units (the word across two languages), a unit a language
# has no words for, and a currency code that is not one. Without it "the
# generator refuses" would be an untested claim.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  unitsFile,
  languages,
}:

let
  generator = ./scripts/gen-units.py;
  declared = import unitsFile;
  asJson = name: value: writeText name (builtins.toJSON value);
  languagesJson = asJson "languages.json" languages;

  fixture = declared // {
    units = declared.units ++ [
      {
        # `metre` already answers to `m`.
        id = "fixture-duplicate";
        category = "length";
        dimension.length = 1;
        factor = "2";
        symbols = [ "m" ];
        words.en = [
          "fixture"
          "fixtures"
        ];
        words.pt = [
          "fixture"
          "fixtures"
        ];
      }
      {
        # `metro` is Portuguese for `metre` already; here it is English, for
        # another unit — one meaning per word across languages.
        id = "fixture-word";
        category = "length";
        dimension.length = 1;
        factor = "4";
        symbols = [ ];
        words.en = [
          "fixture word"
          "metro"
        ];
        words.pt = [
          "palavra de teste"
          "palavras de teste"
        ];
      }
      {
        # No Portuguese at all.
        id = "fixture-wordless";
        category = "length";
        dimension.length = 1;
        factor = "3";
        symbols = [ ];
        words.en = [
          "wordless"
          "wordlesses"
        ];
      }
    ];
    # Lowercase: not an ISO 4217 code.
    quoted = declared.quoted ++ [ "usd" ];
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
      dontInstall = true;
      buildPhase =
        let
          # A shell FUNCTION, for the reason the update's `sequence.nix` records: a
          # redirection after an interpolated multi-line command lands on a line
          # of its own and applies to nothing.
          cmd = ''
            gen() {
              python3 ${generator} \
                --units ${asJson "units.json" table} \
                --languages ${languagesJson} \
                --out-rust "$1"/units.rs \
                --out-json "$1"/units.json
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            ${cmd}
            set +e
            gen $out > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt
            if [ $rc -eq 0 ]; then
              echo "ERROR: the generator ACCEPTED a symbol and a word that name two units, a unit with no Portuguese and a code that is not one" >&2
              exit 1
            fi
            grep -q "fixture-duplicate.*(\*symbol) already means" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the duplicate symbol" >&2; exit 1; }
            grep -q "fixture-word.*.metro. (word) already means the unit .metre." $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the word that already means metre" >&2; exit 1; }
            grep -q "fixture-wordless.*no words in .pt." $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the unit without Portuguese" >&2; exit 1; }
            grep -q "quoted code .* .usd.: not an ISO 4217 code" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the lowercase code" >&2; exit 1; }
            echo "OK: a duplicate symbol, a duplicate word, a unit without words in a language and a bad code take the build down"
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
      meta = {
        description = "The calculator's unit table, generated from assets/units.nix";
        platforms = lib.platforms.all;
      };
    };
in
{
  table = mkTable {
    name = "ronoctua-units-rs";
    table = declared;
  };

  negativeFixture = mkTable {
    name = "ronoctua-units-negative-fixture";
    table = fixture;
    expectFailure = true;
  };
}
