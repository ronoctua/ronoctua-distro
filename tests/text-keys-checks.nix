# EVERY TEXT KEY IS SAID (owner, 2026-10-31): a sentence nobody can ever show
# does not sit in the texts, translated in every language, for ever.
#
#   - `every-key-is-said`: every key of every fragment — each speaking
#     crate's `i18n/`, each brain module's and the extension's — is named by
#     something that runs or that a program reads. A key is SAID when:
#
#       * its constant (`key::A_B`, `nix/text-keys.nix`; a brain module's
#         generated `key` module) is named in the Rust that runs — through
#         `key::`, an alias of it or bare, never another module's path;
#       * its name is a string literal of that Rust;
#       * a template of that Rust composes it — a literal with a key-shaped
#         head and holes (`"about-{id}"`, `"notif-{}-summary"`), or a bare
#         prefix put to use (`named(texts, "icons-set-", s)`, a constant in a
#         `{NAME}`; one only handed to `strip_prefix`/`starts_with` takes a
#         word apart and composes nothing) — and what fills each hole is an id
#         the code or its data holds: a literal (or one lower-cased), a string
#         of the brain's catalogue, a reason `assets/distro-apps.nix` declares
#         a program essential; a set's or a category's name only a word of the
#         icon tables. A template of several holes takes them from its own
#         crate and the crates it stands on, or a file name with two dashes
#         would compose half the texts;
#       * a brain module's table reads it by its VALUE: the prefix a
#         `tables.nix` hands its generator, each word a literal of the code;
#       * a generator owes it for a table the crates include (`keys.txt`
#         beside the table: what the table makes a program say, through a
#         spelling the check cannot follow — `largestFirst` drawn as
#         `largest-first`);
#       * a string of the brain's catalogue names it (what the footer reads
#         of the modules at run time);
#       * the extension says it (`TEXT.<camelCase>` in its scripts);
#       * it is referenced from a message that is said (`{ other-key }`);
#       * it is the outside's word: a family whose last word a daemon says
#         (`OUTSIDE`, each with the line that composes it).
#
#     The Rust is every crate's and buffer's, tests taken out (a test's use is
#     not a screen), and every generated `.rs` the build hands the crates
#     (`rust.generated`), each generated `key` module taken out: it defines
#     every key and says none. What is left is refused by name, unless the
#     key is a debt on `UNSAID`, which says why; a debt paid or gone is
#     refused too — take it off the list.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  mkCheck,
  pyPrelude,
  rust,
  brainCatalogue,
  iconset,
  languages,
}:
let
  # THE TREE THE CHECK READS, and nothing else of it: the crates' and the
  # buffers' Rust and manifests, every fragment, the brain modules' tables
  # and the extension's scripts. A sentence edited elsewhere does not run it.
  tree = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      (lib.fileset.fileFilter (
        f: f.hasExt "rs" || f.hasExt "ftl" || f.name == "Cargo.toml" || f.name == "tables.nix"
      ) ../crates)
      (lib.fileset.fileFilter (f: f.hasExt "rs" || f.name == "Cargo.toml") ../buffers)
      (lib.fileset.fileFilter (f: f.hasExt "js" || f.hasExt "ftl") ../extension)
    ];
  };

  # WHAT THE BUILD GENERATES FOR THE CRATES: every `.rs` of `rust.generated`,
  # and the three folders of generated Rust it hands them by name.
  generated = lib.filterAttrs (_: path: lib.hasSuffix ".rs" path) rust.generated // {
    inherit (rust.generated)
      RONOCTUA_BRAIN_MODULES_DIR
      RONOCTUA_BRAIN_REGISTRY_DIR
      RONOCTUA_MIME_DIR
      ;
  };

  inputs = pkgs.writeText "every-key-is-said.json" (
    builtins.toJSON {
      tree = "${tree}";
      inherit generated languages;
      catalogue = "${brainCatalogue}/${brainCatalogue.file}";
      iconset = "${iconset}";
      # The reasons the distro's app table declares a program essential.
      essential = (import ../assets/distro-apps.nix).essential;
    }
  );
