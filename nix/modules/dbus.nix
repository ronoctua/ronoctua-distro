# D-Bus, with each name listed once.
#
# # What the broker said at every load
#
# NixOS lists every package of `services.dbus.packages` and, among them,
# `system-path`, which already links the same files. Each name then arrives
# twice, and on the session bus a third time through `XDG_DATA_DIRS`, and the
# broker writes `Ignoring duplicate name` at error priority: 41 lines at every
# boot and every switch, about nothing, on the page that shows errors. A
# filter there would hide the one that means something: a person's or a
# Flatpak's service shadowing a name of the system.
#
# # Why the configuration is filtered, and not the list
#
# Which directories are redundant depends on the files inside them, and
# evaluation cannot see those without importing from a derivation. So the
# whole configuration is built exactly as upstream builds it, and a build step
# (`nix/scripts/dbus-conf.py`, where the rule is) takes out what `system-path`
# carries, byte for byte. The decision and its proof are the same code: a
# directory nothing covers stays.
#
# Only the broker, the implementation every machine here runs: under
# `dbus-daemon` this file changes nothing.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.dbus;

  # UPSTREAM'S `configDir`, the same expression and so the same derivation
  # (`nixos/modules/services/system/dbus.nix`). Asking
  # `environment.etc."dbus-1".source` for it would be infinite recursion.
  whole = pkgs.makeDBusConf.override {
    inherit (cfg) apparmor;
    dbus = cfg.dbusPackage;
    suidHelper = "${config.security.wrapperDir}/dbus-daemon-launch-helper";
    serviceDirectories = cfg.packages;
  };

  thin =
    pkgs.runCommand "dbus-1"
      {
        preferLocalBuild = true;
        allowSubstitutes = false;
        nativeBuildInputs = [ pkgs.python3 ];
      }
      ''
        python3 ${../scripts/dbus-conf.py} ${whole} ${config.system.path} $out
      '';
in
{
  config = lib.mkIf (cfg.enable && cfg.implementation == "broker") {
    environment.etc."dbus-1".source = lib.mkForce thin;

    # Upstream's triggers name `configDir`, so a change of the filter alone
    # would not reach a running bus.
    systemd.services.dbus-broker.restartTriggers = [ thin ];
    systemd.user.services.dbus-broker.restartTriggers = [ thin ];
  };
}
