# The parts' watch's NixOS half: nothing.
#
# It is a WATCH on the folder of records the parts' realizer writes
# (`nix/modules/parts.nix` declares it, root's and readable by everybody), so
# it needs no unit, no timer and no permission — the brain's own thread hears
# the kernel.
{ ... }:
{ }
