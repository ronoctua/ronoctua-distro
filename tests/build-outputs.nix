# THE PROGRAMS IN THEIR PLACES — nix-design §2 (`nix/rust/outputs.nix`, lane
# N2): what each program of the plan is for, what the shell's bundle holds, and
# that a program's closure holds what it cites.
#
# Joined into `tests/checks.nix` with one line, beside `tests/rust-units.nix`.
{
  pkgs,
  lib,
  self,
  system,
  rust,
  mkCheck,
  pyPrelude,
}:

let
  # A DERIVATION'S BUILD GRAPH WITHOUT BUILDING IT: interpolated this way, its
  # `.drv` and everything that `.drv` was made from are inputs of the check,
  # and none of its outputs is.
  graphOf = drv: builtins.unsafeDiscardOutputDependency drv.drvPath;
  # A path asked about, and NOT handed in: with no string context, naming it
  # brings nothing into the sandbox — whether a root's graph reaches it is the
  # walk's answer.
  probe = path: builtins.unsafeDiscardStringContext path;

  # WHAT A MACHINE IS BUILT FROM: the two profiles' systems and the ISO, which
  # carries the seed (`rust.shipped`) besides its own system.
  roots = {
    noctua = self.nixosConfigurations.noctua.config.system.build.toplevel;
    minimal = self.nixosConfigurations.minimal.config.system.build.toplevel;
    iso = self.packages.${system}.iso;
  };

  # THE WALK, once for both checks: every `.drv` the roots were made from, and
  # every `<bundle>/bin/<program>` their strings name — in a derivation's
  # builder, arguments and environment (structured attributes included) and in
  # a file handed to one as a source (a unit file written at evaluation). The
  # walk follows input derivations from each root, and each `.drv` it reaches
  # is in this sandbox, because the store's references of a `.drv` are exactly
  # what it was made from.
  walk =
    pkgs.runCommand "ronoctua-machines-build-graph"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        roots = lib.mapAttrsToList (_: graphOf) roots;
        bundle = probe rust.shell.outPath;
      }
      ''
        python3 - $out $bundle $roots <<'END'
        import json, os, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: a `.drv` is a derivation written down in ATerm —
        # `Derive([outputs],[input derivations],[input sources],system,
        # builder,[args],[env])` — read by that grammar: strings with their
        # escapes undone, lists and tuples.
        out, bundle, roots = sys.argv[1], sys.argv[2], sys.argv[3:]
        TOKEN = re.compile(r'"((?:[^"\\]+|\\.)*)"|([\[\]()])', re.S)
        ESCAPE = re.compile(r"\\(.)", re.S)
        UNESCAPED = {"n": "\n", "t": "\t", "r": "\r"}

        def aterm(text):
            stack, top = [], []
            for m in TOKEN.finditer(text):
                string, bracket = m.groups()
                if string is not None:
                    top.append(ESCAPE.sub(lambda e: UNESCAPED.get(e.group(1), e.group(1)), string))
                elif bracket in "[(":
                    stack.append(top)
                    top = []
                else:
                    done, top = top, stack.pop()
                    top.append(done)
            return top[0]

        NAMED = re.compile(re.escape(bundle) + r"/bin/([A-Za-z0-9._+-]+)")
        mentions, drvs, seen_sources = {}, set(), set()

        def mention(text, where):
            for name in NAMED.findall(text):
                mentions.setdefault(name, set()).add(where)

        todo = list(roots)
        while todo:
            path = todo.pop()
            if path in drvs:
                continue
            drvs.add(path)
            fields = aterm(open(path, errors="surrogateescape").read())
            if isinstance(fields[0], str):
                fields = fields[1:]  # `DrvWithVersion("…", …)`
            _outputs, inputs, sources, _system, builder, args, env = fields
            todo.extend(i[0] for i in inputs)
            for text in [builder, *args, *(value for _name, value in env)]:
                mention(text, path)
            for source in sources:
                if source in seen_sources:
                    continue
                seen_sources.add(source)
                if os.path.isfile(source) and os.path.getsize(source) < 4 << 20:
                    mention(open(source, errors="surrogateescape").read(), source)

        floor(len(drvs), 1000, "derivations the three roots were made from")
        json.dump(
            {
                "drvs": sorted(drvs),
                "mentions": {name: sorted(where) for name, where in sorted(mentions.items())},
            },
            open(out, "w"),
            indent=0,
        )
        END
      '';

  # Each program's crate, for the manifests the checks read.
  crateDirs = lib.mapAttrs (_: c: c.dir) rust.unitCrates;
