# AppImage — a program in one file, run by `appimage-run`.
#
# An AppImage expects the library layout of the distribution it was built on,
# and NixOS has none: `appimage-run` is an FHS environment that gives it one.
# `binfmt` registers the two AppImage magics with the kernel, so a file the
# person downloaded and marked executable simply runs when opened — the same
# door, with no step in between.
#
# What the software view writes never depends on `binfmt`: an integrated
# AppImage's `.desktop` line names `/run/current-system/sw/bin/appimage-run`
# with the file as its argument, so it works whether the registration is there
# or not, and it disappears (through `TryExec=`) when this support is turned
# off. The files themselves are the person's, in `~/Applications`.
#
# `mkDefault full` in `nix/modules/profile.nix`: the FHS environment's closure
# is 1,53 GB on its own, measured, and a reduced machine is where that is
# declined.
{
  config,
  lib,
  ...
}:

{
  config = lib.mkIf config.ronoctua.software.appimage {
    programs.appimage = {
      enable = true;
      binfmt = true;
    };
  };
}
