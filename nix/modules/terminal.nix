# Terminal — section 7.10.
#
# "Official foreign emulator: Ghostty. Installed, available, and it is the
# default terminal UNTIL MILESTONE 7. After that it stays as an escape hatch."
#
# The final default is the native buffer, with the grid as a first-class node in
# the display list (`scene::grid`), NEVER `raw-surface`. That arrives in
# milestone 7; until then Ghostty is the real terminal.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua;

  # GHOSTTY READS ONE FILE, AND IT IS NOT `/etc/xdg`. In nixpkgs 26.05 (Ghostty
  # 1.3.1) the default configuration path is `$XDG_CONFIG_HOME/ghostty/config`
  # only — `XDG_CONFIG_DIRS` is not consulted (`ghostty +show-config --docs`,
  # `config-default-files`). Every key this module wrote under `/etc/xdg`,
  # decoration included, was a no-op until the file moved into the home: the
  # D5 review measured it with `+show-config` under both variables. So the
  # file is placed by home-manager, which already manages this user's
  # dotfiles (`nix/modules/home.nix`), at the one path Ghostty
  # reads — declared in the flake, not written by hand (invariant 6).

  # ONE WINDOW, ONE PROCESS, WHICHEVER DOOR OPENS IT. The config below says
  # `gtk-single-instance = false` — section 7.10: a Ghostty window is a foreign
  # buffer and the compositor does the layout, and the shell correlates a
  # window's PID with its buffer scope (7.5) — but the upstream desktop entry
  # passes `--gtk-single-instance=true` on its `Exec` lines, and a flag beats
  # the file. So a Ghostty started by the launcher (`LEADER a`, which runs the
  # entry's `Exec`) put every later window into the first process, and a
  # theme chosen afterwards reached none of them until the last one closed;
  # started by the footer (`ghostty`, by name) each window was its own.
  #
  # The entry is rewritten, not the binary: `symlinkJoin` costs no rebuild of
  # Ghostty, and the `Exec` paths are absolute, so a `PATH` wrapper would not
  # have been consulted. `DBusActivatable=true` goes with the flag: a launcher
  # that honours it starts Ghostty through the D-Bus service instead of
  # `Exec`, and that service is single-instance by construction (the process
  # it starts has to own the bus name to answer `Activate`), so the line was
  # a second door to the same defect; without it every launcher falls back to
  # `Exec`. The service file goes with it: NixOS links every package's
  # `share/dbus-1` into the session bus's search path, so an explicit
  # activation by name would still have found it. The day upstream drops the
  # flag and the activation this becomes a copy of the entry and nothing
  # more; `ghostty-windows-are-processes` measures what is installed either
  # way.
  ghosttyPerWindow = pkgs.symlinkJoin {
    name = "ghostty-per-window";
    paths = [ pkgs.ghostty ];
    postBuild = ''
      entry=share/applications/com.mitchellh.ghostty.desktop
      test -e "$out/$entry" || { echo "ERROR: Ghostty's desktop entry moved: $entry" >&2; exit 1; }
      rm "$out/$entry"
      sed -e 's/ --gtk-single-instance=true//g' -e '/^DBusActivatable=true$/d' \
        "${pkgs.ghostty}/$entry" > "$out/$entry"
      rm -f "$out/share/dbus-1/services/com.mitchellh.ghostty.service"
    '';
  };
