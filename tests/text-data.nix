# THE TEXTS AS DATA — the 2026-10-12 wave, lane D2.
#
# The sentences left the compilation: each crate that speaks is compiled
# against the key NAMES of its own fragment, written at evaluation
# (`nix/text-keys.nix`, `texts.keys`, one file per crate), and the values are
# one merged `<lang>.ftl` per language (`nix/i18n.nix`), which every program
# that speaks reads at run time through `RONOCTUA_TEXTS` (`ronoctua_i18n`).
# These checks hold each part of that:
#
# - `texts-are-data`: no Rust and no WASM derivation is built from a `.ftl`, a
#   sentence, the texts' derivation or their generator — read off the build
#   graph of each, which is what Nix rebuilds by, and recognised by content;
#   and each crate's keys come out of evaluation, unmoved by a value and moved
#   by a name, and a unit is handed its own crate's alone;
# - `every-key-exists-at-run-time`: every key a binary was compiled with is in
#   every language of the texts it is pointed at (`check-texts`), every program
#   that speaks refuses — one line naming the variable, exit 78 — texts that
#   are missing or another build's, and every wrapper carries the build's own.
{
  pkgs,
  lib,
  mkCheck,
  pyPrelude,
  rust,
  buffers,
  texts,
  languages,
  brainModules,
}:

let
  # A DERIVATION, as the `.drv` file that says what it is BUILT FROM — read,
  # never built (`tests/theme-data.nix` says why `exportReferencesGraph` will
  # not do).
  graphOf = drv: builtins.unsafeDiscardOutputDependency drv.drvPath;
  derivationsIn = set: lib.filterAttrs (_: lib.isDerivation) set;

  base = builtins.head languages;
  inherit (texts) textKeys;
  # EVERY CRATE'S FRAGMENT (and the extension's), as the texts read them; the
  # foundation's is the one the plants carry.
  crateFileOf = name: language: "${texts.fragments.${name}}/${language}.ftl";
  crateFile = crateFileOf "ronoctua-shell-kit" base;
  fragments = brainModules.i18nFragments;
  fragmentFileOf = f: language: "${f.dir}/${language}.ftl";
  readName = file: builtins.unsafeDiscardStringContext (builtins.readFile file);

  # SENTENCES THAT CAN ONLY BE A COPY OF THE TEXTS: the longest plain values
  # (48 to 100 characters, no placeable, no selector) of every crate's
  # fragment, and a few of every module's. Short enough that the old registry's doc
  # comments — cut at 110 characters — carried them whole; long enough that
  # nothing writes one by chance. Chosen here, once, so the plants below and
  # the detector agree on what a sentence is.
  valueOf =
    line:
    let
      m = builtins.match "[a-z][a-z0-9-]*[[:space:]]*=[[:space:]]*(.*)" line;
    in
    if m == null then null else builtins.head m;
  plain =
    v:
    v != null
    && builtins.stringLength v >= 48
    && builtins.stringLength v <= 100
    && !(builtins.any (c: lib.hasInfix c v) [
      "{"
      "}"
      "["
      "]"
      "*"
      "\\"
    ]);
  longestFirst =
    a: b:
    let
      la = builtins.stringLength a;
      lb = builtins.stringLength b;
    in
    la > lb || (la == lb && a < b);
  sentencesOf =
    n: file:
    lib.take n (
      lib.sort longestFirst (
        lib.unique (lib.filter plain (map valueOf (lib.splitString "\n" (readName file))))
      )
    );
  sentences = lib.unique (
    lib.concatMap (name: lib.concatMap (l: sentencesOf 2 (crateFileOf name l)) languages) (
      lib.attrNames texts.fragments
    )
    ++ lib.concatMap (f: lib.concatMap (l: sentencesOf 3 (fragmentFileOf f l)) languages) fragments
  );
  # The foundation's longest in the base language: the one every plant carries.
  sentence = builtins.head (sentencesOf 1 crateFile);
