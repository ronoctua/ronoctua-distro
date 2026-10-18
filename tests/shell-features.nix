# THE SHELL'S FEATURES IN CRATES OF THEIR OWN (the 2026-10-12 wave): the
# check that a feature which became a region of the footer's slot host stays
# a crate a line of which recompiles itself and the binary, and nothing else.
#
# A hosted feature — a view, its window, its camera, its keys and its
# `region::Region` — is a crate on the kit and the door. The compiler holds
# half of what makes it worth having: a cycle is refused. The other half is
# no compiler's question, and this holds it:
#
#   - `a-feature-stands-on-the-foundation`: a hosted feature's crate stands
#     on the kit and the door, on another feature only where the design's
#     graph says so, and never on what composes the features (the binary, the
#     footer); and only the binary — and those few — depend on it. A sideways
#     edge compiles, and makes every line of one view recompile another. A
#     shell crate that is no feature of its own (the work, the inspection, a
#     piece two features share) is foundation to the ones that need it.
#
# What names a feature INSIDE the binary is not asked here: until the footer
# is a crate of its own (lane R6), some of its files still name a feature
# they draw a line of (a modeline mark, a sub-view), and once it is one the
# compiler refuses a footer that names a feature, with `footer-names-no-feature`
# reading the manifests beside it.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellPrelude,
  shellManifestPrelude,
}:
let
  # THE REGIONS THE BINARY REGISTERS, and the crate of each, read from
  # `regions.rs` — the one list that says which features are hosted. An
  # entry is named by its crate's path (`ronoctua_shell_stick::stick_region::
  # ENTRY`) or by the module the binary's root takes from that crate
  # (`crate::stick_region::ENTRY` beside `use ronoctua_shell_stick::{…,
  # stick_region}` in `main.rs`) — both are that crate's region. One whose
  # module is still the binary's own is no crate of its own yet.
  registered = ''
    def hosted_crates():
        text = code_of(shell_file("regions.rs").read_text())
        root = code_of(shell_file("main.rs").read_text())
        taken_from = {}
        for crate, names in re.findall(r"\buse\s+ronoctua_shell_(\w+)::\{([^}]*)\}", root):
            for n in names.split(","):
                n = n.strip().split(" as ")[-1].strip()
                if n:
                    taken_from[n] = crate
        for crate, n in re.findall(r"\buse\s+ronoctua_shell_(\w+)::(\w+)\s*;", root):
            taken_from[n] = crate
        named = set(re.findall(r"\bronoctua_shell_(\w+)::\w+::ENTRY\b", text))
        named |= {taken_from[m] for m in re.findall(r"\bcrate::(\w+)::ENTRY\b", text) if m in taken_from}
        found = sorted("ronoctua-shell-" + c.replace("_", "-") for c in named)
        floor(len(found), 4, "hosted features `regions.rs` registers")
        return found
  '';
in
{
  a-feature-stands-on-the-foundation =
    mkCheck "a-feature-stands-on-the-foundation" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import re, sys, tomllib
        exec(open("${pyPrelude}").read())
        exec(open("${shellPrelude}").read())
        exec(open("${shellManifestPrelude}").read())
        ${registered}

        # PARSED: every shell crate's manifest (`tomllib`) — its dependencies,
        # dev-dependencies and target tables — and the regions `regions.rs`
        # registers, by the crate each `ENTRY` is named from (`code_of`).
        # FILE-WIDE: `regions.rs` is the one list of entries and the root's
        # `use` lines are where a module is taken from a crate (`hosted_crates`)
        # — a list and a set of imports, with no function to narrow to.
        manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
        floor(len(manifests), 15, "shell crates' manifests")
        # THE DESIGN'S FEW EDGES BETWEEN FEATURES (shell-data-design §1.2): the
        # file manager shows a repository's line and seals a file; the software
        # centre and the update say the belongings' words; fast-access answers
        # with the calculator and the icons; and the player's two pages written
        # above it — channels by theme, and the video catalogue whose ranking is
        # a page of the player — stand on the player, whose view asks them
        # through its seam (`player_seam`), and on no other. Any other is a
        # view recompiling another.
        EDGES = {
            "ronoctua-shell-files": {"ronoctua-shell-git", "ronoctua-shell-security"},
            "ronoctua-shell-software": {"ronoctua-shell-belongings"},
            "ronoctua-shell-update": {"ronoctua-shell-belongings"},
            "ronoctua-shell-fast-access": {"ronoctua-shell-calc", "ronoctua-shell-icons"},
            "ronoctua-shell-catalogue": {"ronoctua-shell-player"},
            "ronoctua-shell-video-metrics": {"ronoctua-shell-player"},
        }
        BINARY = "ronoctua-shell"
        # WHAT COMPOSES THE FEATURES, which no feature stands on: the binary,
        # and the footer once it is a crate of its own (it hosts a region by
        # its id).
        COMPOSERS = {BINARY, "ronoctua-shell-footer"}

        def shell_deps(m):
            tables = [m.get(t, {}) for t in ("dependencies", "dev-dependencies", "build-dependencies")]
            for target in m.get("target", {}).values():
                tables += [target.get(t, {}) for t in ("dependencies", "dev-dependencies")]
            return {d for t in tables for d in t if d in manifests}

        hosted = hosted_crates()
        wrong = []
        for crate in hosted:
            if crate not in manifests:
                wrong.append(f"`regions.rs` registers a region of {crate}, which is no shell crate")
                continue
            deps = shell_deps(manifests[crate])
            for must_have in ("ronoctua-shell-kit", "ronoctua-shell-door"):
                if must_have not in deps:
                    wrong.append(f"{crate} does not stand on {must_have}: a region is written over the door's `region`")
            # ANOTHER FEATURE only where the design's graph says; a shell crate
            # that is no feature of its own (the work, the inspection, a piece
            # two features share) is a foundation of the ones that need it.
            for d in sorted((deps & set(hosted)) - EDGES.get(crate, set())):
                wrong.append(f"{crate} stands on the hosted feature {d}: a line of {d} would recompile {crate}")
            for d in sorted(deps & COMPOSERS):
                wrong.append(f"{crate} stands on {d}, which composes the features: every line of it would recompile {crate}")
        # AND ONLY THE BINARY STANDS ON ONE — and the design's few features
        # (a hosted feature's own edges were read above).
        for crate, m in sorted(manifests.items()):
            if crate == BINARY or crate in hosted:
                continue
            for d in sorted(shell_deps(m) & set(hosted)):
                if d != crate and d not in EDGES.get(crate, set()):
                    wrong.append(f"{crate} stands on the hosted feature {d}: a line of {d} would recompile {crate}")
        for crate in hosted:
            if crate in manifests and crate not in shell_deps(manifests.get(BINARY, {})):
                wrong.append(f"the binary does not link {crate}, whose region it registers")

        if wrong:
            print("ERROR: a feature of the shell does not stand where its crate was cut to stand.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(hosted)} hosted features stand on the foundation, and only the binary stands on them")
        END
      '';
}
