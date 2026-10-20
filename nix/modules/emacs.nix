# Emacs, plain, as a daemon — and no configuration of the distro's.
#
# WHAT THE DISTRO DOES, AND WHAT IT LEAVES ALONE
# ----------------------------------------------
# The package (`emacs30-pgtk`, as nixpkgs builds it), the daemon (`services.emacs`,
# a user unit, so `emacsclient` opens in a process that has already loaded
# everything), the variable the file manager reaches `emacsclient` by, and the
# switch. Nothing else: no `init.el`, no `default.el`, no file under
# `~/.config/emacs` or `~/.emacs.d`, nothing in `/etc` that Emacs reads — at
# most the EMPTY folder `$XDG_CONFIG_HOME/emacs` (`emacsHome`, below). The owner's
# decision (2026-10-19): the configuration is the PERSON's, and it travels
# with their belongings (the target `emacs`, `assets/belongings.nix`). A
# configuration the distro wrote would either be overwritten by theirs or
# overwrite it on every rebuild.
#
# The package carries nixpkgs' own `site-start.el` — the load paths of the Nix
# profiles and of native compilation, `tramp`'s remote `PATH` — which is how the
# package finds what is installed, and sets no preference of anybody's.
#
# NO TOOLS RIDE WITH IT. What a configuration wants beside the editor (a
# document converter, a spell checker, a language server) is the person's to
# declare, like any other program: the editor's entry is Emacs alone.
#
# WHY A DAEMON AT ALL: one process loads the person's configuration ONCE.
# Without it, every `$EDITOR` invocation pays the whole startup — seconds, with
# a configuration of any size — which is enough that one stops using it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua;
  # THE PURE-GTK BUILD, Wayland's own: its frames are `xdg_toplevel`s of the
  # compositor, never X windows through XWayland. Its `emacsclient -c` reads
  # `WAYLAND_DISPLAY` first and `DISPLAY` only when the server refuses that
  # name, so the session exporting XWayland's `DISPLAY` beside it
  # (`nix/session-environment.nix`) changes nothing here — `tests/emacs-vm.nix`
  # opens the frame with both. The X11 build's (`emacs30`, Lucid) reads only
  # `DISPLAY`, which the session did not export when this was chosen: started
  # from the session, it found no display and no terminal, and opened nothing.
  #
  # WHAT IT COSTS: GTK keeps a display it drew on, and leaves with it — a
  # compositor that ends on its own (a crash) takes the daemon with it,
  # through GDK's `_exit(1)`, and the unit's `Restart=always` brings it
  # back empty: every buffer is gone, process buffers too (shells,
  # compilations, TRAMP connections), and nobody was asked about the unsaved
  # ones. And `_exit` skips `kill-emacs` whole: the edits since the last
  # auto-save are not auto-saved, and `kill-emacs-hook` never runs (what
  # savehist, recentf, bookmarks and desktop-save-mode write at the end is not
  # written). The Lucid daemon outlived the same end. A STOP of the person's
  # manager is not that: the unit is ordered so the daemon stops first, on a
  # SIGTERM that runs `kill-emacs` (below). Nor is `s q q`: the footer
  # restarts the daemon before it asks the compositor to leave (below). The
  # buffers still go, but through `kill-emacs`. `tests/emacs-vm.nix` proves
  # the frame native and holds the measure (`docs/ARCHITECTURE.md`).
  emacsPkg = pkgs.emacs30-pgtk;

  # WHERE A FIRST CONFIGURATION IS BORN — before the daemon starts, an EMPTY
  # `$XDG_CONFIG_HOME/emacs` in a home that has none of the places Emacs
  # reads (`~/.emacs.el`, `~/.emacs.elc`, `~/.emacs`, `~/.emacs.d`, that
  # folder). Emacs 30 picks its init folder in `startup.el`: `~/.emacs.d`
  # when it or `~/.emacs` exists, else the XDG one when it exists, else
  # `~/.emacs.d` — which the daemon's first start then makes, with nothing of
  # the person's in it (measured in a bare home: the folder and the native
  # compiler's `eln-cache/`, where nixpkgs' `site-start.el` lands compiled
  # within seconds; the server's socket is under `$XDG_RUNTIME_DIR`) — and
  # Customize, `package-install` and an enabled command write `~/.emacs`.
  # Neither is the one place the belongings carry (target `emacs`:
  # `~/.config/emacs`), and that `~/.emacs.d` alone hides the XDG folder from
  # every start after it. With the folder there
  # first, Emacs reads `~/.config/emacs/init.el`, writes into it, and puts its
  # packages and caches under it — all inside the carried place, the packages
  # skipped. Measured with Emacs 30.2 in throwaway homes
  # (`docs/ARCHITECTURE.md`).
  #
  # A folder and never a file; and never in a home that has any of them — a
  # `~/.emacs.el` alone is read only while no XDG folder exists, so making
  # one there would switch the person's configuration off. Emacs's own test
  # is `file-exists-p`, which follows a link, and a DANGLING link is not
  # there for it; but a link at one of these names is the person's doing,
  # and `mkdir -p` over a dangling one fails: so a name counts when it is
  # there or is a link (`-e || -L`), and the home is left alone.
  #
  # IT NEVER STOPS THE DAEMON. The step is a convenience: whatever fails in
  # it (a dangling link raced in, a read-only home, a full disk) is said on
  # stderr — the daemon's journal — and the step still exits 0; and the unit
  # runs it with `-`, so not even a shell that cannot start keeps the daemon
  # from starting (`Restart=always` would otherwise retry it for ever).
  #
  # The folder is Emacs's own XDG folder, as `startup.el` makes it: the
  # variable's value joined by `/emacs` — set but empty, that is `/emacs`,
  # which cannot be made (and Emacs then uses `~/.emacs.d`) —, a leading `~`
  # the home, a relative value under the unit's working directory (the home,
  # as Emacs sees it there), and `~/.config/emacs` without the variable.
  emacsHome = pkgs.writeShellScript "ronoctua-emacs-home" ''
    if [ "''${XDG_CONFIG_HOME+set}" = set ]; then
      xdg="$XDG_CONFIG_HOME/emacs"
    else
      xdg="$HOME/.config/emacs"
    fi
    case "$xdg" in
      "~"/*) xdg="$HOME/''${xdg#"~/"}" ;;
    esac
    for place in "$HOME/.emacs.el" "$HOME/.emacs.elc" "$HOME/.emacs" "$HOME/.emacs.d" "$xdg"; do
      if [ -e "$place" ] || [ -L "$place" ]; then
        exit 0
      fi
    done
    if ! ${pkgs.coreutils}/bin/mkdir -p -m 0700 "$xdg"; then
      echo "ronoctua-emacs-home: could not make $xdg; Emacs starts without it" >&2
    fi
    exit 0
  '';
in
{
  # The registry this module writes the editor's entry into (see
  # `browser.nix` for why it is imported here too).
  imports = [ ./distro-apps.nix ];

  options.ronoctua.editor.emacs = lib.mkEnableOption "Emacs 30 and its daemon (~0.5 GiB)" // {
    default = true;
    # A DECLINE TURNS THE EDITOR OFF, daemon included — the package alone
    # would leave a daemon unit starting a program that is not on the
    # machine's `PATH`. An `apply`, as Chrome's switch has (`browser.nix`
    # says why).
    apply = carried: carried && !(lib.elem "emacs" config.ronoctua.distroApps.honoured);
  };

  config = lib.mkMerge [
    {
      # EMACS IS A PROGRAM OF THE REGISTRY (`nix/modules/distro-apps.nix`).
      # Its four launcher entries are the package's own: `services.emacs.enable`
      # alone puts the package in `systemPackages` in nixpkgs 26.05
      # (`install = false` below does not keep it out), and `emacs.desktop` and
      # `emacsclient.desktop` with it.
      #
      # OUTSIDE the `mkIf` below, because the switch reads the registry (its
      # `apply`, above): an entry inside it would make the registry's names
      # depend on the switch (see `distro-apps.nix`).
      ronoctua.apps.emacs = {
        package = emacsPkg;
        group = "editor";
        desktopIds = [
          "emacs.desktop"
          "emacsclient.desktop"
          "emacs-mail.desktop"
          "emacsclient-mail.desktop"
        ];
      };
    }

    # EVERYTHING ELSE hangs off the switch.
    (lib.mkIf cfg.editor.emacs {
      # WHERE THE FILE MANAGER SENDS TEXT — and it is set HERE, inside this
      # `mkIf`, rather than baked into the binary.
      #
      # `nix/rust.nix` used to hold it as an `option_env!`, and that made the
      # whole of Emacs a reference the workspace carried into every closure:
      # `ronoctua.editor.emacs = false` took the package off `systemPackages` and
      # changed nothing at all. It is the same trap the icon superpack sprang, and
      # the same answer — read it from the environment, set it where the switch
      # is. `profile-minimal-closes` is the check that refuses the other way.
      #
      # A store PATH and never a name: the footer runs inside a scope whose `PATH`
      # is minimal, and a bare name fails far from its cause.
      systemd.user.services.ronoctua-footer.environment.RONOCTUA_EMACSCLIENT =
        "${emacsPkg}/bin/emacsclient";

      # THE FOLDER FIRST (`emacsHome`), in the environment the daemon starts
      # in — the same `set-environment` its `ExecStart` sources, so both see
      # the same `XDG_CONFIG_HOME`. By store path: a user unit's `PATH` is
      # minimal. `-`: its failure never keeps the daemon from starting. An
      # attribute set and not a dotted path: the dotted one spells the unit's
      # file name, which `brain-modules-are-sealed` reads as this file naming
      # the unit a brain module watches.
      #
      # AND IT STOPS BEFORE THE COMPOSITOR (`After=`, which systemd reverses
      # at a stop). When the person's manager ends — a power-off, a reboot,
      # the end of the last login — every unit is stopped at once unless an
      # order says otherwise, and the pure-GTK daemon with a frame open raced
      # the compositor: when the compositor went first, GDK ended the process
      # (`_exit(1)`, after "Error reading events from display" or "Lost
      # connection to Wayland compositor") in the middle of the SIGTERM's
      # `kill-emacs` — measured, the hook had run and the auto-save had not,
      # in four stops of six. Ordered, Emacs gets the SIGTERM while its
      # display is still there, and the compositor waits for it to finish. It costs nothing at boot: the
      # compositor has no job there (the login shell starts it), and an order
      # with no job beside it waits for nothing — the daemon still comes up
      # at `default.target` with no session. The compositor ENDING on its own
      # is not a stop of this unit: a crash still takes the daemon
      # (`emacsPkg`, above), and `s q q` restarts it first (below).
      # `tests/emacs-vm.nix` stops the manager with a frame open and an edit
      # unsaved, again and again.
      systemd.user.services.emacs = {
        after = [ "scroll.service" ];
        serviceConfig.ExecStartPre = "-${pkgs.runtimeShell} -c 'source ${config.system.build.setEnvironment}; exec ${emacsHome}'";
      };

      # AND IT ENDS BEFORE THE SESSION DOES (`ronoctua.session.restartBeforeQuit`,
      # `base.nix`): `s q q` restarts the daemon, if it runs, before the
      # footer asks the compositor to leave — the SIGTERM's `kill-emacs` runs
      # (the hook, the auto-save) with the display still there, and the
      # daemon that takes its place has no frame, so the compositor's end
      # leaves it alone and it is there at the next login. The unit by the
      # name its option gives it: spelt out, the name is the one a brain
      # module watches (`brain-modules-are-sealed`). `tests/emacs-vm.nix`
      # quits through the real footer with a frame open and an edit unsaved.
      ronoctua.session.restartBeforeQuit = [ config.systemd.user.services.emacs.name ];

      services.emacs = {
        enable = true;
        package = emacsPkg;
        # `install = false`, and it keeps NOTHING out of the launcher. The line
        # used to say it did: that `install = true` would add `emacs.desktop`
        # and a `client` entry, two Emacsen in the locator (`LEADER a`), one of
        # which does not use the daemon. Read in the pinned nixpkgs, the module
        # puts `package` in `systemPackages` whenever `enable` is on, so both
        # entries are there either way. The registry owns them now
        # (`distro-apps-are-registered` reads the built `system.path`), and
        # `docs/PENDING.md` keeps the question of hiding them.
        install = false;
        defaultEditor = false; # `$EDITOR` is set by assets/shell/shellrc.sh
        startWithGraphical = false; # a terminal Emacs must work without a session
      };
    })
  ];
}
