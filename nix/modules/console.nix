# THE KERNEL'S CONSOLE: its keyboard and its font, on every console a machine
# boots on, with no line of error in the journal — the ISO's and every
# machine's (`flake.nix` gives both this module).
#
# # Why the font is not just `console.font`
#
# `systemd-vconsole-setup` runs `setfont`, and `setfont` writes `ERROR` to the
# journal every time the console refuses the font. Three consoles answer three
# ways (`crates/ronoctua-vt/src/console.rs`): the framebuffer console takes
# any font, the VGA text console of a BIOS boot only one eight pixels wide,
# and the dummy console none — and the dummy one is bound while the firmware's
# frame buffer is handed to the graphics driver, which is during the boot.
# Measured: on a BIOS machine the initrd's setup refused the 15 × 32 every
# boot, and the system's own run fell into the handover.
#
# So, in the initrd and in the system alike:
#
#   - the setup loads the 8 × 16 (`console.font`), which both consoles that
#     take a font take, and the keyboard map with it;
#   - it does not run on the dummy console at all: the unit's main process is
#     `ronoctua-console-font setup`, which becomes `systemd-vconsole-setup`
#     on a console that takes a font and succeeds with nothing done on the
#     dummy one; the console that binds after it starts the setup again —
#     systemd's own `vtconsole` rule in the system, this module's in the
#     initrd, whose udev carries five rules and not that one. Not an
#     `ExecCondition`: udev restarts the setup in bursts, and a restart that
#     lands while the condition answers "skip" stops the unit first, where a
#     condition's exit 1 is a FAILURE (measured, one boot in many under
#     load); a main process exits 0 or is stopped by the `SIGTERM` the
#     upstream unit counts as success, and no interleaving fails;
#   - after it (`ExecStartPost`), on the framebuffer console, the 15 × 32 on
#     every allocated terminal in text mode; the compositor's terminal, in
#     graphics mode, would refuse it with `EINVAL`, and is passed over;
#   - a terminal allocated later (a getty on a second terminal, the rescue
#     console on the eighth) starts the setup again: the kernel gives it the
#     font of the one in front, which may be the compositor's.
#
# AND `fbcon=nodefer`: the framebuffer console binds the moment there is a
# frame buffer, instead of waiting for the first text. Deferred, the console
# stayed the dummy one through the initrd — the keyboard map would wait for
# the passphrase's prompt to bind it — and the firmware's logo stayed on the
# screen, which section 3 does not want: no logo, from the first pixel.
{ tool }:
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  fonts = ronoctua.fonts;
  program = "${tool}/bin/ronoctua-console-font";
  # `-` BEFORE THE LOAD: a switch restarts the setup when the console's
  # configuration changes, and a font the console refused there would fail
  # the switch (`nixos-rebuild`'s 4) — the console keeps its 8 × 16, and
  # `load` has said why.
  steps = systemd: kbd: {
    ExecStart = [
      ""
      "${program} setup ${systemd}/lib/systemd/systemd-vconsole-setup"
    ];
    ExecStartPost = "-${program} load --setfont ${kbd}/bin/setfont --font ${fonts.consoleFont}";
  };
  # A terminal the kernel allocates later is the one the setup has not seen.
  allocated = systemctl: ''
    ACTION=="add", SUBSYSTEM=="vc", KERNEL=="vcs[0-9]*", RUN+="${systemctl} --no-block restart systemd-vconsole-setup.service"
  '';
  initrdSystemctl = "${config.boot.initrd.systemd.package}/bin/systemctl";
in
{
  console = {
    earlySetup = true;
    font = fonts.consoleFontVga;
  };

  boot.kernelParams = [ "fbcon=nodefer" ];

  systemd.services.systemd-vconsole-setup.serviceConfig = steps config.systemd.package pkgs.kbd;
  # A switch sets the console up again when its configuration changed —
  # NixOS's own trigger is `vconsole.conf`, which names the 8 × 16 only: the
  # 15 × 32 and the program that loads it are part of it too. (The unit is
  # NixOS's only while the console is on.)
  systemd.services.reload-systemd-vconsole-setup.restartTriggers = lib.mkIf config.console.enable [
    fonts.consoleFont
    tool
  ];
  services.udev.extraRules = allocated "${config.systemd.package}/bin/systemctl";

  boot.initrd.systemd.services.systemd-vconsole-setup.serviceConfig =
    steps config.boot.initrd.systemd.package config.boot.initrd.systemd.package.kbd;
  boot.initrd.systemd.storePaths = [
    program
    fonts.consoleFont
    initrdSystemctl
  ];
  boot.initrd.services.udev.rules = ''
    ACTION=="add", SUBSYSTEM=="vtconsole", KERNEL=="vtcon*", RUN+="${initrdSystemctl} --no-block restart systemd-vconsole-setup.service"
  ''
  + allocated initrdSystemctl;
}
