# THE BOOT TRIAL, TOLD — one line when a new system did not start.
#
# The trial itself is the machine's (`ronoctua-machine`, `nix/modules/
# machine.nix`): an apply arms it, the boot tries it once, and a minute after
# the desktop `ronoctua-boot-bless.service` reads which system came up. When
# it was NOT the new one, the machine is already back on the last good system
# by itself — and the person was not looking. This module reads the machine's
# record and says so, once. Why each piece is so is in `why.md`, beside this.
{
  what = "Say once, on the minibuffer, when a new system did not start and the machine came back to the last good one.";

  needs = {
    root = false;
    network = false;
    # THE MACHINE'S RECORD, read through the crate that writes it.
    boundaries = [ "ronoctua-machine" ];
  };

  # A WATCH: inotify on the record's folder, and nothing it runs.
  schedule = null;

  # ONE EVENT, said once per failed trial; never a mark — at rest the modeline
  # says nothing (invariant 14), and there is nothing for the person to do: the
  # machine already went back.
  events = {
    failed = {
      notification = {
        show = "full";
        life = "long";
        loud = true;
        level = 80;
        icon = "history";
        tags = [ "system" ];
        actions = [ ];
      };
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the crate that keeps the record, and
  # the module that declares its folder and the bless.
  uses = [
    "crates/ronoctua-machine"
    "nix/modules/machine.nix"
  ];

  namedBy = { };
}