in
{
  # EVERY PROGRAM HAS A ROLE, AND THE ROLE IS WHERE IT GOES.
  #
  # A `[[bin]]` is `ship` unless its crate says otherwise
  # (`[package.metadata.ronoctua] bins`): `driver` (a check or a VM test runs
  # it), `bench` (`.#bench`), `apart` (built elsewhere: `ronoctua-git-askpass`).
  # The role decides the program's place, and this proves the places from the
  # manifests and the plan, read here and not through the Nix that placed them:
  #
  # - every program of the plan's release graph has exactly one role, and the
  #   one the outputs used is the manifest's;
  # - what the ISO's seed carries (`rust.shipped`) is the `ship` programs, in
  #   the form a machine runs them, and the bundle the footer's; `.#bench`'s
  #   benchmarks are the `bench` ones; a check can run any program but the
  #   `apart` one (`rust.bins`);
  # - and no `driver`, `bench` or `apart` program is anything a system of
  #   either profile or the ISO is BUILT FROM — crane's workspace put all fifty
  #   programs, the test drivers and the benchmarks among them, into every
  #   closure that reached the shell.
  every-bin-has-a-role =
    let
      # By the role each unit carries, apart from `rust.roles` (which the
      # check holds against the manifests).
      notShipped = lib.filterAttrs (_: unit: unit.passthru.role != "ship") rust.release.bins;
      facts = pkgs.writeText "every-bin-has-a-role.json" (
        builtins.toJSON {
          roles = rust.roles;
          crates = crateDirs;
          bins = lib.attrNames rust.bins;
          benches = rust.benches.passthru.members;
          shipped = lib.concatMap (
            d: if d ? passthru.members then d.passthru.members else [ d.passthru.bin ]
          ) rust.shipped;
          # The release unit of every program that may not reach a machine,
          # by its `.drv` — asked about, not handed in.
          probes = lib.mapAttrs (_: unit: probe unit.drvPath) notShipped;
          # And two that must be reached, or the walk proves nothing.
          reached = {
            ronoctua-brain = probe rust.brain.drvPath;
            ronoctua-shell = probe rust.shell.drvPath;
          };
        }
      );
    in
    mkCheck "every-bin-has-a-role" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, pathlib, sys, tomllib
      exec(open("${pyPrelude}").read())

      # PARSED: the plan (JSON), the crates' manifests (TOML), what the
      # outputs placed (JSON, from evaluation) and the walk of what the
      # machines are built from (JSON).
      # FILE-WIDE: a manifest is read whole by its parser, and only its
      # `[package.metadata.ronoctua] bins` table is asked.
      facts = json.load(open("${facts}"))
      plan = json.load(open("${../nix/rust/plan.json}"))
      walked = set(json.load(open("${walk}"))["drvs"])
      root = pathlib.Path("${self}")
      wrong = []

      ROLES = {"ship", "driver", "bench", "apart"}
      units = plan["graphs"]["release"]["units"]
      programs = {
          u["target"]: u["pkg"]
          for u in units.values()
          if u["workspace"] and u["mode"] == "build" and "bin" in u["kind"]
      }
      floor(len(programs), 40, "programs in the plan's release graph")

      # THE MANIFESTS' WORD, read here.
      declared = {}
      for pkg in sorted(set(programs.values())):
          manifest = tomllib.loads((root / facts["crates"][pkg] / "Cargo.toml").read_text())
          roles = manifest.get("package", {}).get("metadata", {}).get("ronoctua", {}).get("bins", {})
          for program, role in roles.items():
              if role not in ROLES:
                  wrong.append(f"{pkg} gives {program} the role {role!r}, which is not one of {sorted(ROLES)}")
              declared[program] = role
      role = {program: declared.get(program, "ship") for program in programs}
      for program in sorted(set(declared) - set(programs)):
          wrong.append(f"{program} has a role and is no program of the plan")
      if facts["roles"] != role:
          for program in sorted(set(role) | set(facts["roles"])):
              if facts["roles"].get(program) != role.get(program):
                  wrong.append(f"{program}: the manifest says {role.get(program)!r}, the outputs used {facts['roles'].get(program)!r}")

      def of(r):
          return {p for p, x in role.items() if x == r}

      floor(len(of("ship")), 20, "ship programs")
      floor(len(of("driver")), 5, "driver programs")
      floor(len(of("bench")), 1, "bench programs")

      # THE PLACES.
      shipped = set(facts["shipped"])
      if shipped != of("ship"):
          if shipped - of("ship"):
              wrong.append(f"the seed carries {sorted(shipped - of('ship'))}, which are not ship programs")
          if of("ship") - shipped:
              wrong.append(f"the seed leaves out {sorted(of('ship') - shipped)}, which ship")
      if set(facts["benches"]) != of("bench"):
          wrong.append(f".#bench runs {sorted(facts['benches'])}, and the bench programs are {sorted(of('bench'))}")
      if set(facts["bins"]) != set(role) - of("apart"):
          wrong.append(f"rust.bins is {sorted(facts['bins'])}, not every program but {sorted(of('apart'))}")

      # NOTHING THAT IS NOT SHIPPED IS WHAT A MACHINE IS MADE FROM.
      for program, drv in facts["reached"].items():
          if drv not in walked:
              wrong.append(f"{program} is not in what the machines are built from: the walk proves nothing")
      floor(len(facts["probes"]), 15, "programs that may not reach a machine")
      for program, drv in sorted(facts["probes"].items()):
          if drv in walked:
              wrong.append(f"{program} ({role.get(program, '?')}) is something a machine or the ISO is built from")

      if wrong:
          print("ERROR: a program is not where its role puts it:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(role)} programs, {len(of('ship'))} shipped, {len(of('driver'))} drivers and "
            f"{len(of('bench'))} benchmarks reach no machine, {len(of('apart'))} built apart")
      END
    '';

  # THE SHELL'S BUNDLE IS THE SHELL, and nothing reaches it for another program.
  #
  # `rust.shell` is the footer and the programs its code finds beside
  # `current_exe()` — copied into one `bin/`, so each finds the others
  # (`nix/rust/outputs.nix`). Crane's bundle was the whole workspace, and three
  # root units ran their programs out of it (`${shell}/bin/ronoctua-rescue`,
  # `…-network-console`, `…-network-agent`), putting the desktop in their
  # closures. So:
  #
  # - the bundle holds exactly the shell and what `beside` names over every
  #   crate whose code is in the shell program — worked out here from the plan
  #   and the manifests — release and loop alike, the shell and the painters
  #   among them wrapped;
  # - and in what either profile's system or the ISO is built from, every
  #   `<bundle>/bin/<program>` names `ronoctua-shell`: a unit, a bind, a
  #   desktop entry that needs another program names that program's own.
  shell-bundle-is-only-the-shell =
    let
      facts = pkgs.writeText "shell-bundle-is-only-the-shell.json" (
        builtins.toJSON {
          crates = crateDirs;
          painters = rust.painters;
          bundles = {
            release = "${rust.shell}";
            loop = "${rust.loopShell}";
          };
        }
      );
    in
    mkCheck "shell-bundle-is-only-the-shell" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, os, pathlib, sys, tomllib
      exec(open("${pyPrelude}").read())

      # PARSED: the plan (JSON), the manifests (TOML), a directory listing and
      # the walk of what the machines are built from (JSON).
      # FILE-WIDE: a manifest is read whole by its parser, and only its
      # `[package.metadata.ronoctua] beside` list is asked.
      facts = json.load(open("${facts}"))
      plan = json.load(open("${../nix/rust/plan.json}"))
      mentions = json.load(open("${walk}"))["mentions"]
      root = pathlib.Path("${self}")
      wrong = []

      def beside(pkg):
          manifest = tomllib.loads((root / facts["crates"][pkg] / "Cargo.toml").read_text())
          return manifest.get("package", {}).get("metadata", {}).get("ronoctua", {}).get("beside", [])

      for flavour, bundle in sorted(facts["bundles"].items()):
          units = plan["graphs"]["release" if flavour == "release" else "loop"]["units"]
          shell = [k for k, u in units.items() if u["workspace"] and u["mode"] == "build"
                   and "bin" in u["kind"] and u["target"] == "ronoctua-shell"]
          floor(len(shell), 1, f"shell programs in the {flavour} graph")
          # Every workspace library inside the program: its dependencies,
          # transitively, but never a build script's or a proc-macro's.
          crates, todo, seen = {units[shell[0]]["pkg"]}, [shell[0]], set()
          while todo:
              key = todo.pop()
              if key in seen:
                  continue
              seen.add(key)
              for d in units[key]["deps"]:
                  u = units[d["key"]]
                  if u["mode"] == "run-custom-build" or "custom-build" in u["kind"] or "proc-macro" in u["kind"]:
                      continue
                  if u["workspace"]:
                      crates.add(u["pkg"])
                  todo.append(d["key"])
          floor(len(crates), 30, f"workspace crates inside the {flavour} shell")
          want = {"ronoctua-shell"} | {b for c in crates for b in beside(c)}
          floor(len(want), 3, f"programs the {flavour} shell looks for beside itself")
          listed = os.listdir(os.path.join(bundle, "bin"))
          held = {n for n in listed if not n.startswith(".")}
          if held != want:
              if held - want:
                  wrong.append(f"the {flavour} bundle holds {sorted(held - want)}, which the shell never looks for beside itself")
              if want - held:
                  wrong.append(f"the {flavour} bundle lacks {sorted(want - held)}, which the shell looks for beside itself")
          wrapped = {n[1:-len("-wrapped")] for n in listed if n.startswith(".") and n.endswith("-wrapped")}
          should = {"ronoctua-shell"} | (want & set(facts["painters"]))
          if wrapped != should:
              wrong.append(f"the {flavour} bundle wraps {sorted(wrapped)}, and should wrap {sorted(should)}")
          for n in held:
              if os.path.islink(os.path.join(bundle, "bin", n)):
                  wrong.append(f"{n} is a link in the {flavour} bundle: it would look for its neighbours where its target is")

      # WHAT THE SYSTEMS NAME OUT OF IT.
      floor(len(mentions.get("ronoctua-shell", [])), 3, "places a system names the footer from the bundle")
      for name, where in sorted(mentions.items()):
          if name != "ronoctua-shell":
              wrong.append(f"{name} is run out of the shell's bundle by {where[0]}" + (f" and {len(where) - 1} more" if len(where) > 1 else ""))

      if wrong:
          print("ERROR: the shell's bundle is not only the shell:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: the bundles hold the shell and what it finds beside itself; "
            f"{len(mentions['ronoctua-shell'])} places name the footer from it, and nothing else")
      END
    '';

  # NO LOOP OR TEST BUILD IS WHAT A MACHINE IS MADE FROM.
  #
  # `[profile.loop]` trades run time for build time, and a program built at it
  # runs, draws and passes every test: nothing on a screen says so. Since the
  # bundle COPIES its programs (`nix/rust/outputs.nix`), a copy cites nothing of
  # the unit it came from, and the RUN-TIME closure of a footer copied out of the
  # loop's units, or of a system handed `rust.loopShell` for `rust.shell`, has no
  # loop path to find: the loop's programs are `<bin>-loop-<version>` and its
  # bundle `ronoctua-shell-loop`, and the marks `the-loop-profile-is-never-installed`
  # read in run-time closures named none of them (review of 2026-10-12).
  #
  # So the question is asked of what the machines are BUILT FROM — the walk of
  # both profiles' systems and the ISO, through every `.drv` — and with the
  # loop's and the test graph's own derivations, by path, not by a mark a
  # name may or may not carry: every library, build script, program, test
  # harness and third-party unit (or, with the layers, layer) of those two
  # flavours — no third-party unit of theirs is one of the release graph's,
  # their profiles differ —, the loop's bundle and the aggregates of the test
  # runs and clippy. A module, a flake edit or an
  # `overrideAttrs` that hands a machine any of them is named here. And the
  # walk has to reach what a copy hides: the release shell's own unit, which
  # only the bundle's copy step names, must be in it, or the walk proves
  # nothing about a copy.
  no-loop-build-reaches-a-machine =
    let
      unitsOf =
        flavour: units:
        lib.concatMapAttrs
          (kind: set: lib.mapAttrs' (n: d: lib.nameValuePair "${flavour}.${kind}.${n}" (probe d.drvPath)) set)
          {
            inherit (units)
              libs
              scripts
              bins
              deps
              ;
          }
        # With the layers (phase 1), the third-party crates are one layer per
        # flavour; with third-party units (phase 2), `deps` above.
        // lib.optionalAttrs (units.layer != null) {
          "${flavour}.layer" = probe units.layer.drvPath;
        };
      # A third-party unit the release shell is linked from: what the walk
      # must reach of the release graph's own third-party half (phase 2).
      releaseDep = lib.findFirst (
        d: (d.kind or null) == "dep"
      ) null rust.release.bins.ronoctua-shell.passthru.unitDeps;
      facts = pkgs.writeText "no-loop-build-reaches-a-machine.json" (
        builtins.toJSON {
          probes =
            unitsOf "loop" rust.loop
            // unitsOf "test" rust.test
            // lib.mapAttrs' (n: d: lib.nameValuePair "test.testBins.${n}" (probe d.drvPath)) rust.test.testBins
            // {
              loopShell = probe rust.loopShell.drvPath;
              # The aggregates of the test graph's runs and clippy.
              tests = probe rust.tests.drvPath;
              clippy = probe rust.clippy.drvPath;
            };
          # What a machine IS built from: the bundle, the release shell's unit
          # (which the bundle reaches by copying it, the way a copied loop
          # program would be reached) and the release layer or, with
          # third-party units, one the shell is linked from.
          reached = {
            "rust.shell" = probe rust.shell.drvPath;
            "release.bins.ronoctua-shell" = probe rust.release.bins.ronoctua-shell.drvPath;
          }
          // lib.optionalAttrs (rust.release.layer != null) {
            "release.layer" = probe rust.release.layer.drvPath;
          }
          // lib.optionalAttrs (releaseDep != null) {
            "release.deps.${releaseDep.name}" = probe releaseDep.drvPath;
          };
        }
      );
    in
    mkCheck "no-loop-build-reaches-a-machine" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, sys
      exec(open("${pyPrelude}").read())

      # PARSED: what evaluation named (JSON) and the walk of what the machines
      # are built from (JSON).
      facts = json.load(open("${facts}"))
      walked = set(json.load(open("${walk}"))["drvs"])
      wrong = []
      for what, drv in sorted(facts["reached"].items()):
          if drv not in walked:
              wrong.append(f"{what} is not in what the machines are built from: the walk does not see through a copy, and proves nothing")
      probes = facts["probes"]
      floor(sum(1 for k in probes if k.startswith("loop.")), 100, "loop derivations asked about")
      floor(sum(1 for k in probes if k.startswith("test.")), 100, "test-graph derivations asked about")
      for what, drv in sorted(probes.items()):
          if drv in walked:
              wrong.append(f"{what} ({drv.rsplit('/', 1)[-1][33:]}) is something a machine or the ISO is built from")
      if wrong:
          print("ERROR: a machine is built from the loop's or the tests' builds:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          print("       It runs, and it is slower, and nothing on a screen says so.", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(probes)} loop and test-graph derivations, none of them in the "
            f"{len(walked)} the machines and the ISO are built from")
      END
    '';

  # A PROGRAM'S CLOSURE HOLDS EVERY STORE PATH IT CITES.
  #
  # Nix registers as a reference only a path among a derivation's inputs. A
  # release library is LLVM bitcode (`-C linker-plugin-lto`), where the path
  # it bakes with `option_env!` is not bytes the scanner can find, so it is
  # never that library unit's reference — and in the linked program, where
  # it does become bytes, it was not an input either. The release shell cited
  # git, 7zz, bsdtar's libarchive, `pamu2fcfg`, `visudo`, the askpass and two
  # of the post-quantum files without holding them: on a machine, a closure
  # that lacks the program a button runs (`nix/rust/units.nix`, `cited`). So
  # every release program (`rust.bins`) is read here, byte by byte, and each
  # store path in its bytes must be in its closure — the scrubbed `eeee…`
  # hash, which names nothing, apart.
  programs-hold-what-they-cite =
    let
      programs = lib.mapAttrs (bin: unit: {
        exe = "${unit}/bin/${bin}";
        closure = "${pkgs.closureInfo { rootPaths = [ unit ]; }}/store-paths";
      }) rust.bins;
      facts = pkgs.writeText "programs-hold-what-they-cite.json" (builtins.toJSON programs);
    in
    mkCheck "programs-hold-what-they-cite" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: each program's bytes, searched for the one shape a store path
      # has, and its closure's list of paths.
      programs = json.load(open("${facts}"))
      floor(len(programs), 40, "release programs")
      PATH = re.compile(rb"/nix/store/([0-9a-z]{32})-")
      NOTHING = b"e" * 32
      wrong, cited_total = [], 0
      for bin, p in sorted(programs.items()):
          cited = set(PATH.findall(open(p["exe"], "rb").read())) - {NOTHING}
          cited_total += len(cited)
          held = {line.strip()[len("/nix/store/"):][:32].encode() for line in open(p["closure"]) if line.strip()}
          missing = sorted(h.decode() for h in cited - held)
          if missing:
              wrong.append(f"{bin} cites {len(missing)} store path(s) its closure does not hold: " + ", ".join(missing[:8]))
      floor(cited_total, 100, "store paths the programs cite")
      if wrong:
          print("ERROR: a program would run without what it names:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(programs)} programs cite {cited_total} store paths, and each is in its program's closure")
      END
    '';
}
