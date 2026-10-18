# WHAT THE FOOTER LENDS ITS REGIONS AND TELLS THEM (the 2026-10-12 wave): a
# region asks for the footer's lines by TYPE and hears its news by TYPE, and a
# type nobody lends or tells is silence the compiler cannot see.
#
# A view of the footer that became a `region::Region` reads the machine through
# the lines the footer keeps for its own sake — the last reading of the sound,
# the power, Syncthing, Atuin, the pointers; the line it asks each daemon
# through; the wishes it follows — lent to every call by type
# (`Footer::kept_for_regions`, `region::kept::<T>`), and a line raised only
# when a view first needs it is asked for by type too (`region::want::<T>`,
# `Footer::raise_for_a_region`). Each new reading reaches the open regions as
# news (`Footer::hosted_heard::<T>`), which a region's `heard` reads by
# downcasting to a type of its own choosing. The compiler sees none of this: a
# region that asks for a type the footer does not lend gets `None`, and draws
# "the sound server is not answering" on a machine where it answers; one that
# downcasts to a type the footer does not tell never refreshes — the battery's
# rows freeze, an unplugged device keeps its row — and every test stays green.
# The silent fallback AGENTS.md says needs a check.
#
#   - `regions-ask-what-the-footer-lends`: every `region::kept::<T>` in a
#     region's product code names a type the footer lends, and every
#     `region::want::<T>` one the footer raises — types resolved through each
#     file's own `use` lines, and the footer's from the fields
#     `kept_for_regions` pushes, as `struct Footer` declares them.
#   - `regions-hear-what-the-footer-tells`: every type a region's `heard`
#     downcasts its news to is one the footer tells by name
#     (`hosted_heard::<T>`, or `Box::<T>::new` among what it tells a region
#     coming to life, `Footer::standing` — where the compiler holds the
#     argument to that type), and every type the footer tells by name is one
#     some region hears.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellPrelude,
}:
let
  # HOW A TYPE WRITTEN IN A FILE IS READ, for both checks: through the file's
  # own `use` lines, to one spelling per type. A prelude of its own, opened
  # after `pyPrelude`; `wrong` is the opening check's list, read when a helper
  # is called.
  typesPrelude = pkgs.writeText "ronoctua-check-region-types.py" ''
    import re as _types_re

    def uses_of(text):
        # THE NAMES A FILE'S `use` LINES BRING IN, each to its full path —
        # `Cell` to `std::cell::Cell`, `Snapshot` to `ronoctua_audio::Snapshot`.
        out = {}
        for m in _types_re.finditer(r"^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([^;]+);", text, _types_re.M):
            # One space between words, none around a path's punctuation — so
            # ` as ` is still a word and `Task` is not split at its `as`.
            spec = _types_re.sub(r"\s*(::|\{|\}|,)\s*", r"\1", " ".join(m.group(1).split()))
            if spec.count("{") > 1:
                wrong.append("a `use` with nested braces this check cannot read: use " + spec)
                continue
            if "{" in spec:
                base, inner = spec.split("{", 1)
                base = base.rstrip(":")
                items = [i for i in inner.rsplit("}", 1)[0].split(",") if i]
            else:
                base, items = "", [spec]
            for item in items:
                path, _, alias = item.partition(" as ")
                if path == "self":
                    full = base
                else:
                    full = base + "::" + path if base else path
                out[alias or full.split("::")[-1]] = full
        return out

    def resolved(ty, uses):
        # A type written in a file, every path in it resolved through the
        # file's `use` lines; a path that is already whole stays.
        def one(m):
            head, rest = m.group(1), m.group(2) or ""
            return uses.get(head, head) + rest
        return _types_re.sub(r"\b([A-Za-z_]\w*)((?:::\w+)*)", one, "".join(ty.split()))

    def unwrapped(ty):
        # A field's type, out of the `Option` and the `Rc` it is lent in.
        ty = "".join(ty.split())
        while True:
            m = _types_re.fullmatch(r"(?:Option|std::rc::Rc|Rc)<(.*)>", ty)
            if not m:
                return ty
            ty = m.group(1)

    # THE NAMES EVERY FILE HAS WITHOUT A `use`, which stay as they are.
    _PRELUDE = {"Option", "Vec", "String", "Box", "Result", "bool", "char", "str",
                "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
                "i128", "isize", "f32", "f64"}

    def canonical(ty, module):
        # ONE SPELLING PER TYPE, whichever crate wrote it: every shell crate
        # names the foundation's modules at its root by their own names
        # (`crate::region`, `ronoctua_shell_door::region`), and module names
        # are one per shell (`shell_file`), so a path into a shell crate is
        # spelled from its module; a bare name no `use` brought in is the
        # writing file's own (`module`).
        def one(m):
            path = m.group(0)
            for lead in ("crate::", "self::"):
                if path.startswith(lead):
                    return path[len(lead):]
            if _types_re.match(r"ronoctua_shell(?:_\w+)?::|ronoctua_render_png::", path):
                return path.split("::", 1)[1]
            if "::" not in path and path not in _PRELUDE:
                return module + "::" + path
            return path
        return _types_re.sub(r"\b[A-Za-z_]\w*(?:::\w+)*", one, ty)
  '';
