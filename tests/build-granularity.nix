# BUILD GRANULARITY — the 2026-10-12 wave, lane N3 (nix-design §5–§6).
#
# "Recompiling the whole distro because a small part of one component
# changed is a granularity error." The build is one derivation per Rust
# compilation unit (`nix/rust/`), so what an edit rebuilds is a property of
# the build graph — and it is asked here at EVALUATION, never by compiling:
# the same build with the edit simulated (`rust.override {
# crateSourceOverrides = …; }`, or a data derivation swapped), its entries
# compared with the real ones (`rust.unitIndex`, the index `nix run
# .#rebuild-cost` reads).
#
# - `granularity-of-an-edit`, `…-of-a-test-edit`, `…-of-a-bin-edit`: a
#   one-byte edit rebuilds exactly what cargo's graph says it must, both
#   ways: every entry it reaches moves (or a program keeps the old code), and
#   no other entry moves (the granularity the build exists for). The
#   expected set is written from the plan in this file's own words
#   (`granularityModel`); a sample may promise more (a module's code is the
#   daemon's alone).
# - `granularity-of-a-manifest-edit`: a test, a dependency between two of
#   our crates, a feature of ours move no third-party unit and no layer.
# - `buffer-edit-rebuilds-no-rust`, `colour-seeds-rebuild-no-rust`,
#   `text-values-rebuild-no-rust`, `person-module-rebuilds-only-the-daemon`:
#   data reaches the programs at run time and compiles nothing.
# - what a unit is given and what a crate reads (`units-see-only-what-they-read`,
#   `crate-reads-are-declared`, `data-entries-name-files`,
#   `every-generated-name-is-read`, `siblings-are-beside`), what a flavour is
#   built from (`flavours-never-mix`), what a program carries and where it goes
#   (`graphics-rpath-only-where-graphics-are`, `seed-holds-every-shipped-bin`)
#   and the tests no unit runs (`no-runnable-doctests`). A program's role and
#   the shell's bundle are `tests/build-outputs.nix`'s (`every-bin-has-a-role`,
#   `shell-bundle-is-only-the-shell`, lane N2), the outputs' own file.
{
  pkgs,
  lib,
  self,
  rust,
  mkCheck,
  pyPrelude,
  buffers,
  themed,
  defaultTheme,
  # The texts as the build makes them (`nix/i18n.nix`), for the edit of one
  # sentence's value.
  texts,
  withPersonModules,
}:

