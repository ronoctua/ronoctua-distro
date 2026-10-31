# WHAT WENT WRONG ON THE MACHINE, grouped, until somebody has seen it.
#
# A watch on the journal the core already follows: every entry at `err` or
# worse, of the system's and the person's journals, from the moment it began
# watching. This boot's last few come too only when it is already on at the
# brain's very first start, before any cursor is kept; switched on later — and
# it ships off — it hears from then on (`docs/PENDING.md`). Entries of one kind
# — the same unit saying the same thing with other numbers — are one row with
# a count. While a kind is not seen yet, a mark on the modeline says how many
# there are, in the colour of a watch. A snippet the person filters by holds the
# lines that carry it — off the list and off the mark — and one is shipped on
# (the `dbus-broker` duplicate-name noise), to turn off. Why each is so is in
# `why.md`.
{
  what = "Group the journal's errors by unit and kind, and mark the modeline until they are seen.";

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
  # action asked by name, with no row; a filter asked for with no snippet; and
  # a person's file (of ignored kinds or of filters) which does not parse and
  # is left as it is rather than written over.
  whys = [
    "gone"
    "no-row"
    "empty"
    "unreadable"
  ];

  glyphs = {
    unseen = "md-file_alert_outline";
  };

  # ONE MARK, with the number of kinds not seen yet — and only while there is
  # one (invariant 14). The colour of a watch, the gestures' glyph's. A click
  # opens its page, where each kind is.
  marks = {
    unseen = {
      glyph = "unseen";
      hint = "journal-errors-mark-unseen";
    };
  };

  # WHAT A PERSON DOES WITH A KIND. Each row of its page does ONE of them
  # (the row's own action): a kind not seen yet is marked seen, a seen one is
  # ignored from then on, and an ignored one is heard again. `seen-all` is the
  # page's, for everything at once.
  #
  # AND WHAT A PERSON DOES WITH A FILTER. `filter` is the page's: it asks for
  # a snippet and holds from then on every line whose `<unit>: <message>` carries
  # it (case folded) — off the list and off the mark. A filter's row turns it
  # off (`unfilter`: the person's own leaves the page, a shipped one stays to
  # turn on again) or back on (`refilter`). All are also commands, by name; a
  # row's by name with no row says to choose one on the page.
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
    unfilter = {
      row = true;
    };
    refilter = {
      row = true;
    };
  };

  uses = [
    "crates/ronoctua-journal"
  ];

  namedBy = { };
}
