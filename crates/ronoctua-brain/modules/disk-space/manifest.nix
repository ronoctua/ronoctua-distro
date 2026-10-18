# THE DISK MAKES ROOM FOR ITSELF, and says so while it cannot.
#
# The first module that moved into its folder, and the one the others copy:
# a run the machine calls for (no calendar, no gate), a mark on the modeline,
# a page of its own, and two things it tells the footer. Why each is so is in
# `why.md`, beside this.
{
  what = "Make room on the disk when it is short; mark the modeline while it is nearly full.";

  needs = {
    root = false;
    network = false;
    # THE PULSE'S READING of the filesystems comes through the one crate that
    # may look (`statvfs`, through `crate::pulse`); the module reads bytes and
    # never opens anything itself.
    boundaries = [ "ronoctua-memory" ];
  };

  # NO CALENDAR, NO WINDOW, NO COUNTDOWN AND NO GATE: a disk short of space has
  # no good hour, a warning before making room would be a countdown the person
  # can only answer by letting the disk fill, and a relief the gate held until
  # the memory calmed down would wait for the build that fills the disk to end.
  # AN HOUR OF PATIENCE: a collection of a large store at idle priority is
  # minutes, not the two hours of a rebuild.
  schedule = {
    kind = "routine";
    calendar = false;
    partOfDay = "any";
    countdownSeconds = 0;
    runawayMinutes = 60;
    gated = false;
  };

  # TWO UNITS AND NOT A FLAG: `m e` on the brain's page rehearses, and a flag
  # crossing from the brain would be a flag somebody else could cross with.
  # Declared by `module.nix`, under these names.
  units = {
    run = "ronoctua-disk-relief.service";
    rehearse = "ronoctua-disk-relief-rehearsal.service";
  };

  # Its row says "when the disk is short" where a calendar row says a date.
  whys = [ "when-short" ];

  # STORAGE WITH AN ALERT: the disk token's own glyph with a badge would have
  # been the first choice, and Material has none that says "full" — its
  # `harddisk_remove` draws a cross, which reads as a disk that failed.
  glyphs = {
    full = "md-database_alert";
  };

  # THE ONE MARK, and only while it says something (invariant 14): while any
  # filesystem the relief measures is under its "nearly full" line
  # (`assets/cache.nix`, `diskWarnPercent` with its cap). A click opens the
  # module's page, which says how much is free and where the two lines are.
  marks = {
    full = {
      glyph = "full";
      hint = "disk-space-mark-full";
    };
  };

  # WHAT A RELIEF DID, told once. The first is a fact that goes by, like the
  # charger. The second is the one event the person has something to DO about
  # — nothing more could be freed without them — so the bell keeps it (no
  # auto-read), and it is a step louder in the list.
  #
  # THE OLD TAGS RIDE ALONG (`tags`): the two events were `disk-relieved` and
  # `disk-still-short` before the relief moved into this folder, and a filter
  # somebody saved under either still catches its event.
  events = {
    relieved = {
      notification = {
        show = "full";
        life = "quick";
        loud = false;
        level = 25;
        auto-read = true;
        icon = "md-broom";
        tags = [
          "disk-relieved"
          "disk"
        ];
      };
    };
    still-short = {
      notification = {
        show = "full";
        life = "quick";
        loud = false;
        level = 30;
        icon = "disk";
        tags = [
          "disk-still-short"
          "disk"
        ];
      };
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the relief's binary (the sweep's
  # crate), and the catalogue its numbers come from, through the one
  # generator that refuses them when they stop holding each other.
  uses = [
    "crates/ronoctua-cache-sweep"
    "assets/cache.nix"
    "nix/cache.nix"
  ];

  # WHO NAMES IT FROM OUTSIDE: the granularity checks take this module as
  # their sample of a brain module edited (`granularity-of-an-edit`) and of a
  # data entry that names no file (`data-entries-name-files`).
  namedBy = {
    "tests/build-granularity.nix" = [ "disk-space" ];
  };
}
