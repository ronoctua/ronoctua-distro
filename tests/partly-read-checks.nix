# A PERSON'S FILE READ ONLY IN PART SAYS SO IN ITS OWN VIEW (owner,
# 2026-11-01): the store reads a preference file leniently — a value this
# machine cannot take is repaired, the rest kept — and the view that shows the
# file says one line when that happened: which places were not read, and why
# (`ronoctua_shell_kit::partly_read`). The belongings list the repairs too,
# but a person changing the sound settings is in the sound view.
#
#   - `a-view-says-what-it-did-not-read`: every preference file of the
#     belongings' census (`assets/belongings.nix`, `stores`, every file a
#     `Store` of the tree writes) is either the file of a view that says the
#     line, or named with why no view owes it (read whole or refused, shown by
#     no view), or a debt, with what stands in the way. A view says it when
#     its crate's code, tests off, asks the kit what of THAT file did not read
#     — the call naming its store — and draws the sentence (`partly.said(`).
#     A preference file the tables do not name is refused, and so is an entry
#     the census no longer has: a new store is a decision.
#
#     The rule is asked of a planted tree first — a view of a lenient store
#     that says nothing, one that calls the kit for another store, one that
#     never draws the sentence, and one that does it right — so the sweep is
#     known to refuse what it is for before it is believed about the tree.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
  shellPrelude,
  belongingsTable,
}:
{
  a-view-says-what-it-did-not-read = mkCheck "a-view-says-what-it-did-not-read" [ pkgs.python3 ] ''
    python3 - <<'END'
    import json, pathlib, re, sys, tempfile
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # PARSED: the census arrives as the generator's JSON (`belongings.json`).
    # FILE-WIDE: a view may read and say its file in any module of its crate
    # (the view, its window, its region), so every source of the crate is read
    # whole — comments off, strings kept, its tests blanked (`uncommented`,
    # `outside_tests`): a test that says the line is no screen that does.
    census = json.load(open("${belongingsTable.table}/belongings.json"))
    preferences = sorted(s["path"] for s in census["stores"] if s["reason"] == "preference")
    floor(len(preferences), 25, "preference files in the census")

    # THE VIEWS: each preference file, the shell crate of the view that shows
    # it, and the call by which that crate asks the kit what of THAT file did
    # not read — naming its store, its constant or the kit's reader of it —
    # compared with the code's spaces taken out (`flat`), so a line broken by
    # `rustfmt` is the same call.
    VIEWS = {
        "config/atuin.json": ("ronoctua-shell-history", "partly_read::read(&ronoctua_atuin::prefs::store())"),
        "config/audio.json": ("ronoctua-shell-audio", "partly_read::read(&ronoctua_audio::prefs::store())"),
        "config/brain/agent-accounts/lines.json": ("ronoctua-shell-agents", "partly_read::judged(&agent_accounts::lines_store("),
        "config/calc.json": ("ronoctua-shell-calc", "partly_read::judged(&self.store)"),
        "config/clipcascade.json": ("ronoctua-shell-clipboard", "partly_read::judged(&ronoctua_clipcascade::prefs::store())"),
        "config/camera.json": ("ronoctua-shell-camera", "partly_read::read(&ronoctua_camera::prefs::store())"),
        "config/consent.json": ("ronoctua-shell-player", "partly_read::judged(&ronoctua_consent::store())"),
        "config/display.json": ("ronoctua-shell-display", "PartlyRead::of_repairs(ronoctua_display::prefs::FILE,"),
        "config/fast-access.json": ("ronoctua-shell-fast-access", "partly_read::judged(&self.store)"),
        "config/language.json": ("ronoctua-shell-locale", "language::read_in_part()"),
        "config/llm-accounts.json": ("ronoctua-shell-agents", "partly_read::judged(&Accounts::store())"),
        "config/locale.json": ("ronoctua-shell-locale", "prefs::read_in_part()"),
        "config/terminal.json": ("ronoctua-shell-terminal", "partly_read::PartlyRead::of(FILE,&reading)"),
        "config/modeline.json": ("ronoctua-shell-footer", "partly_read::PartlyRead::of(FILE,reading)"),
        "config/notifications.json": ("ronoctua-shell-notifications", "preferences_store().and_then(|store|crate::partly_read::judged(&store))"),
        "config/osk.json": ("ronoctua-shell-footer", "partly_read::read(&crate::osk::store())"),
        "config/player-favourites.json": ("ronoctua-shell-player", "partly_read::judged(favourites.store())"),
        "config/player-websites.json": ("ronoctua-shell-player", "partly_read::read(&store())"),
        "config/player.json": ("ronoctua-shell-player", "partly_read::judged(&ronoctua_player::prefs::store())"),
        "config/pointers.json": ("ronoctua-shell-pointers", "PartlyRead::of_repairs(ronoctua_pointers::prefs::FILE,"),
        "config/power.json": ("ronoctua-shell-power", "PartlyRead::of_repairs(ronoctua_power::prefs::FILE,"),
        "config/software.json": ("ronoctua-shell-software", "partly_read::read(&ronoctua_software::prefs::store(&config))"),
        "config/studio.json": ("ronoctua-shell-camera", "partly_read::read(&prefs::store())"),
        "config/studio/scenes.json": ("ronoctua-shell-camera", "partly_read::judged(&prefs::book())"),
        "config/syncthing.json": ("ronoctua-shell-syncthing", "partly_read::read(&ronoctua_syncthing::prefs::store())"),
        "config/tor.json": ("ronoctua-shell-network", "PartlyRead::of_repairs(ronoctua_tunnel::prefs::FILE,"),
        "config/world.json": ("ronoctua-shell-locale", "partly_read::read(&ronoctua_world::prefs::store())"),
    }

    # THE FILES NO VIEW OWES THE LINE, each with why. Exact both ways.
    NO_LINE = {
        "config/brain/reminders/reminders.json": "read whole or refused (`Store::whole`): a reminder repaired away is"
                                                 " one that does not ring, and its page says the file does not read",
        "config/file-picker.json": "no view shows it: the folder each purpose of the picker opens in",
        "config/gestures.json": "no view shows it: the area calibrated, read as the gestures come on",
        "config/lock.json": "read whole or refused (`lock_choice::armed_by`), and the security centre says the refusal",
        "config/theme.json": "its chooser says each answer of it refused, by field and value (`theme::Complaint`,"
                             " `echo-theme-refused`), and its writer refuses a file with a repair (`read_whole`)",
        "config/toolkits.json": "no view shows it: the sizes it keeps are the toolkits' thread's own",
    }

    # THE DEBT: a view shows the file and does not say the line yet, each with
    # what stands in the way — printed on every run, so it is never silent.
    # When its view says it, the file moves to VIEWS.
    OWED = {
        "config/brain.json": "read by the brain's daemon and shown by the brain's buffer: the repairs would"
                             " travel in the `Mind`, through the protocol",
        "config/brain/journal-errors/filters.json": "read leniently by the brain's daemon and shown by the module's"
                                                    " page, whose cells carry a key with no arguments",
        "config/brain/journal-errors/ignored.json": "read leniently by the brain's daemon and shown by the module's"
                                                    " page, whose cells carry a key with no arguments",
        "config/brain/journal-errors/options.json": "read leniently by the brain's daemon and shown by the module's"
                                                    " page, whose cells carry a key with no arguments",
        "config/files-sidebar.json": "the sidebar has no line for a sentence, by design (`files::side_fields`)",
        "config/index.json": "shown by a brain module's page, whose cells carry a key with no arguments",
        "config/keyboards.json": "shown by a buffer: the host hands it `Boards` through the WIT, which has no field"
                                 " for a sentence the host has said",
    }

    # THE SENTENCE DRAWN: the kit's value said, by the name every view gives
    # it (`partly`).
    SAID = re.compile(r"\bpartly\.said\(")

    def sources_of(root, crate):
        found = []
        for c in shell_crates(root):
            if c.name == crate:
                found = sorted((c / "src").rglob("*.rs"))
        return found

    def flat(code):
        # The code with its spaces out, and the comma `rustfmt` leaves before
        # a bracket it broke a call's arguments onto lines for.
        return re.sub(r",([)\]}])", r"\1", re.sub(r"\s+", "", code))

    def judged(root, views):
        wrong, asked = [], 0
        for logical, (crate, call) in sorted(views.items()):
            files = sources_of(root, crate)
            if not files:
                wrong.append(f"{logical}: its view {crate} is not a shell crate")
                continue
            named, said = False, False
            for f in files:
                code = uncommented(outside_tests(f.read_text()))
                if "partly_read" in code or "read_in_part" in code:
                    asked += 1
                if re.sub(r"\s+", "", call) in flat(code):
                    named = True
                if SAID.search(code):
                    said = True
            if not named:
                wrong.append(f"{logical}: {crate} never asks the kit what of it did not read (`{call}`):"
                             " a part of the file it shows that did not read is shown as if the file said it")
            elif not said:
                wrong.append(f"{logical}: {crate} judges the file and never draws the sentence (`partly.said(`)")
        return wrong, asked

    # 1) THE RULE, ASKED OF A PLANTED TREE FIRST.
    PLANTED = {
        "silent": "pub fn rows() { let prefs = ronoctua_x::prefs::store().read(); draw(prefs); }\n",
        "another": ("pub fn rows(t: &Texts) { let (p, partly) = ronoctua_shell_kit::partly_read::read("
                    "&ronoctua_y::prefs::store()); if let Some(partly) = partly { note(partly.said(t)); } }\n"),
        "unsaid": "pub fn rows() { let (p, _partly) = partly_read::read(&ronoctua_x::prefs::store()); }\n",
        "tested": ("pub fn rows() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let (_, l) = partly_read::read("
                   "&ronoctua_x::prefs::store()); l.unwrap().said(&texts()); partly.said(&t); }\n}\n"),
        "commented": ("// partly_read::read(&ronoctua_x::prefs::store()) and partly.said(\n"
                      "pub fn rows() { let prefs = ronoctua_x::prefs::store().read(); }\n"),
        "right": ("pub fn rows(t: &Texts) { let (p, partly) = ronoctua_shell_kit::partly_read::read("
                  "&ronoctua_x::prefs::store()); if let Some(partly) = partly { note(partly.said(t)); } }\n"),
    }
    with tempfile.TemporaryDirectory() as planted:
        for case, text in PLANTED.items():
            src = pathlib.Path(planted) / "crates" / f"ronoctua-shell-{case}" / "src"
            src.mkdir(parents=True)
            (src / "view.rs").write_text(text)
        for case in PLANTED:
            wrong, _ = judged(planted, {"config/x.json": (f"ronoctua-shell-{case}",
                                                           "partly_read::read(&ronoctua_x::prefs::store())")})
            if (case == "right") == bool(wrong):
                print(f"ERROR: the rule misjudges a planted view ({case}): {wrong or 'it passed'}", file=sys.stderr)
                raise SystemExit(1)

    # 2) THE TREE.
    wrong = []
    tables = {"VIEWS": set(VIEWS), "NO_LINE": set(NO_LINE), "OWED": set(OWED)}
    for a, b in [("VIEWS", "NO_LINE"), ("VIEWS", "OWED"), ("NO_LINE", "OWED")]:
        for logical in sorted(tables[a] & tables[b]):
            wrong.append(f"{logical} is in {a} and in {b}: which is it?")
    named = tables["VIEWS"] | tables["NO_LINE"] | tables["OWED"]
    for logical in preferences:
        if logical not in named:
            wrong.append(f"{logical} is a preference file of the census that names no view: say which view shows"
                         " it (VIEWS) and make it say the kit's line, or why none owes it (NO_LINE)")
    for logical in sorted(named - set(preferences)):
        wrong.append(f"{logical} is no preference file of the census any more: take it off this check")
    found, asked = judged(None, VIEWS)
    wrong += found
    floor(len(VIEWS), 20, "views of a preference file")
    floor(asked, 15, "files of the views that ask the kit")

    if wrong:
        print("ERROR: a view of a person's file read only in part does not say so.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        raise SystemExit(1)
    print(f"OK: {len(VIEWS)} preference files said by their views when read in part, {len(NO_LINE)} owed by none,"
          f" {len(OWED)} still owed (" + ", ".join(sorted(OWED)) + f"); {len(PLANTED)} planted views judged")
    END
  '';
}
