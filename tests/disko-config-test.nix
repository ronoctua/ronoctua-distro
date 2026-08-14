# A test wrapper over the PRODUCTION layout.
#
# WHY IT EXISTS
# -------------
# The `makeDiskoTest` takes the config file's path and imports it with the
# DEFAULT arguments — there is no way of passing `swapSize` through it. With
# the production default (16 GiB), the `btrfs filesystem mkswapfile --size 16G`
# fails on a 4 GiB test disk and leaves a 0-byte swapfile. The symptom looks
# like a bug in the layout and is not: it is the test asking for more than fits.
#
# This wrapper changes ONE thing — the swap's size — and nothing else. The
# layout under test is still the production one, byte for byte: same
# partitions, same LUKS2, same 7 subvolumes, same mount options. If one day
# this diverges from the real file, the test stops being worth anything.
{ ... }@args:

import ../nix/disko/default.nix (
  args
  // {
    # >= RAM in production (section 7.2, for hibernation). Here, whatever fits on
    # the VM's disk.
    swapSize = "256M";
  }
)
