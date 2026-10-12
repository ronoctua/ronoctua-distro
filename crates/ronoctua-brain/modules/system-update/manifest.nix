# THE SYSTEM UPDATES ITSELF, and a person can watch it or start it by hand.
#
# The brain's first feature, moved into its folder: a run on a calendar (weekly,
# in the afternoon, behind the gate, with a minute of warning), a sequence of
# steps declared once (`steps.nix`) and generated into the table the screen
# draws, the two runners the units run and the text keys both languages owe
# (`sequence.nix`, `gen-updates.py`). Why each is so is in `why.md`, beside
# this.
{
  # WHAT IT IS, as the page says it: the person's declarations gathered,
  # the distro moved forward, the system built and switched to (not "a
  # rebuild for the next boot" — the switch happens now, and only a new
  # kernel waits for the restart, which the boot trial guards), the person's
  # own programs and caches, and what this machine did published.
  what = "Update the system: gather what you declared, move the distro forward, build and switch to the new system with a boot trial, update your own programs, and publish what this machine did.";

  needs = {
    root = false;
    # THE INPUTS COME FROM THE NETWORK. Not a gate the core applies (a machine
    # offline still rebuilds what it has, and each step that needs the network
    # says so and is skipped): a fact the brain's page states.
    network = true;
    # WHAT IT READS, through the crates that own each: whether the way out is
    # metered (NetworkManager) and how charged the battery is (`/sys`), which
    # a scheduled run waits on; and, for the view, the machine's record (the
    # boot trial, a hold), what the verification held, and the plain part's
    # conflicts — of the repository the person names now, read through the
    # store that keeps `belongings.json`. It starts units through the core's
    # own `systemd`, and runs nothing itself.
    boundaries = [
      "ronoctua-network"
      "ronoctua-memory"
      "ronoctua-machine"
      "ronoctua-person"
      "ronoctua-belongings"
      "ronoctua-store"
    ];
  };

  # WEEKLY, IN THE AFTERNOON, WITH A MINUTE OF WARNING AND TWO HOURS OF
  # PATIENCE: a rebuild compiles, a person nearby should see it coming, and the
  # gate holds it while the machine is busy or on battery.
  schedule = {
    kind = "routine";
    calendar = true;
    intervalDays = 7;
    partOfDay = "afternoon";
    countdownSeconds = 60;
    runawayMinutes = 120;
    gated = true;
  };

  # FOUR UNITS, all declared by `module.nix` under these names.
  #
  # `run` and `rehearse` are the person's half, which the brain starts and
  # which starts the root half; they run the `binary` steps, so they inherit
  # the session's `PATH` (`sessionPath`). `system` and `system-rehearsal` are
  # the root half, which only the person's half starts, through the one polkit
  # rule that names them. Two units and not a flag, for the reason the disk's
  # relief gives: a flag crossing from the brain would be a flag somebody else
  # could cross with.
  units = {
    run = {
      name = "ronoctua-update.service";
      sessionPath = true;
    };
    rehearse = {
      name = "ronoctua-update-rehearsal.service";
      sessionPath = true;
    };
    system = "ronoctua-update-system.service";
    system-rehearsal = "ronoctua-update-system-rehearsal.service";
  };

  # THE ONE ACTION ITS POLKIT RULE NAMES: starting its own two root units.
  polkit = [ "org.freedesktop.systemd1.manage-units" ];

  # WHAT IT RELIES ON AND DOES NOT OWN: the store's trim of the system's
  # generations, with the trim's rehearsal; the wrapped software CLI the
  # person's programs are updated, pinned, staged and reconciled through; the
  # machine's life (`ronoctua-machine`: migrate, apply and its boot trial,
  # the hold), the person's declarations (`ronoctua-person`: root's lock,
  # verify, the record), the belongings' plain part, pulled and pushed, and
  # the push to the person's binary cache (`ronoctua-cachix`, wrapped by
  # `nix/modules/binary-cache.nix`).
  uses = [
    "nix/modules/store.nix"
    "nix/modules/software.nix"
    "nix/modules/machine.nix"
    "nix/modules/person-apply.nix"
    "nix/modules/belongings.nix"
    "nix/modules/binary-cache.nix"
    "crates/ronoctua-software"
    "crates/ronoctua-machine"
    "crates/ronoctua-person"
    "crates/ronoctua-belongings"
    "crates/ronoctua-cachix"
  ];

  # THE ONE FILE OUTSIDE THE FOLDER THAT SPEAKS OF IT: the view that watches a
  # run in both doors, the mark on the modeline and the door that starts one
  # by hand with no brain in the way. It owns the three commands, and the
  # leader and the footer reach them through its tables without naming them.
  # And `tests/checks.nix`, by the folder's path alone: `update-apply-rule-is-
  # narrow` reads the root half's lock and the module's `stateDir` to hold
  # them against `ronoctua-machine`, which takes the same lock.
  namedBy = {
    "crates/ronoctua-shell/src/update_window.rs" = "*";
    "tests/checks.nix" = [ "system-update" ];
  };
}
