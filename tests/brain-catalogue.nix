# THE BRAIN'S MODULES AS DATA (the 2026-10-12 wave, lane B2): what the footer
# draws of every module — its marks and their measured glyphs, the actions
# the keymap registers, its events and their notifications — is the brain's
# catalogue (`nix/brain-catalogue.nix`), read at RUN time
# (`RONOCTUA_BRAIN_CATALOGUE`); a module's code is the daemon's alone.
#
#   - `brain-catalogue-is-what-the-daemon-compiled`: the catalogue the footer
#     reads and the modules the daemon compiled are one set — what
#     `ronoctua-brain modules --catalogue` says each module's manifest
#     declares (its marks, notices, actions, events and reasons) against the
#     file, module by module; every module glyph measured by the glyph
#     generator into the file and none in the chrome's icon table (which
#     every crate that draws compiles); and every event that declared a
#     notification carrying it, and no other. And the generator's refusals,
#     each by its sentence, on its own inputs changed in one place: a module's
#     token that is also the chrome's, an event whose tag is an entry of the
#     notification table, and a key no text holds.
#
#   - `the-shell-reads-the-brain-catalogue`: the footer's binary reads it at
#     run time, and refuses it loudly. Of one built program, never rebuilt:
#     `check-catalogue` over the build's file says what it holds; the
#     keymap's dump lists exactly its actions; a copy with one action more
#     makes the dump list it, and a copy with a mark's glyph moved draws the
#     mark with the other glyph — so neither is compiled in. With no file, an
#     unreadable one, another format, keys its texts lack or a notification
#     the book cannot read, the program says so in one line naming the
#     variable and exits 78, at its start (before anything else) and in
#     `check-catalogue`. The footer's wrapper carries the build's file
#     (`--set`: a session's other copy loses to it), and no painter carries
#     one (they draw no module's mark).
#
#   - `the-glyph-browser-names-the-modules-glyphs`: the icon browser and the
#     fast access name a module's glyph by its token, as they did when the
#     glyph database measured the modules' tokens — the `token` fact, the
#     `token:` filter, copying it as `icon.<token>` —, out of the glyph
#     index `RONOCTUA_ICONSET` holds (`nix/iconset.nix`): the database's rows,
#     each named by the last token, in name order, of the chrome's and the
#     catalogue's that draw it (the database's own rule, which its rows are
#     asked to follow over the chrome's alone). Then the window, opened on
#     each glyph a module's token names, says that token, and the `token:`
#     filter finds every glyph the index names. And the generator's refusals,
#     by their sentences: a module's token that is also the chrome's, and a
#     database that names its glyphs another way.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  rust,
  brainModules,
  brainCatalogue,
  glyphdb,
  # The icon tables the icon browser reads at run time (`nix/iconset.nix`).
  iconset,
  # The software rasteriser the footer's camera draws with (`compositor-xvfb.nix`).
  xvfb,
}:
let
  catalogueFile = "${brainCatalogue}/${brainCatalogue.file}";
