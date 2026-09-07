# The screens — brightness, and the door for everything else about them.
#
# WHAT THIS MODULE IS FOR, AND WHAT IT IS NOT
# -------------------------------------------
# Almost everything about a monitor — its mode, its scale, how it is turned,
# where it sits — is the COMPOSITOR's, and the shell asks niri for it over the
# IPC it already speaks. None of that needs a line here.
#
# What does need a line is the one thing niri cannot answer: how bright the
# picture is. There are two kinds of screen and they answer differently, and
# neither of them answers to a Wayland client:
#
#   a BUILT-IN PANEL has `/sys/class/backlight/<device>/brightness`, a
#   root-owned sysfs attribute;
#   a MONITOR ON A CABLE has nothing in `/sys` at all, and is asked over
#   DDC/CI — I2C on the two wires the video cable has always carried for the
#   EDID — through `/dev/i2c-N`.
#
# NO POLKIT RULE, and that is deliberate, the same decision `nix/modules/
# power.nix` records: this session has no polkit agent, so a rule that asks
# would be a refusal nobody sees. `display-adds-no-polkit-rule-of-its-own`
# holds it both ways.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.display;

  # The one attribute the shell writes, named ONCE — the rule below and the
  # check that reads it both come from here.
  attribute = "brightness";

  # THE PANEL'S DOOR.
  #
  # `GROUP=` and `MODE=` in a udev rule only reach DEVICE NODES; a sysfs
  # attribute needs a `RUN`, and the programs it runs are store paths,
  # because a udev worker's `PATH` is minimal. `TEST==` keeps the rule inert
  # on a backlight that has no such file, and `change` re-applies it after a
  # resume — a laptop that suspends comes back with the ownership reset
  # otherwise, and the symptom is a brightness key that worked this morning.
  #
  # It is the arrangement `power.nix` uses for the charge thresholds, learned
  # there.
  #
  # IT IS NOT THE WHOLE OF THE PRIVILEGE, and this comment said it was.
  # `externalBrightness` turns on `hardware.i2c`, and the upstream module's
  # rule is `KERNEL=="i2c-[0-9]*", TAG+="uaccess", GROUP="i2c", MODE="660"` —
  # EVERY I2C adapter on the machine, not the ones a video cable ends at. That
  # includes the SMBus controllers, and behind those sit the DIMM SPD EEPROMs
  # at 0x50-0x57; `uaccess` hands the lot to whoever is logged in at the seat,
  # read and write, for every process running as them.
  #
  # Our own crate never opens anything but the node named by a connector's
  # `ddc/i2c-dev`, never addresses anything but 0x37, and uses `I2C_SLAVE`
  # rather than `I2C_SLAVE_FORCE` so the kernel refuses an address a driver has
  # claimed. That is discipline in OUR code and not a bound on the grant, which
  # is the distinction the old sentence hid. `externalBrightness = false` is
  # what actually closes it, and then a monitor on a cable says its brightness
  # is on its own buttons — which for that machine is the truth.
  grant = ''ACTION=="add|change", SUBSYSTEM=="backlight", TEST=="${attribute}", RUN+="${pkgs.coreutils}/bin/chgrp ${cfg.brightnessGroup} /sys$devpath/${attribute}", RUN+="${pkgs.coreutils}/bin/chmod g+w /sys$devpath/${attribute}"'';
in
{
  options.ronoctua.display = {
    enable = lib.mkEnableOption "the screens: brightness, and the display view" // {
      default = true;
    };

    # `users` and not a group of our own, for the reason `power.nix` gives at
    # length: this machine has one user by design (`base.nix`), that user is
    # in `users`, and a second group would be one more line reaching into the
    # identity `base.nix` owns.
    #
    # `video` WOULD BE NARROWER, and the owner is already in it (`base.nix`) —
    # it is the conventional group for `/sys/class/backlight`. It is not the
    # default here because the two features that write the same kind of sysfs
    # attribute would then disagree with each other, and this is an OPTION: a
    # machine with a second account sets it to `video` in one line. Written
    # down because a reader who knows the convention will ask why not.
    brightnessGroup = lib.mkOption {
      type = lib.types.str;
      default = "users";
      description = "The group that may write a built-in panel's brightness.";
    };

    # THE OTHER HALF, and it costs a kernel module rather than a group.
    #
    # `hardware.i2c` loads `i2c-dev` and tags the nodes `uaccess`, which hands
    # them to whoever is logged in at the seat — so nobody has to be put into
    # a group, and a machine with no external monitor pays a module it never
    # opens. It is off only for somebody who wants the buses closed; then a
    # monitor on a cable says its brightness is on its own buttons, which is
    # the truth for that machine.
    externalBrightness = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Let the shell reach an external monitor's own brightness over DDC/CI.
        Loads `i2c-dev` and opens `/dev/i2c-*` to the seat's user.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.udev.extraRules = grant + "\n";

    hardware.i2c.enable = cfg.externalBrightness;
  };
}
