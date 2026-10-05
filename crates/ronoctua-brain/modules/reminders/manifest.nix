# ALARMS AND REMINDERS, asked for in the person's own words.
#
# A watch with no source but its own file and one clock, armed only for the
# next reminder. What it raises is an ALARM on the brain's line, which beats a
# countdown and leaves only when a person answers it. Why each piece is so is
# in `why.md`, beside this.
{
  what = "Alarms and reminders, once or every day, week, month or year.";

  needs = {
    root = false;
    network = false;
    # THE CHIME goes through the one crate that knows PipeWire's programs
    # (`ronoctua_audio::chime`, `pw-play` by the store path the brain's unit
    # is given); the FILE through the store every preference goes through;
    # and the CALENDAR — a day number into a date — through the one copy of
    # that arithmetic in the tree (`ronoctua_files::when`).
    boundaries = [
      "ronoctua-audio"
      "ronoctua-store"
      "ronoctua-files"
    ];
  };

  # NO SCHEDULE: a watch. Its only clock is `next_wake`, armed for the next
  # reminder and for nothing when there is none.
  schedule = null;

  # The words an action may be refused with — each drawn from its own key,
  # `reminders-why-<word>`.
  whys = [
    "not-understood"
    "passed"
    "no-such-day"
    "unreadable"
    "gone"
    "not-ringing"
    "full"
  ];

  glyphs = {
    ringing = "md-alarm";
  };

  # WHILE SOMETHING RINGS, and never at rest (invariant 14). A click opens the
  # module's page, where every ringing reminder can be dismissed or put off.
  marks = {
    ringing = {
      glyph = "ringing";
      hint = "reminders-mark-ringing";
    };
  };

  # `due` IS THE ALARM: first on the brain's line, and it leaves only when it
  # is dismissed or put off — for five minutes from the line (the first
  # snooze), or for any of the three from the module's page. `missed` is what
  # came due while the machine was off: a warning, until it is dismissed.
  # `unreadable` is the file that will not parse, said on the line while it
  # lasts — it leaves when the file is fixed, not when it is dismissed.
  notices = {
    due = {
      level = "alarm";
      dismiss = true;
      snooze = [
        5
        10
        60
      ];
    };
    missed = {
      level = "warn";
      dismiss = true;
      snooze = [ ];
    };
    unreadable = {
      level = "warn";
      dismiss = false;
      snooze = [ ];
    };
  };

  # `new` asks when and what; the rest act on a row of the module's page.
  actions = {
    new = {
      asks = "ask-reminders-new";
    };
    when = {
      row = true;
      asks = "ask-reminders-when";
    };
    repeat = {
      row = true;
    };
    sound = {
      row = true;
    };
    forget = {
      row = true;
    };
    dismiss = {
      row = true;
    };
    snooze = {
      row = true;
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the chime's program, from the crate
  # that knows PipeWire; the table of the words its answers are read with is
  # made by the texts' own parser.
  uses = [
    "crates/ronoctua-audio/src/pipewire.rs"
    "nix/scripts/gen-i18n.py"
    "assets/i18n/languages.nix"
  ];

  # THE VM HARNESS runs every module's `vm.nix` without naming any: nothing
  # outside this folder names it.
  namedBy = { };
}
