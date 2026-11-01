# A PROBE MACHINE, never installed: what an installation that answers
# differently from the reference (`tests/fixtures/machine`) builds, so the
# offline ISO carries what that build needs (`nix/installer/offline.nix`).
# Its hardware report is written by hand, with only the fields the facter
# modules read.
{ ... }:
{
  networking.hostName = "noctua-elsewhere";
  ronoctua.disk = "/dev/disk/by-id/nvme-probe-elsewhere";
  ronoctua.language = "pt-BR";
  ronoctua.profile = "full";
  ronoctua.theme = "frost";
  ronoctua.locale = "pt_BR.UTF-8";
  ronoctua.keyboard.layout = "br";
  # `br` IS the ABNT2 keyboard: xkb lists no `abnt2` variant of it (that
  # name is a model), and the console maps are compiled from what it lists.
  ronoctua.keyboard.variant = "";
  ronoctua.keyboard.compose = "compose:ralt";
  ronoctua.facterReport = ./facter.json;
  ronoctua.swapSize = "16G";
  boot.kernelParams = [ "resume_offset=533760" ];
}
