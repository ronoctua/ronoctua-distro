# Flatpak — the support, and the door for the day an app should be declared.
#
# # What is declared, and what is deliberately not
#
# ONE THING: the Flathub remote, in both installations. The apps are the
# person's (invariant 6's dated exception): `ronoctua-software` installs them
# with `flatpak --user`, and nothing here names one. nix-flatpak is imported,
# as a NixOS module AND as a Home Manager module, so that declaring an app is
# one line the day somebody wants it (`ronoctua.software.declaredFlatpaks`) and
# not a new piece of infrastructure.
#
# # The remote is VENDORED, and that is what makes activation work offline
#
# `assets/flatpak/flathub.flatpakrepo` is Flathub's own public file — the URL
# and the GPG key, nothing personal. Given the URL instead, `remote-add` would
# reach the network inside a `nixos-rebuild switch`, and the switch is the
# update's `system-rebuild` step, whose failure STOPS the sequence. The store
# path keeps the `.flatpakrepo` suffix, which is how `flatpak remote-add` knows
# the location is a description of a remote and not a repository.
#
# # The dark theme is NOT forced, and never will be
#
# `GTK_THEME` is not set here or anywhere: an app then draws whatever theme the
# variable names instead of its own. Dark comes the ordinary way: the settings
# portal (`xdg-desktop-portal-gtk`, `nix/modules/scroll.nix`) serves the dconf
# keys `color-scheme` and `gtk-theme` that `nix/modules/foreign.nix` writes,
# and every toolkit that reads them follows them live.
#
# # Two lines the experiment asked for (docs/ARCHITECTURE.md, "o modo escuro")
#
# Measured on `nix run .#shot -- --apps` with four Flathub apps, before and
# after, by window luminance:
#
# * `org.gtk.Gtk3theme.Adwaita-dark` is what turns a PLAIN GTK 3 app dark —
#   Geany went from L 0.931 to a dark window without it being asked — and it
#   changed nothing else: libadwaita moved 0.0002, gedit and KCalc, which
#   follow the portal themselves, did not move, and Electron stayed where it
#   was. Flatpak also pulls it in by itself when it sees `gtk-theme`; declaring
#   it makes that true before the first GTK 3 app, and brings it back after a
#   `flatpak uninstall --unused` would take it.
# * The session's `QT_QPA_PLATFORMTHEME=gnome` and `QT_STYLE_OVERRIDE` (the
#   `qt` block of `nix/modules/foreign.nix`) reach a sandbox, name a plugin and
#   a style the KDE runtime does not have, and a dark Qt app comes out LIGHT —
#   KCalc from L 0.155 to 0.944. Unsetting them for every Flatpak put it back
#   at 0.153: the KDE runtime reads the portal on its own.
{
  config,
  lib,
  pkgs,
  inputs,
  ...
}:

let
  cfg = config.ronoctua.software;
  user = config.ronoctua.user;

  # THE REMOTE, the same for both installations.
  flathub = {
    name = "flathub";
    location = "${../../assets/flatpak/flathub.flatpakrepo}";
  };

  # THE PERSON'S UNITS THAT REACH FLATHUB: the one of each start and the timer's.
  personUnits = [
    "flatpak-managed-install"
    "flatpak-managed-install-timer"
  ];

  # WHAT BOTH INSTALLATIONS SHARE, and every line of it is a decision.
  common = {
    remotes = [ flathub ];
    # FALSE, AND IT IS THE LINE THAT MATTERS MOST. `true` makes nix-flatpak
    # uninstall, at the next activation, every app and every remote it did not
    # declare — which is every Flatpak the person installed through the
    # software view, gone on the next `nixos-rebuild`. It stays false under the
    # new model too: the machine keeps apps CURRENT, it never removes the
    # person's.
    uninstallUnmanaged = false;
    # AUTO-UPDATE FROM HERE (owner amendment 2026-10-30, "the machine keeps
    # itself on the latest"): nix-flatpak installs and updates the DECLARED
    # Flatpaks — the dark extension, the distro's, `declaredFlatpaks` — on its
    # own timer and on each activation (`--or-update` on its installs; it
    # updates nothing it did not declare). The person's other Flatpaks are the
    # update sequence's `flatpak-system` / `flatpak-user` steps, with every
    # update, scheduled or by hand. This traffic reaches Flathub with no
    # `Pass` from `ronoctua-consent`: a declared updater of the system is the
    # owner's written exception to the consent rule (AGENTS.md, 2026-10-31).
    update.onActivation = true;
    update.auto.enable = true;
  };
