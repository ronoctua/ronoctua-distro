# The belongings' reminder's NixOS half: the one unit it starts.
#
# THE PLAIN PART, SENT BY ITSELF. The person's declarations and brain
# modules travel in clear, with no passphrase (the belongings' plain part), so
# the machine may send them without asking anybody: the reminder watches the
# plain roots and, once they stayed quiet for 30 seconds, starts this unit —
# `ronoctua-belongings sync-plain`, the plain part alone (fetch, merge,
# commit, push; `index.age` and every chunk untouched). What clashes is left
# as each side has it, for the next sync with the person. Under the run lock:
# while a sync a door started holds it, this one is refused — the reminder
# keeps the change and starts this again once it sees the lock let go with
# no run having taken it since it asked (`module.rs`, "A send is kept until a
# run read it").
#
# Gated with the module (`ronoctua.brain.modules.<id>.enable`, through
# `nix/brain-modules.nix`): a person who turns the reminder off sends the
# plain part with their syncs alone.
{ config, ... }:
{
  systemd.user.services.ronoctua-belongings-plain = {
    description = "Sends the plain part of the belongings: the person's declarations and brain modules";
    # A USER UNIT'S PATH is minimal: the CLI by its store path, and git and
    # ssh are compiled into it (`nix/rust.nix`).
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${config.ronoctua.belongings.cli}/bin/ronoctua-belongings sync-plain";
      # A FEW KILOBYTES of data: never in the way of what the person does.
      Nice = 10;
      IOSchedulingClass = "idle";
      TimeoutStartSec = "5min";
    };
  };
}
