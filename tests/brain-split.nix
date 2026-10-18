# THE BRAIN IN TWO CRATES (the 2026-10-12 wave): the checks that prove the
# split holds.
#
# The brain was one crate, `ronoctua-brain`, and the footer linked all of it —
# the daemon's core and every module's code, the person's included on a
# machine with some. So a line in any module recompiled the footer, and a
# person's own module recompiled it on every machine of theirs. Now the
# LIBRARY (`ronoctua-brain`: the wire, the mind, a module's vocabulary, the
# distro's manifests and the shared halves a `namedBy` reads) is what the
# footer links, and the DAEMON (`ronoctua-brain-daemon`, whose binary is still
# `ronoctua-brain`) compiles what runs and every `module.rs`. These three
# checks are what keeps that true:
#
#   - `brain-library-links-no-module` BUILDS the library's unit with every
#     module's code poisoned, and a person's module poisoned and handed in (its
#     `shared.rs` too): it builds; the daemon's unit over the same poison does
#     not (so the poison is real); and the library's over a poisoned shared
#     half does not (so the half is the library's);
#   - `brain-daemon-is-a-leaf` reads every manifest of the workspace: nothing
#     but the panel's driver depends on the daemon, the footer and the runtime
#     never reach it, and its binary keeps the unit's name;
#   - `brain-shared-half-is-the-distros` asks the loader and the generator: a
#     person's folder with a `shared.rs` loads and its record has no shared
#     half, and a record that says otherwise is refused; the library's part
#     never takes a person's module; and every shared half of the tree is
#     listed, and read by a crate its module's `namedBy` names.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  self,
  rust,
  mkCheck,
  pyPrelude,
  languages,
  brainModules,
  brainManifestsRs,
  withPersonModules,
}:

let
  # A PERSON'S MODULE, as a machine hands it in: the fixture the person's
  # check loads, and the registry the daemon would compile it from.
  person = withPersonModules { dir = ../crates/ronoctua-brain/tests/person; };
  registry = person.brainModulesRs;
