# THE THEMES AS DATA — the 2026-10-12 wave, lane D1.
#
# The colours left the compilation: the Rust and the WIT are compiled against
# the tokens' NAMES, read at evaluation (`nix/token-names.nix`), and the bytes
# are `palettes.bin`, which the colour generator writes (`nix/themes.nix`) and
# every binary that paints reads at run time through `RONOCTUA_THEMES`
# (`ronoctua_tokens::palettes`). These checks hold each part of that:
#
# - `themes-are-data`: no Rust and no WASM derivation is built from a palette,
#   a seed, a recipe's value, the theme list, the default or the colour
#   generator — read off the build graph of each, which is what Nix rebuilds
#   by, and recognised by content, whatever a route names them;
# - `palettes-are-read-at-run-time`: the binaries carry no palette, paint the
#   palettes they are pointed at, and refuse a missing or foreign one with a
#   sentence that names the variable;
# - `names-are-extracted-at-evaluation`: the role list and the WIT come out of
#   evaluation, unmoved by a value and moved by a name, and the WIT holds no
#   brain module's glyph;
# - `generators-cite-their-scripts`: every generator cites the scripts it runs
#   one by one, never the whole `nix/scripts`, so editing one generator
#   rebuilds what it generates and nothing else.
{
  pkgs,
  lib,
  self,
  system,
  mkCheck,
  pyPrelude,
  rust,
  buffers,
  # The theme list and the default, as the flake read them from
  # `assets/themes/themes.nix`: what `themes-are-data` looks for by name.
  themes,
  defaultTheme,
  themed,
  tokensRs,
  wit,
  tokenNames,
  brainModules,
  # The generators, for the census of what they cite.
  glyphdb,
  unicodedb,
  lucidedb,
  marksdb,
  primaryFont,
  atlasData,
  notificationsRs,
  texts,
  brainModulesRs,
  bluetoothProfilesRs,
  audioNamesRs,
  broadcastRs,
  syncthingOptionsRs,
  torOptionsRs,
  atuinOptionsRs,
  keyboardsRs,
  syntaxTable,
  mimeDb,
  compositorDeeds,
  extension,
  iconSuperpack,
  cursors,
}:

let
  # The software Vulkan the other picture checks draw with.
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };

  # A DERIVATION, as the `.drv` file that says what it is BUILT FROM. The
  # output dependency is dropped, so naming one here hands the check that file
  # — and, as the store's references of a `.drv` are its input `.drv`s and
  # sources, every `.drv` it was made from — and never builds it.
  # (`exportReferencesGraph` would not do: it adds every output in the graph,
  # and an output not built yet is a path the store does not have.)
  graphOf = drv: builtins.unsafeDiscardOutputDependency drv.drvPath;

  derivationsIn = set: lib.filterAttrs (_: lib.isDerivation) set;
