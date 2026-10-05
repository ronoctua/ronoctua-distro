# THE SUPER INDEX OF FAST-ACCESS IS STARTED, and the modeline says so only
# while it is built from scratch.
#
# The index is the unit's (`ronoctua-index serve`, declared by
# `nix/modules/index.nix`, with no `wantedBy` on purpose); the brain owns the
# WHEN — the first start of a session, under the gate the person configured —
# the one mark, and the page that says where the index stands and what it
# reads. Why each is so is in `why.md`, beside this.
{
  what = "Start fast-access's super index when it is not running; mark the modeline while it is built from scratch.";

  needs = {
    root = false;
    network = false;
    # WHAT THE UNIT SAYS (`status.json`, beside the index) and what the person
    # chose (`index.json`) come through the one crate that knows both, without
    # its engine: the brain never links tantivy.
    boundaries = [ "ronoctua-index" ];
  };

  # NO CALENDAR AND NO COUNTDOWN: the unit runs for as long as the session
  # does, and what the brain decides is the moment it starts — when it is not
  # running — which `wanted_now` answers. INSIDE THE GATE: a first walk of a
  # home is minutes of disk, not something to begin on battery in the middle
  # of a call. A few minutes of patience: starting a unit is quick.
  schedule = {
    kind = "routine";
    calendar = false;
    partOfDay = "any";
    countdownSeconds = 0;
    runawayMinutes = 5;
    gated = true;
  };

  # ONE UNIT AND NO REHEARSAL: a rehearsal of "start the index" starts
  # nothing, and says so.
  units = {
    run = "ronoctua-index.service";
  };

  # Its row says "when it is not running" where a calendar row says a date;
  # the rest are why a setting typed on its page was not kept.
  whys = [
    "when-stopped"
    "not-a-folder"
    "not-a-skip"
    "unkept"
  ];

  # A DATABASE BEING SEARCHED, for an index being built.
  glyphs = {
    building = "md-database_search";
  };

  # THE ONE MARK, and only while it says something (invariant 14): while the
  # unit walks an index that began EMPTY — a first walk, or one of another
  # format wiped. Following changes afterwards is seconds a day and no mark.
  marks = {
    building = {
      glyph = "building";
      hint = "search-index-mark-building";
    };
  };

  # WHAT IT READS, changed from its page: each writes `index.json`, which the
  # unit watches and walks again by. A root and a name left out are typed on
  # the door's line; a row's own action forgets it.
  actions = {
    add-root = {
      asks = "ask-search-index-add-root";
    };
    forget-root = {
      row = true;
    };
    contents = {
      row = true;
    };
    hidden = {
      row = true;
    };
    add-skip = {
      asks = "ask-search-index-add-skip";
    };
    forget-skip = {
      row = true;
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the index's crate, and the unit.
  uses = [
    "crates/ronoctua-index"
    "nix/modules/index.nix"
  ];

  # THE FILES THAT SAY THE UNIT'S NAME: the cache's numbers, whose comment
  # names the one unit they are handed to, and the central checks of the
  # index, which run it.
  namedBy = {
    "assets/cache.nix" = [ "ronoctua-index.service" ];
    "tests/checks.nix" = [ "ronoctua-index.service" ];
    "tests/fast-access-vm.nix" = [ "ronoctua-index.service" ];
  };
}
