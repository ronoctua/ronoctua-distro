# The person's own programs — the SUPPORT, declared; the programs, not.
#
# # What invariant 6 lets through, and what it does not
#
# The dated exception (`AGENTS.md`, 2026-09-26) lets the person install, try,
# update and remove programs FOR THEMSELVES — the user's Nix profile, the
# `--user` Flatpak installation, AppImages in `~/Applications`, apps inside
# Android, web apps. Those live under `$HOME` and are never a distro default.
# What makes them possible is not an exception at all, and it is all here or
# beside here: the Flatpak and nix-flatpak modules (`flatpak.nix`), AppImage
# (`appimage.nix`), Waydroid (`android.nix`), the search index, and the CLI
# that does the work (`crates/ronoctua-software`).
#
# # Why the CLI is WRAPPED, and why the wrapper is per machine
#
# The update sequence runs `${package}/bin/ronoctua-software`, and a runner
# sees no unit environment at all — so the only thing that reaches every
# caller (the queue, the update, a `.desktop` line, a terminal) is the binary's
# own wrapper. It sets, as defaults, exactly the tools THIS machine has: a
# reduced profile without Waydroid gets no `RONOCTUA_WAYDROID`, so its closure
# carries no Waydroid through the update runner, and the crate answers
# `Refusal::SourceAbsent` with a sentence instead of an `exec` that fails.
#
# The table itself — which variable gets which path — is `nix/software-tools.nix`,
# because the nested loop and the photograph stage need it too and neither can
# read a NixOS option.
{
  config,
  lib,
  pkgs,
  inputs,
  ronoctua,
  ...
}:

let
  superpack = config.ronoctua.icons.superpack;

  # WHAT THIS MACHINE HAS, asked of the modules that turned it on and never of
  # this module's own switches: a host that enables Flatpak by hand has it, and
  # a switch that is on in a module that did not load does not.
  tools = import ../software-tools.nix {
    inherit lib pkgs;
    # The machine's own Nix, the one that fills this store.
    nix = config.nix.package;
    index = ronoctua.softwareIndex;
    nixpkgsRev = inputs.nixpkgs.rev;
    flatpak = if config.services.flatpak.enable then config.services.flatpak.package else null;
    appimageRun = if config.programs.appimage.enable then config.programs.appimage.package else null;
    waydroid =
      if config.virtualisation.waydroid.enable then config.virtualisation.waydroid.package else null;
    chrome = config.ronoctua.browser.enable;
    # THE SUPERPACK'S ROOT is `nix/modules/icons.nix`'s to write — read from
    # where it wrote it, so the two can never name different directories.
    icons = if superpack then config.environment.sessionVariables.RONOCTUA_ICONS else null;
    iconNames = if superpack then ronoctua.iconNames else null;
  };

  # WHAT ONLY THE WRAPPER CARRIES.
  #
  # GIO's dconf module, so a `flatpak` started from a unit reads the person's
  # `gtk-theme` from dconf. It is how flatpak chooses the GTK3 theme extension
  # to pull beside an app; without the module GSettings falls back to its
  # in-memory backend, answers `Adwaita`, and a dark desktop gets light GTK3
  # apps with nothing saying why. `programs.dconf` puts the same value in the
  # LOGIN session, which a user unit does not inherit.
  #
  # NOT on the footer's unit: every foreign app the footer launches would
  # inherit it, and that is a change to how every GTK program on the machine
  # reads its settings — a decision of its own, not a side effect of this one.
  # NOT in the table either: the nested loop runs on a host that may not be
  # NixOS, and a GIO module built against our glib, loaded by the host's, is a
  # crash in a program that has nothing to do with this.
  wrapperOnly = {
    GIO_EXTRA_MODULES = "${pkgs.dconf.lib}/lib/gio/modules";
  };

  # THE ANSWER TO A DOUBLE CLICK on a file one installs: the file manager
  # opens it with this, and the shell draws the install sheet for it.
  #
  # `NoDisplay`: it is not a program one starts, it is an answer to a question
  # the system asks — the shape of `ronoctua-transfers.desktop`. The shell is
  # named by its STABLE path and not its store path: the store path would put
  # the whole desktop in the closure of this package, which the update runs as
  # root. English, because a `.desktop` carries one language (invariant 18).
  #
  # No `mimeapps.list` beside it: the player already writes one into
  # `share/applications`, two packages writing the same path into the system
  # profile is a collision, and the launcher prefers `br.ronoctua.*` entries on
  # its own (`apps::opens`).
  desktopEntry = pkgs.writeText "br.ronoctua.software.desktop" ''
    [Desktop Entry]
    Type=Application
    Name=Software
    Comment=Installs this file for you: an AppImage, an Android app or a Flatpak
    Exec=/run/current-system/sw/bin/ronoctua-shell software-file %f
    TryExec=/run/current-system/sw/bin/ronoctua-shell
    Terminal=false
    NoDisplay=true
    MimeType=application/vnd.appimage;application/x-iso9660-appimage;application/vnd.android.package-archive;application/vnd.flatpak.ref;
  '';

  # THE CLI, WRAPPED FOR THIS MACHINE.
  #
  # `makeBinaryWrapper` and not `makeWrapper`, for `nix/rust.nix`'s reason: the
  # second one puts a bash in the closure. `--set-default` and not `--set`, so
  # a variable exported by hand (a test, a person debugging) still wins.
  #
  # `RONOCTUA_SOFTWARE` is the wrapper ITSELF: the queue runs whatever that
  # variable names, and it has to be this program with this environment, not
  # the bare binary the wrapper wraps.
  package = pkgs.runCommand "ronoctua-software" { nativeBuildInputs = [ pkgs.makeBinaryWrapper ]; } ''
    mkdir -p $out/bin $out/share/applications
    makeBinaryWrapper ${ronoctua.software}/bin/ronoctua-software $out/bin/ronoctua-software \
      ${
        lib.concatStringsSep " \\\n  " (
          lib.mapAttrsToList (name: value: "--set-default ${name} ${lib.escapeShellArg value}") (
            tools // wrapperOnly
          )
        )
      } \
      --set-default RONOCTUA_SOFTWARE $out/bin/ronoctua-software
    cp ${desktopEntry} $out/share/applications/br.ronoctua.software.desktop
  '';

  # WHAT THE TWO UNITS GET: the table, and the wrapper by its store path — the
  # queue runs the CLI of its own generation, never whatever the profile
  # points at after a switch.
  unitTools = tools // {
    RONOCTUA_SOFTWARE = "${package}/bin/ronoctua-software";
  };
