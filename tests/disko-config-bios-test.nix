# The test layout of `disko-config-test.nix` on a machine that starts from
# BIOS: the same file, with GRUB's own partition (`bios`). `makeDiskoTest`
# imports a layout with its default arguments, so the firmware is a file of
# its own, as the swap's size is.
{ ... }@args:

import ./disko-config-test.nix (args // { bios = true; })
