# The containers module's NixOS half: nothing.
#
# It is a WATCH on a folder crun makes in the person's runtime directory, so
# it needs no unit, no timer and no permission — the brain's own thread hears
# the kernel. What puts crun there at all is the containers' module
# (`nix/modules/containers.nix`, `ronoctua.containers.enable`); on a profile
# without it the folder never appears and the mark never shows, so there is
# nothing to switch off here either.
{ ... }:
{ }