in
{
  options.ronoctua.software = {
    # Flatpak stays on in BOTH profiles: it is support and not an application,
    # and without it the reduced machine has no way to a graphical program the
    # flake does not carry.
    flatpak =
      lib.mkEnableOption "Flatpak apps for this person (Flathub, the `--user` installation)"
      // {
        default = true;
      };
    # AppImage and Android are `mkDefault full` in `nix/modules/profile.nix`.
    # Measured on their own: `appimage-run` is an FHS environment whose closure
    # is 1,53 GB, and Waydroid's is 0,72 GB before the Android image, which is
    # downloaded only when the person asks (`nix/modules/android.nix`).
    appimage = lib.mkEnableOption "AppImage apps (`appimage-run`, `~/Applications`)" // {
      default = true;
    };
    android = lib.mkEnableOption "Android apps (Waydroid)" // {
      default = true;
    };

    declaredFlatpaks = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "org.gnome.Calculator" ];
      description = ''
        Flathub application ids this machine keeps installed for the person,
        declaratively, written by hand in `host.nix` — the software view
        declares a program in the person's `declared.json` instead, which
        follows them to every machine (`nix/modules/person.nix`). They go to
        the person's own `--user` installation
        through nix-flatpak's Home Manager module (`nix/modules/flatpak.nix`),
        which adopts an app that is already installed instead of installing it
        twice.

        Removing an id from here UNINSTALLS it at the next activation:
        nix-flatpak keeps the list it applied last, and takes back what
        leaves it — read in its generated script, not assumed. What the
        person installed by hand and never declared is never touched, which
        is what `uninstallUnmanaged = false` is for.
      '';
    };

    # THE WRAPPED CLI and THE TABLE, as handles and not knobs — the shape of
    # `ronoctua.update.package`. The update module needs the first (a step
    # runs it by store path); a check reads the second without digging it out
    # of a unit.
    package = lib.mkOption {
      type = lib.types.package;
      readOnly = true;
      internal = true;
      default = package;
      defaultText = "ronoctua-software, wrapped with this machine's tools";
      description = "The software CLI, with the paths of the tools this machine has.";
    };
    tools = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      readOnly = true;
      internal = true;
      default = unitTools;
      defaultText = "RONOCTUA_* -> the paths of the tools this machine has";
      description = "The variables the footer's and the queue's units carry.";
    };
  };

  # `nixpkgs#…` IS THE SYSTEM'S OWN nixpkgs, LOCKED, and the global registry
  # comes from the store: `nix-registries.nix`, beside this module, which is
  # who needs them (a VM test imports it alone).
  imports = [ ./nix-registries.nix ];

  config = {
    # THE STABLE PATH: `/run/current-system/sw/bin/ronoctua-software`, which
    # is what a web app's `.desktop` line names — a store path there would
    # open a deleted program after the next garbage collection.
    environment.systemPackages = [ package ];

    # THE FOOTER AND THE QUEUE, each in its unit: a user unit inherits the
    # user manager's environment and not a login shell's, so a variable set
    # only in `sessionVariables` reaches neither (`nix/modules/scroll.nix`).
    #
    # `RONOCTUA_ICONS` stays out of the footer's: `nix/modules/icons.nix`
    # already puts it there, and one variable has one writer.
    systemd.user.services.ronoctua-footer.environment = removeAttrs unitTools [ "RONOCTUA_ICONS" ];
    systemd.user.services.ronoctua-transfers = lib.mkIf config.ronoctua.transfers.enable {
      environment = unitTools;
    };

    # THE UPDATE MOVES WHAT THE MACHINE CAN MOVE.
    #
    # The update's `steps.nix` declares the steps; which of them apply is decided by
    # the module that knows the support is there — the shape `nix/modules/cli.nix`
    # has for `tldr-cache`. A step for a source this machine does not have
    # would only ever be skipped, and a skipped line on every run teaches
    # people to stop reading the list. `android-images` is not here: it
    # rewrites the system image as root, and it ships off everywhere until a
    # machine's own configuration turns it on.
    #
    # `mkDefault`, so a machine that wants otherwise says so in one line.
    ronoctua.update.steps = {
      flatpak-system = lib.mkDefault config.services.flatpak.enable;
      flatpak-user = lib.mkDefault config.services.flatpak.enable;
      flatpak-unused = lib.mkDefault config.services.flatpak.enable;
      appimages = lib.mkDefault config.programs.appimage.enable;
      appimage-cache = lib.mkDefault config.programs.appimage.enable;
      android-apps = lib.mkDefault config.virtualisation.waydroid.enable;
    };

    # THE 2026-10-05 WAVE: one region per package, after its anchor.
    # ── anchor: L03 ──
    # ── anchor: L18 ──
  };
}
