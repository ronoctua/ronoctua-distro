# THE BUILD IS UNITS — the 2026-10-12 wave, lane N7 (closing the build).
#
# Every crate of ours is compiled by one derivation per unit of cargo's plan
# (`nix/rust/units.nix`), and crane compiles none of them: it vendors the
# sources, builds the planner, the two layers the profile bench measures on
# and the check that sets the third-party units beside cargo. crane's
# whole-workspace builds (`workspace`, `loopWorkspace`, `tests`, `clippy`,
# `fmt`) are gone, and with them the reason one line anywhere recompiled
# everything: a build like those is one `cargo` over the workspace's lock, and
# a program a module runs, a battery's test or the nested loop built that way
# would bring it back without anything failing — it would only be slow again.
#
# - `our-crates-are-built-unit-by-unit`: nothing the machines, the ISO, the
#   batteries' Rust aggregates, the nested loop, `.#bench` or `.#png` are
#   built from runs cargo over this workspace's lock.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
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
  plain = builtins.unsafeDiscardStringContext;
in
{
  # NOTHING THAT SHIPS, TESTS OR LOOPS IS A CARGO BUILD OF THE WORKSPACE.
  #
  # A cargo build of ours is a derivation handed the workspace's vendored
  # sources — the one vendoring of `Cargo.lock` (`rust.planner.vendor`), which
  # crane names `cargoVendorDir` in every derivation it makes: a crate of ours
  # cannot be compiled by cargo without them, and a unit is never handed them
  # (a third-party unit compiles its own package's source). So this walks
  # the build graph of each root, every `.drv` it was made from, and refuses
  # one whose environment names the vendored sources:
  #
  #   - the two profiles' systems and the ISO (what a machine runs and what
  #     an installation copies);
  #   - the batteries' Rust aggregates (`rust-tests`, `rust-clippy`,
  #     `rust-fmt`);
  #   - the nested loop's bundle (`rust.loopShell`), `.#bench`'s programs
  #     (`rust.benches`) and `.#png`'s (`rust.loop.bins.ronoctua-render-png`).
  #
  # Another program's crane build (a pinned tool built upstream with crane)
  # is handed its own lock's vendoring, never this one, and is not asked.
  # What it cannot see is our lock vendored some other way first (nixpkgs'
  # `importCargoLock`, a copy of `Cargo.lock` under another name): every
  # route the repository has ever compiled its crates by, crane's, it sees.
  #
  # AND IT IS NOT BLIND: the profile bench's release layer, a real cargo build
  # over the workspace's lock, is walked the same way and must be found, and
  # every root's graph has to reach the units it is built from.
  our-crates-are-built-unit-by-unit =
    let
      roots = {
        noctua = self.nixosConfigurations.noctua.config.system.build.toplevel;
        minimal = self.nixosConfigurations.minimal.config.system.build.toplevel;
        iso = self.packages.${system}.iso;
        rust-tests = rust.tests;
        rust-clippy = rust.clippy;
        rust-fmt = rust.fmt;
        loop-shell = rust.loopShell;
        benches = rust.benches;
        png = rust.loop.bins.ronoctua-render-png;
      };
      facts = pkgs.writeText "our-crates-are-built-unit-by-unit.json" (
        builtins.toJSON {
          roots = lib.mapAttrs (_: graphOf) roots;
          planted = graphOf rust.layers.release.raw;
          vendor = plain rust.planner.vendor.outPath;
          # A unit each root must be seen built from, or the walk proves
          # nothing about it: the shell's release program for what runs it,
          # a crate's test run, clippy and rustfmt for the aggregates, the
          # loop's for the loop's.
          reach = lib.mapAttrs (_: d: plain d.drvPath) {
            noctua = rust.release.bins.ronoctua-shell;
            minimal = rust.release.bins.ronoctua-shell;
            iso = rust.release.bins.ronoctua-install;
            rust-tests = rust.test.testRuns.ronoctua-zip;
            rust-clippy = rust.test.clippy.ronoctua-zip;
            rust-fmt = rust.test.fmt.ronoctua-zip;
            loop-shell = rust.loop.bins.ronoctua-shell;
            benches = rust.release.libs.ronoctua-render;
            png = rust.loop.libs.ronoctua-render;
          };
        }
      );
    in
    mkCheck "our-crates-are-built-unit-by-unit" [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import json, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: a `.drv` is a derivation written down in ATerm —
      # `Derive([outputs],[input derivations],[input sources],system,builder,
      # [args],[env])` — read by that grammar: strings with their escapes
      # undone, lists and tuples; structured attributes are the JSON of
      # `__json`. Every `.drv` a root's walk reaches is in this sandbox,
      # because the store's references of a `.drv` are what it was made from.
      facts = json.load(open(sys.argv[1]))
      VENDOR = facts["vendor"]
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


      parsed = {}


      def read(path):
          """A `.drv`'s input derivations, and whether it is handed the
          workspace's vendored sources (as `cargoVendorDir`, plain or in its
          structured attributes)."""
          if path not in parsed:
              fields = aterm(open(path, errors="surrogateescape").read())
              if isinstance(fields[0], str):
                  fields = fields[1:]  # `DrvWithVersion("…", …)`
              _outputs, inputs, _sources, _system, _builder, _args, env = fields
              env = dict(env)
              if "__json" in env:
                  env.update({k: v for k, v in json.loads(env["__json"]).items() if isinstance(v, str)})
              parsed[path] = ([i[0] for i in inputs], env.get("cargoVendorDir") == VENDOR)
          return parsed[path]


      def walk(root):
          seen, todo, cargo = set(), [root], []
          while todo:
              path = todo.pop()
              if path in seen:
                  continue
              seen.add(path)
              inputs, vendored = read(path)
              if vendored:
                  cargo.append(path)
              todo += inputs
          return seen, cargo


      def name(path):
          return path.rsplit("/", 1)[-1][33:].removesuffix(".drv")


      wrong = []
      _, found = walk(facts["planted"])
      if facts["planted"] not in found:
          wrong.append("the profile bench's release layer, a cargo build over the workspace's lock, was not found: "
                       "this census is blind")
      floor(len(facts["roots"]), 9, "roots walked")
      read_total = 0
      for root, path in sorted(facts["roots"].items()):
          seen, cargo = walk(path)
          read_total += len(seen)
          floor(len(seen), 100, f"derivations {root} is built from")
          if facts["reach"][root] not in seen:
              wrong.append(f"{root} is not seen built from {name(facts['reach'][root])}: the walk proves nothing about it")
          for c in sorted(cargo):
              wrong.append(f"{root} is built from {name(c)}, a cargo build over the workspace's lock")
      if wrong:
          print("ERROR: a crate of ours is compiled by cargo over the workspace, outside the units:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          print("  One such build is every crate of the workspace in one derivation: a line anywhere", file=sys.stderr)
          print("  recompiles it. Name the program's unit (`rust.bins.<bin>`, nix/rust/outputs.nix).", file=sys.stderr)
          sys.exit(1)
      print(f"OK: the {len(facts['roots'])} roots are built from {len(parsed)} derivations, none of them a cargo build"
            " over the workspace's lock; the profile bench's layer, which is one, is found")
      END
    '';
}
