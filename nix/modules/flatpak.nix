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

  # WHAT BOTH INSTALLATIONS SHARE, and every line of it is a decision.
  common = {
    remotes = [ flathub ];
    # FALSE, AND IT IS THE LINE THAT MATTERS MOST. `true` makes nix-flatpak
    # uninstall, at the next activation, every app and every remote it did not
    # declare — which is every Flatpak the person installed through the
    # software view, gone on the next `nixos-rebuild`.
    uninstallUnmanaged = false;
    # NO UPDATES FROM HERE. The distro's own sequence updates Flatpaks
    # (`flatpak-system` and `flatpak-user` in the update's `steps.nix`) when the
    # brain or the person decides; a second updater on its own timer would be
    # a second answer to "when does this machine change".
    update.onActivation = false;
    update.auto.enable = false;
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

    # THE PERSON'S INSTALLATION, through Home Manager.
    #
    # Its activation runs `systemctl --user start … || true`, so a Flathub that
    # does not answer never fails a rebuild. It SKIPS, in silence, when the
    # system reports itself `degraded` — which is why Waydroid's container is
    # made to skip rather than fail before its image exists
    # (`nix/modules/android.nix`).
    home-manager.sharedModules = [ inputs.nix-flatpak.homeManagerModules.nix-flatpak ];
    home-manager.users.${user}.services.flatpak = common // {
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

    # `/var/lib/flatpak` survives the root wipe through `nix/modules/persist.nix`,
    # which holds the whole list; a second list here would be a second place
    # to change. The person's installation lives in `$HOME`, which is a
    # subvolume of its own and never wiped.
  };
}
