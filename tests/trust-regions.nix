# THE BELONGINGS, THE SECURITY CENTRE AND THE UPDATE IN CRATES OF THEIR OWN
# (the 2026-10-12 wave): the check that each stands where the design put it.
#
# Each of the three is one crate — its view, its window, its region in the
# footer, its camera and its keys — and what that buys is a line of one
# recompiling that one and the crates above it, never a sibling. The compiler
# refuses a cycle, not a sideways edge: the update depending on the security
# centre compiles, and makes every line of the centre recompile the update;
# the lifeline depending on the belongings compiles, and puts a feature under
# the one screen that must outlive the footer.
#
#   - `the-trust-crates-stand-where-the-design-says`: each crate's shell
#     dependencies are the ones the design's graph gives it (the foundation,
#     the belongings under the update, and the update's words leaf the footer
#     says too), and the shell crates that stand on
#     each are the ones the graph names (the binary; the update and the
#     software centre on the belongings; the file manager on the centre).
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellManifestPrelude,
}:
{
  the-trust-crates-stand-where-the-design-says =
    mkCheck "the-trust-crates-stand-where-the-design-says" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import sys, tomllib
        exec(open("${pyPrelude}").read())
        exec(open("${shellManifestPrelude}").read())

        # PARSED: every shell crate's manifest (`tomllib`) — its dependencies,
        # its dev-dependencies and its build-dependencies, which are edges a
        # line recompiles along as much as the others.
        # FILE-WIDE: a manifest's dependency tables are the whole question;
        # there is no function to narrow to.
        manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
        floor(len(manifests), 14, "shell crates' manifests")

        def stands_on(crate):
            m = manifests[crate]
            tables = [m.get(t, {}) for t in ("dependencies", "dev-dependencies", "build-dependencies")]
            return {d for t in tables for d in t if d in manifests}

        # THE DESIGN'S GRAPH (shell-data-design §1.2, with R1's report and
        # keep): for each crate, the shell crates it may stand on, and the
        # ones that may stand on it.
        FOUNDATION = {"ronoctua-shell-kit", "ronoctua-shell-door"}
        GRAPH = {
            "ronoctua-shell-belongings": (
                FOUNDATION | {"ronoctua-shell-keep"},
                {"ronoctua-shell", "ronoctua-shell-update", "ronoctua-shell-software"},
            ),
            "ronoctua-shell-security": (
                FOUNDATION | {"ronoctua-shell-report", "ronoctua-shell-work"},
                {"ronoctua-shell", "ronoctua-shell-files"},
            ),
            # AND THE UPDATE'S WORDS (lane R6): the mark the footer draws while
            # a run is under way, a leaf under the update and the footer —
            # what moved DOWN for both, as the design says to (`<view>-words`,
            # on no shell crate: `a-view-stands-on-the-foundation`).
            "ronoctua-shell-update": (
                FOUNDATION | {"ronoctua-shell-belongings", "ronoctua-shell-update-words"},
                {"ronoctua-shell"},
            ),
        }
        wrong = []
        for crate, (below, above) in GRAPH.items():
            if crate not in manifests:
                wrong.append(f"{crate} is not a shell crate of this tree any more")
                continue
            for d in sorted(stands_on(crate) - below):
                wrong.append(f"{crate} stands on {d}, which the design does not put under it")
            for d in sorted(FOUNDATION - stands_on(crate)):
                wrong.append(f"{crate} no longer stands on {d}: has it stopped being a view of its own?")
            users = sorted(c for c in manifests if crate in stands_on(c))
            for c in users:
                if c not in above:
                    wrong.append(f"{c} stands on {crate}, which the design keeps from it")
            if "ronoctua-shell" not in users:
                wrong.append(f"the binary does not stand on {crate}: who registers its region?")

        if wrong:
            print("ERROR: a crate of the trust family stands where the design did not put it.", file=sys.stderr)
            print("", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            print("", file=sys.stderr)
            print("       Each edge is a recompilation: a line of the crate below rebuilds", file=sys.stderr)
            print("       the crate above. An edge the design does not have is a line of one", file=sys.stderr)
            print("       feature rebuilding another — move what both need down instead.", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(GRAPH)} crates of the trust family stand where the design put them, among {len(manifests)} shell crates")
        END
      '';
}
