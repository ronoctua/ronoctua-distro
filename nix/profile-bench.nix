# `nix run .#profile-bench` — the release profile chosen by measurement
# (nix-design §3, lane N5).
#
# For each variant of `nix/rust/profiles.nix` (A thin LTO cgu 1, B no LTO
# cgu 1, C no LTO cgu 16, D thin LTO cgu 16, E = A with the shell alone in
# sixteen codegen units): its whole release graph is built (the cold build),
# `.#bench` (`nix/bench.nix`) is run over its programs, five rounds with the
# variants interleaved so that whatever else the machine does falls on all of
# them alike, and `nix/scripts/profile-bench.py` applies the rule:
#
#   acceptable  every median ≤ 1.03× A's, every p99 ≤ 1.05× A's, every
#               budget of the benchmark passing and no program of the
#               variant failing in any round (a number measured with A's
#               very program, and the size of every program, are reported,
#               not judged);
#   winner      among the acceptable, the lowest cost of one edited line in
#               a release program, then the lowest cold build.
#
# # Why the app builds the variants by name, at run time
#
# A variant's units exist only once its plan is read from a build
# (import-from-derivation, `nix/rust/profiles.nix`). An app whose script named
# them would make every evaluation of the flake — `nix flake check` among
# them — build a planner per variant first. So the script names the flake
# attribute (`legacyPackages.<system>.profileBench.<letter>`) and `nix build`s
# it when it runs, and the letters it passes come from the data table
# (`nix/rust/profile-variants.nix`); evaluating this app builds nothing.
#
# # Where it writes
#
# Under `$XDG_CACHE_HOME/ronoctua-distro/profile-bench/<time>/` (or `--out`),
# never in the tree and never in `/tmp`: one directory per variant with the
# logs of its rounds and of its build, `t/` (the `TMPDIR` every round runs
# with, so the nested compositor's folder is on disk too), and `report.json`
# / `report.md` with every number the decision read. `profile-bench report <dir>` recomputes the
# decision from a directory without building or measuring anything.
#
# # Sized as every machine builds
#
# A variant's units are other derivations than the tree's, so `.#sized-build`
# (whose heavy units are the tree's) does not know them: each variant has its
# own (`sized`, the same `nix/build-sizing.nix` over the variant's heavy units,
# `rust.releaseHeavyOf`). Its layer is built first and alone, one cargo with as
# many compilations inside it as fit; then its units, the bench and the bundle
# through that program — the shell's thin-LTO link alone, with the cores the
# memory leaves it, everything else as many Rust units as fit. The edit costs
# and the cold build the decision reads are the units' own measured seconds,
# and a heavy unit's depend on its cores: measured at the general numbers they
# would be another machine's.
{
  lib,
  pkgs,
  system,
  # `nix/rust.nix`'s `profileVariants`, and the heavy units of a release
  # graph (`releaseHeavyOf`).
  profileVariants,
  releaseHeavyOf,
  # What `nix/bench.nix` and `nix/bench-frames.nix` take besides the programs.
  scroll,
  graphicsHost,
  themes,
  component,
}:

let
  # THE BUILDS ARE SIZED TO THE MACHINE (`nix/build-sizing.nix`) unless
  # `--max-jobs` and `--cores` say otherwise: the cold build is measured at
  # what this machine holds, the heavy unit alone (see the header).
  sizing = pkgs.callPackage ./build-sizing.nix { };

  framesOf =
    shell:
    pkgs.callPackage ./bench-frames.nix {
      inherit scroll graphicsHost;
      ronoctua-shell = shell;
    };

  # `.#bench`, over one variant's programs.
  runnerOf =
    letter: v:
    pkgs.writeShellApplication {
      name = "ronoctua-bench-profile-${lib.toLower letter}";
      text = import ./bench.nix {
        benches = v.bundle;
        frames = framesOf v.bundle;
        inherit themes component;
      };
    };

  variants = lib.mapAttrs (
    letter: v:
    v
    // {
      runner = runnerOf letter v;
      sized = sizing.sizedBuild (releaseHeavyOf v.units);
    }
  ) profileVariants.variants;

  # The letters, from the data table: naming them forces no variant.
  letters = lib.attrNames (import ./rust/profile-variants.nix).variants;

  app = pkgs.writeShellApplication {
    name = "ronoctua-profile-bench";
    runtimeInputs = [
      pkgs.python3
      pkgs.git
      pkgs.coreutils
    ];
    text = ''
      exec python3 ${./scripts/profile-bench.py} \
        --attr "legacyPackages.${system}.profileBench" \
        --variants ${lib.concatStringsSep "," letters} \
        --sizing ${sizing.program}/bin/ronoctua-build-sizing \
        "$@"
    '';
  };
in
{
  inherit variants app;
}
