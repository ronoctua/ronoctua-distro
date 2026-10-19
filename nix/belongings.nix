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
    # A TARGET WHOSE `private` IS NOT A BOOLEAN, and chunk sizes out of order.
    targets = declared.targets // {
      ssh = {
        paths = [ "home/.ssh" ];
        private = "yes";
      };
      # A PATH TWO TARGETS OWN, and the deploy key as a target (the access
      # would travel).
      doubled = {
        paths = [ "home/.shellrc.local" ];
      };
      # A PATH CARRIED OUT OF NOTHING SKIPPED.
      strange = {
        paths = [ "home/.strange" ];
        carry = [ "plain/file" ];
      };
      keys = {
        paths = [ "config/belongings-keys/deploy" ];
      };
    };
    # A PLAIN ROOT INSIDE ANOTHER: one file, two places in the repository.
    plain = declared.plain // {
      "state/person/inner" = {
        at = "inner";
        reason = "person-data";
      };
    };
    chunk = {
      minKiB = 4096;
      avgKiB = 1024;
      maxKiB = 256;
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
            grep -q "targets.ssh: .private. is a boolean" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the target whose private is not a boolean" >&2; exit 1; }
            grep -q "chunk: the sizes are not ordered" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the chunk sizes out of order" >&2; exit 1; }
            grep -q "plain: .state/person. and .state/person/inner. overlap" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the plain root inside another" >&2; exit 1; }
            grep -q "targets: .home/.shellrc.local. of .doubled. and .home/.shellrc.local. of .shell. overlap" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse a path two targets own" >&2; exit 1; }
            grep -q "targets.keys: .config/belongings-keys/deploy. overlaps .config/belongings-keys., which is never kept" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the deploy key as a target" >&2; exit 1; }
            grep -q "targets.strange: .plain/file. carries what nothing it skips holds" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse a carry out of nothing skipped" >&2; exit 1; }
            echo "OK: an unknown field, a climbing path, a hard pattern, a bad reason, a bad target, unordered chunks, overlapping plain roots, overlapping targets and a carry out of nothing take the build down"
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