in
{
  regions-ask-what-the-footer-lends = mkCheck "regions-ask-what-the-footer-lends" [ pkgs.python3 ] ''
    python3 - <<'END'
    import re, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())
    exec(open("${typesPrelude}").read())

    # FILE-WIDE: a region asks for a line anywhere in its file's product, and
    # the footer lends from fields its struct declares; the two methods that
    # lend and raise are narrowed by name (`body_of`). `uncommented`, so a
    # comment naming a type answers nothing, and the product only
    # (`without_tests`): a test lends what it likes.
    footer = uncommented(without_tests(shell_source("surface.rs")))
    floor(len(footer), 100000, "characters of the footer's module")
    wrong = []

    # 1) WHAT THE FOOTER LENDS: the fields `kept_for_regions` pushes, by the
    # type `struct Footer` declares each with.
    lend = must(body_of(footer, "kept_for_regions"), "`Footer::kept_for_regions`")
    struct = must(scope(footer, r"\bstruct Footer\b"), "`struct Footer`")
    fields = dict(re.findall(r"^\s{4}(\w+):\s*(.+?),\s*$", struct, re.M))
    floor(len(fields), 100, "fields of `struct Footer`")
    lent = {}
    for field in re.findall(r"\bself\.(\w+)\b", lend):
        if field not in fields:
            wrong.append(f"`kept_for_regions` lends `self.{field}`, which `struct Footer` does not declare")
            continue
        lent[unwrapped(fields[field])] = field
    floor(len(lent), 5, "lines the footer lends its regions")

    # 2) WHAT THE FOOTER RAISES when a region asks: the types
    # `raise_for_a_region` compares the asked line with.
    raise_ = must(body_of(footer, "raise_for_a_region"), "`Footer::raise_for_a_region`")
    raised = {resolved(t, {}) for t in re.findall(r"TypeId::of::<([^()]+?)>\(\)", raise_)}

    # 3) WHAT EACH REGION ASKS FOR.
    regions = asked = 0
    for f in shell_files():
        text = uncommented(without_tests(f.read_text()))
        if not re.search(r"\bimpl\s+(?:crate::region::|region::)?Region\s+for\s+\w+", text):
            continue
        regions += 1
        name = shell_name(f)
        uses = uses_of(text)
        for kind, ty in re.findall(r"\bregion::(kept|want)::<\s*([^()]+?)\s*>\s*\(", text):
            asked += 1
            whole = resolved(ty, uses)
            if kind == "kept" and whole not in lent:
                wrong.append(f"{name} asks the footer for `{whole}`, which `kept_for_regions` never lends: it reads `None` for ever")
            if kind == "want" and whole not in raised:
                wrong.append(f"{name} asks the footer to raise `{whole}`, which `raise_for_a_region` never raises")
    floor(regions, 1, "regions of the slot host")
    floor(asked, 5, "lines the regions ask the footer for")

    if wrong:
        print("ERROR: a region asks the footer for a line it does not lend.", file=sys.stderr)
        print("", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        print("", file=sys.stderr)
        print("       A region reads the machine through the footer's own lines,", file=sys.stderr)
        print("       lent by type (`region::kept`): one the footer does not lend is a", file=sys.stderr)
        print("       `None` the compiler cannot see, and the view says the daemon is", file=sys.stderr)
        print("       not answering on a machine where it answers.", file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {regions} regions ask {asked} times for {len(lent)} lines the footer lends, and {len(raised)} it raises on demand")
    END
  '';

  regions-hear-what-the-footer-tells =
    mkCheck "regions-hear-what-the-footer-tells" [ pkgs.python3 ]
      ''
        python3 - <<'END'
        import re, sys
        exec(open("${pyPrelude}").read())
        exec(open("${shellPrelude}").read())
        exec(open("${typesPrelude}").read())

        # FILE-WIDE: the footer tells its news anywhere in its module, by the type
        # each call names; a region reads it in its `heard`, narrowed to the
        # `impl Region` and then to the method (`scope`, `body_of`). `uncommented`,
        # so a comment naming a type answers nothing, and the product only
        # (`without_tests`): a test tells a region what it likes.
        footer = uncommented(without_tests(shell_source("surface.rs")))
        floor(len(footer), 100000, "characters of the footer's module")
        wrong = []

        # 1) WHAT THE FOOTER TELLS BY NAME: the type of each `hosted_heard::<T>`,
        # written by its whole path — the footer's `use` lines are not read, so a
        # bare name is the footer's own, and names nothing a region hears.
        told = {}
        for ty in re.findall(r"\bhosted_heard::<\s*([^()]+?)\s*>\s*\(", footer):
            whole = canonical(resolved(ty, {}), "surface")
            told[whole] = told.get(whole, 0) + 1
        unnamed = len(re.findall(r"\bhosted_heard\s*\(", footer))
        # AND WHAT IT TELLS A REGION AS IT COMES TO LIFE — what the footer
        # heard already (`Footer::standing`, replayed by `hosted_replay`), each
        # boxed by the type it names (`Box::<T>::new`), where the compiler
        # holds the value to that type; a bare `Box::new` names none.
        standing = must(body_of(footer, "standing"), "`Footer::standing`")
        for ty in re.findall(r"\bBox::<\s*([^()]+?)\s*>::new\s*\(", standing):
            whole = canonical(resolved(ty, {}), "surface")
            told[whole] = told.get(whole, 0) + 1
        unnamed += len(re.findall(r"\bBox::new\s*\(", standing))
        floor(len(told), 5, "types the footer tells its regions by name")

        # 2) WHAT EACH REGION HEARS: every type its `heard` downcasts the news to.
        heard = {}
        regions = 0
        for f in shell_files():
            text = uncommented(without_tests(f.read_text()))
            impl = re.search(r"\bimpl\s+(?:crate::region::|region::)?Region\s+for\s+\w+", text)
            if not impl:
                continue
            regions += 1
            name = shell_name(f)
            module = name.rsplit("/", 1)[-1][:-3]
            body = body_of(must(scope(text, re.escape(impl.group(0))), name + ": its `impl Region`"), "heard")
            if body is None:
                continue
            uses = uses_of(text)
            for ty in re.findall(r"\.(?:downcast_ref|downcast|is)::<\s*([^()]+?)\s*>\s*\(\s*\)", body):
                heard.setdefault(canonical(resolved(ty, uses), module), []).append(name)
        floor(regions, 1, "regions of the slot host")
        floor(sum(len(v) for v in heard.values()), 5, "types the regions hear")

        for ty, where in sorted(heard.items()):
            if ty not in told:
                for name in sorted(set(where)):
                    wrong.append(f"{name} hears `{ty}`, which the footer never tells by name"
                                 f" (`hosted_heard::<{ty}>`): the region never hears it")
        for ty in sorted(told):
            if ty not in heard:
                wrong.append(f"the footer tells `{ty}` (`hosted_heard::<{ty}>`), which no region's `heard` reads:"
                             " news told to nobody")

        if wrong:
            print("ERROR: what the footer tells and what its regions hear do not meet.", file=sys.stderr)
            print("", file=sys.stderr)
            for w in wrong:
                print("       " + w, file=sys.stderr)
            print("", file=sys.stderr)
            print("       A region's `heard` reads the footer's news by the type it downcasts", file=sys.stderr)
            print("       to, and a type the footer does not tell is a region that never", file=sys.stderr)
            print("       refreshes — no build, test or clippy says so. Tell it with its type", file=sys.stderr)
            print("       named by its whole path (`hosted_heard::<T>(…)`), where the compiler", file=sys.stderr)
            print(f"       holds the argument to it; {unnamed} tell(s) name no type and pair with nothing.", file=sys.stderr)
            raise SystemExit(1)
        print(f"OK: {regions} regions hear {len(heard)} types, each told by name;"
              f" {sum(told.values())} tells of {len(told)} types, {unnamed} more naming none")
        END
      '';
}
