# Bluetooth — the radio, its devices, and the door that was already open.
#
# WHAT IS PRIVILEGED HERE: NOTHING, AND NOT EVEN A GROUP.
# --------------------------------------------------------
# BlueZ has no polkit actions at all. Its own bus policy carries
# `<policy context="default"><allow send_destination="org.bluez"/></policy>`
# — no group, no `at_console`, no authentication — so any client on the system
# bus may connect a device, pair one or turn the radio off. That is upstream's
# decision and this module does not widen or narrow it; what it does is PIN
# it: `bluetooth-bus-policy-is-open` reads the installed file, so the day a
# release changes its mind, a check says so instead of a screen going quiet.
#
# NOTHING HERE ASKS EITHER. There is no authentication agent in this session,
# and BlueZ never wanted one.
#
# THE PAIRING AGENT IS OURS, AND IT CLAIMS NO NAME.
# --------------------------------------------------
# `AgentManager1.RegisterAgent` takes an object path and bluetoothd remembers
# the caller's unique connection name, so the footer registers an object and
# never a well-known name — which dbus's default policy (`<deny own="*"/>`)
# would refuse anyway, with an error that reads exactly like an absent daemon.
# The callback direction is what the root policy already allows
# (`<allow send_interface="org.bluez.Agent1"/>`).
#
# WHAT THIS MODULE ACTUALLY SETS, AND WHAT IT DELIBERATELY DOES NOT.
# -------------------------------------------------------------------
# Four of the settings the plan listed are ALREADY the shipped defaults, read
# out of `main.conf` in the store rather than assumed: `AutoEnable = true`
# (which is what `powerOnBoot` writes), `ReconnectAttempts = 7`,
# `ReconnectIntervals = 1,2,4,8,16,32,64` and `DiscoverableTimeout = 180`.
# Restating a default is a line that says nothing and hides the day upstream
# changes it, so this module writes only what differs: `FastConnectable`,
# which trades a little idle power for a keyboard that wakes without a pause.
#
# AND `Experimental` STAYS OFF. It was thought to be needed for `Battery1` —
# it is not: in bluez 5.86 the battery profile carries no experimental flag
# and its two properties are declared plain, so a device's percentage arrives
# with the flag off. Turning it on would register the LE-audio profiles
# (`bap`, `vcp`, `csip`) that nothing here uses.
{
  config,
  lib,
  ...
}:

let
  cfg = config.ronoctua.bluetooth;
in
{
  options.ronoctua.bluetooth = {
    enable = lib.mkEnableOption "the Bluetooth radio and its devices" // {
      default = true;
    };

    # ON IN BOTH PROFILES, and deliberately not in `profile.nix`: the owner's
    # keyboard is a Bluetooth keyboard, and a minimal install with no radio is
    # a machine with no way to type. See `nix/modules/profile.nix`'s header for
    # what that file is for.
    fastConnectable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Keep the radio listening more often, so that a device that reconnects
        by itself — a keyboard waking up, headphones leaving their case —
        does it at once instead of after a pause of a second or two. It costs
        a little idle power on the controller, which is the trade the owner
        chose for the machine whose keyboard is on this radio.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    hardware.bluetooth = {
      enable = true;
      settings.General = {
        FastConnectable = cfg.fastConnectable;
      };
    };

    # THE AUDIO SIDE IS PIPEWIRE'S, AND IT IS ALREADY RIGHT.
    #
    # The plan named five WirePlumber settings; all five were read out of
    # `wireplumber.conf.d/bluetooth.conf` in the store before writing any of
    # them, and none of them belongs here.
    #
    # `bluetooth.autoswitch-to-headset-profile` is already `true` by default.
    # `bluez5.enable-sbc-xq`, `-msbc` and `-hw-volume` are enabled by
    # upstream's HARDWARE DATABASE, device by device, and the option exists to
    # FORCE them on or off for everything — so writing `true` would not turn
    # them on, it would overrule the one place that knows which headsets they
    # break. And `bluez5.roles` in the plan was the file's example line, not
    # its default: it adds `hsp_ag`, which upstream leaves out because some
    # headsets stop working with both HSP and HFP gateways enabled.
    #
    # So the audio configuration of this module is nothing at all, and the
    # sentence above is the reason it is nothing.

    # THE BONDING KEYS ARE ALREADY KEPT: `/var/lib/bluetooth` is in
    # `nix/modules/persist.nix`'s list, where every path this system must not
    # lose is named once. Repeating it here would be a second place to change.
  };
}