in
{
  brain-catalogue-is-what-the-daemon-compiled =
    let
      facts = pkgs.writeText "brain-catalogue-facts.json" (
        builtins.toJSON {
          ids = brainModules.ids;
          # Every module glyph token, and the font's name for it.
          glyphs = brainModules.glyphs;
          # Every event that declared a notification, by its tag.
          notifying = builtins.attrNames brainModules.notifications;
        }
      );
    in
    mkCheck "brain-catalogue-is-what-the-daemon-compiled" [ pkgs.python3 ] ''
      ${rust.brain}/bin/ronoctua-brain modules --catalogue > declared.json
      python3 - declared.json ${catalogueFile} ${brainCatalogue.measured}/icons.json \
        ${glyphdb.db}/icons.json ${facts} <<'END'
      import json, sys
      exec(open("${pyPrelude}").read())

      # PARSED: what the daemon says each module it MADE declares (its own
      # JSON), the catalogue the build wrote, the glyphs the generator
      # measured for it, the chrome's icon table and the loader's view — all
      # JSON, compared field by field.
      # FILE-WIDE: no source is searched — the catalogue's generator is RUN
      # on its own inputs (changed in one place), and what it writes and
      # says is compared whole.
      declared, catalogue, measured, chrome, facts = (json.load(open(p)) for p in sys.argv[1:6])
      wrong = []
      floor(len(declared), 10, "modules the daemon made")
      ids = [m["id"] for m in catalogue["modules"]]
      made = [d["id"] for d in declared]
      if ids != made:
          wrong.append(f"the catalogue holds {ids}, and the daemon made {made}")
      if ids != sorted(facts["ids"]):
          wrong.append(f"the catalogue holds {ids}, and the loader read {sorted(facts['ids'])}")
      by_id = {m["id"]: m for m in catalogue["modules"]}
      for d in declared:
          m = by_id.get(d["id"])
          if m is None:
              continue
          for field in ("name-key", "does-key", "marks", "notices", "actions", "whys"):
              if m[field] != d[field]:
                  wrong.append(f"`{d['id']}`'s {field}: the catalogue says {m[field]}, the daemon compiled {d[field]}")
          events = [{"name": e["name"], "tag": e["tag"]} for e in m["events"]]
          if events != d["events"]:
              wrong.append(f"`{d['id']}`'s events: the catalogue says {events}, the daemon compiled {d['events']}")
      floor(sum(len(d["actions"]) for d in declared), 10, "actions the daemon compiled")
      floor(sum(len(d["marks"]) for d in declared), 10, "marks the daemon compiled")

      # THE GLYPHS: every module token, measured into the file as the
      # generator measured it, and none in the table the crates compile.
      glyphs = {g["token"]: g for g in catalogue["glyphs"]}
      floor(len(facts["glyphs"]), 5, "module glyph tokens")
      if sorted(glyphs) != sorted(facts["glyphs"]):
          wrong.append(f"the catalogue measures {sorted(glyphs)}, and the modules declare {sorted(facts['glyphs'])}")
      for token, name in sorted(facts["glyphs"].items()):
          g, m = glyphs.get(token), measured.get(token)
          if g is None or m is None:
              wrong.append(f"`{token}` was not measured")
              continue
          got = (g["glyph"], g["codepoint"], g["size"], g["ink"], g["across"])
          want = (name, m["codepoint"], float(m["chrome"]), m["ink"], m["across"])
          if got != want:
              wrong.append(f"`{token}` is {got} in the catalogue, and the generator measured {want}")
          if token in chrome:
              wrong.append(f"`{token}` is in the chrome's icon table, which every crate that draws compiles")
      for m in catalogue["modules"]:
          for k in m["marks"]:
              if k["glyph"] not in glyphs:
                  wrong.append(f"`{m['id']}`'s mark `{k['name']}` names `{k['glyph']}`, which the file does not measure")

      # THE NOTIFICATIONS: the events that declared one carry it, by its tag.
      told = sorted(e["tag"] for m in catalogue["modules"] for e in m["events"] if e["notification"] is not None)
      floor(len(facts["notifying"]), 3, "events that declare a notification")
      if told != sorted(facts["notifying"]):
          wrong.append(f"the catalogue's notifications are {told}, and the modules declare {sorted(facts['notifying'])}")
      for m in catalogue["modules"]:
          for e in m["events"]:
              n = e["notification"]
              if n is not None and n.get("id") != e["tag"]:
                  wrong.append(f"`{e['tag']}`'s notification is called `{n.get('id')}`")

      # AND THE GENERATOR REFUSES, each by its sentence, its own inputs
      # changed in one place — after the real ones are accepted, so a refusal
      # is the change's and nothing else's.
      import pathlib, subprocess, tempfile
      work = pathlib.Path(tempfile.mkdtemp())
      given = {"modules": "${brainCatalogue.records}", "measured": "${brainCatalogue.measured}/icons.json",
               "icons": "${glyphdb.db}/icons.json", "glyph-names": "${glyphdb.db}/glyphs.json",
               "notifier": "${../nix/scripts/gen-notifications.py}",
               "notifications": "${brainCatalogue.declared}", "shared": "${../assets/shared/notifications.json}",
               "keys": "${brainCatalogue.keys}"}
      def generate(**changed):
          args = []
          for name, path in {**given, **changed}.items():
              args += [f"--{name}", str(path)]
          return subprocess.run([sys.executable, "${../nix/scripts/gen-brain-catalogue.py}", *args,
                                 "--out", str(work / "out.json")], capture_output=True, text=True)
      def planted(name, value):
          path = work / name
          path.write_text(json.dumps(value))
          return path
      r = generate()
      if r.returncode != 0 or json.loads((work / "out.json").read_text()) != catalogue:
          wrong.append("the generator, handed its own inputs again, did not write the build's catalogue: " + r.stderr)
      token = sorted(facts["glyphs"])[0]
      tag = sorted(facts["notifying"])[0]
      needed = sorted({k for m in catalogue["modules"] for k in m["keys"]})
      for what, changed, said in (
          ("a module's token that is also the chrome's",
           {"icons": planted("icons.json", {**chrome, token: measured[token]})},
           "is also one of assets/tokens/icons.nix"),
          ("an event whose tag is an entry of the notification table",
           {"notifications": planted("notifications.json",
                                     {**json.load(open("${brainCatalogue.declared}")),
                                      "entries": {**json.load(open("${brainCatalogue.declared}"))["entries"], tag: {}}})},
           f"`{tag}` is also an entry or a filter"),
          ("a key no text holds",
           {"keys": planted("keys.json", sorted(set(json.load(open("${brainCatalogue.keys}"))) - {needed[0]}))},
           f"no text of the build has `{needed[0]}`"),
      ):
          r = generate(**changed)
          if r.returncode == 0 or said not in r.stderr:
              wrong.append(f"the generator did not refuse {what} with `{said}`: {r.stderr.strip()[-300:]}")

      if wrong:
          print("ERROR: the brain's catalogue is not what the daemon compiled:", file=sys.stderr)
          for w in wrong:
              print("  " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: {len(made)} modules, as the daemon compiled them; {len(glyphs)} glyphs measured outside the"
            f" chrome's table; {len(told)} notifications; the generator refuses its 3 plants")
      END
    '';

  the-shell-reads-the-brain-catalogue =
    mkCheck "the-shell-reads-the-brain-catalogue"
      [
        pkgs.python3
        pkgs.gnugrep
        pkgs.coreutils
        pkgs.diffutils
      ]
      ''
        # The palettes and the texts (`rust.keyed`): this binary is the
        # unwrapped one; the catalogue is what each case below hands it.
        export RONOCTUA_THEMES=${rust.keyed.RONOCTUA_THEMES}
        export RONOCTUA_TEXTS=${rust.keyed.RONOCTUA_TEXTS}
        export VK_DRIVER_FILES=${xvfb.icd}
        export VK_ICD_FILENAMES=${xvfb.icd}
        export LIBGL_ALWAYS_SOFTWARE=1
        mkdir -p home run
        chmod 700 run
        export HOME=$PWD/home XDG_RUNTIME_DIR=$PWD/run
        shell=${rust.bins.ronoctua-shell}/bin/ronoctua-shell
        own=${catalogueFile}

        # THE COPIES, written from the build's own file: one action more and a
        # mark's glyph moved (`planted`, read with texts holding the action's
        # label — a command's label is its own `cmd-<id>`), a key no text has
        # (`lacking`), a notification spelt another way (`misspelt`), another
        # format (`newer`); and a mind publishing the mark whose glyph moved.
        mkdir texts-planted
        cp $RONOCTUA_TEXTS/*.ftl texts-planted/
        chmod u+w texts-planted/*.ftl
        python3 - "$own" <<'END'
        import copy, json, sys

        # PARSED: the build's catalogue, as JSON; each copy is it changed in
        # one place.
        own = json.load(open(sys.argv[1]))
        def write(name, value):
            with open(name, "w", encoding="utf-8") as f:
                json.dump(value, f, ensure_ascii=False)
        acting = next(m for m in own["modules"] if m["actions"])
        marked = next(m for m in own["modules"] if m["marks"])
        mark = marked["marks"][0]
        glyphs = {g["token"]: g for g in own["glyphs"]}
        moved = glyphs[mark["glyph"]]
        # ANOTHER MODULE GLYPH, measured to another size when one is — so the
        # size the mark is drawn at is the file's too, not only its codepoint.
        others = [g for g in own["glyphs"] if g["codepoint"] != moved["codepoint"]]
        other = next((g for g in others if g["size"] != moved["size"]), others[0])
        planted = copy.deepcopy(own)
        label = f"cmd-{acting['id']}-planted"
        for m in planted["modules"]:
            if m["id"] == acting["id"]:
                m["actions"].append({"id": "planted", "command": f"{acting['id']}-planted",
                                     "label": label, "row": False, "asks": None})
                m["keys"] = sorted(m["keys"] + [label])
        import pathlib
        for ftl in pathlib.Path("texts-planted").glob("*.ftl"):
            with open(ftl, "a", encoding="utf-8") as f:
                f.write(f"\n{label} = Planted\n")
        for g in planted["glyphs"]:
            if g["token"] == mark["glyph"]:
                for field in ("glyph", "codepoint", "size", "ink", "across"):
                    g[field] = other[field]
        write("planted.json", planted)
        lacking = copy.deepcopy(own)
        lacking["modules"][0]["keys"].append("made-up-key-nobody-has")
        write("lacking.json", lacking)
        misspelt = copy.deepcopy(own)
        told = next(e for m in misspelt["modules"] for e in m["events"] if e["notification"])
        told["notification"]["apply"]["show"] = "full"
        write("misspelt.json", misspelt)
        newer = copy.deepcopy(own)
        newer["version"] += 1
        write("newer.json", newer)
        write("mind.json", {"now": 1789800000, "marks": [{
            "module": marked["id"],
            "what": {"id": {"name": mark["name"], "glyph": mark["glyph"], "hint": mark["hint"]},
                     "tone": "danger", "count": 0}}]})
        write("expected.json", {
            "acting": acting["id"], "told": told["tag"],
            "moved": moved["codepoint"], "other": other["codepoint"],
            "size": moved["size"], "other-size": other["size"],
        })
        END

        # THE BUILD'S OWN FILE, and the copies the program reads at run time.
        RONOCTUA_BRAIN_CATALOGUE=$own "$shell" check-catalogue > ok.txt
        RONOCTUA_BRAIN_CATALOGUE=$own "$shell" check-keymap --dump > dump-own.txt
        RONOCTUA_BRAIN_CATALOGUE=$PWD/planted.json RONOCTUA_TEXTS=$PWD/texts-planted \
          "$shell" check-keymap --dump > dump-planted.txt
        RONOCTUA_BRAIN_CATALOGUE=$own "$shell" photograph --door footer --mind mind.json \
          --list --out own.png > list-own.txt
        RONOCTUA_BRAIN_CATALOGUE=$PWD/planted.json RONOCTUA_TEXTS=$PWD/texts-planted \
          "$shell" photograph --door footer --mind mind.json --list --out planted.png > list-planted.txt
        python3 - "$own" <<'END'
        import json, re, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the verdict `check-catalogue` prints, the `brain-action`
        # lines of the keymap's dump, the runs of the footer's display list
        # (`--list`), and the catalogue and the copies' facts as JSON.
        own = json.load(open(sys.argv[1]))
        expected = json.load(open("expected.json"))
        wrong = []
        said = open("ok.txt").read()
        m = re.search(r"catalogue OK: (\d+) modules, (\d+) marks, (\d+) actions, (\d+) events \((\d+) notifications\), (\d+) glyphs", said)
        if not m:
            sys.exit(f"ERROR: check-catalogue did not say its verdict: {said!r}")
        counted = tuple(int(x) for x in m.groups())
        held = (len(own["modules"]),
                sum(len(x["marks"]) for x in own["modules"]),
                sum(len(x["actions"]) for x in own["modules"]),
                sum(len(x["events"]) for x in own["modules"]),
                sum(1 for x in own["modules"] for e in x["events"] if e["notification"]),
                len(own["glyphs"]))
        if counted != held:
            wrong.append(f"check-catalogue counted {counted}, and the file holds {held}")
        floor(held[2], 10, "actions of the build's catalogue")

        # THE KEYMAP REGISTERS EACH AS A COMMAND (`command` lines: the
        # registry the leader runs), and the dump lists it as the
        # catalogue's (`brain-action` lines).
        def lines(path, kind):
            return sorted(l for l in open(path).read().splitlines() if l.startswith(kind + "\t"))
        listed = sorted(
            f"brain-action\t{x['id']}\t{a['id']}\t{a['command']}\t{a['label']}\trow={'true' if a['row'] else 'false'}\tasks={a['asks'] or '-'}"
            for x in own["modules"] for a in x["actions"])
        registered = {f"command\t{a['command']}\t{a['label']}" for x in own["modules"] for a in x["actions"]}
        if lines("dump-own.txt", "brain-action") != listed:
            wrong.append("the dump's actions are not the catalogue's: "
                         + ", ".join(sorted(set(lines("dump-own.txt", "brain-action")) ^ set(listed))))
        unregistered = sorted(registered - set(lines("dump-own.txt", "command")))
        if unregistered:
            wrong.append("the keymap registers no command for the catalogue's " + ", ".join(unregistered))
        planted = sorted(set(lines("dump-planted.txt", "command")) - set(lines("dump-own.txt", "command")))
        if len(planted) != 1 or not planted[0].startswith(f"command\t{expected['acting']}-planted\t"):
            wrong.append(f"the copy with one action more did not reach the keymap at run time: {planted}")

        # THE MARK, drawn with the file's glyph at the file's measured size.
        def mark_runs(path):
            return re.findall(r'OnTheCell\(([0-9.]+)\) \S+ "\\u\{([0-9a-f]+)\}"', open(path).read())
        def drew(path, codepoint, size):
            return (f"{float(size):.1f}", f"{codepoint:x}") in mark_runs(path)
        if not drew("list-own.txt", expected["moved"], expected["size"]):
            wrong.append(f"the footer did not draw the mark's glyph U+{expected['moved']:X} at its size: {mark_runs('list-own.txt')}")
        if not drew("list-planted.txt", expected["other"], expected["other-size"]) \
                or drew("list-planted.txt", expected["moved"], expected["size"]):
            wrong.append(f"the copy with the glyph moved did not draw U+{expected['other']:X}: {mark_runs('list-planted.txt')}")

        if wrong:
            print("ERROR: the footer's binary does not read the brain's catalogue at run time:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: {held[0]} modules read; the keymap lists their {held[2]} actions and one planted more;"
              " a moved glyph is drawn moved")
        END

        # REFUSED, IN ONE LINE NAMING THE VARIABLE, AND 78 — at the program's
        # start, before anything else (a window that would draw), and by
        # `check-catalogue`.
        refused() {
          what=$1
          said=$2
          shift 2
          for asked in "check-catalogue" "photograph --door calc --list"; do
            status=0
            # shellcheck disable=SC2086
            "$@" "$shell" $asked < /dev/null > /dev/null 2> said.txt || status=$?
            if [ "$status" != 78 ] || ! grep -q "RONOCTUA_BRAIN_CATALOGUE" said.txt || ! grep -qF -- "$said" said.txt; then
              echo "ERROR: $what was not refused by \`$asked\` with 78, naming the variable and \`$said\` (status $status):" >&2
              cat said.txt >&2
              exit 1
            fi
          done
          echo "OK: $what is refused with 78, naming the variable"
        }
        told=$(python3 -c 'import json; print(json.load(open("expected.json"))["told"])')
        refused "no catalogue" "is not set" env -u RONOCTUA_BRAIN_CATALOGUE
        refused "a file that is not there" "cannot be read" env RONOCTUA_BRAIN_CATALOGUE=$PWD/nowhere.json
        refused "another format" "format" env RONOCTUA_BRAIN_CATALOGUE=$PWD/newer.json
        refused "keys the texts lack" "made-up-key-nobody-has" env RONOCTUA_BRAIN_CATALOGUE=$PWD/lacking.json
        refused "a notification the book cannot read" "$told" env RONOCTUA_BRAIN_CATALOGUE=$PWD/misspelt.json
        if [ "$(env -u RONOCTUA_BRAIN_CATALOGUE "$shell" check-catalogue 2>&1 | grep -c 'is not set')" != 1 ]; then
          echo "ERROR: no catalogue is said more than once" >&2
          exit 1
        fi
        # AND THE LIFELINE IS NOT REFUSED FOR IT: it comes up because the
        # footer died, and draws no module's mark — here it gets as far as
        # asking for a compositor, which a check has none of.
        status=0
        env -u RONOCTUA_BRAIN_CATALOGUE -u WAYLAND_DISPLAY "$shell" lifeline made-up.service \
          < /dev/null > /dev/null 2> lifeline.txt || status=$?
        if [ "$status" = 78 ] || grep -q "RONOCTUA_BRAIN_CATALOGUE" lifeline.txt; then
          echo "ERROR: the lifeline was refused for the brain's catalogue, which it never reads (status $status):" >&2
          cat lifeline.txt >&2
          exit 1
        fi
        echo "OK: the lifeline passes its start with no catalogue (status $status, past the guard)"

        # THE FOOTER'S WRAPPER CARRIES THE BUILD'S FILE, `--set`: with none in
        # its environment and with another, it says what the build's says.
        for case in none planted; do
          status=0
          if [ "$case" = none ]; then
            env -u RONOCTUA_BRAIN_CATALOGUE ${rust.shell}/bin/ronoctua-shell check-catalogue \
              > "got.$case" 2> "said.$case" || status=$?
          else
            env RONOCTUA_BRAIN_CATALOGUE=$PWD/planted.json ${rust.shell}/bin/ronoctua-shell check-catalogue \
              > "got.$case" 2> "said.$case" || status=$?
          fi
          if [ "$status" != 0 ] || ! cmp -s ok.txt "got.$case"; then
            echo "ERROR: the footer's wrapper, with $case in its environment, did not read the build's catalogue (status $status):" >&2
            cat "said.$case" >&2
            diff ok.txt "got.$case" >&2 || true
            exit 1
          fi
        done
        grep -aqF "$own" ${rust.shell}/bin/ronoctua-shell || {
          echo "ERROR: the footer's wrapper does not name the build's catalogue" >&2
          exit 1
        }
        # AND NO PAINTER CARRIES ONE: none of them draws a module's mark.
        for painter in ${rust.rescue}/bin/ronoctua-rescue ${rust.installer}/bin/ronoctua-install \
          ${rust.player}/bin/ronoctua-player ${rust.shell}/bin/ronoctua-rescue ${rust.shell}/bin/ronoctua-player; do
          if grep -aq "RONOCTUA_BRAIN_CATALOGUE" "$painter"; then
            echo "ERROR: $painter carries the brain's catalogue, and draws no module's mark" >&2
            exit 1
          fi
        done
        echo "OK: the footer's wrapper reads the build's catalogue whatever its environment; no painter carries one"
      '';

  the-glyph-browser-names-the-modules-glyphs =
    mkCheck "the-glyph-browser-names-the-modules-glyphs"
      [
        pkgs.python3
        pkgs.coreutils
      ]
      ''
        # The data the icon browser's window reads, as the wrapper hands it:
        # this binary is the unwrapped one.
        export RONOCTUA_THEMES=${rust.keyed.RONOCTUA_THEMES}
        export RONOCTUA_TEXTS=${rust.keyed.RONOCTUA_TEXTS}
        export RONOCTUA_BRAIN_CATALOGUE=${catalogueFile}
        export RONOCTUA_ICONSET=${iconset}
        export VK_DRIVER_FILES=${xvfb.icd}
        export VK_ICD_FILENAMES=${xvfb.icd}
        export LIBGL_ALWAYS_SOFTWARE=1
        mkdir -p home run
        chmod 700 run
        export HOME=$PWD/home XDG_RUNTIME_DIR=$PWD/run
        python3 - ${iconset}/glyphs.json ${glyphdb.db}/glyphs.json ${glyphdb.db}/icons.json \
          ${catalogueFile} ${brainCatalogue.measured}/icons.json <<'END'
        import json, pathlib, re, subprocess, sys
        exec(open("${pyPrelude}").read())

        # PARSED: the browser's index, the glyph database's index and icon
        # table, the brain's catalogue and the glyphs measured for it — all
        # JSON, compared row by row; the window's display list (`--list`), run
        # by run; and the generator RUN on its own inputs, changed in one place.
        # FILE-WIDE: no source is searched; the label and the count the window
        # draws are read out of the build's texts, whole.
        index, database, chrome, catalogue, measured = (json.load(open(p, encoding="utf-8")) for p in sys.argv[1:6])
        wrong = []
        floor(len(index), 10000, "glyphs of the browser's index")
        modules = {g["token"]: g["glyph"] for g in catalogue["glyphs"]}
        floor(len(modules), 10, "glyph tokens of the brain's catalogue")

        # A glyph is named by the last token, in name order, that draws it.
        def named_by(tokens):
            by = {}
            for token in sorted(tokens):
                by[tokens[token]] = token
            return by
        drawn_by = {t: v["glyph"] for t, v in chrome.items()}
        alone, together = named_by(drawn_by), named_by({**drawn_by, **modules})

        # THE RULE IS THE GLYPH DATABASE'S: its own rows follow it over the
        # chrome's tokens, which are all it was given.
        astray = [r["name"] for r in database if r.get("token") != alone.get(r.get("name"))]
        if astray:
            wrong.append(f"the glyph database names {astray[:5]} by another rule than the last token that draws them")
        # THE BROWSER'S INDEX IS THE DATABASE'S, row for row, and names each
        # glyph by the chrome's and the modules' tokens together.
        if [r.get("name") for r in index] != [r.get("name") for r in database]:
            wrong.append("the browser's index does not hold the glyph database's rows, in its order")
        for got, was in zip(index, database):
            if {k: v for k, v in got.items() if k != "token"} != {k: v for k, v in was.items() if k != "token"}:
                wrong.append(f"the browser's row of {was.get('name')!r} is not the glyph database's")
                break
            if got.get("token") != together.get(got.get("name")):
                wrong.append(f"the browser names {got.get('name')!r} `{got.get('token')}`, and the tokens that"
                             f" draw it say `{together.get(got.get('name'))}`")
        # The glyphs a module's token names, as the rule says — not as the
        # index says, which is what is asked.
        rows = {r.get("name") for r in database}
        theirs = sorted((name, token) for name, token in together.items() if token in modules and name in rows)
        floor(len(theirs), 10, "glyphs the rule names by a module's token")

        # AT RUN TIME: the window, opened on each of those glyphs, says the
        # module's token in its facts; and the `token:` filter finds every
        # glyph the index names — the modules' among the chrome's.
        shell = "${rust.bins.ronoctua-shell}/bin/ronoctua-shell"
        texts = pathlib.Path("${rust.keyed.RONOCTUA_TEXTS}/en-GB.ftl").read_text(encoding="utf-8")
        def message(key):
            m = re.search(rf"^{key} = (.+)$", texts, re.M)
            if not m:
                sys.exit(f"ERROR: the build's texts have no `{key}`")
            return m.group(1)
        label = message("icons-fact-token")
        def photograph(*steps):
            r = subprocess.run([shell, "photograph", "--door", "icons", "--language", "en-GB", "--columns", "160",
                                *steps, "--list", "--out", "icons.png"], capture_output=True, text=True)
            if r.returncode != 0:
                sys.exit(f"ERROR: the icon browser's camera failed ({r.returncode}): {r.stderr.strip()[-400:]}")
            return re.findall(r'^run line (\d+) column \d+ \S+ \S+ "(.*)"$', r.stdout, re.M)
        for name, token in theirs:
            runs = photograph("--type", name, "--key", "RET")
            lines = {line for line, text in runs if text == label}
            said = [text for line, text in runs if line in lines and text != label]
            if said != [token]:
                wrong.append(f"the icon browser, opened on {name}, says the token {said}, not `{token}`")
        count = re.escape(message("icons-count")).replace(re.escape("{ $shown }"), r"(\d+)").replace(
            re.escape("{ $total }"), r"(\d+)")
        shown = [re.fullmatch(r"\s*" + count, text) for _, text in photograph("--type", "token:")]
        shown = [m for m in shown if m]
        named = sum(1 for r in index if r.get("token") is not None)
        if [(int(m.group(1)), int(m.group(2))) for m in shown] != [(named, len(index))]:
            wrong.append(f"the `token:` filter shows {[m.group(0) for m in shown]}, and the index names {named}"
                         f" of {len(index)}")

        # AND THE GENERATOR REFUSES, each by its sentence, its own inputs
        # changed in one place — after the real ones are written again.
        import tempfile
        work = pathlib.Path(tempfile.mkdtemp())
        given = {"glyphs": sys.argv[2], "icons": sys.argv[3], "modules": sys.argv[5]}
        def generate(**changed):
            args = []
            for name, path in {**given, **changed}.items():
                args += [f"--{name}", str(path)]
            return subprocess.run([sys.executable, "${../nix/scripts/gen-iconset.py}", *args,
                                   "--out", str(work / "glyphs.json")], capture_output=True, text=True)
        def planted(name, value):
            path = work / name
            path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
            return path
        r = generate()
        if r.returncode != 0 or (work / "glyphs.json").read_bytes() != pathlib.Path(sys.argv[1]).read_bytes():
            wrong.append("the generator, handed its own inputs again, did not write the build's index: " + r.stderr)
        token = sorted(modules)[0]
        renamed = [dict(row) for row in database]
        first = next(row for row in renamed if row.get("token"))
        first["token"] = "a-token-that-names-nothing"
        for what, changed, said in (
            ("a module's token that is also the chrome's",
             {"modules": planted("modules.json", {**measured, next(iter(chrome)): measured[token]})},
             "is a brain module's glyph token and one of assets/tokens/icons.nix"),
            ("a database that names its glyphs another way",
             {"glyphs": planted("renamed.json", renamed)},
             "names its glyphs another way"),
        ):
            r = generate(**changed)
            if r.returncode == 0 or said not in r.stderr:
                wrong.append(f"the generator did not refuse {what} with `{said}`: {r.stderr.strip()[-300:]}")

        if wrong:
            print("ERROR: the glyph browser does not name the brain modules' glyphs by their tokens:", file=sys.stderr)
            for w in wrong:
                print("  " + w, file=sys.stderr)
            sys.exit(1)
        print(f"OK: the browser names {named} glyphs by a token, {len(theirs)} by a module's, and says each of"
              f" those when opened on it; the generator refuses its 2 plants")
        END
      '';
}
