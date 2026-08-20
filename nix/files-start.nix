# Where the file manager opens, in ONE place.
#
# A name RELATIVE to home, never a path: an absolute path would name somebody's
# machine, and this repository is public (invariant 13). If the directory is not
# there, the manager opens in the home and says nothing — it is a preference,
# not a requirement.
#
# WHY A FILE OF ITS OWN
# ---------------------
# Two things read it and they are different kinds of thing: `nix/modules/files.nix`
# makes it the NixOS option's default, and `nix/dev.nix` exports it into the
# nested loop, which is not a NixOS system and cannot read an option. Written
# twice, the loop and the installed system would open in different places the
# first time somebody changed one — and nothing would say so.
#
# It is the same shape as `nix/keyboard-xkb.nix`: one file, several readers.
"main"