let
  plain = builtins.unsafeDiscardStringContext;
  toFile = name: value: pkgs.writeText name (builtins.toJSON value);
  planFile = ../nix/rust/plan.json;
  plan = builtins.fromJSON (builtins.readFile planFile);
  version = plan.packages.ronoctua-shell.version;

  # THE BUILD AS DATA: every unit, layer and aggregate by an id that names it
  # the same way in every tree, with its derivation's path (no string
  # context: nothing is built for being named here).
  base = rust.unitIndex;
  baseFile = toFile "rust-units.json" base;
  # The same build with files edited by one byte — the edit `nix run
  # .#rebuild-cost -- <file>` simulates, so this proves its simulation too.
  editedIndex = files: (rust.override { crateSourceOverrides = rust.oneByteEdits files; }).unitIndex;
  exists = file: builtins.pathExists (../. + "/${file}");
  # What each crate's `data` names, as the units read it.
  dataFile = toFile "rust-unit-data.json" (lib.mapAttrs (_: c: c.data) rust.unitCrates);
  # Every name the build's tables hold.
  tables = {
    generated = lib.attrNames rust.generated;
    testEnv = lib.attrNames rust.testEnv;
    atRunTime = lib.attrNames rust.atRunTime;
    optional = [ "RONOCTUA_BRAIN_PERSON_MODULES_DIR" ];
  };
  tablesFile = toFile "rust-tables.json" tables;

  # THE UNITS, by flavour and kind, for the censuses.
  kinds = [
    "libs"
    "scripts"
    "bins"
    "testBins"
    "testRuns"
    "clippy"
  ];
  unitsOf =
    r:
    lib.concatMap
      (
        flavour:
        lib.concatMap (
          kind:
          lib.mapAttrsToList (name: drv: {
            id = "${flavour}/${kind}/${name}";
            inherit
              flavour
              kind
              name
              drv
              ;
          }) (r.${flavour}.${kind} or { })
        ) kinds
      )
      [
        "release"
        "loop"
        "test"
      ]
    ++ lib.mapAttrsToList (name: drv: {
      id = "fmt/${name}";
      flavour = null;
      kind = "fmt";
      inherit name drv;
    }) r.test.fmt;
  allUnits = unitsOf rust;

  # THE GRANULARITY THE BUILD PROMISES, written from the plan and nothing
  # else — read by every check below that compares an edit with the graph.
  granularityModel = pkgs.writeText "granularity-model.py" ''
    # THE GRANULARITY THE BUILD PROMISES, written from the plan and nothing else.
    #
    # A unit is rebuilt by an edit when its own target's sources hold the edited
    # file, or when a unit it is built from is rebuilt. The first half is the
    # rule each target's sources follow (`nix/rust/units.nix`, said here again in
    # its own words); the second is cargo's own graph, read from `plan.json`.
    # What the build ACTUALLY rebuilds is read off the derivations; the check
    # compares the two, both ways.
    import json
    import posixpath

    PLAN = DATA = DIRS = None
    SOURCE_EXT = (".rs", ".wit", ".wgsl")
    TEST_DIRS = ("tests", "fixtures", "benches", "examples")


    def model_load(plan_path, data_path):
        global PLAN, DATA, DIRS
        PLAN = json.load(open(plan_path))
        DATA = json.load(open(data_path))
        DIRS = {c: p["dir"] for c, p in PLAN["packages"].items()}


    def _parts(p):
        return posixpath.normpath(p).split("/")


    def _matches(pattern, f):
        """A `data` entry's pattern (`*` one directory, `**/*.<ext>` every file
        with that extension below) against a path; a plain path matches itself
        and everything under it."""
        pp, fp = _parts(pattern), _parts(f)
        if len(pp) >= 2 and pp[-2] == "**" and pp[-1].startswith("*."):
            pre = pp[:-2]
            return (len(fp) > len(pre) and all(p in ("*", q) for p, q in zip(pre, fp))
                    and fp[-1].endswith(pp[-1][1:]))
        return len(fp) >= len(pp) and all(p in ("*", q) for p, q in zip(pp, fp))


    def in_data(c, f):
        return any(_matches(posixpath.join(DIRS[c], e), f) for e in DATA.get(c, []))


    def own(c, f):
        return f.startswith(DIRS[c] + "/")


    def rel(c, f):
        return f[len(DIRS[c]) + 1:]


    def sees_lib(c, f):
        if in_data(c, f):
            return True
        if not own(c, f):
            return False
        r = rel(c, f)
        return r in ("Cargo.toml", "build.rs") or (
            r.startswith("src/") and not r.startswith("src/bin/") and r != "src/main.rs" and r.endswith(SOURCE_EXT))


    def sees_script(c, f):
        return in_data(c, f) or (own(c, f) and rel(c, f) in ("Cargo.toml", "build.rs"))


    def sees_bin(c, src, f):
        if in_data(c, f):
            return True
        if not own(c, f):
            return False
        r = rel(c, f)
        if r == "Cargo.toml":
            return True
        if src == "src/main.rs":
            return r.startswith("src/")
        if r == src or r.startswith(src[: -len(".rs")] + "/"):
            return True
        return posixpath.basename(src) == "main.rs" and r.startswith(posixpath.dirname(src) + "/")


    def roots_of(c):
        units = PLAN["graphs"]["test"]["units"]
        return sorted({u["src"] for u in units.values() if u["workspace"] and u["pkg"] == c
                       and u["mode"] != "run-custom-build" and "custom-build" not in u["kind"]})


    def sees_tests(c, f):
        if sees_lib(c, f) or any(sees_bin(c, s, f) for s in roots_of(c)):
            return True
        return own(c, f) and rel(c, f).split("/")[0] in TEST_DIRS


    def sees_fmt(c, f):
        return own(c, f) and (rel(c, f) == "Cargo.toml" or rel(c, f).endswith(".rs"))


    def _is_lib(u):
        return u["mode"] == "build" and ("lib" in u["kind"] or "proc-macro" in u["kind"])


    def _changed_keys(flavour, f):
        """The workspace units of one graph an edit of `f` reaches: those whose
        own sources hold it, and everything built from one of them."""
        ws = {k: u for k, u in PLAN["graphs"][flavour]["units"].items() if u["workspace"]}
        changed = set()
        for k, u in ws.items():
            c = u["pkg"]
            if _is_lib(u):
                hit = sees_lib(c, f)
            elif "custom-build" in u["kind"] and u["mode"] == "build":
                hit = sees_script(c, f)
            elif u["mode"] == "build" and "bin" in u["kind"]:
                hit = sees_bin(c, u["src"], f)
            elif u["mode"] in ("test", "bench"):
                hit = sees_tests(c, f)
            else:
                hit = False
            if hit:
                changed.add(k)
        grew = True
        while grew:
            grew = False
            for k, u in ws.items():
                if k not in changed and any(d["key"] in changed for d in u["deps"]):
                    changed.add(k)
                    grew = True
        return ws, changed


    def expected(f):
        """Every entry of the index (`rustUnits`) an edit of `f` must rebuild."""
        out = set()
        for flavour in ("release", "loop", "test"):
            ws, changed = _changed_keys(flavour, f)
            by_pkg = {}
            for k, u in ws.items():
                by_pkg.setdefault(u["pkg"], []).append((k, u))
            for c, units in by_pkg.items():
                libs = [k for k, u in units if _is_lib(u)]
                runs = [k for k, u in units if u["mode"] == "run-custom-build"]
                base_moved = any(k in changed for k in libs + runs)
                if libs and base_moved:
                    out.add(f"{flavour}/libs/{c}")
                if not libs and runs and base_moved:
                    out.add(f"{flavour}/scripts/{c}")
                for k, u in units:
                    if u["mode"] == "build" and "bin" in u["kind"] and k in changed:
                        out.add(f"{flavour}/bins/{u['target']}")
                if flavour == "test":
                    tests = [k for k, u in units if u["mode"] in ("test", "bench")]
                    if not tests:
                        continue
                    bin_keys = {d["key"] for k in tests for d in ws[k]["deps"]
                                if d["key"] in ws and ws[d["key"]]["kind"] == ["bin"]}
                    if sees_tests(c, f) or base_moved or any(k in changed for k in tests) or bin_keys & changed:
                        out.add(f"test/testBins/{c}")
                        out.add(f"test/testRuns/{c}")
            if flavour == "test":
                for c, targets in PLAN["clippy"].items():
                    own_base = [k for k, u in ws.items() if u["pkg"] == c and (_is_lib(u) or u["mode"] == "run-custom-build")]
                    deps = {d["key"] for t in targets for d in t["deps"]}
                    if sees_tests(c, f) or deps & changed or any(k in changed for k in own_base):
                        out.add(f"test/clippy/{c}")
        for c in DIRS:
            if sees_fmt(c, f):
                out.add(f"fmt/{c}")
        for kind, agg in (("testRuns", "rust-tests"), ("clippy", "rust-clippy")):
            if any(e.startswith(f"test/{kind}/") for e in out):
                out.add(f"aggregate/{agg}")
        if any(e.startswith("fmt/") for e in out):
            out.add("aggregate/rust-fmt")
        return out


    def changed_entries(before, after):
        """What an edit actually rebuilt: every entry whose derivation moved, and
        every entry one side has and the other does not."""
        return {k for k in set(before) | set(after) if before.get(k, {}).get("drv") != after.get(k, {}).get("drv")}

    # WHAT A SAMPLE PROMISES beyond the plan: an entry it may never rebuild,
    # by kind, by crate (`crates`), outside some crates (`onlyCrates`), or a
    # program but these (`exceptNames`).
    def promised_against(p, entry):
        if entry is None:
            return False
        if "kinds" in p and entry["kind"] not in p["kinds"]:
            return False
        if "crates" in p and entry["crate"] not in p["crates"]:
            return False
        if "onlyCrates" in p and (entry["crate"] is None or entry["crate"] in p["onlyCrates"]):
            return False
        if "exceptNames" in p and entry["name"] in p["exceptNames"]:
            return False
        return True
  '';

  # ONE EDIT AT A TIME: each sample's file edited by one byte, its index
  # compared with the build's, against the plan and against its promises.
  layersNever = {
    kinds = [ "layer" ];
    why = "a third-party layer is built from the plan's third-party half alone";
  };
  granularity =
    name: samples:
    let
      present = lib.filter (s: exists s.file) samples;
      facts = toFile "${name}.json" {
        missing = map (s: s.file) (lib.filter (s: !exists s.file) samples);
        samples = map (s: {
          inherit (s) file least;
          never = [ layersNever ] ++ (s.never or [ ]);
          after = editedIndex [ s.file ];
        }) present;
      };
    in
    mkCheck name [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import json, sys
      exec(open("${pyPrelude}").read())
      exec(open("${granularityModel}").read())

      # PARSED: cargo's plan, and the build's index before and after each
      # edit as Nix evaluated it — the derivations' paths, nothing built.
      model_load("${planFile}", "${dataFile}")
      facts = json.load(open(sys.argv[1]))
      before = json.load(open("${baseFile}"))
      floor(len(before), 600, "entries of the build's index")
      floor(len(facts["samples"]) + len(facts["missing"]), 1, "samples")
      wrong = [f"the sample {f} is gone: point this check at the file that took its place"
               for f in facts["missing"]]
      for s in facts["samples"]:
          f, after = s["file"], s["after"]
          if set(after) != set(before):
              wrong.append(f"{f}: an edit of a source changed which entries the build has: "
                           + ", ".join(sorted(set(after) ^ set(before))))
          want, got = expected(f), changed_entries(before, after)
          floor(len(want), s["least"], f"entries an edit of {f} has to rebuild")
          for e in sorted(want - got):
              wrong.append(f"{f}: {e} is built from it and was NOT rebuilt: it would keep the old code")
          for e in sorted(got - want):
              wrong.append(f"{f}: {e} was rebuilt, and nothing of it is built from that file")
          for e in sorted(got):
              for p in s["never"]:
                  if promised_against(p, after.get(e) or before.get(e)):
                      wrong.append(f"{f}: {e} was rebuilt, and {p['why']}")
          kinds = {}
          for e in got:
              k = (after.get(e) or before.get(e))["kind"]
              kinds[k] = kinds.get(k, 0) + 1
          print(f"{f}: {len(got)} of {len(before)} entries rebuilt — "
                + ", ".join(f"{n} {k}" for k, n in sorted(kinds.items())))
      if wrong:
          print("ERROR: an edit does not rebuild what the plan says it reaches, and only that:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(facts['samples'])} edits, each rebuilding exactly what cargo's graph reaches")
      END
    '';

  # A DATUM EDITED: the build with a data derivation swapped, entry by entry.
  # What compiles may not move; what RUNS a test that reads the datum may
  # (`reruns` must be among those: the datum reached the tests that read it),
  # and what carries the datum to a program must (`carries`: the edit reached
  # the build at all — or the comparison proves nothing).
  compiles = [
    "deps"
    "libs"
    "scripts"
    "bins"
    "testBins"
    "clippy"
    "fmt"
    "layer"
  ];
  dataEdit =
    {
      name,
      what,
      after,
      carries,
      reruns ? [ ],
      # Files of the datum no unit may hold among its sources — the route
      # around the derivation (a crate's `data` naming one of them).
      untouched ? [ ],
      # A precondition that is not met: the check fails, saying it, rather
      # than comparing the build with itself.
      unmet ? null,
    }:
    let
      facts = toFile "${name}.json" {
        inherit
          what
          reruns
          unmet
          untouched
          ;
        missing = lib.filter (f: !exists f) untouched;
        after = if unmet == null then after.unitIndex else { };
        carries = lib.mapAttrs (n: d: {
          before = plain d.before.drvPath;
          after = plain d.after.drvPath;
        }) (if unmet == null then carries else { });
        inherit compiles;
      };
    in
    mkCheck name [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import json, sys
      exec(open("${pyPrelude}").read())
      exec(open("${granularityModel}").read())

      # PARSED: the build's index before and after the edit, as Nix
      # evaluated it.
      facts = json.load(open(sys.argv[1]))
      what = facts["what"]
      if facts["unmet"]:
          print(f"ERROR: {what}: {facts['unmet']}", file=sys.stderr)
          sys.exit(1)
      before, after = json.load(open("${baseFile}")), facts["after"]
      floor(len(before), 600, "entries of the build's index")
      got = changed_entries(before, after)
      wrong = [f"{f} is gone: point this check at the datum's file that took its place" for f in facts["missing"]]
      model_load("${planFile}", "${dataFile}")
      for f in facts["untouched"]:
          for c in sorted(DIRS):
              if sees_tests(c, f) or sees_fmt(c, f):
                  wrong.append(f"{f} is among {c}'s sources: an edit of it reaches the compilation around the derivation")
      for n, d in sorted(facts["carries"].items()):
          if d["before"] == d["after"]:
              wrong.append(f"{n} did not move: the edit never reached the build, and this compares it with itself")
      floor(len(facts["carries"]), 1, "derivations that carry the datum")
      compiled = sorted(e for e in got if (after.get(e) or before.get(e))["kind"] in facts["compiles"])
      for e in compiled:
          wrong.append(f"{e} is compiled again")
      runs = sorted(e for e in got if (after.get(e) or before.get(e))["kind"] == "testRuns")
      for e in facts["reruns"]:
          if e not in got:
              wrong.append(f"{e} reads the datum and was not run again")
      if wrong:
          print(f"ERROR: {what} reaches the compilation:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          print(f"  ({len(compiled)} compiled entries of {len(before)} moved; data is read at run time)", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {what} compiles nothing; {len(runs)} test runs read it and run again"
            + (": " + ", ".join(r.split("/")[-1] for r in runs) if runs else ""))
      END
    '';

  # THE EDITS of the data checks.
  #
  # A BUFFER: its derivation moved, as a source edit moves it. The route a
  # file could take around the derivation (a crate's `data` naming a
  # buffer's file) is asked in the check's own script.
  bufferEdited = rust.override {
    bufferMemory = buffers.memory.overrideAttrs (old: {
      postPatch = (old.postPatch or "") + "\n# a buffer's source, edited\n";
    });
  };
  # A COLOUR SEED: the default theme's accent turned by one degree, through
  # the generator that reads the seeds (`nix/themes.nix`, `seedEdits`).
  defaultSeed = import (../assets/themes + "/${defaultTheme}.nix");
  themedEdited = themed.override {
    seedEdits.${defaultTheme}.families.accent.hue = defaultSeed.families.accent.hue + 1;
  };
  colourEdited = rust.override { themed = themedEdited; };
  # A SENTENCE: one value of a crate's `en-GB.ftl` changed — the stick's, a
  # view's own fragment; its key, its arguments and every other file as they
  # were — through the generator of the texts, handed over as TEXT
  # (`nix/i18n.nix`, `fileEdits`): the keys' names are read at evaluation, and
  # a folder a derivation edited would be import-from-derivation there. Which
  # argument of `nix/rust.nix` carries the texts, and whether `nix/i18n.nix`
  # takes an edit, is read off the two files: a chain that moved is said,
  # never compared with itself.
  textArg = if (builtins.functionArgs (import ../nix/rust.nix)) ? texts then "texts" else null;
  textsTakeAnEdit = (builtins.functionArgs (import ../nix/i18n.nix)) ? fileEdits;
  textCrate = "ronoctua-shell-stick";
  baseLines = lib.splitString "\n" (builtins.readFile (../crates + "/${textCrate}/i18n/en-GB.ftl"));
  # The first plain message (no placeable, no attribute), its value longer
  # by one word.
  plainAt = lib.lists.findFirstIndex (
    l: builtins.match "[a-z][a-z0-9-]* = [A-Za-z][A-Za-z ]*" l != null
  ) null baseLines;
  textsEdited = texts.override {
    fileEdits.${textCrate}.en-GB = lib.concatStringsSep "\n" (
      lib.imap0 (i: l: if i == plainAt then l + " edited" else l) baseLines
    );
  };
  textEdited = rust.override { ${textArg} = textsEdited; };
  # A PERSON'S MODULE: the fixture `my-sample`, as a machine compiles it.
  person = withPersonModules { dir = ../crates/ronoctua-brain/tests/person; };
  personEdited = person.rust;
in
{
  checks = {
    # A ONE-BYTE EDIT OF A LIBRARY REBUILDS WHAT IS BUILT FROM IT, AND ONLY
    # THAT — in a leaf (`zip`), a crate in the middle (`http`), the hub every
    # crate reads (`xdg`), a view of the footer, the brain's library and one
    # brain module's code. Every unit the plan says is built from the file
    # moves, in every flavour, and nothing else: no layer, no unrelated
    # program. A module's code is the daemon's alone: the brain's library
    # (and with it the footer) compiles each module's manifest and shared
    # half, and the edit may not reach it.
    granularity-of-an-edit = granularity "granularity-of-an-edit" [
      {
        file = "crates/ronoctua-zip/src/lib.rs";
        least = 10;
      }
      {
        file = "crates/ronoctua-http/src/lib.rs";
        least = 30;
      }
      {
        file = "crates/ronoctua-xdg/src/lib.rs";
        least = 100;
      }
      # A VIEW OF THE FOOTER is its own crate (the 2026-10-12 wave): an edit
      # of the stick rebuilds the stick's library and the binary that links
      # it — never the footer, the foundation or another view.
      {
        file = "crates/ronoctua-shell-stick/src/stick.rs";
        least = 5;
        never = [
          {
            kinds = [
              "libs"
              "scripts"
            ];
            onlyCrates = [ "ronoctua-shell-stick" ];
            why = "a view is its own crate's library and no other's: the footer, the foundation and the other views are not built from it";
          }
          {
            kinds = [ "bins" ];
            exceptNames = [ "ronoctua-shell" ];
            why = "a view of the footer is linked into the shell alone";
          }
        ];
      }
      # THE FOOTER is a crate of its own (lane R6): a line of it rebuilds the
      # footer's library and the binary that links it — never a view, the
      # foundation or the words under it.
      {
        file = "crates/ronoctua-shell-footer/src/surface/mod.rs";
        least = 3;
        never = [
          {
            kinds = [
              "libs"
              "scripts"
            ];
            onlyCrates = [ "ronoctua-shell-footer" ];
            why = "the footer is its own crate's library: no view, no foundation crate and no words leaf is built from it";
          }
          {
            kinds = [ "bins" ];
            exceptNames = [ "ronoctua-shell" ];
            why = "the footer is linked into the shell alone";
          }
        ];
      }
      # A VIEW'S WORDS, under the view and the footer (lane R6): a line of the
      # radio's words rebuilds them, the radio's view, the footer and the
      # binary — never another view nor the foundation.
      {
        file = "crates/ronoctua-shell-bluetooth-words/src/bluez_words.rs";
        least = 5;
        never = [
          {
            kinds = [
              "libs"
              "scripts"
            ];
            onlyCrates = [
              "ronoctua-shell-bluetooth-words"
              "ronoctua-shell-bluetooth"
              "ronoctua-shell-footer"
            ];
            why = "a view's words are built into that view and the footer alone";
          }
          {
            kinds = [ "bins" ];
            exceptNames = [ "ronoctua-shell" ];
            why = "a view's words are linked into the shell alone";
          }
        ];
      }
      # A PAGE OF THE PLAYER, written above it (the player's seam): a line of
      # channels by theme, or of the ranking, recompiles its crate and the
      # binary — never the player's transport, its stage, its window, nor the
      # other page.
      {
        file = "crates/ronoctua-shell-catalogue/src/player_catalog.rs";
        least = 5;
        never = [
          {
            crates = [
              "ronoctua-shell-player"
              "ronoctua-shell-video-metrics"
            ];
            why = "a page of the player is no source of the player nor of another page (`player_seam`)";
          }
        ];
      }
      {
        file = "crates/ronoctua-shell-video-metrics/src/player_ranking.rs";
        least = 5;
        never = [
          {
            crates = [
              "ronoctua-shell-player"
              "ronoctua-shell-catalogue"
            ];
            why = "a page of the player is no source of the player nor of another page (`player_seam`)";
          }
        ];
      }
      {
        file = "crates/ronoctua-brain/src/lib.rs";
        least = 10;
      }
      {
        file = "crates/ronoctua-brain/modules/disk-space/module.rs";
        least = 5;
        never = [
          {
            crates = [
              "ronoctua-brain"
              "ronoctua-shell"
              "ronoctua-shell-footer"
            ];
            # Compiled, that is: the brain crate's rustfmt unit reads every
            # `.rs` under its folder, and costs a second.
            kinds = [
              "libs"
              "scripts"
              "bins"
              "testBins"
              "testRuns"
              "clippy"
            ];
            why = "a module's code is the daemon's (ronoctua-brain-daemon) alone: here it recompiles the footer";
          }
        ];
      }
    ];

    # A ONE-BYTE EDIT OF AN INTEGRATION TEST rebuilds that crate's test
    # harnesses, their run, its clippy and its formatting — never a library,
    # never a program, nothing of another crate.
    granularity-of-a-test-edit = granularity "granularity-of-a-test-edit" [
      {
        file = "crates/ronoctua-tags/tests/program.rs";
        least = 4;
        never = [
          {
            kinds = [
              "libs"
              "scripts"
              "bins"
            ];
            why = "a test is compiled into the crate's test harnesses alone";
          }
          {
            onlyCrates = [ "ronoctua-tags" ];
            why = "a test of one crate is no other crate's source";
          }
        ];
      }
    ];

    # A ONE-BYTE EDIT OF A PROGRAM'S ROOT relinks that program — in every
    # flavour — and its crate's test harnesses, clippy and formatting; never
    # the library under it, nor the crate's other programs.
    granularity-of-a-bin-edit = granularity "granularity-of-a-bin-edit" [
      {
        file = "crates/ronoctua-files/src/bin/empty-trash.rs";
        least = 4;
        never = [
          {
            kinds = [
              "libs"
              "scripts"
            ];
            why = "a program's root is no library's source";
          }
          {
            kinds = [ "bins" ];
            exceptNames = [ "ronoctua-empty-trash" ];
            why = "only the edited program is linked again";
          }
          {
            onlyCrates = [ "ronoctua-files" ];
            why = "a program of one crate is no other crate's source";
          }
        ];
      }
    ];

    # A MANIFEST EDIT OF OURS MOVES NO THIRD-PARTY UNIT.
    #
    # A third-party unit that moved is it and everything above it rebuilt
    # (`a-third-party-change-rebuilds-only-what-is-above`), and a manifest of
    # ours says nothing a third-party crate is compiled with: it is built
    # from the plan's third-party half alone (`nix/rust/units.nix`, by key).
    # So this asks the third-party units of an edited plan, flavour by
    # flavour, at evaluation: an integration test added to a crate of ours, a
    # dependency between two of our crates, a feature of ours — no
    # third-party unit moves. A feature of a third-party crate moves its
    # units (so the comparison is not blind), and one only the test graph
    # turns on moves the test graph's alone.
    #
    # And the two layers the profile bench measures on (`nix/rust/layers.nix`,
    # built from the same half), the same way: no edit of ours moves them; a
    # third-party feature moves both, one of the test graph's alone the
    # loop's (the layer that builds the test graph's half).
    granularity-of-a-manifest-edit =
      let
        layersOf =
          p:
          let
            l = rust.layersWith { plan = p; };
          in
          {
            release = plain l.release.drvPath;
            loop = plain l.loop.drvPath;
          };
        # The third-party units of each flavour's graph, as the paths of
        # their derivations: a set, so a unit re-keyed counts as moved.
        unitsOf =
          p:
          let
            u = rust.unitsWith { plan = p; };
          in
          lib.genAttrs [ "release" "loop" "test" ] (
            f: lib.sort (a: b: a < b) (map (d: plain d.drvPath) (lib.attrValues (u.units f).deps))
          );
        graph = g: plan.graphs.${g}.units;
        keyOf =
          g: pred:
          lib.findFirst (k: pred (graph g).${k})
            (throw "granularity-of-a-manifest-edit: no unit to edit in the ${g} graph")
            (lib.attrNames (graph g));
        ours = c: u: u.workspace && u.pkg == c && u.mode == "build" && lib.elem "lib" u.kind;
        theirs = c: u: !u.workspace && u.pkg == c && u.mode == "build" && lib.elem "lib" u.kind;
        withUnits =
          g: f:
          plan
          // {
            graphs = plan.graphs // {
              ${g} = plan.graphs.${g} // {
                units = f (graph g);
              };
            };
          };
        inAll =
          f:
          plan
          // {
            graphs = lib.mapAttrs (g: v: v // { units = f g v.units; }) plan.graphs;
          };
        edit =
          g: pred: change: units:
          let
            k = keyOf g pred;
          in
          units // { ${k} = change units.${k}; };
        # A THIRD-PARTY UNIT IS ONE KEY, whichever graph names it (the loop's
        # and the test graph's share theirs): an edit of one is an edit of
        # that key in every graph that holds it, or the plan would describe
        # one derivation two ways.
        editKey =
          k: change: p:
          p
          // {
            graphs = lib.mapAttrs (
              _: v:
              v
              // {
                units = if v.units ? ${k} then v.units // { ${k} = change v.units.${k}; } else v.units;
              }
            ) p.graphs;
          };
        probe = u: u // { features = u.features ++ [ "probe" ]; };
        serdeRelease = keyOf "release" (theirs "serde");
        serdeLoop = keyOf "loop" (theirs "serde");
        # The graphs that hold one of the two edited keys: whose third-party
        # units the edit reaches.
        theirsIn = lib.filter (f: (graph f) ? ${serdeRelease} || (graph f) ? ${serdeLoop}) [
          "release"
          "loop"
          "test"
        ];
        # A FEATURE ONLY THE TEST GRAPH TURNS ON, planned the way cargo would
        # plan it: a key names its unit's features, so the edited unit's key is
        # another, and so is every third-party key above it in that graph (a
        # key also names its dependencies'). The loop's units, which the test
        # graph shares by key until then, stay the loop's. Our own units are
        # no graph's but their flavour's, and keep their keys; what names a
        # renamed key (their dependencies, clippy's targets) is renamed with it.
        inTestsOnly =
          k: change:
          let
            units = graph "test";
            third = lib.filterAttrs (_: u: !u.workspace) units;
            grow =
              set:
              let
                has = lib.genAttrs set (_: true);
                more = lib.filter (x: !(has ? ${x}) && lib.any (d: has ? ${d.key}) third.${x}.deps) (
                  lib.attrNames third
                );
              in
              if more == [ ] then set else grow (set ++ more);
            renamed = lib.genAttrs (grow [ k ]) (x: "${x}-probe");
            rename = key: renamed.${key} or key;
            renameDeps = u: u // { deps = map (d: d // { key = rename d.key; }) u.deps; };
          in
          plan
          // {
            graphs = plan.graphs // {
              test = plan.graphs.test // {
                units = lib.mapAttrs' (
                  key: u: lib.nameValuePair (rename key) (renameDeps (if key == k then change u else u))
                ) units;
              };
            };
            clippy = lib.mapAttrs (_: map renameDeps) plan.clippy;
          };
        zipTest = keyOf "test" (u: u.workspace && u.pkg == "ronoctua-zip" && u.mode == "test");
        edits = {
          test-added = withUnits "test" (
            units:
            units
            // {
              "ronoctua-zip@${version}/test:probe/test/o2/000000000000" = units.${zipTest} // {
                kind = [ "test" ];
                target = "probe";
                src = "tests/probe.rs";
              };
            }
          );
          dependency-between-ours = inAll (
            g:
            edit g (ours "ronoctua-zip") (
              u:
              u
              // {
                deps = u.deps ++ [
                  {
                    extern = "ronoctua_xdg";
                    key = keyOf g (ours "ronoctua-xdg");
                  }
                ];
              }
            )
          );
          feature-of-ours = inAll (g: edit g (ours "ronoctua-zip") probe);
          feature-of-theirs = lib.foldl' (p: k: editKey k probe p) plan [
            serdeRelease
            serdeLoop
          ];
          feature-of-theirs-in-tests = inTestsOnly serdeLoop probe;
        };
        facts = toFile "granularity-of-a-manifest-edit.json" {
          before = {
            layers = layersOf plan;
            units = unitsOf plan;
          };
          after = lib.mapAttrs (_: p: {
            layers = layersOf p;
            units = unitsOf p;
          }) edits;
          inherit theirsIn;
        };
      in
      mkCheck "granularity-of-a-manifest-edit" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the third-party units' and the two layers' derivations, of
        # the plan and of each edit of it, as Nix evaluated them.
        facts = json.load(open(sys.argv[1]))
        before, after = facts["before"], facts["after"]
        flavours = ("release", "loop", "test")
        moves = {
            "test-added": (set(), set()),
            "dependency-between-ours": (set(), set()),
            "feature-of-ours": (set(), set()),
            "feature-of-theirs": (set(facts["theirsIn"]), {"release", "loop"}),
            "feature-of-theirs-in-tests": ({"test"}, {"loop"}),
        }
        for f in flavours:
            floor(len(before["units"][f]), 300, f"third-party units of the {f} graph")
        wrong = []
        if not {"release", "loop"} <= set(facts["theirsIn"]):
            wrong.append(f"the third-party edit reaches the graphs {facts['theirsIn']}: it has to reach release's and loop's")
        if sorted(after) != sorted(moves):
            wrong.append(f"the edits asked are {sorted(after)}, not {sorted(moves)}")
        for edit, (units, layers) in sorted(moves.items()):
            a = after.get(edit, before)
            got = {f for f in flavours if a["units"][f] != before["units"][f]}
            for f in sorted(got - units):
                wrong.append(f"{edit}: a third-party unit of the {f} graph moved, and with it everything above it")
            for f in sorted(units - got):
                wrong.append(f"{edit}: no third-party unit of the {f} graph moved, and one is what the edit changed")
            got = {l for l in ("release", "loop") if a["layers"][l] != before["layers"][l]}
            for l in sorted(got - layers):
                wrong.append(f"{edit}: the {l} layer moved")
            for l in sorted(layers - got):
                wrong.append(f"{edit}: the {l} layer did not move, and it holds what the edit changed")
        if wrong:
            print("ERROR: a third-party unit or layer does not move with the third-party half of the plan alone:",
                  file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: a test, a dependency and a feature of ours move no third-party unit and no layer; a third-party"
              " feature moves its units and layers, one the test graph alone turns on the test graph's units and the"
              " loop's layer")
        END
      '';

    # A BUFFER'S EDIT COMPILES NO RUST: the component reaches the programs
    # through the wrapper (`atRunTime`), which is what moves.
    buffer-edit-rebuilds-no-rust = dataEdit {
      name = "buffer-edit-rebuilds-no-rust";
      what = "a buffer's edit";
      after = bufferEdited;
      untouched = [ "buffers/memory/src/lib.rs" ];
      carries = {
        "the footer's wrapper (rust.shell)" = {
          before = rust.shell;
          after = bufferEdited.shell;
        };
        "the nested loop's wrapper (rust.loopShell)" = {
          before = rust.loopShell;
          after = bufferEdited.loopShell;
        };
      };
    };

    # A COLOUR SEED'S EDIT COMPILES NO RUST: the palettes are read at run time
    # (`RONOCTUA_THEMES`), so the wrapper moves and the tests that resolve a
    # colour run again — `ronoctua-tokens`' first.
    colour-seeds-rebuild-no-rust = dataEdit {
      name = "colour-seeds-rebuild-no-rust";
      what = "a colour seed's edit";
      after = colourEdited;
      carries = {
        "the palettes (nix/themes.nix)" = {
          before = themed;
          after = themedEdited;
        };
        "the footer's wrapper (rust.shell)" = {
          before = rust.shell;
          after = colourEdited.shell;
        };
      };
      reruns = [ "test/testRuns/ronoctua-tokens" ];
    };

    # A SENTENCE'S VALUE COMPILES NO RUST: the texts are read at run time; a
    # crate is compiled against the keys' NAMES, which a value does not move.
    text-values-rebuild-no-rust = dataEdit {
      name = "text-values-rebuild-no-rust";
      what = "one sentence's value";
      after = textEdited;
      unmet =
        if textArg == null then
          "nix/rust.nix takes no `texts`: point this check at the argument that carries them"
        else if !textsTakeAnEdit then
          "nix/i18n.nix takes no `fileEdits`: point this check at what the texts are generated from"
        else if plainAt == null then
          "${textCrate}'s en-GB.ftl has no plain message (no placeable, no attribute) to edit"
        else
          null;
      carries = {
        "the texts" = {
          before = texts;
          after = textsEdited;
        };
        "the footer's wrapper (rust.shell)" = {
          before = rust.shell;
          after = textEdited.shell;
        };
      };
    };

    # A PERSON'S BRAIN MODULE REBUILDS THE DAEMON AND NOTHING ELSE: its code
    # is compiled into `ronoctua-brain-daemon`; its marks and their glyphs,
    # its actions, events and notifications are the brain's catalogue, its
    # sentences the texts, and its glyph tokens a name in the glyph browser's
    # index — data read at run time (`RONOCTUA_BRAIN_CATALOGUE`,
    # `RONOCTUA_TEXTS`, `RONOCTUA_ICONSET`). So of what COMPILES, every entry
    # built from the daemon's library moves and no other: not the footer, not
    # a buffer, not the brain's library, not the tokens, the notification
    # table or the key registry. What RUNS a test may run again for the new
    # data — only a run handed one of the data, as the crate's `run-env`
    # declares. And the data and the footer's wrapper that carries them must
    # move, or the module never reached the build and this compares the
    # build with itself.
    person-module-rebuilds-only-the-daemon =
      let
        data = [
          "RONOCTUA_TEXTS"
          "RONOCTUA_BRAIN_CATALOGUE"
          "RONOCTUA_ICONSET"
        ];
        facts = toFile "person-module.json" {
          after = personEdited.unitIndex;
          daemon =
            plain personEdited.release.libs.ronoctua-brain-daemon.drvPath
            != plain rust.release.libs.ronoctua-brain-daemon.drvPath;
          # What each test run is handed when it runs, by crate.
          runEnv = lib.mapAttrs (_: d: lib.attrNames (d.drvAttrs.unit.runEnv or { })) rust.test.testRuns;
          inherit data compiles;
          carries = lib.mapAttrs (_: d: d.before != d.after) {
            "the brain's catalogue (RONOCTUA_BRAIN_CATALOGUE)" = {
              before = plain rust.keyed.RONOCTUA_BRAIN_CATALOGUE;
              after = plain personEdited.keyed.RONOCTUA_BRAIN_CATALOGUE;
            };
            "the texts (RONOCTUA_TEXTS)" = {
              before = plain rust.keyed.RONOCTUA_TEXTS;
              after = plain personEdited.keyed.RONOCTUA_TEXTS;
            };
            # The fixture's glyph token names its glyph in the browser.
            "the glyph browser's index (RONOCTUA_ICONSET)" = {
              before = plain rust.atRunTime.RONOCTUA_ICONSET;
              after = plain personEdited.atRunTime.RONOCTUA_ICONSET;
            };
            "the footer's wrapper (rust.shell)" = {
              before = plain rust.shell.drvPath;
              after = plain personEdited.shell.drvPath;
            };
          };
        };
      in
      mkCheck "person-module-rebuilds-only-the-daemon" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())
        exec(open("${granularityModel}").read())

        # PARSED: the build's index, of the distro and of a machine with the
        # person's fixture module, as Nix evaluated them; and the plan.
        model_load("${planFile}", "${dataFile}")
        facts = json.load(open(sys.argv[1]))
        before, after = json.load(open("${baseFile}")), facts["after"]
        floor(len(before), 600, "entries of the build's index")
        wrong = []
        if not facts["daemon"]:
            wrong.append("the daemon's library did not move: the person's module never reached the build")
        for what, moved in sorted(facts["carries"].items()):
            if not moved:
                wrong.append(f"{what} did not move: the person's module never reached what the footer reads")
        floor(len(facts["carries"]), 4, "derivations that carry the person's module")
        # Everything built from the daemon's library, as the plan says — its
        # sources are not edited, so not its formatting.
        want = {e for e in expected("crates/ronoctua-brain-daemon/src/lib.rs")
                if not e.startswith("fmt/") and e != "aggregate/rust-fmt"}
        floor(len(want), 5, "entries built from the daemon's library")
        got = changed_entries(before, after)
        kind = lambda e: (after.get(e) or before.get(e))["kind"]
        data = set(facts["data"])
        reran = []
        for e in sorted(got - want):
            if kind(e) == "testRuns" and data & set(facts["runEnv"].get(e.split("/")[-1], [])):
                reran.append(e)
            elif kind(e) in facts["compiles"]:
                wrong.append(f"{e} is compiled again for the person's module")
            else:
                wrong.append(f"{e} moved for the person's module, and reads none of {sorted(data)}")
        for e in sorted(want - got):
            wrong.append(f"{e} is built from the daemon and did not move")
        if wrong:
            print("ERROR: a person's brain module rebuilds more than the daemon:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the person's module moved the {len(want)} entries built from the daemon, and no other that"
              f" compiles; {len(reran)} test runs read its data and run again")
        END
      '';

    # A UNIT IS GIVEN WHAT ITS CRATE DECLARES, AND NOTHING ELSE — and a layer
    # nothing of ours.
    #
    # The values of the build's tables (`generated`, `testEnv`) are store
    # paths, and a path anywhere in a derivation is a reason to rebuild it.
    # So every unit is read whole (every attribute, the unit's description
    # included) and every path that is a table's value is a stray unless the
    # unit was handed that value under a name its crate declares. Which names
    # a unit is handed is checked against the crates' manifests, read here
    # with Python's own TOML reader (`env`, `test-env` in the test graph,
    # `run-env` of the crate and of every library below it for a test run).
    # And the layers: no table's path at all, no `RONOCTUA_*` and no cargo
    # directory — `deps-layer-is-lean`'s question, asked of the units'
    # layers. Not blind: a unit planted with a table's path in its inputs
    # must be found.
    #
    # A PROGRAM IS ALSO HANDED WHAT IT CITES (`cited`, `nix/rust/units.nix`):
    # every value the crates inside it were compiled with, which a release
    # library's bitcode bakes and only the linked program holds as bytes —
    # so a path of those among its inputs is no stray. And what it cites is
    # exactly that, both ways: the crates inside it are the ones the plan
    # links into it, and its `cited` is what their own units were handed —
    # one more would be a reason to relink it that nothing inside reads, one
    # fewer a path its closure would lack.
    units-see-only-what-they-read =
      let
        ctx = v: lib.attrNames (builtins.getContext (builtins.toJSON v));
        table = rust.generated // rust.testEnv;
        derivedBy = lib.foldl' (
          acc: n: lib.foldl' (a: c: a // { ${c} = (a.${c} or [ ]) ++ [ n ]; }) acc (ctx table.${n})
        ) { } (lib.attrNames table);
        given = d: ctx ((d.drvAttrs.unit.env or { }) // (d.drvAttrs.unit.runEnv or { }));
        strayIn =
          d: givenPaths:
          lib.unique (
            lib.concatMap (c: if derivedBy ? ${c} && !(lib.elem c givenPaths) then derivedBy.${c} else [ ]) (
              ctx d.drvAttrs
            )
          );
        top = d: lib.filter (lib.hasPrefix "RONOCTUA_") (lib.attrNames d.drvAttrs);
        # A program of a flavour that reaches a machine, and what it cites.
        isProgram = u: u.kind == "bins" && u.flavour != "test";
        citedOf = d: d.drvAttrs.cited or [ ];
        # What a crate inside a program was handed when compiled: its library
        # unit's environment, or — for the program's own crate when it has no
        # library — the program's own.
        envOfCrate =
          flavour: own: c:
          (rust.${flavour}.libs.${c} or own).drvAttrs.unit.env or { };
        dirs =
          d:
          lib.filter (v: builtins.match "CARGO_[A-Z_]*DIR" v != null) (
            lib.attrNames d.drvAttrs ++ lib.attrNames (d.drvAttrs.env or { })
          );
        census = map (
          u:
          {
            inherit (u) id flavour kind;
            crate = u.drv.passthru.crate;
            env = lib.attrNames (u.drv.drvAttrs.unit.env or { });
            runEnv = lib.attrNames (u.drv.drvAttrs.unit.runEnv or { });
            top = top u.drv;
            dirs = dirs u.drv;
            stray = strayIn u.drv (given u.drv ++ ctx (citedOf u.drv));
          }
          // lib.optionalAttrs (isProgram u) {
            cited = ctx (citedOf u.drv);
            insideEnv = lib.unique (
              lib.concatMap (c: ctx (envOfCrate u.flavour u.drv c)) u.drv.passthru.crates
            );
            inside = u.drv.passthru.crates;
            key = u.drv.passthru.unitKey;
          }
        ) allUnits;
        layerCensus =
          lib.mapAttrs
            (_: d: {
              top = top d;
              dirs = dirs d;
              stray = strayIn d [ ];
            })
            {
              release = rust.layers.release;
              release-raw = rust.layers.release.raw;
              release-workspace = rust.layers.release.raw.workspace;
              loop = rust.layers.loop;
              loop-raw = rust.layers.loop.raw;
              loop-workspace = rust.layers.loop.raw.workspace;
            };
        planted =
          let
            zip = rust.release.libs.ronoctua-zip;
            p = rust.unitsWith {
              buildInputs = zip.drvAttrs.buildInputs ++ [ rust.generated.RONOCTUA_TOKENS_RS ];
            };
          in
          strayIn (p.units "release").libs.ronoctua-zip [ ];
        facts = toFile "units-see-only-what-they-read.json" {
          inherit census planted;
          layers = layerCensus;
          generated = lib.attrNames rust.generated;
          testEnv = lib.attrNames rust.testEnv;
        };
      in
      mkCheck "units-see-only-what-they-read" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, pathlib, sys, tomllib
        exec(open("${pyPrelude}").read())

        # PARSED: every unit's and layer's attributes as Nix evaluated them
        # (the names it is handed, and every table's value its derivation
        # names), the plan, and the crates' manifests by Python's TOML reader.
        # FILE-WIDE: a manifest is read whole, by that parser, for its
        # `[package.metadata.ronoctua]` table — never searched.
        facts = json.load(open(sys.argv[1]))
        plan = json.load(open("${planFile}"))
        root = pathlib.Path("${self}")
        generated, test_env = set(facts["generated"]), set(facts["testEnv"])
        meta = {}
        for c, p in plan["packages"].items():
            m = tomllib.loads((root / p["dir"] / "Cargo.toml").read_text())
            meta[c] = m.get("package", {}).get("metadata", {}).get("ronoctua", {})

        # A test run is handed the `run-env` of its crate and of every
        # library of ours below its test harnesses, in the test graph.
        units = plan["graphs"]["test"]["units"]

        def below(c):
            seen, todo = set(), [k for k, u in units.items() if u["workspace"] and u["pkg"] == c
                                 and u["mode"] in ("test", "bench")]
            while todo:
                k = todo.pop()
                for d in units[k]["deps"]:
                    u = units.get(d["key"])
                    if u and u["workspace"] and d["key"] not in seen:
                        seen.add(d["key"])
                        todo.append(d["key"])
            return {units[k]["pkg"] for k in seen} | {c}

        # The crates whose code the plan links into a program: its own, and
        # every library of ours below it in its graph.
        def inside(flavour, key):
            graph = plan["graphs"][flavour]["units"]
            seen, todo, crates = set(), [key], {graph[key]["pkg"]}
            while todo:
                for d in graph[todo.pop()]["deps"]:
                    u = graph[d["key"]]
                    if d["key"] in seen or not u["workspace"]:
                        continue
                    seen.add(d["key"])
                    todo.append(d["key"])
                    if u["mode"] == "build" and ("lib" in u["kind"] or "proc-macro" in u["kind"]):
                        crates.add(u["pkg"])
            return crates

        wrong = []
        floor(len(facts["census"]), 600, "units")
        handed = 0
        runs_handed = 0
        programs = 0
        for u in facts["census"]:
            c, flavour, kind = u["crate"], u["flavour"], u["kind"]
            if "cited" in u:
                programs += 1
                linked = inside(flavour, u["key"])
                if set(u["inside"]) != linked:
                    wrong.append(f"{u['id']} says the crates inside it are {sorted(u['inside'])}, "
                                 f"and the plan links {sorted(linked)} into it")
                for p in sorted(set(u["cited"]) - set(u["insideEnv"])):
                    wrong.append(f"{u['id']} cites {p}, which no crate inside it was compiled with")
                for p in sorted(set(u["insideEnv"]) - set(u["cited"])):
                    wrong.append(f"{u['id']} does not cite {p}, which a crate inside it was compiled with:"
                                 " its closure would lack it")
            for n in u["top"]:
                wrong.append(f"{u['id']} carries {n} as an attribute of its own, beside what its crate declares")
            for n in u["dirs"]:
                wrong.append(f"{u['id']} names a cargo directory ({n})")
            for n in u["stray"]:
                wrong.append(f"{u['id']} is built from the value of {n}, which it is not handed")
            if kind == "testRuns":
                want = set().union(*(set(meta[b].get("run-env", [])) for b in below(c))) & (generated | test_env)
                runs_handed += bool(u["runEnv"])
                got = set(u["runEnv"])
            elif kind == "fmt":
                want, got = set(), set(u["env"]) | set(u["runEnv"])
            else:
                names = set(meta[c].get("env", [])) | (set(meta[c].get("test-env", [])) if flavour == "test" else set())
                want = names & (generated | (test_env if flavour == "test" else set()))
                handed += bool(u["env"])
                got = set(u["env"])
            for n in sorted(got - want):
                wrong.append(f"{u['id']} is handed {n}, which {c} does not declare")
            for n in sorted(want - got):
                wrong.append(f"{u['id']} is not handed {n}, which {c} declares")
        floor(handed, 20, "units handed a value of the build's tables")
        floor(runs_handed, 10, "test runs handed a value")
        floor(programs, 80, "release and loop programs, each citing what is inside it")
        for name, l in sorted(facts["layers"].items()):
            for n in l["top"]:
                wrong.append(f"the {name} layer is given {n}")
            for n in l["dirs"]:
                wrong.append(f"the {name} layer names a cargo directory ({n})")
            for n in l["stray"]:
                wrong.append(f"the {name} layer is built from the value of {n}")
        floor(len(facts["layers"]), 6, "layer derivations")
        if "RONOCTUA_TOKENS_RS" not in facts["planted"]:
            wrong.append("a unit planted with the tokens' path among its inputs was not found: this census is blind")
        if wrong:
            print("ERROR: a unit sees what its crate does not read:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            print("  A table's value in a derivation rebuilds it when the value moves; a crate", file=sys.stderr)
            print("  declares what it reads in [package.metadata.ronoctua] (nix/rust/units.nix).", file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(facts['census'])} units, each handed what its crate declares ({handed} with values,"
              f" {runs_handed} test runs; {programs} programs cite what their crates were compiled with);"
              f" {len(facts['layers'])} layer derivations with nothing of ours")
        END
      '';

    # WHAT A CRATE READS IS WHAT IT DECLARES, BOTH WAYS — before anything is
    # compiled. The driver refuses an undeclared compile-time read when the
    # unit builds; this finds it in the sources in a second, and it finds
    # what the driver cannot: a declaration nothing reads, which rebuilds the
    # crate every time that value moves. And the run-time half: a name the
    # build gives that the crate's code names is given to its tests or kept
    # from them on purpose.
    crate-reads-are-declared = mkCheck "crate-reads-are-declared" [ pkgs.python3 ] ''
      python3 - ${self} ${tablesFile} ${planFile} ${dataFile} <<'END'
      import json, pathlib, re, sys, tomllib
      exec(open("${pyPrelude}").read())
      exec(open("${granularityModel}").read())


      # FILE-WIDE: a read is an `option_env!`/`env!` anywhere in a crate's
      # sources, or a name a build script says; the question is which names, in
      # which file, so every file is read whole (comments off, strings kept).
      root = pathlib.Path(sys.argv[1])
      tables = json.load(open(sys.argv[2]))
      model_load(sys.argv[3], sys.argv[4])

      COMPILE = re.compile(r'\b(?:option_env|env)!\s*\(\s*"(RONOCTUA_[A-Z0-9_]+)"')
      NAMED = re.compile(r'"(RONOCTUA_[A-Z0-9_]+)"')
      known = set(tables["generated"]) | set(tables["testEnv"]) | set(tables["atRunTime"]) | set(tables["optional"])

      every_rs = sorted(p for p in (root / "crates").rglob("*.rs"))
      floor(len(every_rs), 500, "Rust files under crates/")


      def files_of(c):
          """Every `.rs` a unit of this crate compiles: its own (`src/`,
          `build.rs`, `tests/`, `benches/`, `examples/`, `fixtures/`) and what
          its `data` names in another folder."""
          d = DIRS[c]
          out = []
          for p in every_rs:
              f = str(p.relative_to(root))
              r = f[len(d) + 1:] if f.startswith(d + "/") else None
              if r is not None and (r == "build.rs" or r.split("/")[0] in ("src",) + TEST_DIRS):
                  out.append((f, p))
              elif in_data(c, f):
                  out.append((f, p))
          return out


      # A FILE MAY BE TWO CRATES' (the brain daemon's `data` holds the library's
      # module folders): its read is declared when one crate that compiles it
      # declares it.
      holders = {}
      for c in DIRS:
          for f, _ in files_of(c):
              holders.setdefault(f, []).append(c)
      declared = {}
      for c in DIRS:
          meta = tomllib.loads((root / DIRS[c] / "Cargo.toml").read_text()).get("package", {}).get("metadata", {}).get("ronoctua", {})
          declared[c] = {k: set(meta.get(k, [])) for k in ("env", "test-env", "env-absent", "run-env", "run-env-absent")}

      wrong = []
      swept = 0
      reads_seen = 0
      for c in sorted(DIRS):
          env, test, absent = declared[c]["env"], declared[c]["test-env"], declared[c]["env-absent"]
          run, run_absent = declared[c]["run-env"], declared[c]["run-env-absent"]
          product, tests, script, named = set(), set(), set(), set()
          elsewhere = set()
          for f, p in files_of(c):
              swept += 1
              text = uncommented(p.read_text(errors="replace"))
              if f == DIRS[c] + "/build.rs":
                  script |= set(NAMED.findall(text))
                  continue
              in_tests = f[len(DIRS[c]) + 1:].split("/")[0] in TEST_DIRS
              kept = text if in_tests else without_tests(text)
              here = set(COMPILE.findall(text))
              out_of_tests = set() if in_tests else set(COMPILE.findall(kept))
              others = [o for o in holders[f] if o != c]
              elsewhere |= {n for n in here if any(n in declared[o]["env"] | declared[o]["test-env"] | declared[o]["env-absent"] for o in others)}
              product |= out_of_tests
              tests |= here - out_of_tests
              named |= set(NAMED.findall(COMPILE.sub("", text)))
          reads_seen += len(product | tests | script)
          # ONE WAY: every compile-time read is declared — a product's in `env`
          # or `env-absent`, a test's in those or `test-env`. The driver refuses
          # the same read when the unit builds; this says so before anything does.
          for n in sorted((product | script) - env - absent - elsewhere):
              wrong.append(f"{c} reads {n} at compile time and does not declare it in `env` (or `env-absent`)")
          for n in sorted(tests - env - test - absent - product - script - elsewhere):
              wrong.append(f"{c}'s tests read {n} at compile time and the crate declares it nowhere (`test-env`)")
          # AND RUN TIME: a name the build gives (`generated`, the tests' table, the
          # wrapper's) that the crate's code names is declared — given to its tests
          # (`run-env`) or kept from them on purpose (`run-env-absent`).
          for n in sorted((named & known) - run - run_absent - env - test - absent):
              wrong.append(f"{c} names {n} at run time and declares it in neither `run-env` nor `run-env-absent`")
          # THE OTHER WAY: every declared name is read. A stale `env` is a value
          # nothing reads that still rebuilds the crate when it moves.
          for n in sorted(env - product - script):
              wrong.append(f"{c} declares {n} in `env` and no product code or build script of it reads it")
          for n in sorted(test - product - tests - script):
              wrong.append(f"{c} declares {n} in `test-env` and nothing of it reads it at compile time")
          for n in sorted(absent - product - tests - script):
              wrong.append(f"{c} declares {n} in `env-absent` and nothing of it reads it at compile time")
          for n in sorted((run | run_absent) - named - product - tests - script):
              wrong.append(f"{c} declares {n} for its tests' run and names it nowhere")

      floor(len(DIRS), 80, "crates of the plan")
      floor(swept, 800, "files of the crates swept for a read")
      floor(reads_seen, 60, "compile-time reads found")
      if wrong:
          print("ERROR: what a crate reads and what it declares (`[package.metadata.ronoctua]`) disagree:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          print("  A unit is given what its crate declares and nothing else (`nix/rust/units.nix`).", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(DIRS)} crates, {swept} files, {reads_seen} compile-time reads; each read declared and each declaration read")
      END
    '';

    # A `data` ENTRY NAMES FILES, OR THE BUILD REFUSES IT.
    #
    # `data` is what a unit sees beyond its own sources, and a pattern that
    # walks folders that exist and matches nothing — the Rust it named moved
    # away, an extension mistyped — is a declaration saying the crate reads
    # something while the unit sees nothing new: it builds, and nothing says
    # so. Asked of the units' own reader (`nix/rust/units.nix`, `dataOf`) at
    # evaluation: four planted entries that name no file are each refused,
    # and every real entry of every crate names its files.
    data-entries-name-files =
      let
        inherit (rust.unitsWith { }) dataOf;
        files = c: map toString (lib.fileset.toList (dataOf c));
        refused = c: !(builtins.tryEval (builtins.deepSeq (files c) true)).success;
        zip = rust.unitCrates.ronoctua-zip;
        daemon = rust.unitCrates.ronoctua-brain-daemon;
        planted = {
          "an extension no file of the folder has (src/**/*.zzz)" = zip // {
            data = [ "src/**/*.zzz" ];
          };
          "Rust under a folder of prose only (a module's i18n/**/*.rs)" = daemon // {
            data = [ "../ronoctua-brain/modules/disk-space/i18n/**/*.rs" ];
          };
          "a `*` over folders none of which holds the file" = daemon // {
            data = [ "../ronoctua-brain/modules/*/no-such-file.rs" ];
          };
          "a path that does not exist" = zip // {
            data = [ "no-such-folder" ];
          };
        };
        facts = toFile "data-entries-name-files.json" {
          accepted = lib.attrNames (lib.filterAttrs (_: c: !(refused c)) planted);
          # The real crates' entries, as the units read them: a throw here
          # fails the evaluation of every unit of that crate as well.
          real = lib.mapAttrs (_: c: lib.length (files c)) (
            lib.filterAttrs (_: c: c.data != [ ]) rust.unitCrates
          );
        };
      in
      mkCheck "data-entries-name-files" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: what the units' reader made of each entry, at evaluation.
        facts = json.load(open(sys.argv[1]))
        wrong = [f"a data entry naming no file was accepted: {a}" for a in facts["accepted"]]
        real = facts["real"]
        floor(len(real), 3, "crates that declare data")
        floor(real.get("ronoctua-brain-daemon", 0), 16, "files the brain daemon's data names (every module's Rust)")
        wrong += [f"{c}'s data names no file and was accepted" for c, n in sorted(real.items()) if n == 0]
        if wrong:
            print("ERROR: a `data` entry that names no file is not refused (nix/rust/units.nix, dataOf):", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: 4 entries naming no file refused; {len(real)} crates' data name {sum(real.values())} files")
        END
      '';

    # EVERY NAME THE BUILD BAKES IS READ BY SOME CRATE.
    #
    # A value in `generated` that no crate declares is handed to no unit —
    # the build pays for its derivation, and the program that would need it
    # never sees it (`RONOCTUA_TOR_OBFS4` and `_SNOWFLAKE` were two such, and
    # went). The same of the tests' table and of the names `generated` holds
    # only on some machines.
    every-generated-name-is-read =
      let
        facts = toFile "every-generated-name-is-read.json" {
          inherit tables;
          declared = lib.mapAttrs (_: c: c.declared) rust.unitCrates;
        };
      in
      mkCheck "every-generated-name-is-read" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the build's tables and what each crate declares, as the
        # unit builder read them.
        facts = json.load(open(sys.argv[1]))
        t, d = facts["tables"], facts["declared"].values()
        env = set().union(*(set(c["env"]) | set(c["absent"]) for c in d))
        tests = set().union(*(set(c["env"]) | set(c["test"]) | set(c["run"]) for c in d))
        floor(len(t["generated"]), 60, "names in `generated`")
        floor(len(t["testEnv"]), 15, "names in the tests' table")
        wrong = [f"{n} is in `generated` and no crate declares it (`env`)" for n in sorted(set(t["generated"]) - env)]
        wrong += [f"{n} is in the tests' table and no crate declares it (`test-env`, `run-env`)"
                  for n in sorted(set(t["testEnv"]) - tests)]
        wrong += [f"{n} may be in `generated` and no crate declares it" for n in sorted(set(t["optional"]) - env)]
        if wrong:
            print("ERROR: the build bakes a name nothing reads:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(t['generated'])} baked names and {len(t['testEnv'])} test names, each read by a crate")
        END
      '';

    # A FLAVOUR IS BUILT FROM ITS OWN FLAVOUR.
    #
    # `release` is what a machine runs, `loop` what the nested loop runs and
    # `test` what the tests link (with `test-support` turned on, which must
    # never be in a product). A release program linked against a loop
    # library would run, slower, and nothing on a screen would say so. Read
    # at evaluation: each unit's flavour, the profiles it compiles with (the
    # release graph's are none of the others'), the name that says it in the
    # store — the marks `the-loop-profile-is-never-installed` looks for —
    # and what it is BUILT FROM, read off the derivation itself: every
    # derivation its attributes cite, which are the only ones its sandbox
    # holds and so the only ones the driver can link. Each must be a unit of
    # its own flavour, its own layer or a third-party unit of its own profile
    # (one derivation per key: the release graph's, or the loop's and the
    # test graph's, which share them as they shared a layer), and none a
    # second build of a unit.
    # What the unit declares (`passthru.unitDeps`, the edges the index, the
    # rebuild cost and the battery read) has to agree with that: everything
    # it declares is an input, and every unit input is reached from what it
    # declares (a library is handed the whole chain below it; a test run,
    # only its harness).
    flavours-never-mix =
      let
        # The third-party units, by the profile they are compiled with.
        thirdParty =
          lib.concatMap
            (
              f:
              lib.mapAttrsToList (key: d: {
                inherit key d;
                flavour = if f == "release" then "release" else "loop";
              }) (rust.${f}.deps or { })
            )
            [
              "release"
              "loop"
              "test"
            ];
        # Every derivation of the build by its path: the units, the two
        # layers and the third-party units.
        entries = lib.listToAttrs (
          map (
            u:
            lib.nameValuePair (plain u.drv.drvPath) {
              inherit (u) id flavour kind;
              inherit (u.drv) name;
            }
          ) allUnits
          ++ lib.mapAttrsToList (
            f: l:
            lib.nameValuePair (plain l.drvPath) {
              id = "layer/${f}";
              flavour = f;
              kind = "layer";
              inherit (l) name;
            }
          ) { inherit (rust.layers) release loop; }
          ++ map (
            t:
            lib.nameValuePair (plain t.d.drvPath) {
              id = "${t.flavour}/deps/${t.key}";
              inherit (t) flavour;
              kind = "deps";
              inherit (t.d) name;
            }
          ) thirdParty
        );
        facts = toFile "flavours-never-mix.json" {
          inherit entries;
          # What each third-party unit declares it is built from.
          depsDeclared = lib.listToAttrs (
            map (
              t: lib.nameValuePair (plain t.d.drvPath) (map (d: plain d.drvPath) (t.d.passthru.unitDeps or [ ]))
            ) thirdParty
          );
          units = map (u: {
            inherit (u) id flavour kind;
            said = u.drv.passthru.flavour or null;
            inherit (u.drv) name;
            profiles = lib.attrNames (u.drv.drvAttrs.unit.profiles or { });
            inputs = lib.filter (lib.hasSuffix ".drv") (
              lib.attrNames (builtins.getContext (builtins.toJSON u.drv.drvAttrs))
            );
            declared = map (d: plain d.drvPath) (u.drv.passthru.unitDeps or [ ]);
          }) allUnits;
          layers = {
            release = plain rust.layers.release.drvPath;
            loop = plain rust.layers.loop.drvPath;
          };
        };
      in
      mkCheck "flavours-never-mix" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the units as Nix evaluated them — each derivation's own
        # inputs and its declaration —, and the plan.
        facts = json.load(open(sys.argv[1]))
        plan = json.load(open("${planFile}"))
        entries = facts["entries"]
        used = {g: {u["profile"] for u in plan["graphs"][g]["units"].values() if u["workspace"]}
                for g in ("release", "loop", "test")}
        profiles = {"release": used["release"], "loop": used["loop"] | used["test"], "test": used["loop"] | used["test"]}
        layer_of = {"release": facts["layers"]["release"], "loop": facts["layers"]["loop"], "test": facts["layers"]["loop"]}
        # The profile whose third-party units a flavour links.
        deps_of = {"release": "release", "loop": "loop", "test": "loop"}
        named = {e["name"] for e in entries.values()}
        # What each derivation declares it is built from (a layer, nothing).
        by_id = {e["id"]: d for d, e in entries.items()}
        declared = {d: [] for d in entries}
        declared.update(facts["depsDeclared"])
        for u in facts["units"]:
            declared[by_id[u["id"]]] = u["declared"]


        def store_name(path):
            base = path.rsplit("/", 1)[-1]
            return base[33:].removesuffix(".drv")


        def reached(drvs):
            seen, todo = set(), list(drvs)
            while todo:
                d = todo.pop()
                if d not in seen:
                    seen.add(d)
                    todo += declared.get(d, [])
            return seen


        wrong = []
        if used["release"] & (used["loop"] | used["test"]):
            wrong.append("the release graph shares a profile with the loop's: the two flavours are one, and this proves nothing")
        floor(len(facts["units"]), 500, "units")
        edges = 0
        for u in facts["units"]:
            f, name, uid = u["flavour"], u["name"], u["id"]
            if u["said"] != f:
                wrong.append(f"{uid} says it is {u['said']}")
            if f is not None:
                # A clippy unit (test graph) is a verdict, named `rust-clippy-`.
                marked = {"release": not (name.startswith(("rust-loop-", "rust-test-", "rust-clippy-"))
                                          or f"-loop-${version}" in name),
                          "loop": name.startswith("rust-loop-") or name.endswith(f"-loop-${version}"),
                          "test": name.startswith(("rust-test-", "rust-clippy-"))}[f]
                if not marked:
                    wrong.append(f"{uid} is named {name}, which does not say it is {f}")
                for p in sorted(set(u["profiles"]) - profiles[f]):
                    wrong.append(f"{uid} compiles with profile {p}, which is no profile of the {f} graph")
            real = {d for d in u["inputs"] if d in entries}
            for d in sorted(set(u["inputs"]) - real):
                if store_name(d) in named:
                    wrong.append(f"{uid} is built from {d}, a second build of {store_name(d)} and no derivation of this build")
            for d in sorted(real):
                e = entries[d]
                edges += 1
                if f is None:
                    wrong.append(f"{uid} is built from {e['id']}, and a formatting verdict is built from no unit")
                elif e["kind"] == "layer":
                    if d != layer_of[f]:
                        wrong.append(f"{uid} is built on the {e['flavour']} layer ({e['name']})")
                elif e["kind"] == "deps":
                    if e["flavour"] != deps_of[f]:
                        wrong.append(f"{uid} is built from {e['id']}, a third-party unit of the {e['flavour']} profile")
                elif e["flavour"] != f:
                    wrong.append(f"{uid} is built from {e['id']}, a {e['flavour']} unit")
            said = set(u["declared"])
            for d in sorted(said - real):
                wrong.append(f"{uid} declares {entries.get(d, {}).get('id', d)} (passthru.unitDeps) and its derivation is not built from it")
            for d in sorted(real - reached(said)):
                wrong.append(f"{uid} is built from {entries[d]['id']}, which nothing it declares reaches (passthru.unitDeps)")
        floor(edges, 3000, "inputs of units that are units or layers")
        if wrong:
            print("ERROR: a flavour is built from another, or a unit is built from what it does not declare:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(facts['units'])} units and {edges} inputs read off their derivations, each flavour built from its own and as declared")
        END
      '';

    # WHAT A CRATE LOOKS FOR BESIDE ITS OWN PROGRAM IS WHAT IT DECLARES
    # (`beside`), both ways. `current_exe()`'s folder joined with a name is a
    # program that has to be in the same `bin/` — the shell's bundle is made
    # from these declarations (`rust.shell`), and one left out is a lookup
    # that finds nothing on a machine; one declared and never looked for is a
    # program bundled for nothing.
    siblings-are-beside = mkCheck "siblings-are-beside" [ pkgs.python3 ] ''
      python3 - ${self} <<'END'
      import json, pathlib, re, sys, tomllib
      exec(open("${pyPrelude}").read())
      exec(open("${granularityModel}").read())

      # FILE-WIDE: a lookup is `current_exe()` followed, within a few
      # lines, by its folder joined with a name — `.parent()?.join(<name>)`,
      # a literal or a `const` of the same file — anywhere in a crate's
      # product code (tests cut off: a test writing beside its own harness
      # is not a program looking for a sibling; and a folder of its own
      # joined with a name is not its program's folder).
      root = pathlib.Path(sys.argv[1])
      model_load("${planFile}", "${dataFile}")
      WINDOW = 6
      wrong, seen, swept = [], 0, 0
      for c, d in sorted(DIRS.items()):
          meta = tomllib.loads((root / d / "Cargo.toml").read_text()).get("package", {}).get("metadata", {}).get("ronoctua", {})
          beside = set(meta.get("beside", []))
          found = {}
          for p in sorted((root / d / "src").rglob("*.rs")):
              if p.name == "tests.rs" or "tests" in p.relative_to(root / d / "src").parts[:-1]:
                  continue
              swept += 1
              text = without_tests(uncommented(p.read_text(errors="replace")))
              consts = dict(re.findall(r'const\s+([A-Z_][A-Z0-9_]*)\s*:\s*&str\s*=\s*"([^"]+)"', text))
              lines = text.splitlines()
              for i, line in enumerate(lines):
                  if "current_exe()" not in line:
                      continue
                  window = "\n".join(lines[i : i + WINDOW])
                  for m in re.finditer(r'parent\(\)\s*(?:\?|\.(?:expect|unwrap)\([^)]*\))?\s*'
                                       r'\.join\(\s*(?:"([a-z0-9-]+)"|([A-Z_][A-Z0-9_]*))\s*\)', window):
                      name = m.group(1) or consts.get(m.group(2))
                      if name:
                          found.setdefault(name, str(p.relative_to(root)))
          seen += len(found)
          for name, where in sorted(found.items()):
              if name not in beside:
                  wrong.append(f"{c} looks for {name} beside its program ({where}) and does not declare it in `beside`")
          for name in sorted(beside - set(found)):
              wrong.append(f"{c} declares {name} in `beside` and never looks for it")
      floor(swept, 400, "product files swept")
      floor(seen, 5, "programs looked for beside another")
      if wrong:
          print("ERROR: what a crate looks for beside its program and what it declares disagree:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: {seen} programs looked for beside another, each declared, each declaration looked for")
      END
    '';

    # NO DOCTEST IS RUNNABLE, because no unit runs one.
    #
    # The test graph has no doctest units (the plan drops them): every block
    # in a library's documentation is `text`, `json`, another language, or
    # `ignore`. A block that rustdoc would compile and run (no tag, `rust`,
    # `should_panic`, `no_run`, `compile_fail`, an edition) would be a test
    # nobody runs, green for ever — so it is refused, and the day doctests
    # are wanted they get units and this check goes.
    no-runnable-doctests = mkCheck "no-runnable-doctests" [ pkgs.python3 ] ''
      python3 - ${self} <<'END'
      import json, pathlib, re, sys
      exec(open("${pyPrelude}").read())
      exec(open("${granularityModel}").read())

      # FILE-WIDE: a doc comment's fences, in every file a library target
      # compiles (its `src/` without the programs, and its `data`).
      # PROSE: the subject is the documentation — the code blocks rustdoc
      # reads out of `///` and `//!` — so the comments are what is read.
      root = pathlib.Path(sys.argv[1])
      model_load("${planFile}", "${dataFile}")
      RUST = {"", "rust", "should_panic", "no_run", "compile_fail", "test_harness", "standalone_crate",
              "edition2015", "edition2018", "edition2021", "edition2024"}
      DOC = re.compile(r"^\s*(?://[/!]|\*)\s?(.*)$")
      libs = {u["pkg"] for u in PLAN["graphs"]["test"]["units"].values()
              if u["workspace"] and "lib" in u["kind"] and u["mode"] == "build"}
      floor(len(libs), 60, "libraries of the plan")
      wrong, fences, swept = [], 0, 0
      for p in sorted((root / "crates").rglob("*.rs")):
          f = str(p.relative_to(root))
          owners = [c for c in libs if sees_lib(c, f)]
          if not owners:
              continue
          swept += 1
          text = p.read_text(errors="replace")
          if re.search(r"#!?\[doc\s*=\s*include_str!", text):
              wrong.append(f"{f} takes documentation from a file, which no sweep reads for a doctest")
          inside = False
          for n, line in enumerate(text.splitlines(), 1):
              m = DOC.match(line)
              if not m or not line.lstrip().startswith(("///", "//!")):
                  continue
              body = m.group(1).strip()
              if not body.startswith("```"):
                  continue
              if inside:
                  inside = False
                  continue
              inside = True
              fences += 1
              tags = {t.strip() for t in re.split(r"[,\s]+", body[3:].strip()) if t.strip()}
              if "ignore" in tags:
                  continue
              if all(t in RUST for t in tags):
                  wrong.append(f"{f}:{n}: a doc block rustdoc would run (`{body}`), and no unit runs doctests")
      floor(swept, 300, "library files swept")
      floor(fences, 40, "doc code blocks")
      if wrong:
          print("ERROR: a runnable doctest that nothing runs:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          print("  Tag it `text` (or the language it is), or `ignore`.", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {fences} doc code blocks in {swept} library files, none of them a test nobody runs")
      END
    '';

    # THE GRAPHICS LIBRARIES ARE IN A PROGRAM'S RUNPATH EXACTLY WHEN IT LOADS
    # THEM. wgpu's GPU layer, ash, EGL, Wayland and xkbcommon open their
    # libraries with `dlopen`, which no linker records: a program built from
    # one of those crates and without the library in its runpath falls back
    # to software, or finds no display, in silence; one with a library that
    # nothing in it opens carries it in its closure for nothing (the rescue
    # console, the queue). Read off the built programs (`patchelf
    # --print-rpath`), library by library, against the plan's graph of each
    # and what each of those crates opens — written here, from upstream.
    graphics-rpath-only-where-graphics-are =
      let
        graphicsDirs = lib.mapAttrs (_: p: lib.makeLibraryPath [ p ]) {
          inherit (pkgs)
            vulkan-loader
            wayland
            libxkbcommon
            libGL
            ;
        };
        programs =
          lib.mapAttrsToList (n: b: {
            id = "release/bins/${n}";
            path = "${b}/bin/${n}";
            flavour = "release";
            key = b.passthru.unitKey;
          }) (lib.filterAttrs (_: b: b.passthru.role != "apart") rust.release.bins)
          ++
            map
              (n: {
                id = "loop/bins/${n}";
                path = "${rust.loop.bins.${n}}/bin/${n}";
                flavour = "loop";
                key = rust.loop.bins.${n}.passthru.unitKey;
              })
              [
                "ronoctua-shell"
                "ronoctua-render-png"
              ];
        facts = toFile "graphics-rpath.json" { inherit programs graphicsDirs; };
      in
      mkCheck "graphics-rpath-only-where-graphics-are" [ pkgs.python3 pkgs.patchelf ] ''
        python3 - ${facts} <<'END'
        import json, os, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: each program's runpath and needed libraries as patchelf
        # reads them, and the plan's graph of each program.
        facts = json.load(open(sys.argv[1]))
        plan = json.load(open("${planFile}"))
        # What each crate opens: wgpu's GPU layer the Vulkan loader, EGL and
        # libwayland-egl; ash the loader; khronos-egl EGL; wayland-sys (its
        # `dlopen` feature) libwayland; xkbcommon-dl libxkbcommon.
        OPENS = {"wgpu-hal": {"vulkan-loader", "libGL", "wayland"}, "ash": {"vulkan-loader"},
                 "khronos-egl": {"libGL"}, "wayland-sys": {"wayland"}, "xkbcommon-dl": {"libxkbcommon"}}
        dirs = facts["graphicsDirs"]
        floor(len(dirs), 4, "graphics library directories")
        wrong, graphical = [], 0
        for p in facts["programs"]:
            units = plan["graphs"][p["flavour"]]["units"]
            seen, todo = set(), [p["key"]]
            while todo:
                k = todo.pop()
                for d in units[k]["deps"]:
                    if d["key"] not in seen:
                        seen.add(d["key"])
                        todo.append(d["key"])
            loads = sorted({units[k]["pkg"] for k in seen} & set(OPENS))
            need = set().union(*(OPENS[c] for c in loads))
            rpath = set(subprocess.run(["patchelf", "--print-rpath", p["path"]], capture_output=True, text=True,
                                       check=True).stdout.strip().split(":"))
            has = {name for name, d in dirs.items() if d in rpath}
            # A library the program LINKS (`DT_NEEDED`: libxkbcommon, by the
            # `xkbcommon` crate) is in its runpath by the linker's own hand.
            needed = subprocess.run(["patchelf", "--print-needed", p["path"]], capture_output=True, text=True,
                                    check=True).stdout.split()
            linked = {name for name, d in dirs.items() if any(os.path.exists(os.path.join(d, n)) for n in needed)}
            graphical += bool(loads)
            for name in sorted(need - has):
                wrong.append(f"{p['id']} is built from {', '.join(loads)}, which open {name}, and its runpath lacks it")
            for name in sorted(has - need - linked):
                wrong.append(f"{p['id']} carries {name} in its runpath, and nothing it is built from opens or links it")
        floor(len(facts["programs"]), 40, "programs read")
        floor(graphical, 3, "programs built from a crate that loads a graphics library")
        if wrong:
            print("ERROR: a program's runpath and the graphics it loads disagree:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {graphical} of {len(facts['programs'])} programs load graphics and carry the runpath; the rest do not")
        END
      '';

    # THE INSTALLER'S SEED HOLDS EVERY SHIPPED PROGRAM — the release unit
    # itself, or what it is installed as (the wrapper that hands a painter its
    # palettes, the shell's bundle that copies it beside the footer), which is
    # built from the unit — so a machine installed from the ISO compiles none
    # of them. Read at evaluation from the installer's own configuration
    # (`ronoctua.seed`): every program whose role is `ship` is a member of the
    # seed or a derivation a member is built from directly, and every entry of
    # `rust.shipped` (the outputs' list of what ships, lane N2) is a member.
    seed-holds-every-shipped-bin =
      let
        seed = lib.flatten self.nixosConfigurations.installer._module.args.ronoctua.seed;
        drvs = lib.filter lib.isDerivation seed;
        members = map (d: plain d.drvPath) drvs;
        held = lib.unique (
          members ++ lib.concatMap (d: lib.attrNames (builtins.getContext (builtins.toJSON d.drvAttrs))) drvs
        );
        ship = lib.filterAttrs (_: b: b.passthru.role == "ship") rust.release.bins;
        facts = toFile "seed-holds-every-shipped-bin.json" {
          inherit held members;
          ship = lib.mapAttrs (_: b: plain b.drvPath) ship;
          shipped = if rust ? shipped then map (d: plain d.drvPath) (lib.toList rust.shipped) else null;
        };
      in
      mkCheck "seed-holds-every-shipped-bin" [ pkgs.python3 ] ''
        python3 - ${facts} <<'END'
        import json, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the seed's derivations, what each is built from directly,
        # and the shipped programs', as Nix evaluated them.
        facts = json.load(open(sys.argv[1]))
        held, members, ship = set(facts["held"]), set(facts["members"]), facts["ship"]
        floor(len(ship), 25, "shipped programs")
        floor(len(members), 5, "derivations in the seed")
        wrong = [f"{b} is shipped and the ISO's seed does not hold it: an installation compiles it"
                 for b, d in sorted(ship.items()) if d not in held]
        if facts["shipped"] is None:
            wrong.append("the build names no `rust.shipped`, the list of what ships in the form a machine runs it")
        else:
            floor(len(facts["shipped"]), 2, "entries of rust.shipped")
            missing = [d for d in facts["shipped"] if d not in members]
            if missing:
                wrong.append(f"{len(missing)} entries of rust.shipped are not in the ISO's seed: " + ", ".join(missing[:5]))
        if wrong:
            print("ERROR: the installer's seed does not hold what ships:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the seed holds the {len(ship)} shipped programs and every entry of rust.shipped")
        END
      '';
  };

  # THE MODEL AND THE BUILD AS DATA, for a check of another file that compares
  # an edit with the graph (`tests/text-fragments.nix`).
  model = {
    inherit
      granularityModel
      baseFile
      planFile
      dataFile
      ;
  };
}
