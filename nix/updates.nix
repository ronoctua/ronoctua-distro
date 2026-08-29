# The update sequence, from Nix to Rust and to two shell runners.
#
# "One click updates everything." One file (`assets/updates.nix`) becomes four
# things, and the fourth is the one that matters most:
#
#   updates.rs         the table a view can draw, self-contained
#   keys.txt           the text keys BOTH languages have to carry
#   the two runners    ronoctua-update (the person's half, and the CLI)
#                      ronoctua-update-system (the root half)
#   a refusal          an invalid step takes the build down, with its id
#
# TWO DERIVATIONS AND NOT ONE, and the split is a cost measured elsewhere in
# this repository. `table` is built by the flake and carries NO store path: the
# command crosses into Rust as basenames, so editing a step does not put a new
# reference into anything that compiles. `mkScripts` is built by
# `nix/modules/update.nix`, where the paths and the unit names live, and it is
# the only half that pays for them.
#
# Both halves run the SAME validation. A table that a later wave breaks fails
# the flake's own build, not only a `nixos-rebuild` on somebody's machine.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  shellcheck,
  updatesFile,
}:

let
  generator = ./scripts/gen-updates.py;

  # The bridge is JSON, generated here — there is no Nix parser in Python, and
  # writing one would be reimplementing Nix logic (invariant 5).
  asJson = table: writeText "updates.json" (builtins.toJSON table);

  declared = import updatesFile;
  tableJson = asJson declared;

  # THE NEGATIVE FIXTURE, in the shape `nix/glyphgen.nix` already uses.
  #
  # Two steps, two refusals, and they are the two the whole design rests on:
  # one ships a bare binary ON (invariant 6, which this feature walks right up
  # to), and one installs something (which no step may ever do). Without this,
  # "the generator refuses" is an untested claim about the only code in this
  # repository that decides what runs on somebody's machine.
  fixtureTable = declared // {
    steps = declared.steps ++ [
      {
        id = "fixture-ships-on";
        does = "refresh-cache";
        root = false;
        binary = "mise";
        run = [ "upgrade" ];
        needs = [ { binary = "mise"; } ];
        on-failure = "carry-on";
        enable = true;
      }
      {
        id = "fixture-installs";
        does = "refresh-cache";
        root = false;
        binary = "npm";
        run = [
          "install"
          "--global"
          "typescript"
        ];
        needs = [ { binary = "npm"; } ];
        on-failure = "carry-on";
        enable = false;
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
          # `nix/glyphgen.nix` records: interpolating a multi-line string and
          # then appending `> file` puts the redirection on a line of its own,
          # where it applies to nothing.
          cmd = ''
            gen() {
              python3 ${generator} \
                --updates ${asJson table} \
                --emit table \
                --out-rust "$1"/updates.rs \
                --out-keys "$1"/keys.txt
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            echo "### negative fixture: the generator MUST fail ###"

            # NO pipe: in `cmd | tee` the exit code is `tee`'s, which is always
            # zero — and the fixture would report success while the guard was
            # broken.
            ${cmd}
            set +e
            gen $out > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt

            if [ $rc -eq 0 ]; then
              echo "" >&2
              echo "ERROR: the generator ACCEPTED a step that installs and a step" >&2
              echo "      born switched on citing a binary from outside the flake." >&2
              exit 1
            fi
            grep -q "invariant 6" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the switched-on step" >&2; exit 1; }
            grep -q "install" $out/out_text.txt \
              || { echo "ERROR: it failed, but did not refuse the installation" >&2; exit 1; }
            echo "OK: a step that installs and a step switched on from outside the flake take the build down"
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

      dontInstall = true;

      meta = {
        description = "Update sequence table, generated from assets/updates.nix";
        platforms = lib.platforms.all;
      };
    };
in
{
  # The table and the keys. No store path crosses into it — see the header.
  table = mkTable {
    name = "ronoctua-updates-rs";
    table = declared;
  };

  # Proves the guard works.
  negativeFixture = mkTable {
    name = "ronoctua-updates-negative-fixture";
    table = fixtureTable;
    expectFailure = true;
  };

  # THE TWO RUNNERS, with the paths resolved.
  #
  # `programs` maps the table's `package` to a derivation, and `wiring` carries
  # the paths and unit names `nix/modules/update.nix` owns. The generator
  # refuses BOTH directions: a package no step names is a dependency carried in
  # the closure of a unit that runs as root for nothing, and a step whose
  # package was not resolved is a step that could never run.
  mkScripts =
    {
      programs,
      wiring,
    }:
    stdenvNoCC.mkDerivation {
      name = "ronoctua-update-runners";

      # SHELLCHECK ON THE GENERATED SHELL. What a person writes gets read by a
      # person; what a generator writes gets read by nobody until it misbehaves
      # on a machine at the end of a twenty-minute rebuild.
      nativeBuildInputs = [
        python3
        shellcheck
      ];
      dontUnpack = true;
      dontConfigure = true;

      buildPhase = ''
        runHook preBuild
        mkdir -p $out/bin $out/libexec
        python3 ${generator} \
          --updates ${tableJson} \
          --emit scripts \
          --programs ${
            writeText "update-programs.json" (builtins.toJSON (lib.mapAttrs (_: p: "${p}") programs))
          } \
          --wiring ${writeText "update-wiring.json" (builtins.toJSON wiring)} \
          --out-dir $out/bin
        shellcheck -s bash $out/bin/ronoctua-update $out/bin/ronoctua-update-system

        # THE ROOT HALF IS NOT ON ANYBODY'S PATH. It is what the system unit
        # runs, and a person who typed it would get a permission error instead
        # of the sequence — `ronoctua-update` is the door, and it is the one in
        # `bin/`.
        mv $out/bin/ronoctua-update-system $out/libexec/
        runHook postBuild
      '';

      dontInstall = true;

      meta = {
        description = "The update sequence's two scripts, generated from assets/updates.nix";
        platforms = lib.platforms.all;
        mainProgram = "ronoctua-update";
      };
    };
}
