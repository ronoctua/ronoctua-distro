# What the belongings keep, from Nix to Rust.
#
# `assets/belongings.nix` is where it is declared; this turns it into
# `belongings.rs`, which `ronoctua-belongings` includes through
# `RONOCTUA_BELONGINGS_RS` (a `generated` input of `nix/rust.nix`: it is CODE —
# the rules the engine walks the home with — and a few kilobytes), and into
# `belongings.json`, the same table normalised for the checks. The script is
# `nix/scripts/gen-belongings.py`, and what it refuses is listed at its top.
#
# THE NEGATIVE FIXTURE proves the refusals: a field nobody knows, a path that
# climbs out of its root, a hard name pattern and a reason that is not an id.
# Without it "the generator refuses" would be an untested claim.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  belongingsFile,
}:

let
  generator = ./scripts/gen-belongings.py;
  declared = import belongingsFile;
  asJson = name: value: writeText name (builtins.toJSON value);

  fixture = declared // {
    colour = "blue";
    files = declared.files // {
      "home/../etc/shadow" = "preference";
    };
    never = declared.never ++ [
      {
        name = "*.lock";
        reason = "draft";
        hard = true;
      }
      {
        path = "config/somewhere";
        reason = "Not An Id";
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
      dontInstall = true;
      buildPhase =
        let
          # A shell FUNCTION, for the reason `nix/updates.nix` records: a
          # redirection after an interpolated multi-line command lands on a
          # line of its own and applies to nothing.
          cmd = ''
            gen() {
              python3 ${generator} \
                --table ${asJson "belongings.json" table} \
                --out-rust "$1"/belongings.rs \
                --out-json "$1"/belongings.json
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
              echo "ERROR: the generator ACCEPTED an unknown field, a climbing path, a hard pattern and a bad reason" >&2
              exit 1
            fi
            grep -q "colour. is not a field" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the unknown field" >&2; exit 1; }
            grep -q "home/../etc/shadow. climbs" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the climbing path" >&2; exit 1; }
            grep -q "a name pattern may not be hard" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the hard name pattern" >&2; exit 1; }
            grep -q "Not An Id. is not an English kebab id" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the reason that is not an id" >&2; exit 1; }
            echo "OK: an unknown field, a climbing path, a hard pattern and a bad reason take the build down"
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
    };
in
{
  # The table the crate includes, and its JSON twin for the checks.
  table = mkTable {
    name = "ronoctua-belongings-table";
    table = declared;
  };
  # The generator refusing a broken table — a check.
  refuses = mkTable {
    name = "ronoctua-belongings-table-refuses";
    table = fixture;
    expectFailure = true;
  };
  inherit declared;
}