in
{
  # NO RUST AND NO WASM IS BUILT FROM A SENTENCE, AND THE REGISTRY IS NAMES.
  #
  # A store path in a derivation's inputs is a reason to build it again, and
  # so is a string in its attributes: "does editing a sentence rebuild the
  # workspace?" has an exact answer in the build graph. Asked of every
  # release and loop program (each one's graph holds every library and
  # third-party unit it links), the test graph's harnesses, clippy and
  # rustfmt, and every buffer. A test RUN reads the texts as a run input and
  # is not asked: what it runs is a harness, asked here, and only the run
  # moves with a sentence (`text-values-rebuild-no-rust`).
  #
  # BY CONTENT, AND NOT BY THE NAMES A ROUTE GIVES THEM:
  #
  # - `texts`: the texts' derivation, or another run of their generator (its
  #   `pname`), anywhere in the graph;
  # - `file`: a source whose bytes, or the bytes of a file inside a source
  #   directory, are a crate's `.ftl`, a module's fragment or the generator;
  # - `value`: the whole text of one of those `.ftl` files, in a derivation's
  #   strings (structured attributes decoded) or in the text of any source;
  # - `sentence`: one of the chosen sentences in a derivation's strings or in a
  #   file WRITTEN AT EVALUATION (`builtins.toFile`, as the registry is) —
  #   which is how a value in a doc comment would come back. A file of the
  #   repository is not asked: a test may compare a drawing with its English
  #   sentence, and editing the `.ftl` does not change that file.
  #
  # ONE ROUTE IS LEFT, AND IT IS NAMED: the reminders module's grammar
  # (`modules/reminders/tables.nix`, the `ronoctua-brain-words-rs`
  # derivation), the words a reminder's answer is read with, compiled from its
  # own `reminders-grammar-*` texts with `gen-i18n.py`'s reader — so the words
  # the build checks in every language and the words the brain parses are one
  # list. A module's fragment or the generator found inside that derivation's
  # graph is counted and said, not refused; a crate's `.ftl` there is refused
  # like anywhere else. The brain's MANIFESTS, the route this used to name,
  # are handed the key NAMES now, read at evaluation (`nix/text-names.nix`),
  # and are asked like everything else. The exception expires by itself: the
  # day that graph reads no text, this check fails until the exception goes.
  #
  # AND IT IS NOT BLIND: each route is planted in a derivation that is never
  # built and must be found by its own detector, and the shell's wrapper,
  # which does carry the texts, must be found by the first.
  #
  # THE KEYS, asked at evaluation: every crate's file is what
  # `nix/text-keys.nix` writes from the names of its own fragment; the
  # devshell's `cargo` is handed the directory of them all (`generated`), and
  # every unit the directory of its own crate's file alone (read here, in
  # the build: exactly one file, those bytes); a value edited leaves the names
  # (and so the files) as they were, a key added moves them; and the names are
  # exactly the messages of every merged file the programs read.
  texts-are-data =
    let
      compiled = {
        # Never built: only its `.drv`, whose inputs are every harness's.
        test-harnesses = pkgs.linkFarm "test-harnesses" (
          lib.mapAttrsToList (name: path: { inherit name path; }) rust.test.testBins
        );
        inherit (rust) clippy fmt;
      }
      // lib.mapAttrs' (n: v: lib.nameValuePair "release-${n}" v) rust.release.bins
      // lib.mapAttrs' (n: v: lib.nameValuePair "loop-${n}" v) rust.loop.bins
      // lib.mapAttrs' (n: v: lib.nameValuePair "buffer-${n}" v) (derivationsIn buffers);

      fragment = builtins.head fragments;
      plant = detector: drv: {
        inherit detector;
        drv = graphOf drv;
      };
      planted = {
        ftl-as-a-file = plant "file" (pkgs.runCommand "planted-ftl" { } "cat ${crateFile} > $out");
        ftl-in-a-directory = plant "file" (
          pkgs.runCommand "planted-ftl-dir" { } "ls ${texts.fragments.ronoctua-shell-kit} > $out"
        );
        fragment-as-a-file = plant "file" (
          pkgs.runCommand "planted-fragment" { } "cat ${fragmentFileOf fragment base} > $out"
        );
        generator-renamed = plant "file" (
          pkgs.runCommand "planted-generator" { }
            "cat ${
              builtins.path {
                path = ../nix/scripts/gen-i18n.py;
                name = "planted.py";
              }
            } > $out"
        );
        ftl-as-text = plant "value" (pkgs.writeText "planted-ftl-text" (readName crateFile));
        fragment-as-text = plant "value" (
          pkgs.writeText "planted-fragment-text" (readName (fragmentFileOf fragment base))
        );
        sentence-in-code = plant "sentence" (
          pkgs.writeText "planted.rs" "/// `${base}`: ${sentence}\npub const PLANTED: &str = \"planted\";\n"
        );
        sentence-in-structured-attributes = plant "sentence" (
          pkgs.runCommand "planted-structured" {
            __structuredAttrs = true;
            doc = sentence;
          } "touch $out"
        );
        sentence-in-a-written-file = plant "sentence" (
          pkgs.runCommand "planted-written" { } "cp ${builtins.toFile "planted.rs" "/// ${sentence}\n"} $out"
        );
        texts-copied = plant "texts" (pkgs.runCommand "planted-texts" { } "cp ${texts}/${base}.ftl $out");
        texts-again = plant "texts" (texts.override { languages = lib.reverseList languages; });
        # A MODULE'S FRAGMENT WHERE THE MANIFESTS ARE MADE: the route the
        # brain's manifests used to be, and are no longer excused.
        fragment-beside-the-manifests = plant "file" (
          pkgs.runCommand "ronoctua-brain-manifests-rs-planted" { }
            "cat ${fragmentFileOf fragment base} > $out"
        );
        # A CRATE'S FILE INSIDE THE NAMED ROUTE: the exception covers a
        # module's own fragment and the generator, never a crate's texts.
        crate-beside-the-grammar = plant "file" (
          pkgs.runCommand "ronoctua-brain-words-rs-planted" { } "cat ${crateFile} > $out"
        );
      };

      # THE TEXT DATA, as files, each with its kind.
      dataFiles =
        lib.listToAttrs (
          lib.concatMap (
            name:
            map (
              l:
              lib.nameValuePair "the ${name} fragment's ${l}.ftl" {
                kind = "crate";
                path = crateFileOf name l;
              }
            ) languages
          ) (lib.attrNames texts.fragments)
        )
        // lib.listToAttrs (
          lib.concatMap (
            f:
            map (
              l:
              lib.nameValuePair "the ${f.id} module's ${l}.ftl" {
                kind = "module";
                path = fragmentFileOf f l;
              }
            ) languages
          ) fragments
        )
        // {
          "nix/scripts/gen-i18n.py" = {
            kind = "generator";
            path = "${../nix/scripts/gen-i18n.py}";
          };
        };

      # THE KEYS, asked at evaluation (see the last paragraph above).
      crateText = readName crateFile;
      valueEdited = builtins.replaceStrings [ " = ${sentence}" ] [ " = edited" ] crateText;
      kit = textKeys.crates.ronoctua-shell-kit;
      # EVERY LIBRARY UNIT OF A CRATE THAT SPEAKS, in each flavour, and the
      # directory it is handed (kept as a build input: read in the build).
      unitDirs =
        lib.concatMap
          (
            flavour:
            lib.concatMap (
              name:
              # A crate with a library is asked of its library; a program alone
              # (the shell's binary, the PNG camera) of its program.
              if rust.${flavour}.libs ? ${name} then
                [
                  {
                    unit = "${flavour}/libs/${name}";
                    crate = name;
                    dir = rust.${flavour}.libs.${name}.unit.env.RONOCTUA_KEYS_DIR or "";
                  }
                ]
              else
                lib.optional (rust.${flavour}.bins ? ${name}) {
                  unit = "${flavour}/bins/${name}";
                  crate = name;
                  dir = rust.${flavour}.bins.${name}.unit.env.RONOCTUA_KEYS_DIR or "";
                }
            ) (lib.attrNames textKeys.crates)
          )
          [
            "release"
            "loop"
          ];
      registry = {
        built = textKeys.files;
        fromNames = lib.mapAttrs (
          name: c:
          textKeys.render {
            inherit name;
            inherit (c) dir names uses;
          }
        ) textKeys.crates;
        compiledReads =
          builtins.unsafeDiscardStringContext rust.generated.RONOCTUA_KEYS_DIR
          == builtins.unsafeDiscardStringContext "${texts.keys}";
        units = unitDirs;
        edited = valueEdited != crateText;
        sameNamesAfterAValue = textKeys.namesIn valueEdited == textKeys.namesIn crateText;
        newName = lib.elem "planted-new-key" (textKeys.namesIn (crateText + "\nplanted-new-key = New\n"));
        # By `replaceStrings` and not `lib.hasInfix`, which is a regular
        # expression over a whole file and overflows the evaluator's stack.
        movesWithAName =
          let
            more = textKeys.render {
              name = "ronoctua-shell-kit";
              inherit (kit) dir uses;
              names = kit.names ++ [ "planted-new-key" ];
            };
            needle = "pub const PLANTED_NEW_KEY: &str = \"planted-new-key\";";
          in
          builtins.replaceStrings [ needle ] [ "" ] more != more;
        names = lib.sort (a: b: a < b) (lib.unique texts.names);
      };

      facts = pkgs.writeText "text-data-routes.json" (
        builtins.toJSON {
          roots = lib.mapAttrs (_: graphOf) compiled;
          wrapper = graphOf rust.shell;
          texts = graphOf texts;
          inherit
            planted
            sentences
            registry
            languages
            ;
          files = dataFiles;
          merged = "${texts}";
        }
      );
    in
    mkCheck "texts-are-data" [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import hashlib, json, os, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: a `.drv` is a derivation written down, in ATerm —
      # `Derive([outputs],[input derivations],[input sources],system,builder,
      # [args],[env])` — and is read by that grammar: strings with their
      # escapes undone, lists and tuples. Walked from each root, the walk is
      # the build graph; every `.drv` and source it reaches is in this sandbox,
      # because the store's references of a `.drv` are exactly those. The
      # registry and the routes arrive as JSON written at evaluation; the
      # merged files are read whole, as text.
      # FILE-WIDE: a datum is a datum wherever in a file or a string it sits.
      facts = json.load(open(sys.argv[1]))
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

      def strings_in(value):
          if isinstance(value, str):
              yield value
          elif isinstance(value, list):
              for v in value:
                  yield from strings_in(v)
          elif isinstance(value, dict):
              for k, v in value.items():
                  yield k
                  yield from strings_in(v)

      def derivation(path):
          fields = aterm(open(path, errors="surrogateescape").read())
          if isinstance(fields[0], str):
              fields = fields[1:]  # `DrvWithVersion("…", …)`
          _outputs, inputs, sources, _system, builder, args, env = fields
          strings = [builder, *args]
          for name, value in env:
              strings.append(value)
              if name == "__json":  # structured attributes: JSON inside the string
                  strings += list(strings_in(json.loads(value)))
          return [i[0] for i in inputs], sources, strings

      # WHAT EACH DETECTOR LOOKS FOR.
      files = facts["files"]
      kind_of = {label: f["kind"] for label, f in files.items()}
      by_size = {}
      for label, f in sorted(files.items()):
          raw = open(f["path"], "rb").read()
          by_size.setdefault(len(raw), {})[hashlib.sha256(raw).hexdigest()] = label
      whole = {label: open(f["path"], errors="surrogateescape").read()
               for label, f in files.items() if f["kind"] != "generator"}
      sentences = facts["sentences"]
      floor(sum(1 for k in kind_of.values() if k == "crate"), 30 * len(facts["languages"]), "crate fragments' .ftl files")
      floor(sum(1 for k in kind_of.values() if k == "module"), 2 * len(facts["languages"]), "module fragments")
      floor(len(sentences), 40, "chosen sentences")
      floor(min(map(len, whole.values())), 64, "characters in the shortest .ftl")
      TEXTS = re.compile(r"/nix/store/[0-9a-z]{32}-ronoctua-texts-[^/]*\.drv$")
      ROUTE = re.compile(r"/nix/store/[0-9a-z]{32}-ronoctua-brain-words-rs[^/]*\.drv$")

      def values_in(text):
          return [label for label, t in whole.items() if len(text) >= len(t) and t in text]

      def sentences_in(text):
          return [f"the sentence {s!r}" for s in sentences if s in text][:1]

      # A FILE WRITTEN AT EVALUATION is text-addressed: its store path is
      # Nix's base-32 of the SHA-256 of `text:<references>:sha256:<hex of the
      # contents>:/nix/store:<name>`, folded to 20 bytes. A copied file's is
      # the hash of its archive instead, and never comes out of this.
      ALPHABET = "0123456789abcdfghijklmnpqrsvwxyz"
      STORE_PATH = re.compile(rb"/nix/store/[0-9a-z]{32}-[0-9a-zA-Z+._?=-]+")

      def nix32(digest):
          out = []
          for i in range((len(digest) * 8 - 1) // 5, -1, -1):
              j, k = divmod(i * 5, 8)
              c = digest[j] >> k
              if j + 1 < len(digest):
                  c |= digest[j + 1] << (8 - k)
              out.append(ALPHABET[c & 0x1F])
          return "".join(out)

      def text_path(name, raw, references):
          kind = "text" + "".join(":" + r for r in sorted(references))
          fingerprint = f"{kind}:sha256:{hashlib.sha256(raw).hexdigest()}:/nix/store:{name}"
          folded = bytearray(20)
          for i, b in enumerate(hashlib.sha256(fingerprint.encode()).digest()):
              folded[i % 20] ^= b
          return f"/nix/store/{nix32(bytes(folded))}-{name}"

      def written_at_evaluation(path, raw):
          name = os.path.basename(path)[33:]
          cited = {m.decode() for m in STORE_PATH.findall(raw)}
          return path in (text_path(name, raw, cited), text_path(name, raw, ()))

      drvs, sources = {}, {}

      def found_in_drv(path):
          if path not in drvs:
              inputs, srcs, strings = derivation(path)
              found = []
              if path == facts["texts"] or TEXTS.search(path):
                  found.append(("texts", "the texts' derivation", path))
              for s in strings:
                  found += [("value", v, path) for v in values_in(s)]
                  found += [("sentence", v, path) for v in sentences_in(s)]
              drvs[path] = (inputs, srcs, found)
          return drvs[path]

      def found_in_source(path):
          if path not in sources:
              if os.path.isdir(path):
                  inside = [os.path.join(d, f) for d, _, fs in os.walk(path) for f in fs]
              else:
                  inside = [path]
              found = []
              for f in inside:
                  if os.path.islink(f) or not os.path.isfile(f):
                      continue
                  size = os.path.getsize(f)
                  if size in by_size:
                      label = by_size[size].get(hashlib.sha256(open(f, "rb").read()).hexdigest())
                      if label is not None:
                          found.append(("file", label, f))
                  with open(f, "rb") as h:
                      if b"\0" in h.read(8192):
                          continue  # not a text: only its bytes can be a datum
                  raw = open(f, "rb").read()
                  text = raw.decode("utf-8", "surrogateescape")
                  found += [("value", v, f) for v in values_in(text)]
                  if f == path and written_at_evaluation(path, raw):
                      found += [("sentence", v, f) for v in sentences_in(text)]
              sources[path] = found
          return sources[path]

      def collect(root, stop):
          # The build graph from `root`: hits in its `.drv`s and sources, and
          # the named route's `.drv`s met on the way (not entered when `stop`).
          seen, todo, srcs, hits, routes = set(), [root], set(), [], []
          while todo:
              path = todo.pop()
              if path in seen:
                  continue
              seen.add(path)
              if stop and ROUTE.search(path):
                  routes.append(path)
                  continue
              inputs, handed, found = found_in_drv(path)
              hits += found
              todo += inputs
              srcs.update(handed)
          for s in sorted(srcs):
              hits += found_in_source(s)
          return len(seen) + len(srcs), hits, routes

      def walk(root):
          # Every hit, with the named route it was found inside (or None).
          size, hits, routes = collect(root, True)
          marked = [(d, w, where, None) for d, w, where in hits]
          for r in routes:
              more, inside, _ = collect(r, False)
              size += more
              marked += [(d, w, where, r) for d, w, where in inside]
          return size, marked

      def tolerated(hit):
          detector, what, _, inside = hit
          return (inside is not None and detector in ("file", "value")
                  and kind_of.get(what) in ("module", "generator"))

      # THE CONTROLS FIRST: a census that finds nothing where something was
      # put is not reading.
      floor(len(facts["planted"]), 13, "routes planted")
      blind = []
      for route, p in sorted(facts["planted"].items()):
          _, hits = walk(p["drv"])
          if not any(h[0] == p["detector"] and not tolerated(h) for h in hits):
              blind.append(f"the planted route `{route}` was not refused by the `{p['detector']}` detector")
      _, hits = walk(facts["wrapper"])
      if not any(h[0] == "texts" for h in hits):
          blind.append("the shell's wrapper carries the texts, and was not found built from them")
      if blind:
          print("ERROR: the census of the build graphs is blind:", file=sys.stderr)
          for b in blind:
              print("       " + b, file=sys.stderr)
          sys.exit(1)

      SAYS = {
          "texts": "is built from",
          "file": "is handed the file",
          "value": "holds",
          "sentence": "holds",
      }
      roots = facts["roots"]
      floor(len(roots), 60, "compiled derivations asked")
      wrong, sizes, excepted = [], [], set()
      for name, root in sorted(roots.items()):
          size, hits = walk(root)
          sizes.append(size)
          said = {}
          for hit in hits:
              if tolerated(hit):
                  excepted.add(hit[1])
                  continue
              detector, what, where, inside = hit
              said.setdefault((detector, what), where if inside is None else f"{where} (inside {inside})")
          for (detector, what), where in sorted(said.items())[:6]:
              wrong.append(f"{name} {SAYS[detector]} {what}: {where}")
      floor(min(sizes), 50, "paths in the smallest build graph")
      if not excepted:
          wrong.append("the reminders' grammar reads no module fragment and no generator any more: "
                       "the exception for it (`tolerated`) now excuses nothing — take it out")

      # THE KEYS.
      reg = facts["registry"]
      floor(len(reg["built"]), 30, "crates that speak")
      if reg["built"] != reg["fromNames"]:
          wrong.append("a crate's keys are not what nix/text-keys.nix writes from its fragment's names")
      if not reg["compiledReads"]:
          wrong.append("the devshell's cargo does not compile against texts.keys (RONOCTUA_KEYS_DIR of `generated`)")
      units = reg["units"]
      floor(len(units), 2 * len(reg["built"]) - 4, "units of crates that speak")
      for u in units:
          d = u["dir"]
          held = sorted(os.listdir(d)) if d and os.path.isdir(d) else None
          if held != [u["crate"] + ".rs"]:
              wrong.append(f"{u['unit']} is handed {d!r} holding {held}: a unit sees its own crate's keys alone")
          elif open(os.path.join(d, u["crate"] + ".rs"), encoding="utf-8").read() != reg["built"][u["crate"]]:
              wrong.append(f"{u['unit']} is handed keys that are not its crate's file")
      if not reg["edited"]:
          wrong.append("the planted value edit changed nothing: the control is not editing")
      if not reg["sameNamesAfterAValue"]:
          wrong.append("a value edited moved the names, so a sentence would rebuild the crates that say it")
      if not reg["newName"] or not reg["movesWithAName"]:
          wrong.append("a key added did not reach the names or the crate's keys")
      every_file = "\n".join(reg["built"].values())
      leaked = [s for s in sentences if s in every_file]
      if leaked:
          wrong.append(f"the crates' keys carry {len(leaked)} sentences, e.g. {leaked[0]!r}")
      names = reg["names"]
      floor(len(names), 1000, "key names read at evaluation")
      KEY = re.compile(r"^([a-z][a-z0-9-]*)\s*=", re.M)
      for language in facts["languages"]:
          text = hashless(open(os.path.join(facts["merged"], language + ".ftl"), encoding="utf-8").read())
          ids = sorted(set(KEY.findall(text)))
          if ids != names:
              extra = sorted(set(ids) - set(names))[:3]
              missing = sorted(set(names) - set(ids))[:3]
              wrong.append(f"the merged {language}.ftl and the names read at evaluation differ "
                           f"(only in the file: {extra}; only in the names: {missing})")

      if wrong:
          print("ERROR: a compilation is built from the texts, or a crate's keys are not their names alone.", file=sys.stderr)
          print("", file=sys.stderr)
          for w in wrong:
              print("       " + w, file=sys.stderr)
          print("", file=sys.stderr)
          print("       Each crate is compiled against the key NAMES of its own fragment", file=sys.stderr)
          print("       (nix/text-keys.nix); the sentences are the merged .ftl files, read at", file=sys.stderr)
          print("       run time through RONOCTUA_TEXTS. Point a binary at the data instead.", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(roots)} compiled derivations ({sum(sizes)} graph entries) are built from no .ftl, "
            f"sentence, texts derivation or generator, by content; {len(facts['planted'])} planted routes "
            f"and the wrapper are found; {len(reg['built'])} crates' keys are {len(names)} names, unmoved "
            f"by a value, and {len(units)} units each see their own crate's alone; "
            f"{len(excepted)} module fragments and the generator still reach the reminders' grammar, "
            f"the one route left")
      END
    '';

  # EVERY KEY A BINARY WAS COMPILED WITH IS IN THE TEXTS IT READS — OR IT SAYS SO.
  #
  # Five answers, each of a built binary:
  #
  # - `ronoctua-shell check-texts` against the build's own texts: every key of
  #   every crate it links that speaks and of every distro brain module, in
  #   every language — and every one of them a message of the merged files,
  #   which hold the other programs' (the installer's, the rescue console's)
  #   and the extension's besides;
  # - against a copy whose SECOND language lacks one key: refused, exit 78,
  #   naming the key, the file and `RONOCTUA_TEXTS` — every language is asked,
  #   not only the one the process speaks;
  # - against a copy lacking one key of a brain module: refused the same way,
  #   by `check-texts` and by the PNG camera's guard — a module's words are
  #   said through its manifest, and are the program's keys as much as a
  #   crate's;
  # - with no variable: refused, exit 78, said once;
  # - the start guard of every program that speaks (the footer's binary, the
  #   PNG camera, the installer, the rescue console), with no texts and with
  #   another build's — a copy lacking one key in EVERY language, so whichever
  #   one a program speaks is short: one line naming the variable, exit 78,
  #   before anything else;
  # - every wrapper carries the build's texts, `--set` (a session's older
  #   directory loses to it): each of the four — the footer and the rescue
  #   console in the shell's bundle, the ISO's installer and the extracted
  #   rescue console (the PNG camera is a driver, run unwrapped, and the
  #   bundle holds only what the footer finds beside itself,
  #   `nix/rust/outputs.nix`) —, run with no texts in its environment and
  #   with that copy, passes its start guard and does what the same binary
  #   does from the build's own texts — so a wrapper that lost the variable,
  #   or hands it `--set-default`, meets the guard's 78 — and the player's
  #   daemon, which says no word, carries none, alone or in the bundle.
  every-key-exists-at-run-time =
    mkCheck "every-key-exists-at-run-time"
      [
        pkgs.python3
        pkgs.gnugrep
        pkgs.coreutils
        pkgs.diffutils
      ]
      ''
        # The palettes (`rust.keyed`): every binary below paints as well, and
        # reads them before it reads a word.
        export RONOCTUA_THEMES=${rust.keyed.RONOCTUA_THEMES}
        shell=${rust.bins.ronoctua-shell}/bin/ronoctua-shell
        base=${base}
        last=${lib.last languages}

        # THE BUILD'S OWN TEXTS.
        RONOCTUA_TEXTS=${texts} "$shell" check-texts > ok.txt
        cat ok.txt
        python3 - ok.txt ${texts} ${lib.escapeShellArgs languages} <<'END'
        import pathlib, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the verdict line `check-texts` prints, and the message ids of
        # each merged file by Fluent's shape (a key in column 0), comments off.
        # FILE-WIDE: the question is about every message of each whole file.
        said = pathlib.Path(sys.argv[1]).read_text()
        m = re.search(r"texts OK: (\d+) keys of (\d+) crates and (\d+) brain modules, each in (\d+) languages", said)
        if not m:
            sys.exit(f"ERROR: check-texts did not say its verdict: {said!r}")
        keys, crates, modules, languages = (int(g) for g in m.groups())
        floor(keys, 1000, "keys the binary was compiled with")
        floor(crates, 30, "crates that speak in the binary")
        floor(modules, 10, "distro brain modules whose keys the binary asks for")
        if languages != len(sys.argv[3:]):
            sys.exit(f"ERROR: check-texts asked {languages} languages, and {len(sys.argv[3:])} are declared")
        for language in sys.argv[3:]:
            text = hashless((pathlib.Path(sys.argv[2]) / f"{language}.ftl").read_text())
            ids = set(re.findall(r"^([a-z][a-z0-9-]*)\s*=", text, re.M))
            if len(ids) < keys:
                sys.exit(f"ERROR: the merged {language}.ftl has {len(ids)} messages and the binary knows "
                         f"{keys} keys: a key the texts do not hold")
        print(f"OK: {keys} keys of {crates} crates and {modules} brain modules, all in each of {languages} languages")
        END

        # ANOTHER BUILD'S TEXTS: a copy of the build's (`copied`) from which
        # `take_one` takes one key out of a language — the first message under
        # a section whose header begins with its second word, whose value is
        # all on its line, so the rest still parses and the refusal can only be
        # the missing key. One of the TEXTS' OWN CRATE (`ronoctua-i18n`),
        # which every program that speaks links: whichever program reads the
        # copy, the key is one it was compiled with.
        copied() {
          rm -rf other
          mkdir other
          cp ${texts}/*.ftl other/
          chmod u+w other/*.ftl
        }
        take_one() {
          python3 - "other/$1.ftl" "$2" <<'END'
        import re, sys
        lines = open(sys.argv[1], encoding="utf-8").read().split("\n")
        heads = [i for i, line in enumerate(lines) if line.startswith(sys.argv[2])]
        start = heads[0] if heads else len(lines)
        for i, line in enumerate(lines[:-1]):
            m = re.match(r"([a-z][a-z0-9-]*) *= *\S", line)
            if i > start and m and not lines[i + 1][:1].isspace():
                break
        else:
            sys.exit(f"ERROR: no message under `{sys.argv[2]}` with its value on one line to take out")
        open(sys.argv[1], "w", encoding="utf-8").write("\n".join(lines[:i] + lines[i + 1 :]))
        print(m.group(1))
        END
        }
        crate_of_texts="# ── ronoctua-i18n ──"
        a_module="# ── brain module: "
        copied
        victim=$(take_one "$last" "$crate_of_texts")
        status=0
        RONOCTUA_TEXTS=$PWD/other "$shell" check-texts > /dev/null 2> other.txt || status=$?
        if [ "$status" != 78 ]; then
          echo "ERROR: texts lacking \`$victim\` in $last were not refused with 78 (got $status)" >&2
          cat other.txt >&2
          exit 1
        fi
        grep -q "RONOCTUA_TEXTS" other.txt && grep -q "$victim" other.txt && grep -q "$last.ftl" other.txt || {
          echo "ERROR: the refusal does not name the variable, the key and the file:" >&2
          cat other.txt >&2
          exit 1
        }
        echo "OK: another build's texts are refused with 78, naming RONOCTUA_TEXTS, $last.ftl and \`$victim\`"

        # A BRAIN MODULE'S KEY MISSING: the first distro module's, in the
        # merged files' order. The footer says it through the module's
        # manifest, and the brain's buffer the PNG camera draws from a mind
        # says the module's name and line: both ask for it at their start.
        copied
        victim=$(take_one "$last" "$a_module")
        status=0
        RONOCTUA_TEXTS=$PWD/other "$shell" check-texts > /dev/null 2> module.txt || status=$?
        if [ "$status" != 78 ] || ! grep -q "$victim" module.txt; then
          echo "ERROR: texts lacking the brain module key \`$victim\` in $last were not refused with 78, naming it (got $status)" >&2
          cat module.txt >&2
          exit 1
        fi
        copied
        victim=$(take_one "$base" "$a_module")
        status=0
        RONOCTUA_TEXTS=$PWD/other ${rust.bins.ronoctua-render-png}/bin/ronoctua-render-png \
          ${buffers.format}/lib/ronoctua_buffer_format.wasm --columns 60 --lines 10 --language "$base" \
          --out never.png < /dev/null > /dev/null 2> module.txt || status=$?
        if [ "$status" != 78 ] || ! grep -q "$victim" module.txt; then
          echo "ERROR: the PNG camera started over texts lacking the brain module key \`$victim\` (status $status), or did not name it:" >&2
          cat module.txt >&2
          exit 1
        fi
        echo "OK: texts lacking a brain module's key (\`$victim\`) are refused with 78 by check-texts and by the PNG camera"

        # NO TEXTS AT ALL.
        status=0
        env -u RONOCTUA_TEXTS "$shell" check-texts > /dev/null 2> none.txt || status=$?
        if [ "$status" != 78 ] || [ "$(grep -c 'RONOCTUA_TEXTS is not set' none.txt)" != 1 ]; then
          echo "ERROR: no texts were not refused once with 78 (got $status):" >&2
          cat none.txt >&2
          exit 1
        fi
        echo "OK: no texts at all are refused with 78, said once"

        # FROM HERE ON, ANOTHER BUILD'S TEXTS ARE A COPY LACKING ONE KEY IN
        # EVERY LANGUAGE: whichever language a program speaks here (no `LANG`
        # and no choice in a home say the base one, today), its start guard
        # finds a key missing.
        copied
        for language in ${lib.escapeShellArgs languages}; do
          take_one "$language" "$crate_of_texts" > /dev/null
        done

        # What a case is, said in an error.
        told() {
          if [ "$1" = none ]; then
            echo "no texts"
          else
            echo "another build's texts"
          fi
        }

        # THE START GUARD OF EVERY PROGRAM THAT SPEAKS, before anything else:
        # no display, no terminal and no GPU is reached.
        guarded() {
          what=$1
          shift
          for case in none other; do
            status=0
            if [ "$case" = none ]; then
              env -u RONOCTUA_TEXTS "$@" < /dev/null > /dev/null 2> said.txt || status=$?
            else
              RONOCTUA_TEXTS=$PWD/other "$@" < /dev/null > /dev/null 2> said.txt || status=$?
            fi
            if [ "$status" != 78 ] || ! grep -q "RONOCTUA_TEXTS" said.txt; then
              echo "ERROR: $what started with $(told "$case") (status $status), or did not name the variable:" >&2
              cat said.txt >&2
              exit 1
            fi
          done
          echo "OK: $what refuses no texts and another build's with 78, naming the variable"
        }
        guarded "the footer's binary" "$shell" photograph --door calc --list
        guarded "the PNG camera" ${rust.bins.ronoctua-render-png}/bin/ronoctua-render-png \
          ${buffers.format}/lib/ronoctua_buffer_format.wasm --columns 60 --lines 10 --language "$base" --out never.png
        guarded "the installer" ${rust.bins.ronoctua-install}/bin/ronoctua-install
        guarded "the rescue console" ${rust.bins.ronoctua-rescue}/bin/ronoctua-rescue

        # EVERY WRAPPER CARRIES THE BUILD'S TEXTS, AND `--set`, asked of what
        # it DOES: each wrapped program is run with no texts in its
        # environment (a wrapper that lost the variable) and with the copy
        # above (a `--set-default`, or no variable, would hand the program
        # that one, and the guard just shown answers 78). Each must pass its
        # guard and say, byte for byte, what the SAME binary unwrapped says
        # from the build's own texts — worked out first, and asked to be a
        # real answer, so two empty outputs cannot agree.
        #
        # - the footer's binary: `check-texts`, which names the directory
        #   (`ok.txt`, above);
        # - the installer: `--show network`, a screen of its sentences.
        with_texts() {
          case "$1" in
            none)
              shift
              env -u RONOCTUA_TEXTS "$@"
              ;;
            other)
              shift
              env RONOCTUA_TEXTS="$PWD/other" "$@"
              ;;
            own)
              shift
              env RONOCTUA_TEXTS=${texts} "$@"
              ;;
            *)
              echo "ERROR: no texts called \`$1\`" >&2
              exit 1
              ;;
          esac
        }
        with_texts own ${rust.bins.ronoctua-install}/bin/ronoctua-install --show network < /dev/null > install.want
        if ! grep -qF "at ${texts}" ok.txt; then
          echo "ERROR: check-texts did not name the build's texts: $(cat ok.txt)" >&2
          exit 1
        fi
        if [ "$(grep -c '[[:alpha:]]' install.want)" -lt 3 ] || grep -q '\[inst-' install.want; then
          echo "ERROR: the installer's network screen from the build's texts has no words, or raw keys:" >&2
          cat install.want >&2
          exit 1
        fi
        wrapped() {
          what=$1
          want=$2
          shift 2
          for case in none other; do
            status=0
            with_texts "$case" "$@" < /dev/null > "got.$case" 2> "said.$case" || status=$?
            if [ "$status" != 0 ] || grep -q "RONOCTUA_TEXTS" "said.$case" || ! cmp -s "$want" "got.$case"; then
              echo "ERROR: $what, with $(told "$case") in its environment, did not do what the build's texts do (status $status):" >&2
              cat "said.$case" >&2
              diff "$want" "got.$case" >&2 || true
              exit 1
            fi
          done
          echo "OK: $what reads the build's texts with none in its environment and over another build's"
        }
        wrapped "the footer's wrapper" ok.txt ${rust.shell}/bin/ronoctua-shell check-texts
        wrapped "the ISO's installer" install.want ${rust.installer}/bin/ronoctua-install --show network

        # THE RESCUE CONSOLE has nothing to say without a terminal: past its
        # guards, the next thing it does is take the terminal, and with none
        # (no tty on stdin, no controlling one in the sandbox) it stops there,
        # status 1, `raw mode`. Unwrapped from the build's texts it gets that
        # far — the reference — and so must each wrapper, with no texts and
        # with the copy, where a guard that met them would have said 78.
        reached() {
          status=0
          with_texts "$@" < /dev/null > /dev/null 2> reached.txt || status=$?
          [ "$status" = 1 ] && grep -q "raw mode" reached.txt && ! grep -q "RONOCTUA_TEXTS" reached.txt
        }
        if ! reached own ${rust.bins.ronoctua-rescue}/bin/ronoctua-rescue; then
          echo "ERROR: the rescue console did not reach its terminal from the build's texts (status $status):" >&2
          cat reached.txt >&2
          exit 1
        fi
        for wrapper in ${rust.shell}/bin/ronoctua-rescue ${rust.rescue}/bin/ronoctua-rescue; do
          for case in none other; do
            if ! reached "$case" "$wrapper"; then
              echo "ERROR: $wrapper, with $(told "$case") in its environment, did not pass its guards to the terminal (status $status):" >&2
              cat reached.txt >&2
              exit 1
            fi
          done
        done
        echo "OK: both rescue consoles' wrappers pass their guards with no texts and over another build's"

        # AND THE DIRECTORY EACH CARRIES IS THIS BUILD'S, by its store path.
        for wrapper in \
          ${rust.shell}/bin/ronoctua-shell \
          ${rust.shell}/bin/ronoctua-rescue \
          ${rust.installer}/bin/ronoctua-install \
          ${rust.rescue}/bin/ronoctua-rescue; do
          grep -aqF "${texts}" "$wrapper" || {
            echo "ERROR: $wrapper does not name the build's texts" >&2
            exit 1
          }
        done
        for player in ${rust.player}/bin/ronoctua-player ${rust.shell}/bin/ronoctua-player; do
          if grep -aq "RONOCTUA_TEXTS" "$player"; then
            echo "ERROR: the player's daemon ($player) carries texts it never reads" >&2
            exit 1
          fi
        done
        echo "OK: four wrappers carry the build's texts, --set; the player's daemon carries none"
      '';
}
