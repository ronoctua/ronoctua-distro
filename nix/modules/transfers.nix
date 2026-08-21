# The queue of everything that takes a while — `LEADER t`.
#
# WHY IT IS A UNIT AND NOT PART OF THE FOOTER
# -------------------------------------------
# Two reasons, and each holds on its own.
#
# The same list has to be visible from THREE processes: the footer, which draws
# the modeline's percentage and the panel in its region; the file manager's
# window, which is a re-exec of the shell's binary and therefore somebody else
# entirely; and the wasm buffer's host window. A queue inside the footer would
# be invisible to the other two, and each would grow its own.
#
# And a copy of forty gibibytes must not die when the footer does. On this
# machine the footer restarts on failure in one second — that is right for a
# screen and wrong for an hour of copying.
#
# WHAT IT IS NOT ALLOWED TO BE
# ----------------------------
# It is `ronoctua.transfers`, EXTRACTED from the workspace, and not
# `ronoctua.shell`. A unit whose closure carried wgpu, the wasm runtime, the
# renderer and three embedded fonts would be a hundred megabytes resident to
# copy files, and would hold every one of those store paths alive for as long as
# the machine is on. `nix why-depends` on this unit is what would say so, and
# nobody runs that on a Tuesday — so the check `transfers-unit-is-narrow` runs
# it instead.
#
# WHAT STOPPING IT COSTS, PLAINLY
# -------------------------------
# A copy interrupted by `systemctl --user stop` leaves the half-written file in
# the destination. Cancelling from the panel does not — that path unwinds and
# removes what it wrote — but a kill is a kill, and the process has no handler
# for it. `TimeoutStopSec` is short on purpose: a queue that takes ninety
# seconds to die would hold up every logout.
{
  config,
  lib,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.transfers;
in
{
  options.ronoctua.transfers = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "A fila de transferencias do LEADER t.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.ronoctua-transfers = {
      description = "ronoctua — a fila de transferências";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      serviceConfig = {
        ExecStart = "${ronoctua.transfers}/bin/ronoctua-transfers";
        Restart = "on-failure";
        RestartSec = 1;
        # `session.slice` and NOT `app.slice`.
        #
        # `app.slice` is where every foreign application goes and where
        # `nix/modules/memory.nix` points oomd's pressure killer. A copy holds
        # page cache by definition — that is what copying IS — so a queue in the
        # applications' slice would be among the first things chosen under
        # pressure, and the failure would look like "big copies die at random".
        Slice = "session.slice";
        # If the kernel's OOM killer takes a thread of this, the queue's list is
        # gone and the jobs are orphans. Stopping the unit is the honest answer:
        # it comes back with an empty list instead of a wrong one.
        OOMPolicy = "stop";
        # See the header: a kill leaves a partial file, and a long timeout only
        # delays a logout without saving it.
        TimeoutStopSec = 5;
      };
    };
  };
}
