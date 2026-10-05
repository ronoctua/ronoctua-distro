# The video-metrics module's NixOS half: NOTHING, and on purpose.
#
# The unit it starts — `ronoctua-video-metrics-scan.service`, the name its
# manifest declares — is the APP's, in `nix/modules/video-metrics.nix`, and
# not this folder's: the door's "Scan now" starts it through systemd when no
# brain answers, and a person who switches this module off
# (`ronoctua.brain.modules.video-metrics.enable = false`) takes away the
# cadence and the mark, never the scan they can still ask for. The report the
# unit writes is where the brain reads (`<id>.json` under
# `%t/ronoctua-brain`), which the module's own check holds against the
# evaluated unit.
{ ... }:
{ }
