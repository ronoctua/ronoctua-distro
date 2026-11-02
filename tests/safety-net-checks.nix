# THE SAFETY NET IN VMS (2026-11-01): the boot trial and the generation
# rollback are the one net under a machine that rolls on `nixos-unstable` by
# itself (invariant 6), and two of its parts had never run.
#
#   - `rescue-rolls-back-the-trial`: a trial armed, undone from the rescue
#     console's own rows (`Ctrl+Alt+Del`), and the reboots come up on the
#     generation they came back to (`tests/rescue-rollback-vm.nix`);
#   - `boot-trial-is-blessed-under-secure-boot`: lanzaboote and its keys under
#     an OVMF that enforces them, a trial of a signed UKI armed, booted and
#     blessed, the blessed generation the menu's default, booting signed, and
#     the rescue's rollback still the system that boots after a reboot
#     (`tests/secureboot-trial-vm.nix`).
#
# Both build a disk image of the whole system and reboot it, so both are in
# `tests/slow.nix`. Joined into `tests/checks.nix` with one line, like every
# file of checks a wave adds.
{
  pkgs,
  lib,
  rust,
  sbKeys,
  lanzabooteModule,
  # The rescue's own arguments, as `tests/rescue-vm.nix` takes them.
  ronoctua,
}:
{
  rescue-rolls-back-the-trial = import ./rescue-rollback-vm.nix {
    inherit pkgs lib ronoctua;
  };
  boot-trial-is-blessed-under-secure-boot = import ./secureboot-trial-vm.nix {
    inherit pkgs lib sbKeys;
    machineTool = rust.machine;
    inherit lanzabooteModule;
  };
}