in
{
  # THE FOOTER'S CRATE COMPILES NO MODULE'S CODE.
  #
  # What the split promises is about COMPILATION — a module's line, or a
  # person's module, never reaches the crate the footer links — and the only
  # honest proof of that is a compiler refusing. So this builds the brain's
  # units of the loop graph (`nix/rust/units.nix`) over planted sources —
  # each file edited the way an edit reaches a unit (`crateSourceOverrides`),
  # the person's folder handed in exactly as a machine hands it (the daemon's
  # registry and the folder beside it):
  #
  #   1. the library with `compile_error!` appended to EVERY distro module's
  #      `module.rs`, and to a person's module — its `module.rs` and the
  #      `shared.rs` beside it, a file like any other in a person's folder,
  #      whatever its name: it builds (it is the very derivation of the
  #      unpoisoned build: no unit of the library is built from those files);
  #   2. the daemon over the same poison: it does not, and its log carries all
  #      three poisons — the sabotage that proves 1 asked a real question;
  #   3. the library with every shared half (`shared.rs`) poisoned: it does
  #      not — a shared half IS the library's, and 1 did not pass because the
  #      library skips the modules' folders altogether.
  #
  # A unit that must FAIL is the unit itself under a build that inverts its
  # verdict (`refused`): the driver runs as it always does, and the
  # derivation succeeds only when it stopped on the planted sentence.
  #
  # The footer depends on the library alone (`brain-daemon-is-a-leaf`), so 1
  # is the footer's answer too.
  brain-library-links-no-module =
    let
      modulesDir = ../crates/ronoctua-brain/modules;
      folders = lib.attrNames (lib.filterAttrs (_: t: t == "directory") (builtins.readDir modulesDir));
      holding = file: lib.filter (id: builtins.pathExists (modulesDir + "/${id}/${file}")) folders;
      codeNeedle = "brain-split: a module's code reached this crate";
      personNeedle = "brain-split: a person's module reached this crate";
      personHalfNeedle = "brain-split: a person's shared.rs reached this crate";
      halfNeedle = "brain-split: the shared half is compiled here";
      # A POISON APPENDED, so a file's inner docs stay first and the only
      # error a compiler can find is the one planted.
      poisoned =
        needle: file:
        builtins.toFile (baseNameOf file) (builtins.readFile file + "\ncompile_error!(\"${needle}\");\n");
      editsOf =
        file: needle:
        lib.listToAttrs (
          map (
            id: lib.nameValuePair "modules/${id}/${file}" (poisoned needle (modulesDir + "/${id}/${file}"))
          ) (holding file)
        );
      codes = holding "module.rs";
      halves = holding "shared.rs";
      # THE PERSON'S FOLDER, poisoned, where the daemon reads it from: the
      # registry's own copy (`personModulesDir`, which `nix/rust.nix` hands
      # the daemon as `RONOCTUA_BRAIN_PERSON_MODULES_DIR`).
      personPoisoned = pkgs.runCommand "poisoned-person-brain-modules" { } ''
        cp -r ${registry.personModulesDir} $out
        chmod -R u+w $out
        for f in module.rs shared.rs; do
          if [ ! -f "$out/my-sample/$f" ]; then
            echo "ERROR: the person's fixture has no $f to poison" >&2
            exit 1
          fi
        done
        printf '\ncompile_error!("%s");\n' ${lib.escapeShellArg personNeedle} >> $out/my-sample/module.rs
        printf '\ncompile_error!("%s");\n' ${lib.escapeShellArg personHalfNeedle} >> $out/my-sample/shared.rs
      '';
      registryPoisoned = registry.overrideAttrs (o: {
        passthru = (o.passthru or { }) // {
          personModulesDir = personPoisoned;
        };
      });
      overCode = person.rust.override {
        brainModulesRs = registryPoisoned;
        crateSourceOverrides.ronoctua-brain = editsOf "module.rs" codeNeedle;
      };
      overHalves = rust.override {
        crateSourceOverrides.ronoctua-brain = editsOf "shared.rs" halfNeedle;
      };
      # A UNIT THAT MUST FAIL, by the sentences planted: its own build — the
      # driver's one command, out of the hooks around it — with the verdict
      # turned round. One command, so its status is the driver's: a list of
      # them in a condition runs on past a failure (`errexit` does not hold
      # there).
      refused =
        what: needles: unit:
        unit.overrideAttrs (
          o:
          let
            # `trim` matches a regular expression, which drops a string's
            # context: the driver's path is put back as the input it is.
            command = lib.addContextFrom o.buildPhase (
              lib.trim (lib.replaceStrings [ "runHook preBuild" "runHook postBuild" ] [ "" "" ] o.buildPhase)
            );
          in
          assert lib.assertMsg (
            !(lib.hasInfix "\n" command) && lib.hasInfix "python3 " command
          ) "brain-library-links-no-module: a unit's build is no longer the driver's one command: ${command}";
          {
            name = "${o.name}-refused";
            buildPhase = ''
              runHook preBuild
              if ${command} > "$TMPDIR/unit.log" 2>&1; then
                echo "ERROR: ${what} built" >&2
                exit 1
              fi
              for needle in ${lib.escapeShellArgs needles}; do
                if ! grep -qF -- "$needle" "$TMPDIR/unit.log"; then
                  echo "ERROR: ${what} failed, and not on \`$needle\`:" >&2
                  tail -40 "$TMPDIR/unit.log" >&2
                  exit 1
                fi
              done
              echo "refused, as it must be: ${what}"
              mkdir -p "$out" "$metrics"
              echo ${lib.escapeShellArg what} > "$out/verdict"
              runHook postBuild
            '';
          }
        );
      library = overCode.loop.libs.ronoctua-brain;
      daemonRefused = refused "the daemon over poisoned module code" [
        codeNeedle
        personNeedle
        personHalfNeedle
      ] overCode.loop.libs.ronoctua-brain-daemon;
      halvesRefused = refused "the library over a poisoned shared half" [
        halfNeedle
      ] overHalves.loop.libs.ronoctua-brain;
      # The library over the poison is the library of the unpoisoned build:
      # said, and not only built.
      same =
        builtins.unsafeDiscardStringContext library.drvPath
        == builtins.unsafeDiscardStringContext rust.loop.libs.ronoctua-brain.drvPath;
    in
    mkCheck "brain-library-links-no-module" [ ] ''
      if [ ${toString (lib.length codes)} -lt 10 ]; then
        echo "ERROR: only ${toString (lib.length codes)} modules' code poisoned — what does the glob reach?" >&2
        exit 1
      fi
      if [ ${toString (lib.length halves)} -lt 1 ]; then
        echo "ERROR: no module carries a shared half — what does the glob reach?" >&2
        exit 1
      fi
      if [ ! -d ${library}/lib ]; then
        echo "ERROR: the library over the poisoned modules has no lib/" >&2
        exit 1
      fi
      if [ "${lib.boolToString same}" != true ]; then
        echo "ERROR: the library's unit moved with a module's code: it is built from a file it never compiles" >&2
        exit 1
      fi
      cat ${daemonRefused}/verdict ${halvesRefused}/verdict > /dev/null
      echo "OK: the library built over ${toString (lib.length codes)} modules' code and a person's module" \
        "and shared.rs poisoned (the unpoisoned build's own unit), the daemon did not, and" \
        "${toString (lib.length halves)} shared halves are the library's"
    '';

  # NOTHING BUT THE PANEL'S DRIVER DEPENDS ON THE DAEMON.
  #
  # The library is what the footer links; if the footer — or anything it
  # links — came to depend on the daemon again, every module's code would be
  # back in its build, and nothing would fail: it would only be slow again. So
  # this reads every manifest of the workspace (`crates/*/Cargo.toml`, path
  # dependencies of every table, build and dev included) and refuses:
  #
  #   - a crate other than `ronoctua-test-brain` depending on the daemon;
  #   - the footer (`ronoctua-shell`) or the runtime reaching it, however far;
  #   - the library reaching it, or the footer's crates;
  #   - the daemon without its binary `ronoctua-brain` — the name the unit,
  #     `rust.brain` and every module's `ExecStart` say — or a second binary
  #     of that name elsewhere.
  #
  # And it proves its own answers on the same graph with one edge planted at a
  # time, each refused by its sentence.
  brain-daemon-is-a-leaf = mkCheck "brain-daemon-is-a-leaf" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, sys, tomllib
    exec(open("${pyPrelude}").read())

    # PARSED: every manifest is read with `tomllib`; TOML's comments never
    # reach the tables, so there is nothing to strip.
    # FILE-WIDE: a manifest's dependencies are tables of the whole file.
    root = pathlib.Path("${self}")
    LIBRARY, DAEMON, DRIVER = "ronoctua-brain", "ronoctua-brain-daemon", "ronoctua-test-brain"
    UNIT = "ronoctua-brain"

    def tables(manifest):
        yield from (manifest.get(k, {}) for k in ("dependencies", "build-dependencies", "dev-dependencies"))
        for target in manifest.get("target", {}).values():
            yield from (target.get(k, {}) for k in ("dependencies", "build-dependencies", "dev-dependencies"))

    graph, bins = {}, {}
    for path in sorted((root / "crates").glob("*/Cargo.toml")):
        manifest = tomllib.loads(path.read_text())
        name = manifest["package"]["name"]
        edges = set()
        for table in tables(manifest):
            for dep, spec in table.items():
                if isinstance(spec, dict) and "path" in spec:
                    edges.add(spec.get("package", dep))
        graph[name] = edges
        bins[name] = [b["name"] for b in manifest.get("bin", [])]
    floor(len(graph), 80, "crates of the workspace read")

    def reach(graph, start):
        seen, todo = set(), [start]
        while todo:
            for dep in graph.get(todo.pop(), ()):
                if dep not in seen:
                    seen.add(dep)
                    todo.append(dep)
        return seen

    def refusals(graph, bins):
        said = []
        for needed in (LIBRARY, DAEMON, DRIVER, "ronoctua-shell", "ronoctua-runtime"):
            if needed not in graph:
                said.append(f"there is no crate `{needed}`")
        if said:
            return said
        dependents = sorted(n for n, edges in graph.items() if DAEMON in edges)
        if dependents != [DRIVER]:
            said.append(f"the daemon is depended on by {dependents}, and only {DRIVER} may")
        for reader in ("ronoctua-shell", "ronoctua-runtime", LIBRARY):
            if DAEMON in reach(graph, reader):
                said.append(f"{reader} reaches the daemon: every module's code is in its build")
        shells = sorted(n for n in reach(graph, LIBRARY) if n.startswith("ronoctua-shell"))
        if shells:
            said.append(f"the library reaches the footer's crates {shells}")
        if LIBRARY not in graph[DAEMON]:
            said.append("the daemon does not depend on the library it re-exports")
        if UNIT not in bins.get(DAEMON, []):
            said.append(f"the daemon has no binary `{UNIT}` — the name its unit says")
        twins = sorted(n for n, b in bins.items() if UNIT in b and n != DAEMON)
        if twins:
            said.append(f"{twins} also build a binary `{UNIT}`")
        return said

    wrong = refusals(graph, bins)

    # EACH ANSWER, PROVED on the real graph with one thing planted.
    def planted(edit):
        g = {n: set(e) for n, e in graph.items()}
        b = {n: list(x) for n, x in bins.items()}
        edit(g, b)
        return refusals(g, b)
    plants = [
        ("the footer depending on the daemon", lambda g, b: g["ronoctua-shell"].add(DAEMON),
         "ronoctua-shell reaches the daemon"),
        ("the runtime reaching it through a crate between",
         lambda g, b: (g.setdefault("made-up", set()).add(DAEMON), g["ronoctua-runtime"].add("made-up")),
         "ronoctua-runtime reaches the daemon"),
        ("the library depending on the footer", lambda g, b: g[LIBRARY].add("ronoctua-shell"),
         "the library reaches the footer's crates"),
        ("the daemon's binary renamed", lambda g, b: b.__setitem__(DAEMON, ["ronoctua-brain-daemon"]),
         f"the daemon has no binary `{UNIT}`"),
        ("a second binary of the unit's name", lambda g, b: b[LIBRARY].append(UNIT),
         f"also build a binary `{UNIT}`"),
    ]
    for what, edit, needle in plants:
        said = planted(edit)
        if not any(needle in s for s in said):
            wrong.append(f"{what} was not refused with `{needle}` (it said {said})")

    if wrong:
        print("ERROR: the brain's daemon is not a leaf of the workspace.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(graph)} crates read; only {DRIVER} depends on the daemon, the footer and the"
          f" runtime never reach it, its binary is `{UNIT}`; {len(plants)} plants refused")
    END
  '';

  # A SHARED HALF IS THE DISTRO'S, AND SOMEBODY OUTSIDE READS IT.
  #
  # A distro module's `shared.rs` is compiled into the library the footer
  # links, beside its manifest. Two things would undo the split through it: a
  # person's module taken for one (its code in the footer's crate, on their
  # machines), and a distro module moving code there that nobody outside the
  # brain reads (a module's code in the footer's crate for nothing). A
  # person's folder may still hold a file of that name — a `mod shared;` of
  # its `module.rs`, as it could before the split — which the daemon compiles
  # with the module. So:
  #
  #   - the loader accepts a person's folder with a `shared.rs` and without,
  #     and a distro's with; the person's fixture holds one, and its record
  #     says it has no shared half;
  #   - the generator refuses a person's record handed to the library's part,
  #     and a person's record that says it has a shared half; on the library's
  #     part it lists a distro module's shared half (`shared.txt`), and on the
  #     daemon's writes no distro manifest at all;
  #   - on the tree, the library's `shared.txt` is exactly the folders with a
  #     `shared.rs`, and each of those modules' `namedBy` names a Rust file
  #     outside the brain.
  brain-shared-half-is-the-distros =
    let
      folder = {
        id = "my-made-up";
        origin = "person";
        files = [
          "manifest.nix"
          "module.rs"
        ];
        i18nFiles = map (l: "${l}.ftl") languages;
        manifest = {
          what = "A made-up module.";
          needs = {
            root = false;
            network = false;
            boundaries = [ ];
          };
          schedule = null;
          uses = [ ];
          namedBy = { };
        };
      };
      distro = folder // {
        id = "made-up";
        origin = "distro";
        files = [
          "manifest.nix"
          "module.rs"
          "module.nix"
          "checks.nix"
          "why.md"
          "shared.rs"
        ];
      };
      facts = pkgs.writeText "brain-shared-facts.json" (
        builtins.toJSON {
          personWithout = brainModules.problemsOf folder;
          personWith = brainModules.problemsOf (folder // { files = folder.files ++ [ "shared.rs" ]; });
          distroWith = brainModules.problemsOf distro;
          namedBy = lib.mapAttrs (_: m: m.namedBy) brainModules.modules;
          records = person.brainModules.forGenerator (_: { });
        }
      );
    in
    mkCheck "brain-shared-half-is-the-distros" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, pathlib, subprocess, sys, tempfile
      exec(open("${pyPrelude}").read())

      # PARSED: the loader's answers and records arrive as JSON written by Nix;
      # `shared.txt` and `ids.txt` are the generator's lists, a word per line.
      # FILE-WIDE: which folders hold a `shared.rs` is a listing, not a text.
      wrong = []
      f = json.load(open("${facts}"))
      SENTENCE = "a person's module has no shared half"
      if f["personWithout"]:
          wrong.append(f"a person's smallest good folder was refused: {f['personWithout']}")
      if f["personWith"]:
          wrong.append(f"a person's folder with a shared.rs beside its module.rs was refused: {f['personWith']}")
      if f["distroWith"]:
          wrong.append(f"a distro folder with a shared.rs was refused: {f['distroWith']}")

      # THE GENERATOR, on the person's fixture and a distro record made of it.
      records = f["records"]
      mine = [r for r in records if r["id"] == "my-sample"]
      floor(len(mine), 1, "the person's fixture among the records")
      mine = mine[0]
      # THE FIXTURE HOLDS A `shared.rs`, so its record answers the question.
      if not (pathlib.Path("${../crates/ronoctua-brain/tests/person}") / "my-sample" / "shared.rs").is_file():
          wrong.append("the person's fixture has no shared.rs; its record proves nothing about one")
      if mine.get("shared") is not False:
          wrong.append(f"the person's record says shared = {mine.get('shared')!r}; a person's shared.rs is no shared half")
      work = pathlib.Path(tempfile.mkdtemp())
      def run(part, recs, out):
          (work / "records.json").write_text(json.dumps(recs))
          return subprocess.run(
              [sys.executable, "${../nix/scripts/gen-brain-modules.py}", "--part", part,
               "--modules", str(work / "records.json"),
               "--language", "${builtins.head languages}",
               "--out", str(work / out)], capture_output=True, text=True)
      r = run("library", [mine], "person-library")
      if r.returncode == 0 or "the library's part holds the distro's modules alone" not in r.stderr:
          wrong.append(f"the library's part took a person's module: {r.stderr!r}")
      if (work / "person-library").exists():
          wrong.append("the library's part wrote before it refused a person's module")
      r = run("daemon", [dict(mine, shared=True)], "person-shared")
      if r.returncode == 0 or SENTENCE not in r.stderr:
          wrong.append(f"the daemon's part took a person's record with a shared half: {r.stderr!r}")
      # THE SAME RECORD AS A DISTRO MODULE'S WITH A SHARED HALF (its texts are
      # its id's, so the generator writes it whole).
      twin = dict(mine, origin="distro", shared=True)
      r = run("library", [twin], "distro-library")
      if r.returncode != 0:
          wrong.append("the library's part refused a distro record with a shared half: " + r.stderr)
      else:
          if (work / "distro-library" / "shared.txt").read_text() != "my-sample\n":
              wrong.append("the library's part did not list the distro module's shared half")
          if not (work / "distro-library" / "my-sample" / "manifest.rs").is_file():
              wrong.append("the library's part did not write the distro module's manifest")
      r = run("daemon", [twin], "daemon-distro")
      if r.returncode != 0:
          wrong.append("the daemon's part refused a distro record: " + r.stderr)
      else:
          if (work / "daemon-distro" / "my-sample").exists():
              wrong.append("the daemon's part wrote a distro module's manifest; it is the library's")
          if "my_sample::ID" not in uncommented((work / "daemon-distro" / "all.rs").read_text()):
              wrong.append("the daemon's registry does not list the distro module")
      r = run("daemon", [mine], "daemon-person")
      if r.returncode != 0:
          wrong.append("the daemon's part refused a person's record: " + r.stderr)
      elif not (work / "daemon-person" / "my-sample" / "manifest.rs").is_file():
          wrong.append("the daemon's part did not write the person's manifest")

      # THE TREE: what the library compiles is what the folders hold, and
      # each half has a reader outside the brain.
      listed = sorted(l for l in pathlib.Path("${brainManifestsRs}/shared.txt").read_text().split() if l)
      folders = pathlib.Path("${self}/crates/ronoctua-brain/modules")
      on_disk = sorted(p.parent.name for p in folders.glob("*/shared.rs"))
      floor(len(on_disk), 1, "shared halves in the tree")
      if listed != on_disk:
          wrong.append(f"the library lists shared halves {listed}, and the folders hold {on_disk}")
      for i in on_disk:
          readers = [p for p in f["namedBy"].get(i, {})
                     if p.endswith(".rs") and not p.startswith("crates/ronoctua-brain")]
          if not readers:
              wrong.append(f"{i} has a shared half and its namedBy names no Rust file outside the brain:"
                           " its code would be in the footer's crate for nobody")
      ids = pathlib.Path("${brainManifestsRs}/ids.txt").read_text().splitlines()
      floor(len(ids), 10, "modules in the library's table")
      if any(not l.endswith(" distro") for l in ids):
          wrong.append(f"the library's table lists a module that is not the distro's: {ids}")

      if wrong:
          print("ERROR: a shared half reaches the footer's crate where it must not.", file=sys.stderr)
          for w in wrong:
              print("       " + w, file=sys.stderr)
          raise SystemExit(1)
      print(f"OK: {len(on_disk)} shared halves, each the distro's and each read outside the brain;"
            " a person's shared.rs loads as a file of its module, and a record saying otherwise is refused")
      END
    '';
}
