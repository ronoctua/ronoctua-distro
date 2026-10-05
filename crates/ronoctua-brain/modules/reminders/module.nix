# The reminders' NixOS half: the chime, and the two paths the brain is given
# to play it.
#
# NO UNIT OF ITS OWN: a reminder is a clock inside the brain, and what rings
# is a notice on the brain's line. What the machine needs for it is a sound
# and the program that plays it:
#
#   RONOCTUA_PW_PLAY   PipeWire's `pw-play`, by STORE PATH — a unit's `PATH`
#                      is minimal, and a program found by name fails far from
#                      its cause (AGENTS.md). The package the session's
#                      PipeWire runs, so the client and the daemon agree.
#   RONOCTUA_CHIME     the chime, a WAV made here by `chime.py` from
#                      arithmetic: nothing downloaded, nothing kept in git.
#
# Both reach the brain's unit as ENVIRONMENT, read when a chime is asked for
# (`ronoctua_audio::chime`) — never compiled in, so the brain's binary does not
# carry PipeWire's closure, and the flake decides where the sound is.
#
# `ronoctua.brain.modules.<id>.enable = false` takes both away with the module
# (the gate `nix/brain-modules.nix` puts around this file), and a brain with
# no chime still rings — on the line — and says in its log why it is silent.
{
  config,
  pkgs,
  ...
}:

let
  chime = pkgs.runCommand "ronoctua-chime" { nativeBuildInputs = [ pkgs.python3 ]; } ''
    mkdir -p $out
    python3 ${./chime.py} $out/chime.wav
  '';
in
{
  systemd.user.services.ronoctua-brain.environment = {
    RONOCTUA_PW_PLAY = "${config.services.pipewire.package}/bin/pw-play";
    RONOCTUA_CHIME = "${chime}/chime.wav";
  };
}
