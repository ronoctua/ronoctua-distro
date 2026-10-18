# THE BUILD'S OWN CHECKS — "cargo plans, Nix builds" (nix/rust/, lane N1).
#
# Joined into `tests/checks.nix` with one line; the granularity checks, the
# guards over the layers and the census of what crates read come with lanes
# of their own.
{
  pkgs,
  lib,
  rust,
  mkCheck,
}:

let
  driver = ../nix/scripts/rust-unit.py;

  # A UNIT'S OWN DESCRIPTION, run by the driver here, outside its derivation:
  # the same `unit` the derivation carries (structured attributes), written to
  # the file the driver reads, over a copy of the unit's sources. `edit` is
  # the sabotage the check makes of it.
  rerun =
    {
      drv,
      mode,
      edit ? (unit: unit),
    }:
    let
      attrs = pkgs.writeText "${drv.name}-attrs.json" (builtins.toJSON { unit = edit drv.unit; });
    in
    ''
      rm -rf work && mkdir -p work/top work/out work/metrics
      cp -r --no-preserve=mode ${drv.src} work/top/source
      (
        cd work/top/source
        NIX_ATTRS_JSON_FILE=${attrs} NIX_BUILD_TOP=$PWD/.. out=$PWD/../../out metrics=$PWD/../../metrics \
          python3 ${driver} ${mode}
      )
    '';
  unitInputs =
    drv:
    [
      pkgs.python3
      pkgs.coreutils
      pkgs.gnugrep
    ]
    ++ drv.nativeBuildInputs
    ++ drv.buildInputs;

  # The two units the refusal is proved on, both small: a library whose own
  # code reads four programs' paths with `option_env!` (its dependencies are
  # all third-party), and a crate whose build script reads its generated
  # table.
  archive = rust.release.libs.ronoctua-archive;
  tokens = rust.release.libs.ronoctua-tokens;
  undeclare = name: unit: unit // { declared = lib.remove name unit.declared; };
