# THE TEXTS IN FRAGMENTS — the 2026-10-12 wave, lane T1.
#
# Every crate that speaks owns its sentences (`crates/<crate>/i18n/<lang>.ftl`)
# and is compiled against the names of its own fragment: its `key` module, one
# file per crate written at evaluation (`nix/text-keys.nix`), handed to each
# unit alone. A checkout's `cargo` gets the same files from
# `nix/scripts/gen-keys.py` (the devshell, `.#dev`). These checks hold that:
#
# - `keys-agree`: the evaluation and the script write the same bytes for every
#   crate, the script rewrites only what changed (and says so), and refuses a
#   fragment no crate compiles;
# - `a-new-key-rebuilds-its-crate`: a key added to one crate's fragment
#   rebuilds exactly what an edit of that crate's `key` module would — the
#   crate and what is built from it — and formats nothing;
# - `texts-tool-says-what-is-missing`: `nix run .#texts -- missing <lang>`
#   lists, fragment by fragment, what a language lacks and what it has that
#   the base does not, and `-- where <key>` finds a key's fragment.
{
  pkgs,
  lib,
  self,
  system,
  mkCheck,
  pyPrelude,
  rust,
  texts,
  languages,
  # `tests/build-granularity.nix`'s model and the build as data.
  granularityModel,
}:

let
  base = builtins.head languages;
  toFile = name: value: pkgs.writeText name (builtins.toJSON value);

  # WHAT THE SCRIPT READS OF A CHECKOUT, and nothing else (never `${../.}`,
  # which would copy the whole tree): the manifests and the fragments.
  keysTree = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      (lib.fileset.fileFilter (f: f.name == "Cargo.toml") ../crates)
      (lib.fileset.fileFilter (f: f.hasExt "ftl") ../crates)
    ];
  };

  # A KEY ADDED to a crate's fragment, as the evaluation reads it.
  keyEditOf =
    crate: rust.override { texts = texts.override { keyEdits.${crate} = [ "planted-key" ]; }; };
