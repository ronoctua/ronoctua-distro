# THE SOFTWARE CENTRE AND THE SEARCH, EACH A CRATE AND A REGION (the
# 2026-10-12 wave): two things about them the compiler does not ask.
#
#   - `software-errands-are-carried-out-once`: the software centre's two
#     doors — its window and its footer region — carry out what the view
#     asks through ONE handler (`software_reading::Reading::carry_out`), and
#     neither keeps a `match` of its own over the view's errands. Two handlers
#     are how the two doors came to keep a copied line on screen for two
#     lengths of time, and to log in one door a browser missing that the
#     other said.
#   - `software-and-search-stand-on-what-they-show`: each of the two features
#     stands on the foundation and on the features whose pages it shows (the
#     search: the calculator's answer, an icon's page), and only the binary
#     stands on them — so a line of one recompiles it and the binary, never
#     the foundation or another feature.
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
  software-errands-are-carried-out-once =
    mkCheck "software-errands-are-carried-out-once" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import re, sys
        exec(open("${pyPrelude}").read())
        exec(open("${shellPrelude}").read())

        # PARSED: the view's `enum Errand` (its variants, by `scope`), the one
        # handler's body (`body_of`), and every arm a door would write over the
        # errands — a line that starts with `Errand::<variant>` and goes on to
        # `=>` — with the comments and the literals off (`code_of`), and the
        # tests out (`without_tests`): a test may match on an errand it got.
        view = code_of(without_tests(shell_source("software.rs")))
        enum = must(scope(view, r"pub enum Errand\b"), "`software::Errand`")
        variants = re.findall(r"^\s{4}([A-Z]\w*)\b", enum, re.M)
        floor(len(variants), 12, "variants of `software::Errand`")
        wrong = []

        # 1) ONE HANDLER, and every errand is its.
        reading = code_of(without_tests(shell_file("software_reading.rs").read_text()))
        handlers = len(re.findall(r"\bfn carry_out\(", reading))
        if handlers != 1:
            wrong.append(f"software_reading.rs has {handlers} `fn carry_out`, and the doors one handler")
        handler = must(body_of(reading, "carry_out"), "`Reading::carry_out`")
        for v in variants:
            if not re.search(r"\bErrand::" + v + r"\b", handler):
                wrong.append(f"`Errand::{v}` is not carried out by the one handler")

        # 2) BOTH DOORS CALL IT, and neither keeps an arm of its own.
        ARM = re.compile(r"^\s*(?:crate::software::)?Errand::([A-Z]\w*)\b[^\n]*=>", re.M)
        for door in ("software_window.rs", "software_region.rs"):
            code = code_of(without_tests(shell_file(door).read_text()))
            floor(len(code), 3000, "characters of " + door)
            if ".carry_out(" not in code:
                wrong.append(f"{door} does not carry the view's errands out through the one handler")
            for m in ARM.finditer(code):
                if m.group(1) in variants:
                    wrong.append(f"{door}: an arm of its own over the view's errands — `Errand::{m.group(1)}`")

        # 3) AND NO OTHER FILE OF THE SHELL that names the view's errands
        # carries one out — the footer did, in a file of its own, once.
        swept = 0
        for f in shell_files():
            name = shell_name(f) or f.name
            if name in ("software.rs", "software_reading.rs", "software_window.rs", "software_region.rs"):
                continue
            code = code_of(without_tests(f.read_text()))
            if not re.search(r"\bsoftware::(?:\{[^}]*\bErrand\b|Errand\b)", code):
                continue
            swept += 1
            for m in ARM.finditer(code):
                if m.group(1) in variants:
                    wrong.append(f"{name}: carries out the software view's `Errand::{m.group(1)}` itself")

        if wrong:
            print("ERROR: the software centre's errands are not carried out once.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            print("", file=sys.stderr)
            print("       The window and the footer's region are two doors onto one view, and", file=sys.stderr)
            print("       what the view asks is done by ONE handler over each door's", file=sys.stderr)
            print("       `region::Doorway` — a second handler is a second answer.", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(variants)} errands, one handler, both doors through it ({swept} other files name them)")
        END
      '';

  software-and-search-stand-on-what-they-show =
    mkCheck "software-and-search-stand-on-what-they-show" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import sys, tomllib
        exec(open("${pyPrelude}").read())
        exec(open("${shellManifestPrelude}").read())

        # PARSED: every shell crate's `[dependencies]` (`tomllib`).
        # FILE-WIDE: a dependency is a table of a manifest, and the question is
        # the graph they make — there is no item of one to narrow to.
        manifests = {m.parent.name: tomllib.loads(m.read_text()) for m in shell_manifests()}
        floor(len(manifests), 13, "shell crates' manifests")

        # WHAT EACH MAY STAND ON: the foundation it uses, and the features whose
        # pages it shows — never the footer, the binary, or another feature.
        STANDS = {
            "ronoctua-shell-software": {
                "ronoctua-shell-kit", "ronoctua-shell-door", "ronoctua-shell-work",
                "ronoctua-shell-keep",
            },
            "ronoctua-shell-fast-access": {
                "ronoctua-shell-kit", "ronoctua-shell-door", "ronoctua-shell-report",
                "ronoctua-shell-work", "ronoctua-shell-inspect",
                # The calculator's answer and an icon's page, which it shows.
                "ronoctua-shell-calc", "ronoctua-shell-icons",
            },
        }
        wrong = []
        for crate, allowed in sorted(STANDS.items()):
            if crate not in manifests:
                wrong.append(f"{crate} is not a shell crate any more: what this check guards moved")
                continue
            shell = {d for d in manifests[crate].get("dependencies", {}) if d in manifests}
            for d in sorted(shell - allowed):
                wrong.append(
                    f"{crate} stands on {d}: a line of {d} would recompile {crate}"
                    " — and a feature is where its own lines stop"
                )
        # ONLY THE BINARY STANDS ON THEM: it is where the features meet.
        for crate, m in sorted(manifests.items()):
            if crate == "ronoctua-shell":
                continue
            for d in sorted(set(m.get("dependencies", {})) & set(STANDS)):
                wrong.append(f"{crate} stands on {d}, which only the binary may")
        if not all(c in manifests["ronoctua-shell"].get("dependencies", {}) for c in STANDS):
            wrong.append("the binary does not stand on both features: one of them is not in the shell")

        if wrong:
            print("ERROR: a feature stands where a line of it reaches too far.", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {len(STANDS)} features stand on what they show, and only the binary on them")
        END
      '';
}
