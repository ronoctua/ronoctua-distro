# THE WATCH ON THE EMACS DAEMON: whether the server that holds the session is
# up, and whether the Emacs that is open is its client or a second copy of the
# editor. It looks, says what it found, and offers one thing: to start the
# distro's own unit. Why each choice is what it is is in `why.md`.
{
  what = "Look, now and then, whether the Emacs daemon is up and the open Emacs is its client.";

  needs = {
    root = false;
    network = false;
    # THE PROCESS TABLE, through the one crate that may read `/proc`.
    boundaries = [ "ronoctua-memory" ];
  };

  schedule = {
    kind = "routine";
    # NO CALENDAR: a cadence in minutes and no window — a watch has no good
    # hour, it has an interval.
    calendar = false;
    partOfDay = "any";
    # SIX HOURS, by the owner's choice, against the five minutes it opened
    # with: every look writes a line in the activity log, and at five minutes
    # this watch alone filled it twice an hour. At least an hour, always —
    # `module.rs` refuses to compile under that.
    everyMinutes = 360;
    # NO COUNTDOWN: it reads two things and says what it found.
    countdownSeconds = 0;
    # A minute is already generous for a `/proc` walk and an `emacsclient`
    # with a deadline of its own.
    runawayMinutes = 1;
    gated = true;
  };

  # THE UNIT IT OFFERS TO START, and the only one: the distro's own
  # (`services.emacs.enable` in `nix/modules/emacs.nix`, a USER unit — which
  # asks nobody anything, and is what lets the brain offer a button at all).
  units = {
    watched = "emacs.service";
  };

  # A START THE MACHINE REFUSED says where to look, in its own words.
  whys = [ "refused" ];

  # WHAT IT SAYS: the daemon down (with the button), or an Emacs that is not
  # its client (without one — what is open is somebody's work). Neither is
  # put away by hand: each goes when the next look finds it fixed.
  notices = {
    no-daemon = {
      level = "warn";
      dismiss = false;
    };
    not-the-client = {
      level = "info";
      dismiss = false;
    };
  };

  # START THE DAEMON — `emacs-daemon-start`, by name, on the strip and from
  # the notice.
  actions = {
    start = { };
  };

  uses = [
    "nix/modules/emacs.nix"
    "crates/ronoctua-memory"
  ];

  namedBy = { };
}
