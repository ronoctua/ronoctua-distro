# THE TRASH IS EMPTIED OF WHAT HAS BEEN IN IT LONG ENOUGH — and it is not the
# cache sweep, and must never become it: nothing in the trash can be made
# again. The brain owns the WHEN; the walk is a unit and a binary of its own
# (`module.nix`, `ronoctua-empty-trash`). Why each number is what it is is in
# `why.md`.
let
  # THE CATALOGUE'S cadence and window (`assets/cache.nix`): the cadence is
  # the trash's own number, the window the one every sweep keeps.
  catalogue = import ../../../../assets/cache.nix;
in
{
  what = "Empty the trash of what has been in it long enough, at night, every few days.";

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
    intervalDays = catalogue.trashEveryDays;
    partOfDay = catalogue.partOfDay;
    # NO COUNTDOWN. It interrupts nothing and nothing it does is visible while
    # it happens: a minute of warning about emptying what was thrown away days
    # ago would be a band in the way for no decision.
    countdownSeconds = 0;
    # A PATIENCE OF MINUTES, NOT HOURS: a trash is a handful of notes and what
    # they name, and two hours belongs to a run that compiles a system.
    runawayMinutes = 30;
    gated = true;
  };

  # TWO UNITS AND NOT A FLAG, for the sweep's reason — and it matters more
  # here: *"how much would this take?"* of something that cannot be undone.
  units = {
    run = "ronoctua-empty-trash.service";
    rehearse = "ronoctua-empty-trash-rehearsal.service";
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the binary (beside the file manager's
  # trash, whose rules it shares), the catalogue, and the option a machine
  # may change the number with.
  uses = [
    "crates/ronoctua-files"
    "assets/cache.nix"
    "nix/modules/cache.nix"
  ];

  # `ronoctua.cache.enable = false` IS THIS MODULE'S SWITCH TURNED OFF by
  # default — the catalogue's "throw nothing away" reaching the one switch the
  # units and the brain both answer to — and that line names it by its id.
  namedBy = {
    "nix/modules/cache.nix" = [ "trash-expire" ];
  };
}
