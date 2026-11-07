# WHAT WENT WRONG ON THE MACHINE, grouped, until somebody has seen it.
#
# A watch on the journal the core already follows: every entry at `err` or
# worse, and the warnings in a list of their own, of the system's and the
# person's journals, from this boot or from where it stopped. What the journal
# wrote while it was off is read when it comes back on, or at the person's word
# while it stays off (`run`). Entries of one kind — the same unit saying the same thing
# with other numbers — are one row with a count. While a kind worth a look is
# not seen yet, a mark on the modeline says how many there are, in the colour of
# a watch, or of a crisis while one is critical. A snippet the person filters by
# holds the lines that carry it — off the list and off the mark — and a shipped
# list of them comes on, each with its reason, to turn off. Why each is so is in
# `why.md`.
{
  what = "Group the journal's errors by unit and kind and mark the modeline until they are seen, with the warnings in a list of their own.";

  needs = {
    root = false;
    network = false;
    # THIS BOOT, told apart from the ones before it, is the kernel's boot id
    # (`/proc/sys/kernel/random/boot_id`), read through the one crate that may
    # read `/proc`. And the kinds the person ignores are their file, written
    # through the one crate that writes a person's file (read with `parsed`,
    # never written over when it does not parse).
    boundaries = [
      "ronoctua-memory"
      "ronoctua-store"
    ];
  };

  # A WATCH: nothing to run, nothing to gate, nothing to announce.
  schedule = null;

  # SHIPPED SWITCHED OFF. Reading the machine's own errors is a thing the
  # person opts into, not one they inherit: the row and its switch are on the
  # brain's page from the first boot, and it begins watching only once the
  # person turns it on. Its switch is the same one every module has, so turning
  # it on is a tap and no rebuild (owner, 2026-10-26).
  startsOff = true;

  # The words it refuses with: the kind (or filter) is not in its list any
  # more (ignored, filtered, or forgotten while the page was open); a row's
  # action asked by name, with no row; a filter asked for with no snippet; a
  # person's file (of ignored kinds or of filters) which does not parse and is
  # left as it is rather than written over; and a read asked for while one is
  # on its way.
  whys = [
    "gone"
    "no-row"
    "empty"
    "unreadable"
    "reading"
  ];

  # THE MODELINE'S MARK ALONE: on the list of tasks it is a watch like any
  # other, with the watches' eye.
  glyphs = {
    unseen = "md-skull";
  };

  # ONE MARK, with the number of kinds not seen yet — and only while there is
  # one (invariant 14). The colour of a watch, the gestures' glyph's, and of a
  # crisis while one of them is critical. A click opens its page, where each
  # kind is.
  marks = {
    unseen = {
      glyph = "unseen";
      hint = "journal-errors-mark-unseen";
    };
  };

  # WHAT A PERSON DOES WITH A KIND. Each row of its page does ONE of them
  # (the row's own action) and may offer a second (`also`): a kind not seen yet
  # is marked seen, or ignored from then on; a seen one or a warning is ignored,
  # or filtered by its own line (`filter-line`, which starts from the line for
  # the person to cut down); and an ignored one is heard again. `seen-all` is
  # the page's, for everything at once.
  #
  # AND WHAT A PERSON DOES WITH A FILTER. `filter` is the page's: it asks for
  # a snippet and holds from then on every line whose `<unit>: <message>` carries
  # it (case folded) — off the list and off the mark. A filter's row turns it
  # off (`unfilter`: the person's own leaves the page, a shipped one stays to
  # turn on again) or back on (`refilter`), and says why it exists (`reason`,
  # where a blank answer clears what was said). All are also commands, by name;
  # a row's by name with no row says to choose one on the page.
  #
  # AND WHAT THE JOURNAL ITSELF IS FOR. `follow` is the way to a live terminal
  # on it, which is a door's and no business of this module's. `run` reads what
  # was written since it stopped, once, and only exists while it is off: on, it
  # hears everything as it happens.
  actions = {
    seen = {
      row = true;
    };
    ignore = {
      row = true;
    };
    unignore = {
      row = true;
    };
    seen-all = { };
    filter = {
      asks = "ask-journal-errors-filter";
    };
    filter-line = {
      row = true;
      asks = "ask-journal-errors-filter-line";
    };
    unfilter = {
      row = true;
    };
    refilter = {
      row = true;
    };
    reason = {
      row = true;
      asks = "ask-journal-errors-reason";
      optional = true;
    };
    follow = {
      opens = "journal-follow";
    };
    run = {
      whileOff = true;
    };
  };

  uses = [
    "crates/ronoctua-journal"
  ];

  # THE PERSON'S CHOICES ARE THEIR BELONGINGS: the census lists the kinds
  # they ignored and the filters they wrote as preferences, and the check
  # that every view says what of its file it did not read names the same two.
  namedBy = {
    "assets/belongings.nix" = [ "journal-errors" ];
    "tests/partly-read-checks.nix" = [ "journal-errors" ];
  };
}
