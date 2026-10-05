# THE SYSTEM UPDATES ITSELF, and a person can watch it or start it by hand.
#
# The brain's first feature, moved into its folder: a run on a calendar (weekly,
# in the afternoon, behind the gate, with a minute of warning), a sequence of
# steps declared once (`steps.nix`) and generated into the table the screen
# draws, the two runners the units run and the text keys both languages owe
# (`sequence.nix`, `gen-updates.py`). Why each is so is in `why.md`, beside
# this.
{
  what = "Update the system: the inputs, a rebuild for the next boot, and the steps beside it.";

  needs = {
    root = false;
    # THE INPUTS COME FROM THE NETWORK. Not a gate the core applies (a machine
    # offline still rebuilds what it has, and each step that needs the network
    # says so and is skipped): a fact the brain's page states.
    network = true;
    # NO BOUNDARY CRATE: what it does is start units, through the core's own
    # `systemd`, and read the report they write.
    boundaries = [ ];
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

  # WHAT IT RELIES ON AND DOES NOT OWN: the two units its steps borrow — the
  # rescue console's rebuild and the store's trim of the system's generations,
  # with the trim's rehearsal — and the wrapped software CLI the person's
  # programs are updated through.
  uses = [
    "nix/modules/rescue.nix"
    "nix/modules/store.nix"
    "nix/modules/software.nix"
    "crates/ronoctua-software"
  ];

  # THE ONE FILE OUTSIDE THE FOLDER THAT SPEAKS OF IT: the view that watches a
  # run in both doors, the mark on the modeline and the door that starts one
  # by hand with no brain in the way. It owns the three commands, and the
  # leader and the footer reach them through its tables without naming them.
  namedBy = {
    "crates/ronoctua-shell/src/update_window.rs" = "*";
  };
}
