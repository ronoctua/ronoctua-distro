# A PROBE MACHINE, never installed: what an installation that answers
# differently from the reference (`tests/fixtures/machine`) builds, so the
# offline ISO carries what that build needs (`nix/installer/offline.nix`).
# Its hardware report is written by hand, with only the fields the facter
# modules read.
{ ... }:
{
  networking.hostName = "noctua-guest";
  ronoctua.disk = "/dev/disk/by-id/virtio-probe-guest";
  ronoctua.language = "en-GB";
  ronoctua.profile = "full";
  ronoctua.theme = "forest";
  ronoctua.locale = "fr_FR.UTF-8";
  ronoctua.keyboard.layout = "us";
  ronoctua.keyboard.variant = "intl";
  ronoctua.keyboard.compose = "compose:rwin";
  ronoctua.facterReport = ./facter.json;
  ronoctua.swapSize = "2G";
  ronoctua.hibernation = false;
}
