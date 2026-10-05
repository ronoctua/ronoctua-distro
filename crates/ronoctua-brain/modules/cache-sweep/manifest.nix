# NOTHING CACHED STAYS FOR EVER: what nothing has used for a while is taken,
# at night, every few days.
#
# The brain owns the WHEN and nothing else. The walk is a unit and a binary
# of its own (`module.nix`, `ronoctua-cache-sweep`); what is left here is
# whether this is a good moment — which the gate the person configured
# answers once for every module. Why each number is what it is is in `why.md`.
let
  # THE CATALOGUE'S cadence and window: one declaration for the sweep, the
  # file manager's thumbnails and this schedule (`assets/cache.nix`).
  catalogue = import ../../../../assets/cache.nix;
in
{
  what = "Take cached files nothing has used for a while, at night, every few days.";

  needs = {
    root = false;
    network = false;
    # It starts a unit and reads the report the unit writes: no crate of a
    # boundary is called from here.
    boundaries = [ ];
  };

  schedule = {
    kind = "routine";
    calendar = true;
    intervalDays = catalogue.everyDays;
    partOfDay = catalogue.partOfDay;
    # NO COUNTDOWN. It interrupts nothing and nothing it does is visible while
    # it happens: a minute of warning about deleting files nobody has opened in
    # weeks would be a band in the way for no decision.
    countdownSeconds = 0;
    # A PATIENCE OF MINUTES, NOT HOURS: two hours belongs to a run that
    # compiles a system, and a walk that died silently would not be noticed
    # until the afternoon.
    runawayMinutes = 30;
    # Busy, and on battery, are the gate's questions — asked before it is
    # reached, and configured by the person in one place.
    gated = true;
  };

  # TWO UNITS AND NOT A FLAG: `m e` on the brain's page rehearses, and a flag
  # crossing from the brain would be a flag somebody else could cross with.
  # Declared by `module.nix`, under these names.
  units = {
    run = "ronoctua-cache-sweep.service";
    rehearse = "ronoctua-cache-sweep-rehearsal.service";
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the walk's binary, the catalogue its
  # numbers come from, and the options a machine may change them with.
  uses = [
    "crates/ronoctua-cache-sweep"
    "assets/cache.nix"
    "nix/modules/cache.nix"
  ];

  # `ronoctua.cache.enable = false` IS THIS MODULE'S SWITCH TURNED OFF by
  # default — the catalogue's "throw nothing away" reaching the one switch the
  # units and the brain both answer to — and that line names it by its id.
  namedBy = {
    "nix/modules/cache.nix" = [ "cache-sweep" ];
  };
}
