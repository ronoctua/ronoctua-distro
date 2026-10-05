# THE WATCH AND THE SWITCH ON THE MACHINE'S OWN CLOCK. It does not fetch the
# time — `systemd-timesyncd` does, and doing it twice would be two things
# setting one clock. It keeps the automatic time-keeping on, and says when
# nothing keeps the clock. Why each choice is what it is is in `why.md`.
{
  what = "Keep the machine's clock set by the network, and say when nothing keeps it.";

  needs = {
    root = false;
    # THE NETWORK IS THE TIME CLIENT'S, not this module's: it reads three
    # properties on the system bus and flips one.
    network = false;
    # `timedated`, through the one crate that talks to it.
    boundaries = [ "ronoctua-clock" ];
  };

  schedule = {
    kind = "routine";
    calendar = false;
    partOfDay = "any";
    # SIX HOURS, the Emacs watch's number and for its reason: every look is a
    # line in the activity log. It is also enough for what this watches — a
    # client that stopped or a switch somebody turned off.
    everyMinutes = 360;
    # NO COUNTDOWN: it reads three properties and says what it found.
    countdownSeconds = 0;
    # A minute is generous for three D-Bus properties.
    runawayMinutes = 1;
    gated = true;
  };

  # WHAT IT SAYS: nothing keeps the clock (with the button), or the clock is
  # kept and has never reached a server (without one — what is missing is a
  # network). Each goes when the next look finds it fixed.
  notices = {
    not-automatic = {
      level = "warn";
      dismiss = false;
    };
    not-synchronised = {
      level = "info";
      dismiss = false;
    };
  };

  # KEEP THE CLOCK RIGHT — `clock-sync-keep`: the same work the watch does on
  # its schedule, done NOW on a thread of its own — never queued behind the
  # gate or a running update (`module.rs`).
  actions = {
    keep = { };
  };

  # A TRY THE MACHINE REFUSED says so, and where to look: the press in its own
  # sentence, the strip's line, and the activity log in the machine's words.
  whys = [ "refused" ];

  uses = [
    "crates/ronoctua-clock"
    # The narrow polkit rule that lets the person's session switch the
    # network time on (`timedate-polkit-is-narrow` keeps it narrow).
    "nix/modules/base.nix"
  ];

  # THE DATE-AND-TIME PAGE (`LEADER s l`) CARRIES THIS MODULE'S SWITCH: the
  # owner asked for the switch to be there and on the module's own page,
  # one truth behind the brain and two doors onto it. It names the module by
  # its compiled id and flips it with `Ask::Set { Knob::Enabled }`.
  namedBy = {
    "crates/ronoctua-shell/src/locale.rs" = [ "clock-sync" ];
  };
}