in
{
  imports = [
    inputs.nix-flatpak.nixosModules.nix-flatpak
    # The registry, whose Flatpaks are installed below (see `browser.nix`
    # for why it is imported here too).
    ./distro-apps.nix
  ];

  config = lib.mkIf cfg.flatpak {
    # `xdg.portal.enable` is asserted by the NixOS module, and it is already on
    # (`nix/modules/scroll.nix`). This also puts both installations' `exports`
    # into `environment.profiles`, which is how their `.desktop` files and
    # binaries reach the session's `XDG_DATA_DIRS` and `PATH`.
    services.flatpak = common // {
      enable = true;
      # THE SYSTEM INSTALLATION DECLARES NO APP. The dark extension goes in the
      # Home Manager list below, whose activation can never fail a switch.
      packages = [ ];
    };

    # THE SYSTEM UNIT ASKS FLATHUB FOR METADATA AND NOTHING ELSE. It declares no
    # app, and the vendored `.flatpakrepo` makes `remote-add` offline (see the
    # header), so with no network it warns and ends with success
    # (`tests/software-vm.nix`); the unit that has to reach Flathub is the
    # person's, below. nix-flatpak orders this one after `multi-user.target`
    # alone; ordering it after the network gives the metadata its first try
    # with DNS (`after`/`wants` merge into the generated unit; they do not
    # replace it).
    systemd.services.flatpak-managed-install = {
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
    };

    # THE PERSON'S INSTALLATION, through Home Manager.
    #
    # Its activation runs `systemctl --user start … || true`, so a Flathub that
    # does not answer never fails a rebuild. It SKIPS, in silence, when the
    # system reports itself `degraded` — which is why Waydroid's container is
    # made to skip rather than fail before its image exists
    # (`nix/modules/android.nix`).
    home-manager.sharedModules = [ inputs.nix-flatpak.homeManagerModules.nix-flatpak ];
    home-manager.users.${user} = {
      services.flatpak = common // {
        packages = [
          # The dark theme for GTK 3 apps (see the header). The branch is named:
          # the extension is a runtime, and its only branch is `3.22`.
          {
            appId = "org.gtk.Gtk3theme.Adwaita-dark//3.22";
            origin = flathub.name;
          }
        ]
        ++ map (id: {
          appId = id;
          origin = flathub.name;
        }) cfg.declaredFlatpaks
        # AND THE DISTRO'S OWN (`assets/distro-apps.nix`), the ones this
        # machine carries and its person did not decline. Leaving this list
        # is what uninstalls one (see `declaredFlatpaks`), so a decline
        # reaches an app that is already installed.
        ++ config.ronoctua.distroApps.flatpaks;
        # The Qt variables the session exports are the HOST's plugins; inside a
        # sandbox they name nothing and turn a dark Qt app light (see the header).
        # Written as `flatpak override --user --unset-env` writes them — the file
        # the experiment measured.
        overrides.global = {
          Context."unset-environment" = [
            "QT_QPA_PLATFORMTHEME"
            "QT_STYLE_OVERRIDE"
          ];
          Environment = {
            QT_QPA_PLATFORMTHEME = "";
            QT_STYLE_OVERRIDE = "";
          };
        };
      };

      # ITS UNITS ARE THE ONES THAT REACH FLATHUB, and at a cold boot they
      # started before DNS: `Could not resolve hostname`, a failed start on
      # every boot, three seconds before the network came up. A user manager
      # has no `network-online.target` to be ordered after, so they ask:
      # `nm-online` waits up to thirty seconds and its "no" is a skip and not a
      # failure, which keeps an offline machine silent (and a switch offline
      # waits those thirty seconds). No `-x`: it would answer before
      # NetworkManager's autoconnect did. `Restart=on-failure` stays, for a DNS
      # that fails later; a skip never reaches it (see the dispatcher below).
      systemd.user.services = lib.mkMerge [
        (lib.genAttrs personUnits (_: {
          Service.ExecCondition = "${pkgs.networkmanager}/bin/nm-online -q";
        }))
        # ONE AT A TIME: a network that comes owes both at once, and a
        # persistent timer fires beside the login's run. Two together race on
        # a repository neither has made yet (`opendir(objects)`).
        { flatpak-managed-install-timer.Unit.After = [ "flatpak-managed-install.service" ]; }
      ];
    };

    # A RUN THE NETWORK SKIPPED IS OWED ONE WHEN THE NETWORK COMES. A skip is
    # not a failure, so nothing else would bring it back before the next login,
    # and a weekly run that fired offline would lose its week. Only a skipped
    # run is started: every other `up` asks Flathub nothing, so a network that
    # comes and goes costs no traffic. As the person, because a user manager's
    # socket answers its own uid (`setpriv`, with no PAM session for a network
    # event). No manager means nothing is owed: the next one starts these by
    # itself.
    networking.networkmanager.dispatcherScripts = lib.mkIf config.networking.networkmanager.enable [
      {
        type = "basic";
        source = pkgs.writeShellScript "ronoctua-flatpak-network" ''
          case "$2" in
            up | connectivity-change) ;;
            *) exit 0 ;;
          esac
          XDG_RUNTIME_DIR=/run/user/$(${pkgs.coreutils}/bin/id -u ${user})
          export XDG_RUNTIME_DIR
          [ -S "$XDG_RUNTIME_DIR/systemd/private" ] || exit 0
          person() {
            ${pkgs.util-linux}/bin/setpriv --reuid=${user} \
              --regid=${config.users.users.${user}.group} --init-groups \
              ${config.systemd.package}/bin/systemctl --user "$@"
          }
          for unit in ${lib.concatStringsSep " " personUnits}; do
            if [ "$(person show -p Result --value "$unit.service")" = exec-condition ]; then
              person start --no-block "$unit.service"
            fi
          done
        '';
      }
    ];

    # `/var/lib/flatpak` survives the root wipe through `nix/modules/persist.nix`,
    # which holds the whole list; a second list here would be a second place
    # to change. The person's installation lives in `$HOME`, which is a
    # subvolume of its own and never wiped.
  };
}