in
{
  # NO RUST AND NO WASM IS BUILT FROM A COLOUR.
  #
  # A store path in a derivation's inputs is a reason to build it again, and
  # so is a string in its attributes: the question "does a seed's hue, or the
  # default theme, rebuild the workspace?" has an exact answer in the build
  # graph. Asked of the tokens' Rust, the WIT, every release and loop program
  # (each one's graph holds every library and third-party unit it links), the
  # test graph's harnesses, clippy and rustfmt, and every buffer. A test RUN
  # reads the palettes as a run input and is not asked: what it runs is a
  # harness, asked here, and only the run moves with a seed
  # (`colour-seeds-rebuild-no-rust`).
  #
  # BY CONTENT, AND NOT BY THE NAMES A ROUTE GIVES THEM. A path copied under
  # any name, a directory that holds a seed, a value written into a
  # derivation's text: each route is recognised by what it carries.
  #
  # - `palettes`: the palettes' derivation, or another solve of them (the
  #   generator's own `pname`), anywhere in the graph. Whatever is copied out
  #   of it brings it along;
  # - `file`: a source whose bytes, or the bytes of a file inside a source
  #   directory, are a seed, a reference, the theme list, the recipes or one
  #   of the colour generator's two scripts;
  # - `value`: the text of one of those files, the JSON the generator is
  #   handed of the recipes, a seed or a reference (`themed.handed`), or the
  #   themes in their order, in any spelling — in a derivation's strings
  #   (`readFile`, `writeText`, `toJSON`, structured attributes) or in the
  #   text of any source;
  # - `name`: a theme named as a word — the default in generated code, a file
  #   named after one — in a derivation's strings, or in a file WRITTEN AT
  #   EVALUATION (`builtins.toFile`, as the WIT is). That one is told from a
  #   copied file by its store path: a text's is the hash of its name,
  #   contents and references, and is recomputed here. A file of the
  #   repository is not asked: it may name a theme for its own reasons (a
  #   test of the chooser, "midnight" in the clock), and editing
  #   `assets/themes/themes.nix` does not change it.
  #
  # What it cannot see is a value turned into something else first (a hue
  # copied out as a number, a hash of the default's name). Every route that
  # carries the data itself, it sees.
  #
  # AND IT IS NOT BLIND: each route is planted here in a derivation that is
  # never built — only its `.drv` is read — and must be found by its own
  # detector; and the shell's wrapper, which does carry the palettes, must be
  # found by the first.
  themes-are-data =
    let
      compiled = {
        tokens-rs = tokensRs;
        inherit wit;
        # Never built: only its `.drv`, whose inputs are every harness's.
        test-harnesses = pkgs.linkFarm "test-harnesses" (
          lib.mapAttrsToList (name: path: { inherit name path; }) rust.test.testBins
        );
        inherit (rust) clippy fmt;
      }
      // lib.mapAttrs' (n: v: lib.nameValuePair "release-${n}" v) rust.release.bins
      // lib.mapAttrs' (n: v: lib.nameValuePair "loop-${n}" v) rust.loop.bins
      // lib.mapAttrs' (n: v: lib.nameValuePair "buffer-${n}" v) (derivationsIn buffers);

      themesDir = ../assets/themes;
      # The default's seed, by the default's name: a theme is named only in
      # `assets/themes/themes.nix` (invariant 16).
      seed = themesDir + "/${defaultTheme}.nix";
      plant = detector: drv: {
        inherit detector;
        drv = graphOf drv;
      };
      planted = {
        recipes-as-a-file = plant "file" (
          pkgs.runCommand "planted-recipes" { } "cat ${../assets/tokens/colors.nix} > $out"
        );
        seed-as-a-file = plant "file" (pkgs.runCommand "planted-seed" { } "cat ${seed} > $out");
        seeds-in-a-directory = plant "file" (pkgs.runCommand "planted-seeds" { } "ls ${themesDir} > $out");
        generator-renamed = plant "file" (
          pkgs.runCommand "planted-generator" { }
            "cat ${
              builtins.path {
                path = ../nix/scripts/gen-theme.py;
                name = "planted.py";
              }
            } > $out"
        );
        seed-as-text = plant "value" (pkgs.writeText "planted-seed-text" (builtins.readFile seed));
        recipes-inside-json = plant "value" (
          pkgs.writeText "planted-recipes-json" ''{"planted":${themed.handed.recipes}}''
        );
        seed-in-structured-attributes = plant "value" (
          pkgs.runCommand "planted-structured" {
            __structuredAttrs = true;
            seed = themed.handed.seeds.${defaultTheme};
          } "touch $out"
        );
        theme-list = plant "value" (pkgs.writeText "themes.json" (builtins.toJSON themes));
        theme-list-in-a-file = plant "value" (
          pkgs.runCommand "planted-list-file" { }
            "cp ${builtins.toFile "themes.json" (builtins.toJSON themes)} $out"
        );
        seed-json-in-a-file = plant "value" (
          pkgs.runCommand "planted-seed-file" { }
            "cp ${builtins.toFile "seed.json" themed.handed.seeds.${defaultTheme}} $out"
        );
        default-in-a-file = plant "name" (
          pkgs.runCommand "planted-default-file" { } "cp ${builtins.toFile "default" defaultTheme} $out"
        );
        default-in-code = plant "name" (
          pkgs.writeText "planted.rs" ''pub const DEFAULT_THEME: &str = "${defaultTheme}";''
        );
        # Named only by the store path that cites it: the hash comes off first.
        default-in-a-file-name = plant "name" (
          pkgs.runCommand "planted-default-name" { }
            "cat ${builtins.toFile "${defaultTheme}.json" "{}"} > $out"
        );
        palettes-copied = plant "palettes" (
          pkgs.runCommand "planted-palettes" { } "cp ${themed}/palettes.bin $out"
        );
        palettes-solved-again = plant "palettes" (themed.override { themes = lib.reverseList themes; });
      };

      # THE THEME DATA, as files: every `.nix` under `assets/themes` (the
      # seeds, the references, the list), the recipes, and the colour
      # generator's two scripts.
      dataFiles =
        lib.mapAttrs' (name: _: lib.nameValuePair "assets/themes/${name}" "${themesDir + "/${name}"}") (
          lib.filterAttrs (name: type: type == "regular" && lib.hasSuffix ".nix" name) (
            builtins.readDir themesDir
          )
        )
        // {
          "assets/tokens/colors.nix" = "${../assets/tokens/colors.nix}";
          "nix/scripts/gen-theme.py" = "${../nix/scripts/gen-theme.py}";
          "nix/scripts/colorspace.py" = "${../nix/scripts/colorspace.py}";
        };
      # And as the generator is handed them.
      handedValues = {
        "the recipes' JSON" = themed.handed.recipes;
      }
      // lib.mapAttrs' (id: v: lib.nameValuePair "the JSON of ${id}'s seed" v) themed.handed.seeds
      // lib.mapAttrs' (
        id: v: lib.nameValuePair "the JSON of ${id}'s reference" v
      ) themed.handed.references;

      facts = pkgs.writeText "theme-data-routes.json" (
        builtins.toJSON {
          roots = lib.mapAttrs (_: graphOf) compiled;
          wrapper = graphOf rust.shell;
          palettes = graphOf themed;
          inherit planted;
          files = dataFiles;
          values = handedValues;
          ids = themes;
        }
      );
    in
    mkCheck "themes-are-data" [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import hashlib, json, os, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: a `.drv` is a derivation written down, in ATerm —
      # `Derive([outputs],[input derivations],[input sources],system,builder,
      # [args],[env])` — and is read by that grammar: strings with their
      # escapes undone, lists and tuples. The input derivations are what it
      # was made from, the input sources the files it was handed, and the
      # strings everything its build is told. Walked from each root, the walk
      # is the build graph; every `.drv` and source it reaches is in this
      # sandbox, because the store's references of a `.drv` are exactly those.
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
          # Every string in a decoded JSON value, keys included.
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
      by_size = {}
      for label, path in sorted(facts["files"].items()):
          raw = open(path, "rb").read()
          by_size.setdefault(len(raw), {})[hashlib.sha256(raw).hexdigest()] = label
      texts = {label: open(path, errors="surrogateescape").read() for label, path in facts["files"].items()}
      texts.update(facts["values"])
      ids = facts["ids"]
      floor(len(facts["files"]), 14, "theme data files (seeds, reference, list, recipes, scripts)")
      floor(len(facts["values"]), 12, "values the generator is handed")
      floor(len(ids), 2, "themes")
      floor(min(map(len, texts.values())), 64, "characters in the shortest theme datum")
      NAME = re.compile(r"(?<![A-Za-z0-9_-])(" + "|".join(map(re.escape, ids)) + r")(?![A-Za-z0-9_-])", re.I)
      ORDER = re.compile(r"(?<![A-Za-z0-9_-])" + r"[^A-Za-z0-9_-]+".join(map(re.escape, ids)) + r"(?![A-Za-z0-9_-])", re.I)
      # A store path's hash and an SRI hash are letters that say nothing; they
      # are taken off before a word is looked for, so `…-cyber.json` starts
      # one.
      MEANINGLESS = re.compile(r"/nix/store/[0-9a-z]{32}-|\b(?:md5|sha1|sha256|sha512)-[A-Za-z0-9+/]+=*")
      PALETTES = re.compile(r"/nix/store/[0-9a-z]{32}-ronoctua-themes-[^/]*\.drv$")

      def values_in(text):
          found = [label for label, t in texts.items() if len(text) >= len(t) and t in text]
          if ORDER.search(text):
              found.append("the themes in their order")
          return found

      def names_in(text):
          m = NAME.search(MEANINGLESS.sub(" ", text))
          return [f"the theme `{m.group(1)}`"] if m else []

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
              if path == facts["palettes"] or PALETTES.search(path):
                  found.append(("palettes", "the palettes' derivation", path))
              for s in strings:
                  found += [("value", v, path) for v in values_in(s)]
                  found += [("name", n, path) for n in names_in(s)]
              drvs[path] = (inputs, srcs, found)
          return drvs[path]

      def found_in_source(path):
          if path not in sources:
              if os.path.isdir(path):
                  files = [os.path.join(d, f) for d, _, fs in os.walk(path) for f in fs]
              else:
                  files = [path]
              found = []
              for f in files:
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
                      found += [("name", n, f) for n in names_in(text)]
              sources[path] = found
          return sources[path]

      def walk(root):
          seen, todo, srcs, found = set(), [root], set(), []
          while todo:
              path = todo.pop()
              if path in seen:
                  continue
              seen.add(path)
              inputs, handed, hits = found_in_drv(path)
              found += hits
              todo += inputs
              srcs.update(handed)
          for s in sorted(srcs):
              found += found_in_source(s)
          return len(seen) + len(srcs), found

      # THE CONTROLS FIRST: a census that finds nothing where something was
      # put is not reading.
      floor(len(facts["planted"]), 15, "routes planted")
      blind = []
      for route, p in sorted(facts["planted"].items()):
          _, found = walk(p["drv"])
          if not any(d == p["detector"] for d, _, _ in found):
              blind.append(f"the planted route `{route}` was not found by the `{p['detector']}` detector")
      _, found = walk(facts["wrapper"])
      if not any(d == "palettes" for d, _, _ in found):
          blind.append("the shell's wrapper carries the palettes, and was not found built from them")
      if blind:
          print("ERROR: the census of the build graphs is blind:", file=sys.stderr)
          for b in blind:
              print("       " + b, file=sys.stderr)
          sys.exit(1)

      SAYS = {
          "palettes": "is built from",
          "file": "is handed the file",
          "value": "holds",
          "name": "names",
      }
      roots = facts["roots"]
      floor(len(roots), 60, "compiled derivations asked")
      wrong, sizes = [], []
      for name, root in sorted(roots.items()):
          size, found = walk(root)
          sizes.append(size)
          said = {}
          for detector, what, where in found:
              said.setdefault((detector, what), where)
          for (detector, what), where in sorted(said.items())[:6]:
              wrong.append(f"{name} {SAYS[detector]} {what}: {where}")
      floor(min(sizes), 50, "paths in the smallest build graph")
      if wrong:
          print("ERROR: a compilation is built from a colour or a theme, and editing it would rebuild it.", file=sys.stderr)
          print("", file=sys.stderr)
          for w in wrong:
              print("       " + w, file=sys.stderr)
          print("", file=sys.stderr)
          print("       The Rust and the WIT are compiled against the tokens' NAMES", file=sys.stderr)
          print("       (nix/token-names.nix); the colours, the themes and the default are", file=sys.stderr)
          print("       palettes.bin, read at run time through RONOCTUA_THEMES. Point a binary", file=sys.stderr)
          print("       at the data instead.", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(roots)} compiled derivations ({sum(sizes)} graph entries) are built from no palette, "
            f"seed, reference, recipe, theme list, theme name or colour generator, by content; "
            f"{len(facts['planted'])} planted routes and the wrapper are found")
      END
    '';

  # THE BINARIES PAINT THE PALETTES THEY ARE POINTED AT, AND NO OTHER.
  #
  # Four answers, each of a built binary and none of the source:
  #
  # - no binary carries a palette: a page of every theme's base, the 1020
  #   bytes `palettes.bin` holds for it, is in neither the footer nor the PNG
  #   camera — where the compiled table used to keep them, contiguous;
  # - the picture IS the file's: the same buffer drawn with the palettes and
  #   with a copy whose every colour was turned round comes out different,
  #   and drawn twice with the same file, the same;
  # - a file written for other roles is refused, naming the variable — never
  #   read through the wrong index;
  # - and no variable is refused, naming it.
  palettes-are-read-at-run-time = mkCheck "palettes-are-read-at-run-time" [ pkgs.python3 ] ''
    # The texts (`rust.keyed`): the PNG camera speaks too, and refuses (exit
    # 78) with no texts — a refusal that is not the one asked about below.
    export RONOCTUA_TEXTS=${rust.keyed.RONOCTUA_TEXTS}
    export VK_DRIVER_FILES=${xvfb.icd}
    export VK_ICD_FILENAMES=${xvfb.icd}
    export LIBGL_ALWAYS_SOFTWARE=1

    draw() {
      ${rust.bins.ronoctua-render-png}/bin/ronoctua-render-png \
        ${buffers.format}/lib/ronoctua_buffer_format.wasm \
        --columns 60 --lines 10 --language en-GB --out "$1"
    }

    python3 - ${themed}/palettes.bin ${rust.bins.ronoctua-shell}/bin/ronoctua-shell ${rust.bins.ronoctua-render-png}/bin/ronoctua-render-png <<'END'
    import pathlib, struct, sys

    # PARSED: the palettes' header, by the layout `gen-theme.py` writes
    # (`palettes_bin`), and the binaries' bytes searched for a page of it.
    data = pathlib.Path(sys.argv[1]).read_bytes()
    if data[:8] != b"RONOPAL\0":
        sys.exit("ERROR: palettes.bin does not start with its magic")
    version, pages, themes, roles, default = struct.unpack_from("<HHHHH", data, 8)
    at = 26
    for _ in range(pages):
        at += 1 + data[at]
    for _ in range(themes):
        at += 2 + data[at + 1]
    body = data[at:]
    page = roles * 4
    if len(body) != themes * pages * page or themes < 2:
        sys.exit(f"ERROR: palettes.bin holds {len(body)} bytes for {themes} themes of {pages} pages")
    binaries = [pathlib.Path(p).read_bytes() for p in sys.argv[2:]]
    carried = []
    for t in range(themes):
        needle = body[t * pages * page : t * pages * page + page]
        for path, binary in zip(sys.argv[2:], binaries):
            if needle in binary:
                carried.append(f"{path} carries theme {t}'s base palette")
    if carried:
        print("ERROR: a binary carries a palette: the colours are compiled in again.", file=sys.stderr)
        for c in carried:
            print("       " + c, file=sys.stderr)
        sys.exit(1)
    print(f"OK: no binary carries any of the {themes} themes' base palettes ({page} bytes each)")

    # A COPY WITH EVERY COLOUR TURNED ROUND, and one written for other
    # roles (the hash's first byte flipped).
    turned = bytearray(data)
    for i in range(at, len(turned), 4):
        for c in range(3):
            turned[i + c] = 255 - turned[i + c]
    pathlib.Path("turned").mkdir()
    pathlib.Path("turned/palettes.bin").write_bytes(bytes(turned))
    foreign = bytearray(data)
    foreign[18] ^= 0xFF
    pathlib.Path("foreign").mkdir()
    pathlib.Path("foreign/palettes.bin").write_bytes(bytes(foreign))
    END

    RONOCTUA_THEMES=${themed} draw real.png > /dev/null
    RONOCTUA_THEMES=${themed} draw again.png > /dev/null
    RONOCTUA_THEMES=$PWD/turned draw turned.png > /dev/null
    cmp -s real.png again.png || {
      echo "ERROR: the same palettes drew two different pictures; nothing below can be compared" >&2
      exit 1
    }
    if cmp -s real.png turned.png; then
      echo "ERROR: palettes with every colour turned round drew the same picture:" >&2
      echo "      the binary is not painting the palettes it is pointed at." >&2
      exit 1
    fi
    echo "OK: the picture is the palettes': turned round, it changes; the same file, the same picture"

    if RONOCTUA_THEMES=$PWD/foreign draw foreign.png > /dev/null 2> foreign.txt; then
      echo "ERROR: palettes written for other colour roles were drawn with." >&2
      exit 1
    fi
    grep -q "RONOCTUA_THEMES" foreign.txt && grep -q "other colour roles" foreign.txt || {
      echo "ERROR: palettes for other roles were refused without saying so, naming the variable:" >&2
      cat foreign.txt >&2
      exit 1
    }
    if env -u RONOCTUA_THEMES ${rust.bins.ronoctua-render-png}/bin/ronoctua-render-png \
        ${buffers.format}/lib/ronoctua_buffer_format.wasm \
        --columns 60 --lines 10 --out none.png > /dev/null 2> none.txt; then
      echo "ERROR: a picture was drawn with no palettes at all." >&2
      exit 1
    fi
    grep -q "RONOCTUA_THEMES is not set" none.txt || {
      echo "ERROR: no palettes, and the refusal does not name the variable:" >&2
      cat none.txt >&2
      exit 1
    }
    echo "OK: palettes for other roles, and none at all, are refused naming RONOCTUA_THEMES"
  '';

  # THE NAMES COME OUT OF EVALUATION, AND ONLY THE NAMES.
  #
  # Asked of the function that makes them (`nix/token-names.nix`) on the real
  # files and on edited copies — written at evaluation with `builtins.toFile`,
  # never imported from a build:
  #
  # - a recipe's VALUE edited moves nothing the function returns: the same
  #   roles, the same WIT;
  # - a role's NAME added moves both, and the tokens' derivation with them —
  #   the check is not passing on a function that ignores its input;
  # - an icon's glyph edited moves no WIT; an icon's name added does;
  # - the WIT holds no brain module's glyph, so a person's module rebuilds no
  #   buffer;
  # - a name in both role files, and a name WIT cannot spell, are refused;
  # - and the flake's tokens, WIT and palettes are the ones built from these
  #   names, not from a list of their own.
  #
  # So no value reaches the tokens or the WIT THROUGH the names. Whether one
  # reaches them by another route — a recipe file handed to the tokens'
  # generator, a seed in `generated` — is not a question about this function,
  # and it is not asked here: `themes-are-data` answers it, from the build
  # graphs, by content.
  names-are-extracted-at-evaluation =
    let
      # The texts alone: `builtins.toFile` takes no store path along.
      colorsText = builtins.unsafeDiscardStringContext (builtins.readFile ../assets/tokens/colors.nix);
      iconsText = builtins.unsafeDiscardStringContext (builtins.readFile ../assets/tokens/icons.nix);
      edit =
        what: from: to: text:
        let
          edited = builtins.replaceStrings [ from ] [ to ] text;
        in
        assert lib.assertMsg (
          edited != text
        ) "names-are-extracted-at-evaluation: `${from}` is gone from ${what}";
        edited;
      namesOf =
        {
          recipes ? colorsText,
          icons ? iconsText,
          shaped ? ../assets/tokens/shaped.nix,
        }:
        import ../nix/token-names.nix {
          inherit lib;
          recipesFile = builtins.toFile "colors.nix" recipes;
          iconTokensFile = builtins.toFile "icons.nix" icons;
          shapedFile = shaped;
        };
      recipe = ''base = "neutral.ground";'';
      glyph = ''notification-pending = "cod-bell_dot";'';
      real = namesOf { };
      value = namesOf { recipes = edit "colors.nix" recipe ''base = "neutral.chrome";'' colorsText; };
      named = namesOf {
        recipes =
          edit "colors.nix" recipe ''base = "neutral.ground"; probe = "neutral.ground";''
            colorsText;
      };
      glyphValue = namesOf {
        icons = edit "icons.nix" glyph ''notification-pending = "cod-bell";'' iconsText;
      };
      glyphNamed = namesOf {
        icons =
          edit "icons.nix" glyph ''notification-pending = "cod-bell_dot"; probe-icon = "cod-bell_dot";''
            iconsText;
      };
      twice = namesOf {
        shaped = builtins.toFile "shaped.nix" "{ surface.base = true; }";
      };
      unspellable = namesOf {
        recipes =
          edit "colors.nix" recipe ''base = "neutral.ground"; "0probe" = "neutral.ground";''
            colorsText;
      };
      refused =
        names:
        !(builtins.tryEval (builtins.deepSeq [ names.roles (builtins.readFile names.wit) ] true)).success;
      drv = d: builtins.unsafeDiscardStringContext d.drvPath;
      tokensOf = names: drv (tokensRs.override { roleNames = names.roles; });
      witOf = names: drv (wit.override { tokensWit = names.wit; });
      facts = pkgs.writeText "token-name-facts.json" (
        builtins.toJSON {
          roles = real.roles;
          flakeRoles = real.roles == tokenNames.roles;
          flakeWit = builtins.readFile real.wit == builtins.readFile tokenNames.wit;
          flakeTokens = tokensOf tokenNames == drv tokensRs;
          flakeWitDrv = witOf tokenNames == drv wit;
          flakePalettes = drv (themed.override { roleNames = tokenNames.roles; }) == drv themed;
          valueRoles = value.roles == real.roles;
          valueWit = builtins.readFile value.wit == builtins.readFile real.wit;
          namedRole = builtins.elem "surface.probe" named.roles;
          namedTokens = tokensOf named != drv tokensRs;
          namedWit = lib.hasInfix "        surface-probe,\n" (builtins.readFile named.wit);
          glyphValueWit = builtins.readFile glyphValue.wit == builtins.readFile real.wit;
          glyphNamedWit = lib.hasInfix "        probe-icon,\n" (builtins.readFile glyphNamed.wit);
          twiceRefused = refused twice;
          unspellableRefused = refused unspellable;
          wit = builtins.readFile real.wit;
          icons = real.icons;
          moduleGlyphs = builtins.attrNames brainModules.glyphs;
        }
      );
    in
    mkCheck "names-are-extracted-at-evaluation" [ pkgs.python3 ] ''
      python3 - ${facts} <<'END'
      import json, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the answers arrive as JSON written at evaluation, and the WIT
      # is read line by line, by its own grammar: a case is `name,` inside
      # `enum color {` or `enum icon {`.
      facts = json.load(open(sys.argv[1]))
      wrong = []
      for key, what in [
          ("flakeRoles", "the flake's role list is not the one the names function makes"),
          ("flakeWit", "the flake's WIT is not the one the names function makes"),
          ("flakeTokens", "the flake's tokens are not built from the evaluated role names"),
          ("flakeWitDrv", "the flake's WIT directory is not built from the evaluated WIT"),
          ("flakePalettes", "the palettes are not solved in the evaluated role order"),
          ("valueRoles", "a recipe's VALUE moved the role list"),
          ("valueWit", "a recipe's VALUE moved the WIT"),
          ("namedRole", "a role NAME added did not reach the role list"),
          ("namedTokens", "a role NAME added did not move the tokens' derivation"),
          ("namedWit", "a role NAME added did not reach the WIT"),
          ("glyphValueWit", "an icon's glyph moved the WIT, which holds only names"),
          ("glyphNamedWit", "an icon NAME added did not reach the WIT"),
          ("twiceRefused", "a role named by both colors.nix and shaped.nix was accepted"),
          ("unspellableRefused", "a role WIT cannot spell (`surface.0probe`) was accepted"),
      ]:
          if facts[key] is not True:
              wrong.append(what)

      def cases(enum):
          block = re.search(r"^    enum " + enum + r" \{\n(.*?)^    \}", facts["wit"], re.S | re.M)
          if block is None:
              wrong.append(f"the WIT has no `enum {enum}`")
              return []
          return [l.strip().rstrip(",").lstrip("%") for l in block.group(1).splitlines()
                  if l.strip() and not l.strip().startswith("//")]

      colours = cases("color")
      icons = cases("icon")
      floor(len(colours), 100, "colour cases in the WIT")
      floor(len(icons), 100, "icon cases in the WIT")
      spelled = [re.sub(r"[^a-z0-9]+", "-", r.lower()).strip("-") for r in facts["roles"]]
      if colours != spelled:
          wrong.append("the WIT's colours are not the evaluated roles, in order")
      if icons != [re.sub(r"[^a-z0-9]+", "-", i.lower()).strip("-") for i in facts["icons"]]:
          wrong.append("the WIT's icons are not assets/tokens/icons.nix's names, in order")
      glyphs = facts["moduleGlyphs"]
      floor(len(glyphs), 1, "brain module glyphs (nothing to keep out of the WIT)")
      leaked = sorted(set(glyphs) & set(icons))
      if leaked:
          wrong.append("the WIT carries brain module glyphs, so a person's module rebuilds every buffer: " + ", ".join(leaked))

      if wrong:
          print("ERROR: the tokens' names do not come out of evaluation alone.", file=sys.stderr)
          print("", file=sys.stderr)
          for w in wrong:
              print("       " + w, file=sys.stderr)
          print("", file=sys.stderr)
          print("       The Rust and the WIT are compiled against NAMES (nix/token-names.nix);", file=sys.stderr)
          print("       a value is data the binaries read at run time.", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(colours)} roles and {len(icons)} icons, read at evaluation; a value moves nothing, "
            f"a name moves the tokens and the WIT; {len(glyphs)} module glyphs stay out of the WIT")
      END
    '';

  # EVERY GENERATOR CITES ITS SCRIPTS BY FILE.
  #
  # `${./scripts}` puts the WHOLE directory in a derivation's inputs, so
  # editing any one of the forty generators in it rebuilt every table made
  # through it — the glyphs, the fonts' cut, the atlas, the chain — and,
  # through `generated`, the workspace. Cited by file (`${./scripts/x.py}`),
  # or copied by file beside the one sibling it imports, a generator is
  # rebuilt when ITS script changes.
  #
  # Two halves. The generators the flake builds are asked what their build
  # actually cites (the string context of the derivation): none may cite the
  # directory's store path, and between them they cite the scripts one by
  # one. And because a generator can exist that no list here names, every
  # `.nix` of the repository is read for an interpolation or a path of the
  # directory itself.
  generators-cite-their-scripts =
    let
      scriptsDir = builtins.unsafeDiscardStringContext "${../nix/scripts}";
      generators = {
        inherit
          themed
          tokensRs
          wit
          primaryFont
          atlasData
          notificationsRs
          texts
          brainModulesRs
          bluetoothProfilesRs
          audioNamesRs
          broadcastRs
          syncthingOptionsRs
          torOptionsRs
          atuinOptionsRs
          keyboardsRs
          compositorDeeds
          extension
          iconSuperpack
          cursors
          ;
        mime-db = mimeDb.db or mimeDb;
      }
      // lib.mapAttrs' (n: v: lib.nameValuePair "glyphdb-${n}" v) (derivationsIn glyphdb)
      // lib.mapAttrs' (n: v: lib.nameValuePair "unicodedb-${n}" v) (derivationsIn unicodedb)
      // lib.mapAttrs' (n: v: lib.nameValuePair "lucidedb-${n}" v) (derivationsIn lucidedb)
      // lib.mapAttrs' (n: v: lib.nameValuePair "marksdb-${n}" v) (derivationsIn marksdb)
      // lib.mapAttrs' (n: v: lib.nameValuePair "syntax-${n}" v) (derivationsIn syntaxTable)
      // lib.getAttrs [
        "chain-rs"
        "width-rs"
        "coverage-fonts"
        "world-rs"
        "tongues-rs"
        "units-table"
        "units-negative-fixture"
        "belongings-table"
        "belongings-table-refuses"
        "words-data"
        "iconset"
      ] self.packages.${system};
      # What a derivation's build CITES: the store paths in the string context
      # of its attributes — every `${…}` its scripts interpolate.
      citesOf =
        drv:
        let
          strings =
            v:
            if builtins.isString v then
              [ v ]
            else if builtins.isList v then
              lib.concatMap strings v
            else
              [ ];
        in
        builtins.attrNames (
          lib.foldl' (acc: s: acc // builtins.getContext s) { } (
            lib.concatMap strings (builtins.attrValues drv.drvAttrs)
          )
        );
      facts = pkgs.writeText "script-citations.json" (
        builtins.toJSON {
          inherit scriptsDir;
          generators = lib.mapAttrs (
            _: drv:
            let
              cites = citesOf drv;
            in
            {
              whole = builtins.elem scriptsDir cites;
              scripts = map (p: builtins.unsafeDiscardStringContext (baseNameOf p)) (
                builtins.filter (p: lib.hasSuffix ".py" p) cites
              );
            }
          ) generators;
        }
      );
    in
    mkCheck "generators-cite-their-scripts" [ pkgs.python3 ] ''
      python3 - ${facts} ${self} <<'END'
      import json, pathlib, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the citations arrive as JSON written at evaluation.
      facts = json.load(open(sys.argv[1]))
      generators = facts["generators"]
      floor(len(generators), 25, "generators asked what they cite")
      wrong = [f"{name} cites the whole nix/scripts ({facts['scriptsDir']})"
               for name, g in sorted(generators.items()) if g["whole"]]
      cited = {s.split("-", 1)[1] for g in generators.values() for s in g["scripts"]}
      floor(len(cited), 20, "scripts cited by file")
      for wanted in ("gen-theme.py", "gen-tokens.py", "gen-glyphdb.py", "lexicon.py", "gen-width.py", "gen-atlas.py"):
          if wanted not in cited:
              wrong.append(f"no generator cites {wanted} by file: the census is not reading what it should")

      # FILE-WIDE: a generator no list above names is still a `.nix` file of
      # the repository, and the question — does any expression interpolate the
      # scripts' DIRECTORY, or hold it as a path? — is about every expression
      # in it. `hashless`: a comment that mentions `''${./scripts}` builds
      # nothing. THIS file is left out, by name: it holds the directory to
      # know its store path.
      root = pathlib.Path(sys.argv[2])
      whole = re.compile(r"\$\{\s*(?:\.\.?/)+(?:nix/)?scripts\s*\}|(?<![\w./-])(?:\.\.?/)+(?:nix/)?scripts(?![\w./-])")
      files = [f for f in sorted(root.rglob("*.nix")) if f.relative_to(root).as_posix() != "tests/theme-data.nix"]
      floor(len(files), 100, ".nix files of the repository")
      for f in files:
          for n, line in enumerate(hashless(f.read_text()).splitlines(), 1):
              if whole.search(line):
                  wrong.append(f"{f.relative_to(root)}:{n} cites the whole scripts directory: {line.strip()}")

      if wrong:
          print("ERROR: a generator cites nix/scripts whole, so editing any script rebuilds it.", file=sys.stderr)
          print("", file=sys.stderr)
          for w in wrong:
              print("       " + w, file=sys.stderr)
          print("", file=sys.stderr)
          print("       Cite the script by file — `''${./scripts/gen-x.py}` — or copy it and the", file=sys.stderr)
          print("       sibling it imports into a directory of the build's own, as", file=sys.stderr)
          print("       nix/themes.nix and nix/glyphgen.nix do.", file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(generators)} generators cite {len(cited)} scripts by file, and {len(files)} .nix files "
            "cite the directory nowhere")
      END
    '';
}