in
{
  # The registry this module writes Ghostty's entry into (see `browser.nix`
  # for why it is imported here too).
  imports = [ ./distro-apps.nix ];

  options.ronoctua.terminal.ghostty = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Installs Ghostty as the foreign emulator (section 7.10).

      COST: on Linux Ghostty is GTK4 + libadwaita, so it pulls the whole GTK
      stack into the closure. Section 7.10 says to record that, and this
      option exists for anyone who wants to measure the system without it.
    '';
  };

  # The package that is installed — Ghostty with its desktop entry obeying the
  # module — exposed so a check can measure the entry that actually lands.
  options.ronoctua.terminal.package = lib.mkOption {
    type = lib.types.package;
    default = ghosttyPerWindow;
    defaultText = lib.literalMD "`pkgs.ghostty`, its desktop entry without `--gtk-single-instance`";
    description = "Ghostty as this module installs it: one window, one process, whichever door opens it.";
  };

  config = lib.mkMerge [
    {
      # GHOSTTY IS A PROGRAM OF THE REGISTRY (`nix/modules/distro-apps.nix`):
      # this switch says the machine has it, and a person may still decline
      # it. A decline takes the PACKAGE and its launcher entry and leaves the
      # rest of this module standing, on purpose: the terminfo below is also
      # the native terminal's (libghostty-vt), and the configuration file is
      # inert without the program. `buffer-terminal-external` then refuses
      # with a sentence, which is what a missing program gets.
      ronoctua.apps.ghostty = {
        package = cfg.terminal.package;
        when = cfg.terminal.ghostty;
        desktopIds = [ "com.mitchellh.ghostty.desktop" ];
      };
    }

    (lib.mkIf cfg.terminal.ghostty {
      # -----------------------------------------------------------------
      # Ghostty's configuration.
      #
      # Section 7.10 is explicit: "Configure OUT its own tabs, splits and
      # decoration — the compositor already does that job." A Ghostty window
      # is a foreign buffer; what tiles is the compositor (invariant 4).
      # Tabs inside it would be a second layout tree competing with the
      # compositor's.
      # -----------------------------------------------------------------
      home-manager.users.${cfg.user}.xdg.configFile."ghostty/config".text = ''
        # Generated by nix/modules/terminal.nix. Do not edit by hand.

        # --- the compositor does the layout, not the terminal (section 7.10) ---
        window-decoration = none
        gtk-tabs-location = hidden
        gtk-titlebar = false
        gtk-single-instance = false
        window-padding-x = 0
        window-padding-y = 0
        confirm-close-surface = false

        # --- the system font (section 6.2: it is the default everywhere) ---
        # The name comes from the token, it is not hard-coded here by
        # accident: it is the same `families.mono` that feeds fontconfig and the renderer.
        font-family = ${ronoctua.fonts.families.mono}
        font-size = ${toString ronoctua.fonts.sizes.terminalPoints}
        # The font is monospaced at exactly 0.600 em. Do not let Ghostty
        # adjust cell width, or the grid falls out of line with the rest of the system.
        adjust-cell-width = 0

        # --- section 5.1's aesthetics ------------------------------------
        # No shadow, no elevation, zero radius (invariant 15).
        background-opacity = 1.0
        cursor-style = block
        mouse-hide-while-typing = true

        # No bell, no badge, no notification of its own: notifications live in the
        # modeline (invariant 14).
        desktop-notifications = false

        # --- the palette, solved by the theme generator (nix/themes.nix) -------
        # Sixteen ANSI colours, background, foreground, cursor and selection,
        # mapped from the same families the footer draws with: the machine's
        # default theme first, on the theme's own ground, and after it — because
        # in Ghostty the later file wins — the theme the person chose with
        # `LEADER s t`, on the ground they chose for the windows (the generator
        # writes `ghostty-<ground>.conf` per theme, `theme::fragments` picks
        # one), which the footer copies to this path and which may not exist
        # (`?`). A Ghostty PROCESS
        # started after a change wears it; a running one keeps what it had —
        # and every window is a process: `gtk-single-instance = false` below,
        # which the desktop entry installed by this module no longer overrides
        # (`ghosttyPerWindow`, at the top).
        config-file = ${ronoctua.themed}/${cfg.theme}/ghostty-theme.conf
        config-file = ?/home/${cfg.user}/.config/ronoctua/ghostty-theme.conf

        # --- and the size of the text, which is the person's too ----------
        # THIRD, AND IN A FILE OF ITS OWN. `LEADER s d` moves one number and
        # everything the machine draws follows it — this is how it reaches the
        # terminal. `crates/ronoctua-shell-kit/src/toolkits.rs` writes a single
        # `font-size` line here.
        #
        # NOT INSIDE `ghostty-theme.conf`, and that is not tidiness: the theme
        # fragment is COPIED WHOLE out of the store by `theme::copy_fragments`,
        # so a size written into it would be erased by the next change of
        # theme. One writer per file is the rule the rest of this system keeps.
        #
        # LAST, because in Ghostty the later file wins: the size the person
        # chose beats the `font-size` above, and the theme in between carries
        # only colours.
        config-file = ?/home/${cfg.user}/.config/ronoctua/ghostty-size.conf
      '';

      # Ghostty's terminfo has to be available for remote sessions and for
      # milestone 7's native buffer — which inherits the SAME escape sequence
      # behaviour, by using libghostty-vt (section 7.10).
      environment.enableAllTerminfo = true;
    })
  ];
}
