# A PROBE MACHINE, never installed: what an installation that answers
# differently from the reference (`tests/fixtures/machine`) builds, so the
# offline ISO carries what that build needs (`nix/installer/offline.nix`).
# Its hardware report is written by hand, with only the fields the facter
# modules read.
{ ... }:
{
  networking.hostName = "noctua-bios";
  ronoctua.disk = "/dev/disk/by-id/ata-probe-bios";
  ronoctua.firmware = "bios";
  ronoctua.language = "en-GB";
  ronoctua.profile = "minimal";
  ronoctua.theme = "moss";
  ronoctua.locale = "de_DE.UTF-8";
  ronoctua.keyboard.layout = "de";
  ronoctua.keyboard.variant = "nodeadkeys";
  ronoctua.keyboard.compose = "compose:menu";
  ronoctua.facterReport = ./facter.json;
  ronoctua.swapSize = "4G";
  ronoctua.hibernation = false;
}
