# The clock watch has no NixOS half of its own.
#
# What it switches on is the system's own time client (`systemd-timesyncd`,
# which NixOS enables by default), and what lets the person's session flip it
# without a question is the narrow polkit rule beside the other `timedated`
# ones in `nix/modules/base.nix` — the date-and-time page uses the same rule
# with this watch off. So this file declares nothing, and is here because
# every module's folder has one.
_: { }
