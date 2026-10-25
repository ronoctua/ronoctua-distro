# A MACHINE NO PROBE IS, never installed and never a probe: an AMD laptop
# that boots by UEFI, with a MediaTek radio and a fingerprint reader — the
# AMD of one probe, the laptop of another. `iso-builds-only-glue` installs
# it, and `machine-unseen-bios`, against what the ISO carries: the ISO holds
# what a machine unlike every probe builds, and not only what the probes do
# (2026-10-25: a ThinkPad Z16 asked the network for its microcode). Its
# hardware report is written by hand, with only the fields the facter
# modules read.
{ ... }:
{
  networking.hostName = "noctua-unseen";
  ronoctua.disk = "/dev/disk/by-id/nvme-probe-unseen";
  ronoctua.language = "en-GB";
  ronoctua.profile = "full";
  ronoctua.theme = "owl";
  ronoctua.locale = "en_US.UTF-8";
  ronoctua.keyboard.layout = "us";
  ronoctua.keyboard.variant = "";
  ronoctua.keyboard.compose = "compose:ralt";
  ronoctua.facterReport = ./facter.json;
  ronoctua.swapSize = "32G";
  boot.kernelParams = [ "resume_offset=1067008" ];
}
