# WHAT DIED FOR LACK OF MEMORY, written down and said.
#
# A watch, and nothing more: it runs nothing and has no schedule. It listens
# to the journal the core already follows for three kinds of line — the
# kernel's own kill, systemd's word about a unit that lost a process to it,
# and `systemd-oomd`'s decision — and makes of each kill one record in the
# activity and one notification with what died, where, how much memory it had,
# how stalled the machine was and when. Why each is so is in `why.md`.
{
  what = "Write down and tell every process killed for lack of memory: by the kernel, or by systemd-oomd.";

  needs = {
    root = false;
    network = false;
    # THE STALL AT THE MOMENT OF A KERNEL KILL (`/proc/pressure/memory`)
    # comes through the one crate that may read `/proc`.
    boundaries = [ "ronoctua-memory" ];
  };

  # A WATCH: nothing to run, nothing to gate, nothing to announce.
  schedule = null;

  # ONE EVENT PER KILL, and never two for one: the kernel's line and the
  # unit's line of the same kill are one event, and so are oomd's line and the
  # unit's. It is a notification like the one the footer raises for oomd's
  # bus signal (`memory-killed` in `assets/notifications.nix`) — full, loud
  # and long, because a window vanished — and it carries that entry's tag, so
  # the footer knows the two are the same kind of thing: a kill oomd also
  # announced on the bus is ADDED to the notification already on screen, not
  # raised twice.
  events = {
    killed = {
      notification = {
        show = "full";
        life = "long";
        loud = true;
        level = 95;
        icon = "memory";
        tags = [
          "memory-killed"
          "system"
        ];
        # The panel that shows the policy that did it — the bus
        # notification's own action.
        actions = [
          {
            id = "memory-panel";
            command = "memory-open";
          }
        ];
      };
    };
  };

  uses = [
    "crates/ronoctua-journal"
    "crates/ronoctua-notify/src/oom.rs"
    "crates/ronoctua-shell/src/surface.rs"
    "assets/notifications.nix"
    # Its machine subtest arms oomd as the distro does.
    "nix/modules/memory.nix"
  ];

  namedBy = { };
}