in
{
  # THE EVALUATION AND THE SCRIPT WRITE THE SAME KEYS.
  #
  # Two writers of one set of files — `nix/text-keys.nix` for the build and
  # `nix/scripts/gen-keys.py` for a checkout's `cargo` — and a crate compiled
  # by one against the other's file would not be the crate the other builds.
  # The script is run over the tree's manifests and fragments and every file
  # it writes is compared, byte for byte, with the evaluation's; and it is
  # asked what it is for: run again over its own directory it rewrites
  # nothing (status 0, every file's time kept), a key planted in one crate's
  # fragment rewrites that crate's file alone (status `--written-exit`), and
  # a fragment in a crate that does not say it reads `RONOCTUA_KEYS_DIR` is
  # refused by the crate's name, before anything is written.
  keys-agree =
    mkCheck "keys-agree"
      [
        pkgs.python3
        pkgs.coreutils
      ]
      ''
        python3 - <<'END'
        import os, pathlib, shutil, subprocess, sys, tempfile
        exec(open("${pyPrelude}").read())

        # PARSED: the key files are compared as bytes, the manifests are TOML
        # read by the script itself; nothing here searches a source.
        # FILE-WIDE: a key file is compared whole, byte for byte — the question
        # is the file, not a function in it.
        evaluated = pathlib.Path("${texts.keys}")
        tree = pathlib.Path("${keysTree}")
        script = "${../nix/scripts/gen-keys.py}"
        work = pathlib.Path(tempfile.mkdtemp())

        def run(t, into, *more):
            return subprocess.run([sys.executable, script, "--tree", str(t), "--base", "${base}",
                                   "--into", str(into), *more], capture_output=True, text=True)

        wrong = []
        out = work / "keys"
        r = run(tree, out)
        if r.returncode != 0:
            sys.exit(f"ERROR: gen-keys.py refused the tree: {r.stderr}")
        mine = {p.name: p.read_bytes() for p in out.glob("*.rs")}
        theirs = {p.name: p.read_bytes() for p in evaluated.glob("*.rs")}
        floor(len(theirs), 30, "crates' key files the evaluation wrote")
        for name in sorted(set(mine) | set(theirs)):
            if name not in mine:
                wrong.append(f"{name}: the evaluation writes it and the script does not")
            elif name not in theirs:
                wrong.append(f"{name}: the script writes it and the evaluation does not")
            elif mine[name] != theirs[name]:
                wrong.append(f"{name}: the script and the evaluation write different bytes")

        # AGAIN, OVER ITS OWN DIRECTORY: nothing rewritten.
        times = {p.name: p.stat().st_mtime_ns for p in out.glob("*.rs")}
        r = run(tree, out, "--written-exit", "10")
        if r.returncode != 0 or " 0 written" not in r.stdout:
            wrong.append(f"run again over its own files, the script did not keep them (status {r.returncode}): {r.stdout}")
        if {p.name: p.stat().st_mtime_ns for p in out.glob("*.rs")} != times:
            wrong.append("run again over its own files, the script touched them: cargo would recompile every crate")

        # A KEY PLANTED IN ONE CRATE: that crate's file alone, and said.
        planted = work / "tree"
        shutil.copytree(tree, planted)
        for p in (planted, *planted.rglob("*")):
            p.chmod(0o755 if p.is_dir() else 0o644)
        victim = "ronoctua-shell-stick"
        fragment = planted / "crates" / victim / "i18n" / "${base}.ftl"
        fragment.write_text(fragment.read_text() + "planted-key = Planted\n")
        before = {p.name: p.read_bytes() for p in out.glob("*.rs")}
        r = run(planted, out, "--written-exit", "10")
        after = {p.name: p.read_bytes() for p in out.glob("*.rs")}
        moved = sorted(n for n in after if after[n] != before.get(n))
        if r.returncode != 10:
            wrong.append(f"a key planted did not make the script exit with --written-exit's 10 (status {r.returncode})")
        if moved != [victim + ".rs"]:
            wrong.append(f"a key planted in {victim} rewrote {moved}, not that crate's file alone")
        elif b'pub const PLANTED_KEY: &str = "planted-key";' not in after[victim + ".rs"]:
            wrong.append("the planted key is not a constant of its crate's file")

        # A FRAGMENT NO CRATE COMPILES: refused, by name, before a byte.
        silent = work / "silent"
        shutil.copytree(planted, silent)
        manifest = silent / "crates" / victim / "Cargo.toml"
        text = manifest.read_text()
        cut = text.replace('"RONOCTUA_KEYS_DIR", ', "").replace(', "RONOCTUA_KEYS_DIR"', "").replace('"RONOCTUA_KEYS_DIR"', "")
        if cut == text:
            wrong.append(f"{victim}'s manifest does not declare RONOCTUA_KEYS_DIR: the refusal below asks nothing")
        manifest.write_text(cut)
        r = run(silent, work / "never")
        if r.returncode == 0 or victim not in r.stderr or (work / "never").exists():
            wrong.append(f"a fragment in a crate that does not speak was not refused by name (status {r.returncode}): {r.stderr}")

        # AND NEVER INSIDE THE TREE.
        r = run(planted, planted / "keys")
        if r.returncode == 0 or (planted / "keys").exists():
            wrong.append("the script wrote the keys inside the tree")

        if wrong:
            print("ERROR: the text keys of a checkout are not the build's:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {len(theirs)} crates' key files, the same bytes from the evaluation and the script; "
              "rewritten only when they change, and a fragment no crate compiles refused")
        END
      '';

  # A KEY ADDED REBUILDS THE CRATE THAT SAYS IT, AND WHAT IS BUILT FROM IT.
  #
  # A crate's keys are its own file, handed to its units alone, so a name added
  # to a fragment is, to the build, an edit of that crate's `key` module and
  # nothing else: every entry the plan says an edit of `src/key.rs` reaches,
  # and no other — and no formatting, since the source of `key.rs` does not
  # change. Asked of a view (the stick: its library and the binary that links
  # it) and of the foundation (the kit: every shell crate above it). Before the
  # wave, a key added anywhere rebuilt every crate that speaks.
  a-new-key-rebuilds-its-crate =
    let
      samples = [
        "ronoctua-shell-stick"
        "ronoctua-shell-kit"
      ];
      facts = toFile "a-new-key.json" {
        samples = map (crate: {
          inherit crate;
          file = "${texts.textKeys.crates.${crate}.dir}/src/key.rs";
          after = (keyEditOf crate).unitIndex;
        }) samples;
      };
    in
    mkCheck "a-new-key-rebuilds-its-crate" [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import json, sys
      exec(open("${pyPrelude}").read())
      exec(open("${granularityModel.granularityModel}").read())

      # PARSED: cargo's plan, and the build's index before and after each key
      # added, as Nix evaluated it — the derivations' paths, nothing built.
      model_load("${granularityModel.planFile}", "${granularityModel.dataFile}")
      facts = json.load(open(sys.argv[1]))
      before = json.load(open("${granularityModel.baseFile}"))
      floor(len(before), 600, "entries of the build's index")
      floor(len(facts["samples"]), 2, "crates a key is added to")
      wrong = []
      for s in facts["samples"]:
          f, after = s["file"], s["after"]
          if set(after) != set(before):
              wrong.append(f"{s['crate']}: a key added changed which entries the build has")
          want = {e for e in expected(f) if not e.startswith("fmt/") and e != "aggregate/rust-fmt"}
          got = changed_entries(before, after)
          floor(len(want), 4, f"entries a key of {s['crate']} has to rebuild")
          for e in sorted(want - got):
              wrong.append(f"{s['crate']}: {e} is built from its keys and was NOT rebuilt")
          for e in sorted(got - want):
              wrong.append(f"{s['crate']}: {e} was rebuilt for a key it is not built from")
          print(f"{s['crate']}: a key added rebuilds {len(got)} of {len(before)} entries")
      if wrong:
          print("ERROR: a key added does not rebuild exactly the crate that says it and what is built from it:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: a key added to each of {len(facts['samples'])} crates rebuilds what an edit of its key module reaches, and only that")
      END
    '';

  # THE TRANSLATOR'S HELP SAYS WHAT A LANGUAGE LACKS.
  #
  # `nix run .#texts -- missing <language>`, run inside a checkout made here
  # from two of the tree's fragments: complete, it says so and exits 0; with a
  # key taken out of one language of one fragment and an orphan added to
  # another, it names both — the fragment, the key, the base's sentence — and
  # exits 1; `-- where <key>` names the fragment holding a key, and a key no
  # fragment holds is a refusal.
  texts-tool-says-what-is-missing =
    mkCheck "texts-tool-says-what-is-missing"
      [
        pkgs.python3
        pkgs.coreutils
      ]
      ''
        python3 - <<'END'
        import pathlib, shutil, subprocess, sys, tempfile
        exec(open("${pyPrelude}").read())

        # PARSED: what the tool prints, line by line, over fragments copied here.
        # FILE-WIDE: a fragment is copied and cut whole; the tool's answer is
        # read whole, each needle a line it must print.
        tool = "${self.apps.${system}.texts.program}"
        other = "${lib.last languages}"
        root = pathlib.Path(tempfile.mkdtemp())
        (root / "assets/i18n").mkdir(parents=True)
        shutil.copy("${../assets/i18n/languages.nix}", root / "assets/i18n/languages.nix")
        (root / "Cargo.toml").write_text("[workspace]\n")
        for name, d in (("ronoctua-shell-stick", "${texts.fragments.ronoctua-shell-stick}"),
                        ("ronoctua-shell-kit", "${texts.fragments.ronoctua-shell-kit}")):
            shutil.copytree(d, root / "crates" / name / "i18n")
        for p in (root, *root.rglob("*")):
            p.chmod(0o755 if p.is_dir() else 0o644)

        def ask(*words, at=root):
            return subprocess.run([tool, *words], cwd=at, capture_output=True, text=True)

        wrong = []
        r = ask("missing", other)
        if r.returncode != 0 or "complete" not in r.stdout:
            wrong.append(f"a complete language was not said complete (status {r.returncode}): {r.stdout}{r.stderr}")

        stick = root / "crates/ronoctua-shell-stick/i18n"
        lines = (stick / f"{other}.ftl").read_text().splitlines()
        at = next(i for i, l in enumerate(lines) if l.startswith("stick-down ="))
        (stick / f"{other}.ftl").write_text("\n".join(lines[:at] + lines[at + 1:]) + "\n")
        kit = root / "crates/ronoctua-shell-kit/i18n"
        (kit / f"{other}.ftl").write_text((kit / f"{other}.ftl").read_text() + "only-here = órfã\n")
        r = ask("missing", other, at=root / "crates")
        said = r.stdout
        floor(len(said.splitlines()), 3, "lines the tool said of a language lacking a key")
        if r.returncode != 1:
            wrong.append(f"a language lacking a key was not exit 1 (status {r.returncode})")
        base_down = next(l for l in (stick / "${base}.ftl").read_text().splitlines() if l.startswith("stick-down ="))
        if f"crates/ronoctua-shell-stick/i18n/{other}.ftl lacks 1" not in said or f"  {base_down}" not in said:
            wrong.append(f"the key taken out was not named with its fragment and the base's sentence: {said}")
        if "crates/ronoctua-shell-kit/i18n/" not in said or "only-here" not in said:
            wrong.append(f"the orphan was not named with its fragment: {said}")

        r = ask("where", "stick-down")
        if r.returncode != 0 or "crates/ronoctua-shell-stick/i18n" not in r.stdout:
            wrong.append(f"`where` did not find a key's fragment: {r.stdout}{r.stderr}")
        r = ask("where", "no-such-key")
        if r.returncode == 0:
            wrong.append("`where` found a key no fragment holds")
        r = ask("missing", "${base}")
        if r.returncode != 2:
            wrong.append("the base language was asked what it lacks, and answered")

        if wrong:
            print("ERROR: `nix run .#texts` does not say what a language lacks:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print("OK: `.#texts -- missing` names a key a language lacks and an orphan, by fragment; `where` finds a key")
        END
      '';
}
