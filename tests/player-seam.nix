# THE PLAYER'S SEAM (the 2026-10-12 wave, lane R5i): the pages of the player
# written in crates ABOVE it — channels by theme (`ronoctua-shell-catalogue`)
# and the ranking of the video catalogue (`ronoctua-shell-video-metrics`) —
# are asked by the player's view through `player_seam::Page`, which the
# binary installs at start. The seam is what lets a line of a page recompile
# that page's crate and the binary, never the player's transport: the build
# graph proves that per edit (`granularity-of-an-edit`, which samples a line of
# each page). What no build graph sees is held here:
#
#   - `the-player-pages-are-installed`: every page written above the player
#     stands on the player and on no other page, the player stands on none of
#     them (in no table — a dev-dependency on a page is a second player in its
#     tests), and every one is installed: its `PAGE` is in the binary's
#     `PLAYER_PAGES`, which `main` installs before it reads the subcommand,
#     so every door — the footer, a window, the column, a camera — holds it.
#     A page written and not installed compiles and draws nothing: its rows
#     are asked of nobody, silently.
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
{
  the-player-pages-are-installed = mkCheck "the-player-pages-are-installed" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys, tomllib
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())
    exec(open("${shellManifestPrelude}").read())

    # PARSED: every shell crate's manifest (`tomllib`) — every table of
    # dependencies, the targets' included — and the product of each shell
    # file (`code_of` over `outside_tests`): an `impl` of a test, a fake
    # page, is no page of the player's.
    manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
    floor(len(manifests), 14, "shell crates' manifests")

    def crate_of(path):
        parts = path.parts
        return parts[parts.index("src") - 1]

    EVERY = ("dependencies", "dev-dependencies", "build-dependencies")

    def deps_of(m, kinds=EVERY):
        """The crates `m` depends on in the tables `kinds`, its targets' too."""
        tables = [m.get(t, {}) for t in kinds]
        for target in m.get("target", {}).values():
            tables += [target.get(t, {}) for t in kinds]
        return {d for t in tables for d in t}

    # THE PLAYER: the crate whose `player_seam.rs` writes the trait.
    seam = shell_file("player_seam.rs")
    player = crate_of(seam)
    seam_code = code_of(outside_tests(seam.read_text()))
    if not re.search(r"\bpub trait Page\b", seam_code):
        print("ERROR: player_seam.rs writes no `pub trait Page`", file=sys.stderr)
        raise SystemExit(1)

    # THE PAGES: every product `impl … Page for <T>` of a shell file, and
    # the `PAGE` its crate installs (`pub static PAGE: <T>`).
    # Named by any path (`crate::player_seam::Page`, the crate's own name):
    # `player_seam.rs` writes the one trait called `Page`.
    IMPL = re.compile(r"\bimpl\s+(?:\w+::)*Page\s+for\s+(\w+)")
    pages = {}
    for f in shell_files():
        code = code_of(outside_tests(f.read_text()))
        for t in IMPL.findall(code):
            crate = crate_of(f)
            static = re.search(r"\bpub static (\w+):\s*" + t + r"\s*=", code)
            pages.setdefault(crate, []).append((shell_name(f), t, static and static.group(1)))
    floor(sum(len(v) for v in pages.values()), 2, "pages of the player written above it")

    wrong = []
    # THE PLAYER STANDS ON NO PAGE, in no table.
    for d in sorted(deps_of(manifests.get(player, {})) & set(pages)):
        wrong.append(f"{player} depends on {d}, a page written above it: a line of the page recompiles the transport")
    for crate, found in sorted(pages.items()):
        if crate == player:
            wrong.append(f"{crate} writes its own page ({found[0][1]}): a page is written above the player, never in it")
            continue
        deps = deps_of(manifests.get(crate, {}))
        # EACH PAGE STANDS ON THE PLAYER — its product does, a dev-dependency
        # is only its tests' — and on no other page, in no table.
        if player not in deps_of(manifests.get(crate, {}), ("dependencies",)):
            wrong.append(f"{crate} writes a page of the player and its product does not stand on {player}")
        for d in sorted(deps & (set(pages) - {crate, player})):
            wrong.append(f"{crate} depends on {d}, another page of the player: a line of one recompiles the other")

    # EVERY PAGE IS INSTALLED: its `PAGE` in `PLAYER_PAGES`, which `main`
    # installs before it reads the subcommand.
    root = code_of(shell_file("main.rs").read_text())
    listed = re.search(r"\bconst PLAYER_PAGES:[^=]*=\s*&\[(.*?)\];", root, re.S)
    if listed is None:
        wrong.append("main.rs keeps no `PLAYER_PAGES`: no page of the player is installed")
        installed = set()
    else:
        installed = set(re.findall(r"&\s*(?:\w+::)*(\w+)::(\w+)\s*(?:,|$)", listed.group(1)))
    for crate, found in sorted(pages.items()):
        for name, t, static in found:
            if static is None:
                wrong.append(f"{name} writes the page {t} and keeps no `pub static <NAME>: {t}` to install")
            elif (name[:-3], static) not in installed:
                wrong.append(f"{name}'s {static} ({t}) is not in `PLAYER_PAGES`: its rows are asked of nobody")
    body = must(body_of(root, "main"), "`fn main` in main.rs")
    install = body.find("player_seam::install(PLAYER_PAGES)")
    reads = body.find("std::env::args()")
    if install < 0:
        wrong.append("`main` never installs `PLAYER_PAGES`: every view of this process holds no page")
    elif reads >= 0 and reads < install:
        wrong.append("`main` reads the subcommand before it installs `PLAYER_PAGES`: a door may start with no page")

    if wrong:
        print("ERROR: the player's seam does not hold.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    n = sum(len(v) for v in pages.values())
    print(f"OK: {n} pages of the player stand above it, each on the player alone, and every one is installed")
    END
  '';
}