in
{
  # THE COMMITTED PLAN IS CARGO'S PLAN OF THIS TREE.
  #
  # The units already refuse, at evaluation, a plan made from other manifests
  # (the skeleton's hash); this asks the rest: the planner run again over this
  # tree — this toolchain, this cargo — writes exactly the committed file.
  # A toolchain bump that reorders a unit, a cargo that unifies a feature
  # differently, changes the plan with no manifest moving.
  rust-plan-is-current = mkCheck "rust-plan-is-current" [ pkgs.diffutils pkgs.coreutils ] ''
    # PARSED: the committed file and the planner's output, byte for byte.
    if ! cmp -s ${../nix/rust/plan.json} ${rust.planner.plan}/plan.json; then
      echo "ERROR: nix/rust/plan.json is not the plan cargo makes of this tree:" >&2
      diff ${../nix/rust/plan.json} ${rust.planner.plan}/plan.json | head -40 >&2 || true
      echo "       Run: nix run .#rust-plan" >&2
      exit 1
    fi
    echo "OK: nix/rust/plan.json is cargo's plan of this tree"
  '';

  # A STALE PLAN IS REFUSED BEFORE ANY UNIT IS BUILT.
  #
  # The same builder, told the tree hashes to something else, must not give
  # a single unit a derivation — the evaluation throws, naming the command —
  # while the real one does.
  rust-units-refuse-a-stale-plan =
    let
      stale = rust.unitsWith { inputsHash = "0000"; };
      tried = builtins.tryEval (stale.units "release").libs.ronoctua-zip.drvPath;
      fine = builtins.tryEval rust.release.libs.ronoctua-zip.drvPath;
    in
    mkCheck "rust-units-refuse-a-stale-plan" [ pkgs.coreutils ] ''
      # PARSED: the builder's own evaluation, both ways.
      ${
        if tried.success then
          ''
            echo "ERROR: a plan made from other manifests still built a unit" >&2
            exit 1
          ''
        else
          ""
      }
      ${
        if !fine.success then
          ''
            echo "ERROR: the tree's own plan builds no unit either: this check asks nothing" >&2
            exit 1
          ''
        else
          ""
      }
      echo "OK: a stale plan gives no unit a derivation; the tree's plan does"
    '';

  # A COMPILE-TIME READ NOBODY DECLARED FAILS THE UNIT.
  #
  # Both ways a crate reads the environment are run: `option_env!` in a
  # library's own code (what rustc's dep-info says it read) and a build
  # script's `rerun-if-env-changed`. Each unit is run twice by the same
  # driver: as it is (it must build — or the refusal below would prove a
  # broken harness), and with one name gone from its declaration but its
  # value still given (the read alone is refused, value or not). The refusal
  # must name the variable and the crate.
  rust-units-refuse-undeclared-reads =
    mkCheck "rust-units-refuse-undeclared-reads" (unitInputs archive)
      ''
        # PARSED: the driver's own verdict on a unit description.
        ${rerun {
          drv = archive;
          mode = "lib";
        }}
        test -n "$(ls work/out/lib/*.rlib)"
        ${rerun {
          drv = tokens;
          mode = "lib";
        }}
        test -n "$(ls work/out/lib/*.rlib)"

        if (${
          rerun {
            drv = archive;
            mode = "lib";
            edit = undeclare "RONOCTUA_7ZZ";
          }
        }) 2> said.txt; then
          echo "ERROR: ronoctua-archive read RONOCTUA_7ZZ undeclared and built" >&2
          exit 1
        fi
        grep -q "ronoctua-archive's ronoctua_archive reads RONOCTUA_7ZZ at compile time" said.txt || {
          cat said.txt >&2
          echo "ERROR: the refusal does not name the variable and the crate" >&2
          exit 1
        }

        if (${
          rerun {
            drv = tokens;
            mode = "lib";
            edit = undeclare "RONOCTUA_TOKENS_RS";
          }
        }) 2> said.txt; then
          echo "ERROR: ronoctua-tokens's build script read RONOCTUA_TOKENS_RS undeclared and built" >&2
          exit 1
        fi
        grep -q "ronoctua-tokens's build script reads RONOCTUA_TOKENS_RS at compile time" said.txt || {
          cat said.txt >&2
          echo "ERROR: the refusal does not name the variable and the build script" >&2
          exit 1
        }
        echo "OK: an undeclared read is refused, by rustc's dep-info and by a build script's own word"
      '';

  # ONE TEST OUT OF A HARNESS ALREADY BUILT (`rust.testsOf`): of the four
  # tests in the base directories' harness, one, by its exact name, run and
  # counted — "1 passed", so a runner that dropped the filter (four) or
  # matched nothing (zero) is a failure, not a green build that tested
  # something else.
  rust-tests-of-runs-one-test = rust.testsOf {
    name = "check-rust-tests-of-runs-one-test";
    crate = "ronoctua-xdg";
    exact = "tests::an_instance_is_a_short_plain_name";
    expect = "test result: ok. 1 passed";
  };

  # THE PLANNER'S, THE INDEX'S AND THE DRIVER'S OWN ARITHMETIC, on graphs
  # small enough to read: cargo's LTO per unit, a key that changes exactly
  # when a build does, clippy's check graph named by the test graph; an index
  # that refuses a layer that is not the plan's (a unit, an LTO) and writes a
  # build script's `links` paths relative to the layer; cargo's flags for a
  # unit, a build script's lines read both ways, an undeclared read refused,
  # a `links` path put back as the unit's own layer, a unit kept to its
  # product; and `rebuild-cost`'s moved entries, measured time and critical
  # path.
  rust-build-scripts-pass-their-tests =
    mkCheck "rust-build-scripts-pass-their-tests" [ pkgs.python3 ]
      ''
        # PARSED: the three scripts, run by their own unit tests.
        cp ${../nix/scripts/rust-plan.py} rust-plan.py
        cp ${../nix/scripts/rust-index.py} rust-index.py
        cp ${../nix/scripts/rust-unit.py} rust-unit.py
        cp ${../nix/scripts/test_rust_plan.py} test_rust_plan.py
        cp ${../nix/scripts/test_rust_index.py} test_rust_index.py
        cp ${../nix/scripts/test_rust_unit.py} test_rust_unit.py
        cp ${../nix/scripts/rebuild-cost.py} rebuild-cost.py
        cp ${../nix/scripts/test_rebuild_cost.py} test_rebuild_cost.py
        python3 -m unittest -v test_rust_plan test_rust_index test_rust_unit test_rebuild_cost
        echo "OK: the planner's, the index's, the driver's and rebuild-cost's tests pass"
      '';
  # (The units' three aggregates are `rust-tests`, `rust-clippy` and
  # `rust-fmt`, in `tests/checks.nix`: they took crane's names.)
}
