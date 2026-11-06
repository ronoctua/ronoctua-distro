# WHAT DIED FOR LACK OF MEMORY, written down and said — and the RAM, marked
# on the modeline while it is nearly all in use.
#
# A watch, and nothing more: it runs nothing and has no schedule. It listens
# to the journal the core already follows for three kinds of line — the
# kernel's own kill, systemd's word about a unit that lost a process to it,
# and `systemd-oomd`'s decision — and makes of each kill one record in the
# activity and one notification with what died, where, how much memory it had,
# how stalled the machine was and when. And it reads the RAM in use off the
# heartbeat the core already takes. Why each is so is in `why.md`.
{
  what = "Write down and tell every process killed for lack of memory, by the kernel or by systemd-oomd; mark the modeline while the RAM is nearly all in use.";

  needs = {
    root = false;
    network = false;
    # THE STALL AT THE MOMENT OF A KERNEL KILL (`/proc/pressure/memory`)
    # comes through the one crate that may read `/proc`.
    boundaries = [ "ronoctua-memory" ];
  };

  # A WATCH: nothing to run, nothing to gate, nothing to announce.
  schedule = null;

  # THE RAM IN USE (owner, 2026-11-06: a glyph on the modeline with the per
  # cent, while the use is high): the memory chip, the token the memory's own
  # notification wears.
  glyphs = {
    ram = "md-memory";
  };

  # THE ONE MARK, only while it says something (invariant 14): from 85 per
  # cent of the RAM in use, in the warning's colour, and in danger's from 95;
  # once up, it stays until the use falls 5 points under its line, so a use
  # that hovers on the line does not blink it every heartbeat. Its count is
  # the share in use (`percent`), and a click opens the memory panel (`LEADER
  # s m`): who is eating the memory, and whether the protection is armed.
  marks = {
    ram = {
      glyph = "ram";
      hint = "oom-watch-mark-ram";
      percent = true;
    };
  };

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
    "crates/ronoctua-shell-footer/src/surface/mod.rs"
    "assets/notifications.nix"
    # Its machine subtest arms oomd as the distro does.
    "nix/modules/memory.nix"
  ];

  namedBy = { };
}
