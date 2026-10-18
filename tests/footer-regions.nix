# THE FOOTER'S REGIONS (the 2026-10-12 wave): the check that the slot host
# stays a host.
#
# A feature's view in the footer becomes a `region::Region` of its own crate,
# kept by the footer in `region::Slots` and asked by id from one line of each
# dispatcher (`surface/host.rs`). What makes that worth having is that the
# host names no feature: converting a view is a line in the binary's
# `regions.rs` and nothing in the footer, and a line of a view never
# recompiles the footer's host. Two things keep it so, and neither is a
# compiler's question:
#
#   - `the-slot-host-names-no-feature`: the host's files (`surface/host.rs`,
#     and `surface/sections.rs`, its half for a region laid out in sections)
#     speak only the foundation (the kit's and the door's modules) and read
#     no view's own field — the footer's half of a view still drawn with a
#     field lives in that view's `surface/<view>.rs`, and a host that read
#     one would be the same view written twice.
#   - and every thing a region asks of the footer's door (`region::Effect`) is
#     done, by its name: a wildcard arm would drop the next one silently, and
#     a region whose sentence is never said looks exactly like one that said
#     nothing.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellPrelude,
}:
{
  the-slot-host-names-no-feature = mkCheck "the-slot-host-names-no-feature" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # FILE-WIDE: the question is what the host's FILES name — anywhere in
    # them, their product (`without_tests`) with the comments off and the
    # strings kept (`uncommented`), since a feature can be named in a path
    # as well as in a literal id. Two files: the host (`surface/host.rs`)
    # and its half for a region laid out in sections (`surface/sections.rs`),
    # which lays the region onto the footer's lines without naming it.
    halves = [uncommented(without_tests(shell_file(f).read_text()))
              for f in ("surface/host.rs", "surface/sections.rs")]
    floor(len(halves[1]), 3000, "characters of the host's half for sections")
    host = "\n".join(halves)
    floor(len(host), 3000, "characters of the slot host")
    wrong = []

    # 1) ONLY THE FOUNDATION: every `crate::<module>` the host names is a
    # module of the kit or of the door, as each crate's `lib.rs` declares
    # them — never a feature, whichever crate it is in.
    def modules_of(crate):
        dirs = [c for c in shell_crates() if c.name == crate]
        if len(dirs) != 1:
            print(f"ERROR: no shell crate `{crate}`", file=sys.stderr)
            raise SystemExit(1)
        lib = uncommented((dirs[0] / "src" / "lib.rs").read_text())
        return set(re.findall(r"^\s*pub mod (\w+);", lib, re.M))
    foundation = modules_of("ronoctua-shell-kit") | modules_of("ronoctua-shell-door")
    floor(len(foundation), 40, "modules of the kit and the door")
    if "region" not in foundation:
        wrong.append("the door declares no `region` module, so the host has nothing to host")
    named = set(re.findall(r"\bcrate::(\w+)", host))
    floor(len(named), 4, "modules the host names")
    for m in sorted(named - foundation):
        wrong.append(f"the host names `crate::{m}`, which is no module of the kit or the door")

    # 2) NO VIEW'S OWN FIELD, and no view by its id: the fields are the
    # ones the precedence's own question reads (`region_open`, one line
    # per view the footer still draws), and an id is a `<view>::ID`.
    footer = uncommented(without_tests(shell_file("surface/mod.rs").read_text()))
    asked = must(body_of(footer, "region_open"), "`region_open`")
    fields = set(re.findall(r"\w+::ID\s*=>\s*self\.(\w+)", asked))
    # ONE FIELD PER VIEW THE FOOTER STILL DRAWS (`CONCRETE`), whose number
    # falls as each becomes a region: a fixed floor stops matching the tree,
    # and this one stops matching only when the reading above breaks.
    concrete = re.search(r"\bconst CONCRETE\b[^=]*=\s*&\[(.*?)\];", footer, re.S)
    if concrete is None:
        must(None, "`CONCRETE`")
    # EMPTY once every view is a region of the slot host (lane R5i took the
    # last two, the player and the video catalogue): then no field is left
    # for `region_open` to read, and none is read.
    concrete = concrete.group(1)
    floor(len(fields), len(re.findall(r"\b\w+::ID\b", concrete)), "views' fields `region_open` reads, one per view `CONCRETE` names")
    for f in sorted(fields):
        if re.search(r"\bself\." + f + r"\b", host):
            wrong.append(f"the host reads the view's own field `self.{f}`")
    for v in sorted(set(re.findall(r"\b(\w+)::ID\b", host))):
        wrong.append(f"the host names the view `{v}` by its id")

    # 3) EVERY THING A REGION ASKS IS DONE, by its name — no wildcard.
    door = [c for c in shell_crates() if c.name == "ronoctua-shell-door"][0]
    region = uncommented((door / "src" / "region.rs").read_text())
    effect = must(scope(region, r"pub enum Effect\b"), "`region::Effect`")
    variants = re.findall(r"^\s{4}([A-Z]\w*)\b", effect, re.M)
    floor(len(variants), 8, "variants of `region::Effect`")
    owed = must(body_of(halves[0], "region_owed"), "`Footer::region_owed`")
    for v in variants:
        if not re.search(r"\bEffect::" + v + r"\b", owed):
            wrong.append(f"`Effect::{v}` — what a region asks — is never done by `region_owed`")
    if re.search(r"(?m)^\s*_\s*=>", owed):
        wrong.append("`region_owed` has a wildcard arm: the next thing a region asks would be dropped unsaid")

    if wrong:
        print("ERROR: the footer's slot host is not a host.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A region is known to the footer by its id and by nothing else:", file=sys.stderr)
        print("       converting a view is a line in `regions.rs`, and the host never", file=sys.stderr)
        print("       recompiles for one.", file=sys.stderr)
        raise SystemExit(1)
    print(
        f"OK: the host speaks {len(named)} modules of the foundation, reads none of "
        f"{len(fields)} views' fields, and does every one of {len(variants)} asks"
    )
    END
  '';
}
