# THE MARK GRUB PUTS ON THE KERNEL'S COMMAND LINE of a boot that tries a new
# system (`nix/modules/boot-loader.nix`), for the trial's deadline
# (`nix/modules/machine.nix`), which reads it before anything is mounted. A
# value of its own, and not an option: a machine module imported without the
# boot loader's (a test) still reads it.
"ronoctua.trying"
