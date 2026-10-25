# A MACHINE NO PROBE IS, never installed and never a probe: an Intel desktop
# that boots by BIOS — the Intel of one probe, the BIOS of another.
# `iso-builds-only-glue` installs it against what the ISO carries (see
# `tests/fixtures/machine-unseen`). Its hardware report is written by hand,
# with only the fields the facter modules read.
{ ... }:
{
  networking.hostName = "noctua-unseen-bios";
  ronoctua.disk = "/dev/disk/by-id/ata-probe-unseen-bios";
  ronoctua.firmware = "bios";
  ronoctua.language = "pt-BR";
  ronoctua.profile = "minimal";
  ronoctua.theme = "frost";
  ronoctua.locale = "pt_BR.UTF-8";
  ronoctua.keyboard.layout = "br";
  ronoctua.keyboard.variant = "abnt2";
  ronoctua.keyboard.compose = "compose:ralt";
  ronoctua.facterReport = ./facter.json;
  ronoctua.swapSize = "8G";
  ronoctua.hibernation = false;
}