in
{
  every-key-is-said = mkCheck "every-key-is-said" [ pkgs.python3 ] ''
    python3 - <<'END'
    import collections, json, pathlib, re, sys, tomllib
    exec(open("${pyPrelude}").read())

    # PARSED: the fragments line by line as `gen-i18n.py` reads them (a line
    # that starts with `#` is a comment), the manifests with `tomllib`, the
    # catalogue and the icon tables with `json`, the owed lists by word.
    # FILE-WIDE: a key may be named anywhere in a crate — the question is the
    # whole of the code that runs, so every file is read whole, its comments
    # and its tests taken out (`code_of`, `outside_tests`, `hashless`).
    INPUTS = json.loads(pathlib.Path("${inputs}").read_text())
    TREE = pathlib.Path(INPUTS["tree"])
    BASE = INPUTS["languages"][0]

    # THE OUTSIDE'S WORDS: a family whose last word a daemon says — BlueZ's
    # enumerations, Tailscale's state — and not one of ours, so every key of
    # the family is one the daemon may say. Each with the file and the line
    # that composes it, and whose word it is: a family no longer composed is
    # refused.
    OUTSIDE = {
        "bluetooth-address-type-": ("crates/ronoctua-shell-bluetooth/src/bluetooth.rs",
                                    r'word_or\(\s*\w+\s*,\s*"bluetooth-address-type"',
                                    "BlueZ's AddressType"),
        "bluetooth-audio-": ("crates/ronoctua-shell-bluetooth/src/bluetooth.rs",
                             r'word_or\(\s*\w+\s*,\s*"bluetooth-audio"',
                             "the profile BlueZ says a device's audio is in"),
        "bluetooth-codec-": ("crates/ronoctua-shell-bluetooth/src/bluetooth.rs",
                             r'word_or\(\s*\w+\s*,\s*"bluetooth-codec"',
                             "BlueZ's codec number"),
        "bluetooth-power-state-": ("crates/ronoctua-shell-bluetooth/src/bluetooth.rs",
                                   r'"bluetooth-power-state-\{\}",\s*if\s+\w+\.power_state',
                                   "BlueZ's PowerState"),
        "bluetooth-reconnect-": ("crates/ronoctua-shell-bluetooth/src/bluetooth.rs",
                                 r'word_or\(\s*\w+\s*,\s*"bluetooth-reconnect"',
                                 "BlueZ's ReconnectMode"),
        "bluetooth-transport-": ("crates/ronoctua-shell-bluetooth/src/bluetooth.rs",
                                 r'word_or\(\s*\w+\s*,\s*"bluetooth-transport"',
                                 "a BlueZ transport's State"),
        "network-tailscale-": ("crates/ronoctua-shell-network/src/network.rs",
                               r'"network-tailscale-\{\}",\s*\w+\.state\.to_ascii_lowercase\(\)',
                               "Tailscale's BackendState, lower-cased"),
    }

    # THE DEBT: a key nothing says that stays, and why. Sorted; a key said
    # again or gone is refused — take it off the list.
    UNSAID = {
        "atlas-credits": "the attribution CC BY 4.0 and the ODbL require on screen, which "
                         "`atlas-credits-are-carried` holds in the texts; the world data's "
                         "page that would draw it is not built",
    }

    wrong = []

    # 1) THE KEYS: every fragment's base file, each key with the folder that
    # owns it and the messages its value references.
    KEY = re.compile(r"([a-z][a-z0-9-]*)\s*=")
    REFERENCE = re.compile(r"\{\s*([a-z][a-z0-9-]*)\s*\}")
    folders = (sorted(TREE.glob("crates/*/i18n")) + sorted(TREE.glob("crates/*/modules/*/i18n"))
               + [TREE / "extension" / "i18n"])
    owner, references = {}, {}
    for folder in folders:
        current = None
        for line in (folder / f"{BASE}.ftl").read_text().splitlines():
            if not line.strip() or line.startswith("#"):
                continue
            m = KEY.match(line)
            if m:
                current = m.group(1)
                owner[current] = str(folder.parent.relative_to(TREE))
                references[current] = set(REFERENCE.findall(line[m.end():]))
            elif line[0].isspace() and current is not None:
                references[current] |= set(REFERENCE.findall(line))
            else:
                current = None
    floor(len(folders), 60, "text fragments")
    floor(len(owner), 9000, "text keys")

    # 2) THE RUST THAT RUNS.
    def strings_of(text):
        # Every string literal, where it opens and what it holds — `code_of`'s
        # lexer, keeping what it empties: a raw string ends at its own hashes,
        # a character literal (`'"'`) is no quote, a comment is no code.
        out, i, n = [], 0, len(text)
        while i < n:
            c = text[i]
            if c in "rb" and not text[i - 1 : i].isalnum() and text[i - 1 : i] != "_":
                j = i + 1 + (c == "b" and text[i + 1 : i + 2] == "r")
                h = j
                while h < n and text[h] == "#":
                    h += 1
                if h < n and text[h] == '"' and (h > j or text[j - 1] == "r"):
                    close = '"' + "#" * (h - j)
                    end = text.find(close, h + 1)
                    end = n if end < 0 else end
                    out.append((i, text[h + 1 : end]))
                    i = end + len(close)
                    continue
            if c == '"':
                j, piece = i + 1, []
                while j < n and text[j] != '"':
                    piece.append(text[j : j + 2] if text[j] == "\\" else text[j])
                    j += 2 if text[j] == "\\" else 1
                out.append((i, "".join(piece)))
                i = j + 1
                continue
            if c == "'":
                m = re.match(r"'(?:\\u\{[0-9a-fA-F_]*\}|\\.|[^\\'\n])'", text[i:])
                if m:
                    i += m.end()
                    continue
            if text[i : i + 2] == "//":
                end = text.find("\n", i)
                i = n if end < 0 else end
                continue
            if text[i : i + 2] == "/*":
                depth, j = 1, i + 2
                while j < n and depth:
                    if text[j : j + 2] == "/*":
                        depth, j = depth + 1, j + 2
                    elif text[j : j + 2] == "*/":
                        depth, j = depth - 1, j + 2
                    else:
                        j += 1
                i = j
                continue
            i += 1
        return out

    def test_files(files):
        # The files of the out-of-line test modules (`#[cfg(test)] mod x;`)
        # and everything under them. A crate root, a `mod.rs` and a brain
        # module's file (read by a `#[path]`) keep their children beside them.
        declared = re.compile(r"#\[cfg\(test\)\]\s*((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;")
        found = set()
        for f in files:
            for m in declared.finditer(uncommented(f.read_text())):
                path = re.search(r'#\[path\s*=\s*"([^"]+)"\]', m.group(1))
                beside = (f.name in ("lib.rs", "main.rs", "mod.rs", "build.rs")
                          or f.parent.name == "bin" or f.parent.parent.name == "modules")
                here = f.parent if beside else f.parent / f.stem
                for c in ([f.parent / path.group(1)] if path else [here / (m.group(2) + ".rs"), here / m.group(2) / "mod.rs"]):
                    if c.is_file():
                        found.add(c)
                        below = c.parent if c.name == "mod.rs" else c.parent / c.stem
                        found |= set(below.rglob("*.rs")) if below.is_dir() else set()
        return found

    manifests = {}
    product, crate_of = {}, {}
    for manifest in sorted(TREE.glob("crates/*/Cargo.toml")) + sorted(TREE.glob("buffers/*/Cargo.toml")):
        crate = manifest.parent
        name = tomllib.loads(manifest.read_text())["package"]["name"]
        manifests[name] = tomllib.loads(manifest.read_text())
        files = rust_sources(crate) + [f for f in [crate / "build.rs"] if f.is_file()]
        tests = test_files(files)
        for f in files:
            if f not in tests:
                rel = str(f.relative_to(TREE))
                product[rel] = outside_tests(f.read_text())
                crate_of[rel] = name
    floor(len(manifests), 150, "crates and buffers")
    floor(len(product), 1000, "files of Rust that runs")

    def without_key_modules(text):
        # A generated `key` module defines every key of its module, and says none.
        while True:
            m = re.search(r"\bpub mod key\s*\{", text)
            if m is None:
                return text
            inner = must(scope(text[m.start():], r"\bpub mod key\b"), "a generated `key` module")
            text = text[: m.start()] + text[m.end() + len(inner) + 1 :]

    generated, owed_lists = {}, set()
    for env, path in sorted(INPUTS["generated"].items()):
        p = pathlib.Path(path)
        for f in sorted(p.rglob("*.rs")) if p.is_dir() else [p]:
            name = env + "/" + str(f.relative_to(p)) if p.is_dir() else env
            generated[name] = outside_tests(without_key_modules(f.read_text()))
        # THE KEYS A GENERATOR OWES for its table, beside it in its output.
        beside = (p if p.is_dir() else p.parent) / "keys.txt"
        if beside.is_file():
            owed_lists.add(beside)
    floor(len(generated), 30, "generated files of Rust")
    floor(len(owed_lists), 8, "generators' lists of owed keys")
    owed = {k for f in owed_lists for k in f.read_text().split()}
    floor(len(owed), 300, "keys the generators owe")

    # 3) WHAT IT NAMES: constants (by `key::`, an alias of it, or bare),
    # literals, and the literals of each crate.
    sources = {**product, **generated}
    NAMED = re.compile(r"(?:\b(\w+)\s*::\s*)?\b([A-Z][A-Z0-9_]*)\b")
    constants, literals = set(), set()
    crate_literals = collections.defaultdict(set)
    codes, spans = {}, {}
    for rel, text in sources.items():
        codes[rel] = code = code_of(text)
        through = {None, "key"} | set(re.findall(r"\bkey\s+as\s+(\w+)", code))
        constants |= {m.group(2) for m in NAMED.finditer(code) if m.group(1) in through}
        spans[rel] = strings_of(text)
        mine = {s for _, s in spans[rel]}
        literals |= mine
        crate_literals[crate_of.get(rel)] |= mine
    floor(len(constants), 5000, "constants named by the Rust")
    floor(len(literals), 20000, "string literals of the Rust")

    # THE DATA THE PROGRAMS READ AT RUN TIME: the brain's catalogue; the icon
    # tables' Nerd sets and Lucide categories, which the icon pages name
    # (`icons-set-<set>`, `icons-category-<category>`); and the reasons the
    # distro's app table declares a program essential, which the software
    # centre reads on the machine (`software-essential-why-<reason>`).
    def strings_in(value):
        if isinstance(value, dict):
            for v in value.values():
                yield from strings_in(v)
        elif isinstance(value, list):
            for v in value:
                yield from strings_in(v)
        elif isinstance(value, str):
            yield value
    catalogue = json.loads(pathlib.Path(INPUTS["catalogue"]).read_text())
    floor(len(catalogue["modules"]), 10, "modules in the brain's catalogue")
    catalogued = set(strings_in(catalogue))
    icons = pathlib.Path(INPUTS["iconset"])
    sets = {s for g in json.loads((icons / "glyphs.json").read_text()) for s in g.get("sets", [])}
    categories = {c for f in json.loads((icons / "lucide.json").read_text()) for c in f.get("cats", [])}
    floor(len(sets), 10, "sets of the Nerd glyphs")
    floor(len(categories), 20, "categories of the Lucide figures")
    essential = set(INPUTS["essential"])
    floor(len(essential), 1, "reasons a program is essential")
    # AN ID THE CODE OR ITS DATA HOLDS: a literal (or one lower-cased, as
    # `to_lowercase()` makes it), a string of the catalogue, a declared reason.
    known = literals | {l.lower() for l in literals} | catalogued | essential
    # BUT A SET'S AND A CATEGORY'S NAME COME FROM THE TABLES ALONE: what fills
    # those holes is their words, not something a literal happens to spell.
    FROM_THE_TABLES = {"icons-set-": sets, "icons-category-": categories}

    # WHAT EACH CRATE REACHES: its own literals and those of every crate it
    # stands on, for a template of several holes.
    def stands_on(name, seen):
        for table in ("dependencies", "build-dependencies"):
            for dep, spec in manifests[name].get(table, {}).items():
                dep = spec.get("package", dep) if isinstance(spec, dict) else dep
                if dep in manifests and dep not in seen:
                    seen.add(dep)
                    stands_on(dep, seen)
        return seen
    reach = {}
    def reached(name):
        if name not in reach:
            reach[name] = set().union(*(crate_literals[c] for c in stands_on(name, {name})))
        return reach[name]

    # 4) THE TEMPLATES: a key-shaped head and holes (`"about-{id}"`), or a
    # bare prefix that composes (`"sidebar-"`).
    #
    # A BARE PREFIX COMPOSES unless all it does is take a word apart: one
    # handed to `strip_prefix` or `starts_with` names no key. A constant that
    # holds one composes when its crate uses it some other way — in a format
    # string's `{NAME}`, handed to a helper, added to a word.
    SHAPE = re.compile(r"[a-z][a-z0-9-]*-(?:\{[^{}]*\}[a-z0-9-]*)*")
    PREFIX = re.compile(r"[a-z][a-z0-9-]*-")
    APART = re.compile(r"\.(?:strip_prefix|strip_suffix|starts_with|ends_with|trim_start_matches"
                       r"|trim_end_matches|contains|find|rfind|split_once|rsplit_once|split)\(\s*&?\s*b?$")
    DEFINED = re.compile(r"\b(?:const|static)\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:'static\s+)?str\s*=\s*$")
    def put_to_use(name, crate):
        for rel, code in codes.items():
            if crate_of.get(rel) != crate:
                continue
            for m in re.finditer(r"\b" + name + r"\b", code):
                before = code[max(0, m.start() - 80) : m.start()]
                if not re.search(r"\b(?:const|static)\s+$", before) and not APART.search(before):
                    return True
            if any("{" + name + "}" in s or "{" + name + ":" in s for _, s in spans[rel]):
                return True
        return False
    templates, apart = {}, 0
    for rel, text in sources.items():
        for start, s in spans[rel]:
            if "{" in s and SHAPE.fullmatch(s):
                templates.setdefault(s, set()).add(crate_of.get(rel))
            elif PREFIX.fullmatch(s):
                before = text[max(0, start - 80) : start]
                defined = DEFINED.search(before)
                if APART.search(before) or (defined and not put_to_use(defined.group(1), crate_of.get(rel))):
                    apart += 1
                    continue
                templates.setdefault(s + "{}", set()).add(crate_of.get(rel))
    floor(apart, 5, "bare prefixes that only take a word apart")
    # A BRAIN MODULE'S TABLE READS ITS OWN TEXTS BY A PREFIX: the prefix its
    # `tables.nix` hands the generator, every key under it one the table holds
    # by its value — each word the module's code knows by a literal.
    tables = sorted(TREE.glob("crates/*/modules/*/tables.nix"))
    floor(len(tables), 2, "brain modules' tables")
    by_value = 0
    for t in tables:
        text = hashless(t.read_text())
        if "--prefix" not in text:
            continue
        for p in re.findall(r'prefix\s*=\s*"([^"]+)"', text):
            p = p.replace("''${baseNameOf ./.}", t.parent.name)
            if SHAPE.fullmatch(p):
                templates.setdefault(p + "{}", set()).add(None)
                by_value += 1
    floor(by_value, 1, "prefixes a brain module's table reads its texts by")
    floor(len(templates), 200, "templates that compose a key")

    def composes(template, crates, key):
        out, at = "", 0
        holes = list(re.finditer(r"\{[^{}]*\}", template))
        for h in holes:
            out += re.escape(template[at : h.start()]) + "([a-z0-9][a-z0-9-]*)"
            at = h.end()
        m = re.fullmatch(out + re.escape(template[at:]), key)
        if m is None:
            return False
        if len(holes) == 1:
            return m.group(1) in FROM_THE_TABLES.get(template[: holes[0].start()], known)
        return any(c is not None and all(g in reached(c) for g in m.groups()) for c in crates)

    # The templates by their head, so a key asks only those that can be its.
    heads = collections.defaultdict(list)
    for t, crates in templates.items():
        heads[t.split("{", 1)[0]].append((t, crates))

    # THE EXTENSION'S SCRIPTS: `TEXT.extQueueUp` says `ext-queue-up`.
    scripts = sorted(TREE.glob("extension/*.js"))
    floor(len(scripts), 3, "extension scripts")
    camel = set(re.findall(r"\bTEXT\.([A-Za-z0-9]+)", "".join(code_of(f.read_text()) for f in scripts)))
    floor(len(camel), 10, "texts the extension's scripts say")
    def camel_of(key):
        first, *rest = key.split("-")
        return first + "".join(w[:1].upper() + w[1:] for w in rest)

    # 5) EVERY KEY, BY ITS FIRST ROUTE.
    for prefix, (rel, needle, _) in sorted(OUTSIDE.items()):
        text = uncommented(product.get(rel, ""))
        if not re.search(needle, text):
            wrong.append(f"`{prefix}…` is on OUTSIDE and {rel} no longer composes it — take it off the list")
    said = {}
    for key in owner:
        if key.upper().replace("-", "_") in constants:
            said[key] = "constant"
        elif key in literals:
            said[key] = "literal"
        elif key in catalogued:
            said[key] = "catalogue"
        elif key in owed:
            said[key] = "owed"
        elif owner[key] == "extension" and camel_of(key) in camel:
            said[key] = "extension"
        elif any(key.startswith(p) for p in OUTSIDE):
            said[key] = "outside"
        else:
            for at in range(len(key)):
                if key[at] == "-" and any(composes(t, c, key) for t, c in heads.get(key[: at + 1], ())):
                    said[key] = "template"
                    break
    # A MESSAGE THAT IS SAID SAYS WHAT IT REFERENCES.
    grown = True
    while grown:
        grown = False
        for key in list(said):
            for r in references[key]:
                if r in owner and r not in said:
                    said[r] = "reference"
                    grown = True
    routes = collections.Counter(said.values())
    for route in ("constant", "literal", "template", "catalogue", "owed", "extension", "outside"):
        floor(routes[route], 1, f"keys said through the route `{route}`")

    for key, why in sorted(UNSAID.items()):
        if key not in owner:
            wrong.append(f"`{key}` is on UNSAID and is no key of any fragment — take it off the list")
        elif key in said:
            wrong.append(f"`{key}` is on UNSAID and is said ({said[key]}) — take it off the list")
    if list(UNSAID) != sorted(UNSAID):
        wrong.append("UNSAID is not sorted")
    unsaid = sorted((owner[k], k) for k in owner if k not in said and k not in UNSAID)
    for where, key in unsaid:
        wrong.append(f"{where}: `{key}` is said by nothing that runs — no constant, literal, "
                     "template, table, catalogue or reference names it; delete it from every "
                     "language, or say it")
    for w in wrong:
        print("ERROR: " + w, file=sys.stderr)
    if wrong:
        raise SystemExit(1)
    print(f"OK: the {len(owner)} keys are said — " + ", ".join(f"{n} by {r}" for r, n in routes.most_common())
          + f"; {len(UNSAID)} a debt")
    END
  '';
}
