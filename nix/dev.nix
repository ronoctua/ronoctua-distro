# `nix run .#dev` — the nested loop (section 7.12, speed 1).
#
# The compositor as a WINDOW inside the host session, with our footer inside
# it. **90% of the shell work happens here**, and it is the difference between
# iterating in seconds and iterating in minutes — section 15 puts the feedback
# loop as principle no. 1.
#
# `Mod` becomes `Alt` in this mode, because Super belongs to the compositor
# running OUTSIDE and never reaches in.
#
# # What it looks like on a fractionally-scaled host
#
# SOFT, and that part is not ours: `wlroots`' Wayland backend never calls
# `wl_surface_set_buffer_scale` and never binds `wp_fractional_scale_v1` — the
# word `scale` does not appear in `backend/wayland` at all — so it commits a
# buffer at scale 1 and the host resamples it. Nothing inside can change that.
#
# BUT THE SIZE WAS OURS, AND IT WAS WRONG. The host hands the nested window a
# size in LOGICAL pixels and the backend makes that the output's PIXEL mode, so
# a session left at scale 1 lays out for the logical box and is then blown up
# by the host's scale: on a 1920x1200 panel at 1.6 the nested output came up
# 600x750 for an area with 960x1200 real pixels in it — everything 60% too big,
# and five pixels in eight thrown away. Measured, with a headless host at 1.6
# and this very config nested inside it.
#
# So the loop asks the host what it is doing and gives the nested output the
# reciprocal (`the host's own pixels`, below). The resampling stays; the zoom
# goes, and the session gets the whole area it is drawn on.
{
  lib,
  callPackage,
  gnused,
  runCommand,
  writeText,
  writeShellApplication,
  scroll,
  swaybg,
  # The solved palettes and the list of themes, so the nested compositor wears
  # the default theme's three colours (see `nix/themes.nix`).
  atlasData,
  # Roots the flake's downloads before anything builds (nix/kept-sources.nix).
  keepSources,
  # One Xcursor theme per cursor colour (nix/cursors.nix).
  cursors,
  themed,
  themes,
  defaultTheme,
  ronoctua-shell,
  atRunTime,
  generated,
  nix,
  fonts,
  fontconfigConf,
  iconSuperpack,
  ffmpeg-full,
  exiftool,
  poppler-utils,
  catdoc,
  graphicsHost,
  reloadApp,
  singleFooter,
  rustToolchain,
  # The editor the manager sends text to. It is a parameter and not an
  # `option_env!` for the reason `nix/rust.nix` writes out: baked into the
  # binary, its store path is a reference no profile switch can remove.
  emacs30,
  # The screenshot editor, for the notification a capture raises.
  swappy,
  # `wayland-info`, which asks the HOST what it is doing — see `the host's own
  # pixels`. Dev loop only; it reaches no installed profile.
  wayland-utils,
  # `xdpyinfo`, for the X half of the same question. Dev loop only.
  xdpyinfo,
}:

let
  # The chords, generated from the shell's command table — the same ones as in
  # production, only with a different prefix.
  binds = callPackage ./compositor-binds.nix { inherit ronoctua-shell; };
  # And the window rules, for the same reason: the monitor has to open floating
  # HERE too, or the nested loop stops being where one sees what production does.
  windowRules = callPackage ./compositor-window-rules.nix { inherit ronoctua-shell; };

  # The colours come from the TOKENS, as in nix/modules/scroll.nix — see the
  # comment over there. Here they matter twice over: the nested loop is where one
  # LOOKS at the screen, and a colour different from production's would make the
  # working loop validate something nobody is going to see.
  # The SOLVED palette of the default theme, as files (see `nix/themes.nix`).
  palette = "${themed}/${defaultTheme}";
  # The SAME layout as production: the nested loop is where one types to test, and
  # a different keyboard here would be testing something else.
  keyboard = import ./keyboard-xkb.nix;
  # WITH ONE DIFFERENCE, AND IT IS THE ONE THIS LOOP CANNOT DO WITHOUT: the
  # compiled keymap below is that same layout with Super taken out of it, so a
  # chord the HOST keeps for itself cannot leave a modifier stuck in here for
  # the rest of the session. The whole mechanism, and the measurement, are in
  # the header of `nix/keyboard-nested.nix`.
  nestedKeymap = callPackage ./keyboard-nested.nix { };

  # WHAT THE BUILD BAKES, EXPORTED FOR THE BINARY THIS LOOP COMPILES.
  #
  # Everything `nix/rust.nix` puts in the compilation's environment reaches the
  # Rust through `option_env!`, which resolves at COMPILE time. The binary hot
  # reload builds is compiled HERE, so what it gets is what this script exports
  # and nothing else.
  #
  # It used to export six of these, written out by hand, and miss twenty-five:
  # the reloaded footer had no `btop`, no clipboard, no `ghostty-vt`, no
  # terminfo. Nobody saw it because the reload was wired to a command that did
  # not exist (further down), so no edit ever replaced the store's binary — the
  # two defects were hiding each other.
  #
  # Two spaces between lines: what strips a `''` string is the indentation of
  # the SOURCE lines, and interpolated content goes in as it is — with no
  # separator the second `export` would land in column zero.
  bakedExports = lib.concatStringsSep "\n  " (
    lib.mapAttrsToList (name: value: ''export ${name}="${value}"'') (generated // atRunTime)
  );

  # The compositor's config for nested mode.
  #
  # The DIFFERENCE from production is deliberate and small: no session
  # `spawn-at-startup`, and the leader becomes `Alt` because `Mod` belongs to the
  # HOST compositor when running nested — pressing `Mod` out there would not
  # reach in here.
  # THE CONFIG AS A DERIVATION, and not assembled by the script at run time.
  #
  # It used to be a heredoc + `sed` inside `ronoctua-dev`: assembled only at run
  # time, and for that reason NOTHING validated it. A `duplicate keybind` sailed
  # through the whole `nix flake check` and only showed up as a red box on screen —
  # with the compositor falling back to its own defaults — that is, with no leader.
  #
  # As a derivation, it is a file the guards below can read — and a VM test
  # is what puts it in front of a real compositor.
  nestedConfigText = ''
    # GENERATED by nix/dev.nix — ONLY for the nested loop.
    # The production config is in `nix/modules/scroll.nix`.

    # THE NAG WINDOW IS REFUSED, for the reason `nix/modules/scroll.nix` writes
    # out: the compositor raises it on its own for a config error and for a
    # scale it dislikes, and it is a floating window with its own buttons on a
    # desktop whose only chrome is the footer (invariant 14). `-` is the
    # spelling that leaves the command NULL.
    scrollnag_command -

    # `Mod1` (Alt) AND NOT `Mod4` (Super), which is Scroll's own default.
    #
    # Super belongs to the HOST compositor when running nested — it never
    # reaches in here — and `nix/keyboard-nested.nix` takes it out of the keymap
    # entirely. Alt is also what `.#dev` uses for the CHORD, so the two nested
    # loops answer the same fingers: `Alt+<arrow>` moves the focus in both.
    set $mod Mod1

    input type:keyboard {
        xkb_layout "${keyboard.layout}"
        xkb_variant "${keyboard.variant}"
        xkb_options "${keyboard.options}"
        # `xkb_file` WINS over the three lines above; they stay because they are
        # what the file is compiled FROM, and what sway falls back to if the
        # file cannot be read.
        xkb_file "${nestedKeymap}"
    }

    # No `type:touchpad` block: the touchpad defaults are `ronoctua-pointers`'
    # (see the same note in `nix/modules/scroll.nix`).

    # THE COLOURS COME FROM THE TOKENS (invariant 9), spliced from the solved
    # palette by the derivation below — the same three files `.#dev` and
    # `nix/modules/scroll.nix` reads.
    #
    # `client.focused` takes five colours and only two of them are ours to pick:
    # border and indicator. The rest are sway's own decorations, which invariant
    # 15 would not allow on screen anyway — `default_border pixel` draws no
    # titlebar, so `background` and `text` are never painted.
    client.focused @BORDER_ACTIVE@ @BORDER_ACTIVE@ @BORDER_ACTIVE@ @BORDER_ACTIVE@ @BORDER_ACTIVE@
    client.focused_inactive @BORDER_INACTIVE@ @BORDER_INACTIVE@ @BORDER_INACTIVE@ @BORDER_INACTIVE@ @BORDER_INACTIVE@
    client.unfocused @BORDER_INACTIVE@ @BORDER_INACTIVE@ @BORDER_INACTIVE@ @BORDER_INACTIVE@ @BORDER_INACTIVE@

    # THE DESK. Same token as production,
    # another compositor's spelling.
    output * bg @BACKGROUND@ solid_color

    # THE NESTED OUTPUT'S SIZE AND SCALE ARE APPENDED AT RUN TIME, because
    # both come from the host and the host is not known at build time — see
    # `the host's own pixels`, further down.
    #
    # There used to be a fixed `mode 1600x900` here, described as "the nested
    # window's size". It was neither: under a TILING host the size is the
    # host's configure and the line is inert (measured — asking for 1600x900
    # on a 1200x750 logical desktop gave an output of 600x750), and under a
    # host that lets the client choose it asked for a window a third wider
    # than the whole screen.
    #
    # `output *` and not the backend's own name for it, because that name
    # depends on which backend came up — `WL-1` nested inside Wayland, `X11-1`
    # inside an X server — and there is exactly one output in here either way.
    output * position 0 0

    default_border pixel 2
    # No titlebar, no shadow, no rounded corner (invariant 15).
    titlebar_padding 1
    gaps inner 8
    gaps outer 0

    # THE FOOTER IS NOT SPAWNED FROM HERE, and the reason is the RESTART: there
    # is no way to bring a child of the compositor back without restarting the
    # compositor, and the whole point of this loop is that the footer restarts
    # while the session stays. The script starts it, and `footer-single.nix`
    # keeps it up.
    #
    # Its output is not the reason. `cmd_exec` here forks and `setsid`s and
    # redirects nothing (`sway/commands/exec_always.c`), so a child would
    # inherit the compositor's own stdout — which is more than the previous
    # compositor gave, and still not where a restart can be asked for.
    #
    # THIS `exec` IS HOW THE SCRIPT LEARNS THE DISPLAY. sway picks its Wayland
    # socket with `wl_display_add_socket_auto`, so the name is not knowable from
    # outside and not passable in; the compositor itself is the only honest
    # source. It writes the name to the file the script named, and the script
    # waits for that file.
    exec sh -c 'printf %s "$WAYLAND_DISPLAY" > "$RONOCTUA_SCROLL_DISPLAY_FILE"'

    # ------------------------------------------------------------------
    # The leader, and the four emergency binds
    # ------------------------------------------------------------------
    # THE SAME CHORD AS `.#dev`, and for the same reasons written out there:
    # not `Mod` (the host has it), not plain `Alt+Space` (that is the window
    # menu in most desktops, so the host swallows it).
    #
    # `--no-repeat`: holding the key would re-fire the bind at the keyboard's
    # repeat rate and reopen the sequence on every fire. The flag spells the
    # `repeat=false` this way.
    bindsym --no-repeat Ctrl+Alt+space exec ${ronoctua-shell}/bin/ronoctua-shell leader
    # AND THE SAME CHORD WITH A SHIFT NOBODY PRESSED — a stray Shift from the
    # host is the one modifier this keymap cannot rule out. See `.#dev`.
    bindsym --no-repeat Ctrl+Shift+Alt+space exec ${ronoctua-shell}/bin/ronoctua-shell leader

    @CHORDS@

    # THE FOUR BINDS WITHOUT THE LEADER ARE IN THE CHORDS ABOVE, and not here.
    #
    # And this comment does NOT spell the marker, which is not fussiness: the
    # splice is a `sed` that inserts at EVERY matching line and then deletes
    # them all, so a comment naming it would splice the whole bind file a
    # second time. The compositor answers a duplicate `bindsym` by overwriting
    # and logging one line, so the symptom would be a config twice the size
    # with nothing on screen to say why.
    #
    # They used to be written out, because `export-binds` spoke another
    # compositor's notation and a second renderer for four lines was not worth
    # it. It speaks this one now, so `nix/compositor-binds.nix` emits them and
    # its own guard is what checks all four came out — the same guard the
    # production config gets. Written out here as well they would be four
    # `bindsym` lines for keys that already have one, which this compositor
    # resolves by OVERWRITING with nothing but a log line to say so.

    # A TERMINAL, through the registry and not through a `$term` of its own.
    #
    # `buffer-terminal` is the id `LEADER w o` runs, so this bind proves the
    # control socket end to end under Scroll — and it costs no second opinion
    # about which terminal this project uses.
    bindsym $mod+Return exec ${ronoctua-shell}/bin/ronoctua-shell command buffer-terminal

    # ------------------------------------------------------------------
    # Scroll's own layout — the reason this loop exists
    # ------------------------------------------------------------------
    # Taken from upstream's `config.in` with `$mod` changed, and trimmed to what
    # a person needs to judge the layout. The full table is in Scroll's README.
    bindsym $mod+Backspace kill
    bindsym $mod+Shift+q kill

    # THE SCROLLER. `set_mode h`/`v` is the direction new windows are added in,
    # which is the whole difference from sway.
    bindsym $mod+bracketleft set_mode h
    bindsym $mod+bracketright set_mode v

    bindsym $mod+Left focus left
    bindsym $mod+Down focus down
    bindsym $mod+Up focus up
    bindsym $mod+Right focus right
    bindsym $mod+Home focus beginning
    bindsym $mod+End focus end

    bindsym $mod+Ctrl+Left move left
    bindsym $mod+Ctrl+Down move down
    bindsym $mod+Ctrl+Up move up
    bindsym $mod+Ctrl+Right move right

    # `nomode` — MOVE THE WINDOW, NEVER ITS COLUMN.
    #
    # The four above move whatever the current mode says is the unit: in `h`
    # that is the whole COLUMN, so a window sharing a column with others cannot
    # be taken out of it by them. With `nomode`, left/right EXPEL the window
    # into a column of its own, and pressing again INSERTS it into the column
    # on that side — which is how one window is put underneath another.
    #
    # Upstream binds these to `$mod+Alt+<arrow>`; here `$mod` IS Alt, so that
    # chord cannot be typed. `Shift` takes its place — upstream spends it on
    # `focus output`, which is dead in a nested session with one output.
    bindsym $mod+Shift+Left move left nomode
    bindsym $mod+Shift+Down move down nomode
    bindsym $mod+Shift+Up move up nomode
    bindsym $mod+Shift+Right move right nomode

    # The column's share of the screen, cycled through `layout_widths`.
    bindsym $mod+minus cycle_size h prev
    bindsym $mod+equal cycle_size h next
    bindsym $mod+Shift+minus cycle_size v prev
    bindsym $mod+Shift+equal cycle_size v next

    bindsym $mod+f fullscreen
    bindsym $mod+Shift+f floating toggle
    bindsym $mod+y focus mode_toggle
    bindsym $mod+Shift+y layout_transpose

    # THE OVERVIEW, zoomed out. `--no-repeat` is upstream's.
    bindsym --no-repeat $mod+Tab scale_workspace overview
    bindsym $mod+comma scale_content incr -0.05
    bindsym $mod+period scale_content incr 0.05
    bindsym $mod+Ctrl+period scale_content reset

    # Pinning a column to an edge — Scroll's answer to "keep this one in view".
    bindsym --no-repeat $mod+a pin beginning
    bindsym --no-repeat $mod+Shift+a pin end

    # ------------------------------------------------------------------
    # What Scroll has INSTEAD of tabs and window groups
    # ------------------------------------------------------------------
    # There is no `layout tabbed` and no `layout stacking` here: the fork
    # supports ONE layout, and that is the whole point of it. What it offers in
    # their place is four separate things, and they are bound below because a
    # feature nobody can reach is a feature nobody judges.

    # A COLUMN IS THE GROUP. `set_size v 1.0` gives the focused window the whole
    # height of its column, which pushes its siblings out of view — one visible
    # at a time, `$mod+Up`/`$mod+Down` between them. That is a tab strip without
    # the strip. `fit_size v` shares the column out again.
    bindsym $mod+Ctrl+Return set_size v 1.0
    # `fit_size` TAKES TWO ARGUMENTS, and it is silent without them: written as
    # `fit_size v` the command parses, answers `"success": true`, and changes
    # nothing — measured, three windows kept their 603 px. `all equal` is what
    # shares the column out between every window in it.
    bindsym $mod+Ctrl+Backspace fit_size v all equal

    # THE FOCUS RING — the fork's Alt+Tab, in creation order.
    bindsym $mod+Shift+bracketleft focus_ring prev
    bindsym $mod+Shift+bracketright focus_ring next

    # TRAILS AND TRAILMARKS — a named-less group of windows that ignores which
    # workspace each one is on, which is the thing tabs are usually used FOR.
    # `;` enters the trailmark sub-map, `Shift+;` the trail one, `Esc` leaves.
    mode "trailmark" {
        bindsym bracketright trailmark next
        bindsym bracketleft trailmark prev
        bindsym semicolon trailmark toggle; mode default
        bindsym j trailmark jump; mode default
        bindsym Escape mode "default"
    }
    bindsym $mod+semicolon mode "trailmark"

    mode "trail" {
        bindsym bracketright trail next
        bindsym bracketleft trail prev
        bindsym semicolon trail new; mode default
        bindsym d trail delete; mode default
        bindsym c trail clear; mode default
        bindsym Escape mode "default"
    }
    bindsym $mod+Shift+semicolon mode "trail"

    # SELECTION — mark several windows and move them together, keeping their
    # order and sizes.
    bindsym --no-repeat $mod+Insert selection toggle
    bindsym --no-repeat $mod+Ctrl+Insert selection reset
    bindsym --no-repeat $mod+Shift+Insert selection move

    # JUMP — reach any window by typing its label, easymotion-style.
    mode "jump" {
        bindsym slash jump tiling; mode default
        bindsym w jump workspaces; mode default
        bindsym a jump all; mode default
        bindsym Escape mode "default"
    }
    bindsym $mod+slash mode "jump"

    bindsym $mod+1 workspace number 1
    bindsym $mod+2 workspace number 2
    bindsym $mod+3 workspace number 3
    bindsym $mod+4 workspace number 4
    bindsym $mod+5 workspace number 5

    bindsym $mod+Shift+c reload
    bindsym $mod+Shift+e exit

    layout_default_width 0.5
    layout_default_height 1.0
    layout_widths [0.33333333 0.5 0.666666667 1.0]
    layout_heights [0.33333333 0.5 0.666666667 1.0]

    # ANIMATIONS OFF. Invariant 15 is about shadow and elevation, not motion, so
    # this is not a rule — it is a measurement decision: a loop where one
    # photographs the screen cannot have the screen still moving when the
    # shutter opens. Turn them on by hand to judge them.
    animations {
        enabled no
    }
  '';

  nestedConfig = runCommand "ronoctua-nested.conf" { } ''
    cp ${writeText "nested-static.conf" nestedConfigText} $out
    chmod +w $out
    substituteInPlace $out \
      --subst-var-by BORDER_ACTIVE "$(cat ${palette}/compositor-border-active)" \
      --subst-var-by BORDER_INACTIVE "$(cat ${palette}/compositor-border-inactive)" \
      --subst-var-by BACKGROUND "$(cat ${palette}/compositor-background)"
    # A COLOUR MARKER THAT SURVIVED is a config the compositor refuses to
    # parse, and the symptom would be one that does not come up at all.
    #
    # BY NAME, and not `@[A-Z_]*@`: `@CHORDS@` is still in the file at this
    # point — it is spliced further down — so a catch-all here fires on a
    # marker that is doing its job. The catch-all is at the END, where every
    # marker really is supposed to be gone.
    for marker in BORDER_ACTIVE BORDER_INACTIVE BACKGROUND; do
      if grep -q "@$marker@" $out; then
        echo "ERROR: the colour marker @$marker@ survived in the nested config." >&2
        exit 1
      fi
    done
    # ANCHORED AT THE LINE'S START, the same guard `.#dev` uses: `bg #` is a
    # substring of other lines, and a substitution that came out empty would
    # have passed.
    for line in 'client.focused #' 'client.unfocused #' 'output \* bg #'; do
      if ! grep -qE "^[[:space:]]*$line" $out; then
        echo "ERROR: $line did not get into the nested config." >&2
        exit 1
      fi
    done
    # THE CHORDS, spliced at the marker. The `grep` at the end is the safety
    # net against a `sed` that does not match: without it, a renamed marker
    # would give a config WITHOUT chords and the symptom would be "the keys
    # stopped working", with no error anywhere.
    ${gnused}/bin/sed -i -e "/@CHORDS@/r ${binds.forPrefix "Alt"}" -e "/@CHORDS@/d" $out
    grep -q "focus left" $out || {
      echo "ERROR: the chords did not get into the nested config." >&2; exit 1; }
    # AND NOW NOTHING MAY BE LEFT. Every marker this file uses has been
    # substituted or spliced by here, so one that survives is a name that
    # moved on one side and not the other — and it reaches the compositor as a
    # word it does not know.
    if grep -q "@[A-Z_][A-Z_]*@" $out; then
      echo "ERROR: a marker survived in the nested config:" >&2
      grep -n "@[A-Z_][A-Z_]*@" $out >&2
      exit 1
    fi
    cat ${windowRules} >> $out
    grep -q "floating enable" $out || {
      echo "ERROR: the window rules did not get into the nested config." >&2; exit 1; }

    # AND THE KEYS THAT MATTER MUST BE IN IT.
    #
    # The same guard `nix/compositor-binds.nix` puts around its own output, and for
    # the same reason: a leader that did not make it into the file is a session
    # where nothing responds, with no error anywhere to say so. `grep -F`,
    # never `-E`: the `+` in `Ctrl+Alt+l` is "one or more" in ERE, so the
    # pattern would match text that is not there and the guard would always
    # pass — this repository has been bitten by exactly that once.
    for bind in 'Ctrl+Alt+space' 'Ctrl+Alt+l' 'Ctrl+Shift+l' 'Ctrl+Alt+Escape' 'Ctrl+Alt+Delete'; do
      if ! grep -qF "bindsym" $out || ! grep -qF "$bind" $out; then
        echo "ERROR: the bind $bind did not get into the nested config." >&2
        exit 1
      fi
    done

    # AND THERE IS NO `--validate` HERE. It was written, it failed, and what
    # it failed at is worth keeping.
    #
    # `--validate` brings up the SERVER first — libseat, a VT, a backend, a renderer — and
    # only then looks at the config. In the sandbox that is, in order:
    #
    #   [libseat] Could not get primary session for user: No data available
    #   [backend/backend.c:105] Timeout waiting session to become active
    #   [sway/server.c:274] Unable to create backend
    #
    # ten seconds of it, then a failed build. `WLR_BACKENDS=headless` gets past
    # that one and stops at `Unable to open wayland socket`, with an assertion
    # and a core dump.
    #
    # It is not worth working around, because the flag does not answer the
    # question anyway: run OUTSIDE the sandbox but without a graphics driver,
    # this config and a deliberately BROKEN copy of it produce byte-identical
    # output — `Failed to create renderer`, and never a word about the config.
    # A gate that says the same thing whatever it is shown is not a gate.
    #
    # What stays is what can be answered by reading the file: the greps
    # above. A real syntax gate for this compositor needs a display, so it
    # belongs in a VM test (`tests/`) and not in a derivation.
  '';

  app = writeShellApplication {
    name = "ronoctua-dev";

    runtimeInputs = [
      scroll
      # `swaybg` PAINTS THE DESK, and the compositor does not contain it.
      #
      # `output * bg` is not drawn by the compositor: it spawns a helper, BY
      # NAME. Measured here, from its own log — `failed to execute 'swaybg'
      # (background configuration probably not applied)` — and the reason it
      # took a log to find is that the default theme's desk is black and an
      # unpainted desk is black too.
      swaybg
      # `cargo` for hot reload. It comes from the SAME toolchain `nix/rust.nix`
      # uses: a different `cargo` would rebuild the world on every switch
      # between `nix run .#dev` and `nix flake check`.
      rustToolchain
      # `nix` for the OTHER half of hot reload: a buffer is a component, and
      # what builds a component is the flake (see `rebuild`, further down).
      nix
      # `wayland-info` READS THE HOST, and it reads it by the PROTOCOL: two
      # globals, `wl_output` and `zxdg_output_manager_v1`, and the ratio
      # between what they say is the host's scale. See `the host's own pixels`.
      # It is a dev-loop dependency and reaches no installed profile.
      wayland-utils
      # `xdpyinfo` asks the OTHER half of the same question: whether an X
      # window on this host gets the panel's real pixels or the logical ones.
      # Same section, and the same reason it is a measurement rather than a
      # compositor's name.
      xdpyinfo
    ];

    text = ''
      echo "ronoctua-dev — the nested compositor + footer"
      echo
      ${keepSources}

      ${graphicsHost.prelude}

      # AND ONE VARIABLE THE SHARED PRELUDE DOES NOT SET, because until the
      # compositor changed nothing needed it.
      #
      # wlroots allocates its buffers through GBM, and mesa's GBM loader does
      # NOT read `LIBGL_DRIVERS_PATH`: it has its own, and the copy Nix builds
      # has `/run/opengl-driver/lib/gbm` compiled into it — a directory that
      # exists on NixOS and on no other distribution. Measured on Arch:
      #
      #   MESA-LOADER: failed to open dri: /run/opengl-driver/lib/gbm/dri_gbm.so
      #   [render/allocator/gbm.c:183] gbm_create_device failed
      #   [sway/server.c:310] Failed to create allocator
      #
      # and the compositor never came up. It goes here rather than into
      # `nix/graphics-host.nix` because `.#bench` shares that file and the VM
      # tests do not touch this path at all — they use a software renderer
      # that never opens a DRM node.
      #
      # DERIVED from what the prelude already decided, never spelled out
      # again: two copies of a store path are two things to keep in step.
      # Empty when the prelude took the NixOS or the NVIDIA branch, and then
      # there is nothing to set.
      if [ -n "''${LIBGL_DRIVERS_PATH:-}" ]; then
        export GBM_BACKENDS_PATH="''${LIBGL_DRIVERS_PATH%/dri}/gbm"
      fi

      # THE CONFIG ARRIVES READY AND VALIDATED — see this file's `let`.
      cfg=${nestedConfig}
      # THE PERSON'S THEME AND DESK, included as production includes them.
      # Appended to a COPY at run time, because only here is the home known;
      # the store file stays shared. A `mktemp` file like the log, and it
      # stays with it.
      cfg_themed=$(mktemp -t ronoctua-nested-XXXXXX.conf)
      cp "$cfg" "$cfg_themed"
      # THE INCLUDES ARE WRITTEN AS THE PRODUCTION ONES ARE — with the
      # variable, not with a resolved path. `include` expands shell syntax
      # (`wordexp(3)`), so `''${XDG_CONFIG_HOME:-$HOME/.config}` is the same
      # rule `ronoctua_store::config_dir()` follows, and the two ends of the
      # path come from ONE source. A missing file is not fatal.
      {
        echo "include \"''${XDG_CONFIG_HOME:-$HOME/.config}/ronoctua/scroll-theme.conf\""
        # AND THE SCREENS, so the display view's write-and-reload path is the
        # one exercised in here rather than only in production. The block
        # above sets the nested output's mode, and a block for the person's
        # own monitor simply does not match it.
        echo "include \"''${XDG_CONFIG_HOME:-$HOME/.config}/ronoctua/scroll-display.conf\""
        # AND THE POINTERS: which seat every pointing device is on, and the
        # cursor each seat draws.
        echo "include \"''${XDG_CONFIG_HOME:-$HOME/.config}/ronoctua/scroll-pointers.conf\""
      } >> "$cfg_themed"

      # ── THE HOST'S OWN PIXELS ────────────────────────────────────────────
      #
      # The nested output is given the size the host's window has in LOGICAL
      # pixels and a scale that turns those back into the host's PHYSICAL
      # ones. Without it the session lays out for the logical box and the host
      # blows it up: on this machine's 1920x1200 at 1.6 that is an output of
      # 600x750 stretched over 960x1200 real pixels — 60% too big, with five
      # pixels in eight unused.
      #
      # TWO STANDARD PROTOCOLS, and no compositor's own CLI: `wl_output` says
      # the mode in physical pixels, `zxdg_output_v1` says the logical size,
      # and the ratio between them IS the host's scale. Asking `hyprctl` or
      # its cousins would make the loop know which compositor it is running
      # under, which is the thing invariant 3 exists to prevent.
      #
      # NOT `wl_output.scale`, which is an INTEGER and is wrong here: this
      # host advertises 2 while it is running at 1.6, so the reciprocal would
      # have been 0.5 and the session would have come out half the size it
      # should be. Measured, on the owner's panel and on a headless copy of it.
      #
      # AND EVERY SCALE HERE IS `force`d, which is not bravado — it is the
      # only way to get the one that was asked for.
      #
      # `verify_good_scale` refuses any scale that does not divide the mode
      # into whole logical pixels on BOTH axes, and walks outward in 120ths
      # until it finds one that does. A TILING HOST hands out whatever size
      # its layout produces — measured here, 1882x1075, whose two sides share
      # no divisor — and for a mode like that the only scales it will accept
      # are `1/k`. Asking for 1.6 on it came back **1.0**, silently, because
      # `scrollnag_command -` refuses the window that would have said so.
      #
      # `force` skips that check (`oc->scale_force`, `sway/config/output.c`).
      # The logical size then comes out fractional — 1176.25 becomes 1176 —
      # which is what fractional scaling does on every real monitor anyway.
      #
      # A TILING HOST OVERRIDES THE MODE TOO, and that is fine: the mode is
      # what a host that lets the client choose gets, and there it is a window
      # that fits the screen instead of one a third wider than it. The scale
      # applies to whatever size the host actually gave.
      #
      # ── AND THE X11 BACKEND IS THE SHARP ONE, WHEN THE HOST ALLOWS IT ────
      #
      # All of the above makes the session the right SIZE and none of it makes
      # it sharp: the Wayland backend's framebuffer has the window's LOGICAL
      # pixel count, so on a host at 1.6 the session owns 1/1.6 of the panel's
      # real pixels and the host resamples the rest. Measured on this machine:
      # a tile that covers 960x1200 real pixels got a 600x750 framebuffer.
      #
      # An X window does not go through that. With `xwayland:force_zero_scaling`
      # — which this host has on — Xwayland hands its clients the PHYSICAL
      # pixels, so the same tile measured 1882x1075 on the X11 backend, at
      # scale 1, composited one to one. Three times the resolution, and no
      # resampling anywhere.
      #
      # WHETHER THE HOST DOES THAT IS ASKED, NOT ASSUMED, and asked without
      # naming a compositor: if the X root is as wide as the host's PHYSICAL
      # mode then Xwayland is handing out real pixels, and if it is as wide as
      # the LOGICAL desktop then it is not and there is nothing to gain.
      #
      # THE SCALE THEN GOES THE OTHER WAY. On the Wayland backend the
      # framebuffer is logical and the scale is `logical/physical` to win the
      # size back. On the X11 backend the framebuffer is already physical, so
      # the scale is the host's own — `physical/logical` — which is what makes
      # a logical unit in here the size it is out there.
      #
      # BY THE LABEL AND NOT BY A FIELD NUMBER. `wayland-info` indents with
      # TABS and puts a comma after the value (`logical_width: 1200,
      # logical_height: 750`), so splitting on `[ ,]+` counts the indentation
      # as a field and `$3` comes back the string `logical_height:`. Scanning
      # for the label and taking the digits after it survives the indentation,
      # the comma, and a field moving.
      num_after() {
        awk -v want="$1:" \
          '{ for (i = 1; i <= NF; i++)
               if ($i == want) { gsub(/[^0-9]/, "", $(i + 1)); print $(i + 1); exit } }'
      }
      host_px=""
      host_logical=""
      host_logical_h=""
      if [ -n "''${WAYLAND_DISPLAY:-}" ]; then
        host_px=$(wayland-info -i wl_output 2>/dev/null \
          | awk '/width: [0-9]+ px/ { print $2; exit }') || true
        host_xdg=$(wayland-info -i xdg_output_manager 2>/dev/null) || true
        host_logical=$(printf '%s\n' "$host_xdg" | num_after logical_width) || true
        host_logical_h=$(printf '%s\n' "$host_xdg" | num_after logical_height) || true
      fi
      # NUMBERS, AND A RATIO THAT IS NOT ONE. An X11 host, a compositor with
      # no `zxdg_output_manager_v1`, a `wayland-info` that printed something
      # else: every one of those leaves the output alone, which is the right
      # answer on a host at scale 1 and the old behaviour everywhere else.
      # THE X ROOT'S WIDTH, when there is an X server to ask at all.
      x_root=""
      if [ -n "''${DISPLAY:-}" ]; then
        x_root=$(xdpyinfo 2>/dev/null \
          | awk '/^[[:space:]]*dimensions:/ { split($2, d, "x"); print d[1]; exit }') || true
      fi
      if [ -n "$host_px" ] && [ -n "$host_logical" ] && [ -n "$host_logical_h" ] \
        && [ "$host_px" -gt 0 ] && [ "$host_logical" -gt 0 ] \
        && [ "$host_logical_h" -gt 0 ] \
        && [ "$host_px" -ne "$host_logical" ]; then
        if [ -n "$x_root" ] && [ "$x_root" -eq "$host_px" ]; then
          # REAL PIXELS, AND NO SCALE AT ALL — which is what production does.
          #
          # The window is an X window and this host gives those the panel's
          # own resolution, so the framebuffer already holds physical pixels.
          # What to do with them is then exactly the question production
          # answers, and `nix/modules/scroll.nix` answers it by saying
          # NOTHING: no `output * scale` anywhere, so the compositor's own
          # `compute_default_scale` decides. On a panel under 192 DPI — this
          # one is 143 — that is 1.
          #
          # THE FIRST VERSION OF THIS SET THE HOST'S SCALE, 1.6, so that a
          # logical unit in here would look the size it looks out there. That
          # is the wrong target: what the loop exists to show is the INSTALLED
          # system, and the installed system runs at 1. Everything came out
          # 60% larger than the thing it was standing in for — read off the
          # screen as "tudo está meio grande".
          #
          # Saying nothing also means saying nothing to `verify_good_scale`,
          # so the `force` that a fractional scale needed goes with it: at 1
          # the buffer our own doors commit matches the framebuffer, and
          # nothing between here and the panel resamples anything.
          export WLR_BACKENDS=x11
          echo "output * mode ''${host_px}x$(awk -v h="$host_logical_h" -v l="$host_logical" \
            -v p="$host_px" 'BEGIN { printf "%d", h * p / l }')" >> "$cfg_themed"
          echo "host: ''${host_px}px over ''${host_logical} logical, and X gets the real" \
               "pixels — the nested session runs on the X11 backend at production's own scale"
        else
          # LOGICAL PIXELS, and the host resamples them. The scale wins the
          # SIZE back; the sharpness is the backend's and is not ours.
          nested_scale=$(awk -v l="$host_logical" -v p="$host_px" \
            'BEGIN { printf "%.10g", l / p }')
          {
            echo "output * mode ''${host_logical}x''${host_logical_h}"
            echo "output * scale $nested_scale force"
          } >> "$cfg_themed"
          echo "host: ''${host_px}px over ''${host_logical} logical —" \
               "the nested output takes scale $nested_scale (the host resamples it)"
        fi
      else
        echo "host: no fractional scale to answer for; the nested output is left alone"
      fi
      # ─────────────────────────────────────────────────────────────────────

      cfg=$cfg_themed

      # The renderer does NOT use fontconfig (section 6.4), but the foreign apps that
      # run in here do — and they must see the SAME chain as the system.
      # THE COMPLETE FILE, and not the fragment.
      #
      # `FONTCONFIG_FILE` replaces the ENTIRE configuration. Pointing it at the
      # fragment — only the alias rules, with no `<dir>` at all — left fontconfig
      # with no font directory, and everything that goes through it turned into
      # TOFU: the compositor's error message came out in little boxes, precisely the one that
      # says what is wrong.
      export FONTCONFIG_FILE="${fontconfigConf.fullConf}"
      export XDG_DATA_DIRS="${fonts.noctua-iosevka-ttf-nerdfonts}/share:${iconSuperpack}/share:''${XDG_DATA_DIRS:-/usr/share}"
      export XDG_ICON_THEME=noctua

      # THE LEADER'S LABEL comes from the SAME place as the bind, just above: in the
      # nested loop the key is `Ctrl+Alt+Space`, because `Mod` belongs to the host's
      # compositor. The screen said `SPC` in both modes, and `SPC` is nobody's key
      # here.
      export RONOCTUA_LEADER="CTRL ALT SPC"
      # AND THE CHORD'S LABEL, which in the nested loop is NOT the same.
      #
      # The sequence is `Ctrl+Alt+Space`; the chord is `Alt` alone, held — it is the
      # nested compositor's `Mod`, and changing the sequence did not change it. While the
      # screen called both of them "the leader", holding "the leader" looked like it
      # included Space — and including Space fires the sequence. In production both
      # are `SUPER` and the distinction disappears; here it has to show.
      export RONOCTUA_LEADER_CHORD="ALT"

      # THE NESTED LOOP DOES NOT TAKE THE PAIRING QUESTIONS.
      #
      # `RequestDefaultAgent` gives them to the LAST caller, and this footer is
      # the second one on this machine: without this it took the questions away
      # from the owner's real session, and closing it gave them back to nobody.
      # A device asking to pair while the loop was up went unanswered.
      export RONOCTUA_NESTED=1

      # Useful logging by default; `RUST_LOG` from the environment wins.
      #
      # It is NOT exported: the compositor does not read it, and ours would
      # names our targets leaves it mute. It passes it to the footer when starting it.
      rustlog="''${RUST_LOG:-ronoctua_shell=debug,ronoctua_render=info}"
      # And the compositor gets its own, explicitly, so the log stays useful.
      # The compositor logs on its own terms; ours is passed to the footer.

      echo "  Ctrl+Alt+Space      leader (tapped: opens the list)"
      echo "  Alt+<key>           a chord (HELD: the Alt alone, without the Space)"
      echo "  Ctrl+Alt+L          an alternative leader, if the one above fails"
      echo "  Ctrl+Shift+L        restores the keyboard"
      echo "  Ctrl+Alt+Esc        process monitor (floating)"
      echo "  Ctrl+Alt+Space q q  quit"
      echo
      echo "config: $cfg"
      echo

      # ------------------------------------------------------------------
      # What earlier sessions left behind
      # ------------------------------------------------------------------
      # Every session mints a log and a pidfile with `mktemp` and, by the
      # decision below, leaves the log where it is — so `/tmp` only ever grew.
      # Counted on the owner's machine before this existed: 65 files and 55 MB,
      # the oldest nine days old, plus five pidfiles naming processes that had
      # been gone for a week.
      #
      # ON THE WAY IN, never in the `trap`. A session that is killed hard cleans
      # nothing, so a cleanup that only runs on the way out is a cleanup that
      # misses exactly the sessions worth cleaning up after.
      #
      # TWO conditions, and both have to be true, because another `.#dev` may be
      # running right now: the file is not one of the ten most recent, AND
      # nothing has written to it for a day. A footer that is up but idle can go
      # a long time without logging, which is why the count alone is not enough.
      prune_old() {
        local keep=10
        # THE `|| true` IS INSIDE THE BRACES, and it is load-bearing: `ls` with
        # nothing to list exits non-zero, `pipefail` makes that the pipeline's
        # status, and `errexit` then takes the whole session down — before the
        # `trap` below is even installed, so it goes down without cleaning up
        # and without saying why. `dev-exits-clean` caught exactly that.
        # shellcheck disable=SC2012 # `ls -t` is the ordering; these are our own names
        { ls -t /tmp/ronoctua-footer-*.log /tmp/ronoctua-compositor-*.log 2>/dev/null || true; } \
          | tail -n +$(( keep + 1 )) \
          | while read -r old; do
              # AN `if` AND NOT AN `&&`, and the difference is the whole
              # session: a `test && rm` is the LAST command of this body, so
              # when the test is false the loop's status is 1, `pipefail` makes
              # it the pipeline's, and `errexit` ends `.#dev` right here —
              # after the banner and before the compositor, with nothing said.
              # The owner met exactly that. An `if` with no `else` is 0.
              if [ -n "$(find "$old" -mtime +0 2>/dev/null)" ]; then
                rm -f "$old"
              fi
            done
        # A PIDFILE IS ANSWERABLE, so it takes no heuristic at all: the process
        # it names is alive or it is not.
        for stale in /tmp/ronoctua-footer-*.pid; do
          [ -f "$stale" ] || continue
          held=$(cat "$stale" 2>/dev/null || true)
          case "$held" in
            *[!0-9]* | "") rm -f "$stale" "$stale.onpurpose" "$stale.watcher" ;;
            *) kill -0 "$held" 2>/dev/null || rm -f "$stale" "$stale.onpurpose" "$stale.watcher" ;;
          esac
        done
        # AND THE BUFFER SOCKETS, which carry their owner's PID in the name —
        # `ronoctua-buffer-<pid>.sock`. Seven of them were left over from days
        # gone by. The footer's own socket is NOT swept here: it is named after
        # the display, not a process, and a running session of another display
        # owns one.
        for sock in "''${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"/ronoctua-buffer-*.sock; do
          [ -S "$sock" ] || continue
          owner=''${sock##*/ronoctua-buffer-}
          owner=''${owner%.sock}
          case "$owner" in
            *[!0-9]* | "") ;;
            *) kill -0 "$owner" 2>/dev/null || rm -f "$sock" ;;
          esac
        done
      }
      prune_old

      # ------------------------------------------------------------------
      # Compositor
      # ------------------------------------------------------------------
      # THE SOCKET'S PATH IS CHOSEN HERE, and that is worth a line: the
      # compositor honours `SCROLLSOCK` when the file does not exist yet, so
      # naming it up front means neither this script nor the watcher has to
      # find it by globbing — and the watcher can ask about the compositor's
      # PID rather than about a file a dead server leaves behind.
      export SCROLLSOCK
      SCROLLSOCK=$(mktemp -u -t ronoctua-compositor-XXXXXX.sock)
      # AND THE NAME THE ECOSYSTEM LOOKS FOR, carrying the same path. The
      # compositor sets it for its own children; this is for anything the
      # SCRIPT starts beside it.
      export SWAYSOCK="$SCROLLSOCK"

      # The display's name is not knowable from outside — the socket is picked
      # automatically — so the compositor is asked to say it, by an `exec` in
      # its own config that writes the name to this file.
      display_file=$(mktemp -u -t ronoctua-display-XXXXXX)
      export RONOCTUA_SCROLL_DISPLAY_FILE="$display_file"

      # In the background, and not `exec`: the script stays up to supervise
      # the footer and the watcher.
      compositor_log=$(mktemp -t ronoctua-compositor-XXXXXX.log)
      # THE CURSOR THEMES, where the compositor looks for the one each seat is
      # given (`nix/cursors.nix`) — the same path the installed unit sets.
      export XCURSOR_PATH="${cursors}/share/icons''${XCURSOR_PATH:+:$XCURSOR_PATH}"
      scroll --config "$cfg" > "$compositor_log" 2>&1 &
      compositor_pid=$!
      # AND THE WATCHER IS TOLD WHICH PROCESS THAT IS.
      #
      # It has to answer "is the compositor this footer was born under still
      # there?" without starting a process, and it cannot derive the answer:
      # `SCROLLSOCK` is a `mktemp -u` name here, so the pid is not in it, and
      # a `[ -S … ]` test on the socket would pass with the whole defect in
      # place — a Wayland server killed by a signal does not unlink its own
      # socket. Measured: `/run/user/1000/wayland-2` was still on disk an hour
      # after the compositor that made it was gone.
      #
      # The pid is a fact this script already holds. Handing it over is exact
      # and costs nothing; see `nix/footer-single.nix`, question two.
      export RONOCTUA_COMPOSITOR_PID="$compositor_pid"
      tail -f -n +1 "$compositor_log" & tailpid=$!

      # THE FOOTER'S PID LIVES IN A FILE, and not in a variable.
      #
      # Whoever starts it and whoever restarts it are different processes: this
      # script starts, hot reload's watcher restarts. A shell variable does not
      # cross a process boundary — and when it failed to, hot reload stopped
      # working in silence. See `nix/footer-single.nix`.
      export RONOCTUA_FOOTER_PID
      RONOCTUA_FOOTER_PID=$(mktemp -t ronoctua-footer-XXXXXX.pid)

      # THE FOOTER'S LOG GOES TO A FILE, and not only to the screen.
      #
      # Four times the shortcuts froze, and four times there was no evidence at
      # all: the log was in a terminal that had already been closed, and the compositor's
      # had the four lines from startup. `tail -f` still puts it on the
      # screen; the file is what survives, and it is what gets sent along with
      # "it happened again". See `nix/footer-single.nix`.
      export RONOCTUA_LOG
      RONOCTUA_LOG=$(mktemp -t ronoctua-footer-XXXXXX.log)
      tail -f -n +1 "$RONOCTUA_LOG" & taillogpid=$!

      watcher_pid=""
      shutdown() {
        [ -n "$watcher_pid" ] && kill "$watcher_pid" 2>/dev/null || true

        # THE FOOTER'S WATCHER GOES FIRST, and until now this session could not
        # reach it at all.
        #
        # `footer-single.nix` keeps a subshell alive to notice a footer that
        # dies on its own and raise the rescue line. It is asynchronous, so bash
        # gives it SIG_IGN for SIGINT; it outlives the script that spawned it,
        # so it is reparented to init. Ctrl+C therefore never reached it and
        # nothing here knew its PID — and it read the footer this function kills
        # as a crash, started another, and that one found no compositor and died
        # too. Measured on the owner's machine: one turn every 2.01 s, for an
        # hour, with `Error: raise the lifeline` on his terminal the whole time.
        #
        # It now writes its PID beside the footer's, so stopping it is a kill
        # and not a hope. The `.onpurpose` below is the second of three guards,
        # and it covers the case where this file is stale.
        if [ -s "$RONOCTUA_FOOTER_PID.watcher" ]; then
          kill "$(cat "$RONOCTUA_FOOTER_PID.watcher")" 2>/dev/null || true
        fi

        if [ -s "$RONOCTUA_FOOTER_PID" ]; then
          # SAID OUT LOUD BEFORE THE KILL. The watcher tells a death that was
          # asked for from a crash by this file and by nothing else, and for a
          # year ending the session did not write it — so every Ctrl+C read as a
          # crash. See `nix/footer-single.nix`.
          footer=$(cat "$RONOCTUA_FOOTER_PID")
          echo "$footer" > "$RONOCTUA_FOOTER_PID.onpurpose"
          kill "$footer" 2>/dev/null || true
          # AND WAITED FOR, because the watcher reads `.onpurpose` only AFTER
          # the footer is gone: removing it before that races the guard it is
          # there to arm. Five seconds, the same patience `footer-single` has.
          for _ in $(seq 50); do
            kill -0 "$footer" 2>/dev/null || break
            sleep 0.1
          done
        fi
        kill "$compositor_pid" 2>/dev/null || true
        kill "$tailpid" 2>/dev/null || true
        kill "$taillogpid" 2>/dev/null || true

        # AND WHAT THE FOOTER RAISED NEXT DOOR — but only what THIS build raised.
        #
        # `ronoctua-transfers` and `ronoctua-brain` are started by the FOOTER,
        # not by this script (see `rebuild`, which builds them for exactly that
        # reason), and they are given a session of their own — measured, `PGID
        # == PID`. So neither the pidfile nor any group sweep reaches them, and
        # they were still running forty-seven minutes after a session ended,
        # serving this session's socket to whatever came next.
        #
        # What does reach them is what they ARE: `/proc/<pid>/exe` pointing
        # inside this session's build. A daemon of a real installation points
        # into the system's store path and is therefore never touched. And the
        # list comes from `ps` with the kill by PID, never `pkill -f`, which
        # matches its own command line — this repository has been bitten by that
        # one three times in a day.
        for pid in $(ps -eo pid= 2>/dev/null); do
          exe=$(readlink "/proc/$pid/exe" 2>/dev/null) || continue
          # THE LOOP PROFILE IS THE MARK, and it is a sound one: `[profile.loop]`
          # is never installed on a real system, which `the-loop-profile-is-never-installed`
          # is there to keep true. A daemon built for an installation comes out
          # of `ronoctua-0.1.0` and does not match. The default for
          # `CARGO_TARGET_DIR` is a path that cannot exist, because this
          # function runs under `set -u` and `rebuild` — which exports it — may
          # never have been called.
          case "$exe" in
            "''${CARGO_TARGET_DIR:-/nonexistent}"/loop/*) kill "$pid" 2>/dev/null || true ;;
            /nix/store/*-ronoctua-loop-*) kill "$pid" 2>/dev/null || true ;;
            /nix/store/*-ronoctua-shell-loop/*) kill "$pid" 2>/dev/null || true ;;
          esac
        done

        # ONLY THE PIDFILES. `$cfg` was a STORE path (now a `mktemp` copy of
        # one, left like the log) — the store file is immutable, shared by
        # every run and collected by Nix's garbage collector. It ended up on
        # this list back when the config was written to a temporary file; once it
        # came to be generated by Nix, the removal was left behind and became two
        # `Permission denied` messages in the face of whoever closes the session
        # with Ctrl+C.
        #
        # `-f` does not save it: it silences "does not exist", not "is not yours".
        rm -f "$RONOCTUA_FOOTER_PID" "$RONOCTUA_FOOTER_PID.onpurpose" \
              "$RONOCTUA_FOOTER_PID.watcher"
        # THE LOGS STAY. Deleting them in the `trap` was deleting the evidence
        # exactly when it starts to matter — whoever ends the session after
        # something went wrong is the one who needs them most.
        echo "logs of this session:"
        echo "  compositor: $compositor_log"
        echo "  footer: $RONOCTUA_LOG"
      }
      # ONCE ONLY. With `trap shutdown EXIT INT TERM`, a Ctrl+C runs
      # `shutdown` through INT and the shell exits right after, running it again
      # through EXIT — that is why every shutdown message came out twice.
      #
      # EXIT is the only one that cleans up; INT and TERM only ask for the exit,
      # with the code convention demands (128 + signal).
      trap shutdown EXIT
      trap 'exit 130' INT
      trap 'exit 143' TERM

      # THE COMPOSITOR SAYS ITS OWN DISPLAY, and nothing else can.
      #
      # The Wayland socket's name is chosen automatically, so it is neither
      # knowable from outside nor passable in — and reading the log does not
      # work either, because a compositor that came up perfectly says nothing
      # about it. The `exec` in the config writes the name to the file named
      # above, which makes the compositor itself the only source.
      display=""
      for _ in $(seq 60); do
        if [ -s "$display_file" ]; then
          display=$(cat "$display_file")
          break
        fi
        # If the process died, waiting the full 30 s only delays the error.
        kill -0 "$compositor_pid" 2>/dev/null || break
        sleep 0.5
      done
      if [ -z "$display" ]; then
        echo "ERROR: the nested compositor did not come up." >&2
        tail -20 "$compositor_log" >&2
        exit 1
      fi

      # ------------------------------------------------------------------
      # Footer
      # ------------------------------------------------------------------
      # THE FOOTER'S ENVIRONMENT GOES INTO THE ENVIRONMENT, and not into an `env`
      # at call time. What restarts the footer is the watcher, which is another
      # process: whatever the call needs to know is either exported or does not get there.
      export WAYLAND_DISPLAY="$display"
      export RUST_LOG="$rustlog"
      # `SCROLLSOCK` and `SWAYSOCK` were exported before the compositor
      # started, so the footer and the watcher inherit them already.

      # AND WHO ASKED FOR IT, which is the third of the watcher's three guards.
      #
      # `footer-single.nix`'s subshell is reparented the moment the script that
      # spawned it returns, so it cannot ask about its parent. It can ask about
      # a PID, and this is the one that means "the session that wanted a footer
      # is still running" — true through every hot reload, false the instant
      # Ctrl+C is answered, and answerable no matter who the kernel made its
      # parent.
      export RONOCTUA_DEV_PID="$$"

      # WHAT THE RUNNING FOOTER READS, and it goes BEFORE the footer starts.
      #
      # These two were exported further down, inside the `if [ -f Cargo.toml ]`
      # that sets up the hot reload — that is, AFTER the first footer was
      # already up. So the first footer of every session had neither, and only
      # the one a recompilation restarted did.
      #
      # The symptom of the second one was reported from the screen: `~/main`
      # exists, `LEADER .` opened the home. And the first one had exactly the
      # symptom its own comment describes — the launcher opening with category
      # glyphs that are never replaced by the app's icon. Both are silent: a
      # missing variable is a fallback, not an error.
      #
      # They are not build inputs, so they do not belong to that block at all.
      # `nix/rust.nix` deliberately keeps the superpack OUT of the compilation
      # (baked in, its store path would be a reference no
      # `ronoctua.icons.superpack = false` could remove), and where the manager
      # opens is a NixOS option on an installed system — here there is no NixOS
      # to read one from, so both sides take it from `nix/files-start.nix`.
      export RONOCTUA_ICONS="${iconSuperpack}/share/icons/noctua"
      # THE THEME the loop starts in, and where the generator's fragments are —
      # the same two variables `nix/modules/scroll.nix` exports to the session.
      export RONOCTUA_THEME="${defaultTheme}"
      export RONOCTUA_THEMES="${themed}"
      # AND THE WORLD, for the globe and the address under it. Here for the
      # same reason the two above are: it is not a build input, and a variable
      # that arrives after the footer makes the feature work on an installed
      # machine and do nothing in the loop — which is the first place anybody
      # looks.
      export RONOCTUA_ATLAS="${atlasData}/world.atlas"
      # AND THE EDITOR, for the same reason: it is out of the compilation so
      # that `ronoctua.editor.emacs = false` can really remove it, which means
      # somebody has to put it back for the nested loop.
      export RONOCTUA_EMACSCLIENT="${emacs30}/bin/emacsclient"
      # AND THE SCREENSHOT'S EDITOR, third of the same family: the notification
      # a capture raises offers to edit it, and `swappy` is out of the
      # compilation so `ronoctua.media.enable = false` can really remove it.
      # Without this the action simply is not offered in the nested loop — see
      # `usable_actions` in the shell.
      export RONOCTUA_SWAPPY="${swappy}/bin/swappy"
      # AND THE FILE MANAGER'S DECODERS, fourth of the same family — with one
      # difference worth writing down: these are NOT out of the compilation
      # because a switch might remove them. They are unconditional on an
      # installed system (`nix/modules/media.nix`), and they still come through
      # the environment because `ronoctua-files` also builds the binary that
      # runs as ROOT, whose closure must not learn about video.
      #
      # Without these eight lines the preview and the info panel would work on
      # an installed machine and silently do nothing in the nested loop — which
      # is the first place anybody looks.
      export RONOCTUA_FFMPEG="${ffmpeg-full}/bin/ffmpeg"
      export RONOCTUA_FFPROBE="${ffmpeg-full}/bin/ffprobe"
      export RONOCTUA_EXIFTOOL="${exiftool}/bin/exiftool"
      export RONOCTUA_PDFTOPPM="${poppler-utils}/bin/pdftoppm"
      export RONOCTUA_PDFINFO="${poppler-utils}/bin/pdfinfo"
      export RONOCTUA_XLS2CSV="${catdoc}/bin/xls2csv"
      export RONOCTUA_FILES_START="${import ./files-start.nix}"
      # AND WHAT THE SIDEBAR STARTS WITH — the same two files the installed
      # system resolves, for the same reason: there is no NixOS here to read an
      # option from. Keys into `nix/user-dirs.nix`, resolved to names relative
      # to home; a name whose directory is not on this machine just does not
      # become a row.
      export RONOCTUA_FILES_SIDEBAR="${
        let
          userDirs = import ./user-dirs.nix;
          nameOf = key: if key == "start" then import ./files-start.nix else userDirs.${key};
        in
        lib.concatStringsSep ":" (map nameOf (import ./files-sidebar.nix))
      }"

      # Which binary: the store's until the first recompilation, yours after that.
      # That way startup is instant and does not need `cargo` just to have a look.
      ${singleFooter}/bin/ronoctua-footer-single \
        "$RONOCTUA_FOOTER_PID" "${ronoctua-shell}/bin/ronoctua-shell"

      # ------------------------------------------------------------------
      # Hot reload
      # ------------------------------------------------------------------
      # With `cargo`, and not `nix build`: minutes against seconds. Section 15 puts
      # the feedback loop as principle number 1, and a loop of minutes per
      # edit contradicts that in practice.
      root_dir="$PWD"
      if [ -f "$root_dir/Cargo.toml" ] && [ -d "$root_dir/crates" ]; then
        # OUTSIDE the tree. With `target/` inside, every evaluation of the flake copies
        # gigabytes into the store — it cost 793 GB to find out. See AGENTS.md.
        export CARGO_TARGET_DIR="''${CARGO_TARGET_DIR:-''${XDG_CACHE_HOME:-$HOME/.cache}/ronoctua-distro/target}"
        mkdir -p "$CARGO_TARGET_DIR"
        # EVERYTHING THE BUILD BAKES — see `bakedExports` in this file's `let`.
        ${bakedExports}
        # WHICH BINARY THE FOOTER IS, and it starts as the store's.
        #
        # It used to be `$CARGO_TARGET_DIR/loop/ronoctua-shell`, written into
        # the restart line — which only exists after a `cargo build` has
        # succeeded. Editing a BUFFER restarts the footer without compiling any
        # Rust, so the path has to be whatever is running now.
        export RONOCTUA_FOOTER_BIN="${ronoctua-shell}/bin/ronoctua-shell"

        # THIS FUNCTION RUNS IN ANOTHER PROCESS — the watcher's. It can only read
        # what is EXPORTED; a shell variable does not cross a process boundary, and
        # under the watcher's `set -u` a missing variable does not evaluate to false:
        # it gives `unbound variable`, which takes the whole shell down. That is how
        # hot reload ceased to exist after the first edit, in silence.
        #
        # WHAT CHANGED arrives in `RONOCTUA_RELOAD_FILE`, which the watcher
        # exports before the `eval` — that is the whole contract, and it is what
        # makes the two branches possible without teaching `ronoctua-reload`
        # anything about this project. Absent, it falls to the crate branch,
        # which is what the loop did before there was a second one.
        restart_footer() {
          ${singleFooter}/bin/ronoctua-footer-single \
            "$RONOCTUA_FOOTER_PID" "$RONOCTUA_FOOTER_BIN"
          echo "  footer restarted (the compositor is still standing)"
        }

        rebuild() {
          changed="''${RONOCTUA_RELOAD_FILE:-}"
          case "$changed" in
            */buffers/*)
              # A BUFFER IS A COMPONENT, and it is `nix` that builds it.
              #
              # Not `cargo`: each buffer is a workspace of its own, for
              # `wasm32-wasip2`, with a vendored lock and — for three of them — a
              # generated database arriving through a variable. Reproducing that
              # here would be a second way of building the same artifact, and the
              # two would diverge on the day one of them gained a step.
              name=''${changed#*/buffers/}
              name=''${name%%/*}
              echo "  compiling the buffer $name…"
              # `stderr` stays on the terminal on purpose: it is where nix puts
              # its progress AND its errors, and this is a loop someone is
              # watching. `stdout` is the store path, alone.
              if out=$(nix build --no-link --print-out-paths "$root_dir#buffer-$name") \
                 && [ -d "$out/lib" ]; then
                # The file is whatever came out, and not a name assembled here:
                # the `.wasm` is named after the crate, and guessing it would
                # break the day one of them is renamed.
                wasm=$(echo "$out"/lib/*.wasm)
                var="RONOCTUA_BUFFER_$(echo "$name" | tr '[:lower:]' '[:upper:]')"
                # THE SAME PATH TWICE MEANS NIX SAW NOTHING.
                #
                # The flake reads the GIT tree, and a file that was never added
                # to it is invisible — the build succeeds, unchanged, and the
                # edit is simply not in it. Without this line the symptom is a
                # reload that reports success and changes nothing on screen.
                if [ "$wasm" = "''${!var:-}" ]; then
                  echo "  the component came out identical: a new file needs 'git add -N'"
                else
                  export "$var=$wasm"
                  restart_footer
                fi
              else
                echo "  the buffer did not compile; the previous footer is still running"
              fi
              ;;
            *)
              echo "  compiling…"
              # THE QUEUE'S DAEMON COMES ALONG, and leaving it out was a silent
              # trap waiting to be sprung.
              #
              # The footer finds `ronoctua-transfers` NEXT DOOR — it has to,
              # because both come out of the same build and a store path baked
              # into one would be that build referencing itself. Before the
              # first reload "next door" is the wrapper's `bin/`, where it is;
              # after one it is `$CARGO_TARGET_DIR/loop/`, where it would not
              # have been.
              #
              # The symptom would have been the worst kind: everything works,
              # and then the FIRST edit of the session quietly takes the
              # transfer queue away — with the modeline simply not showing a
              # segment, which is also what "nothing is running" looks like.
              #
              # `ronoctua-rescue` is here for the same reason and was already
              # missing before any of this: `Ctrl+Alt+Del` looks next door too,
              # so the rescue console has been quietly unreachable after the
              # first reload of every session. The check
              # `dev-rebuilds-what-sits-beside` is what found it.
              #
              # It costs nothing: one workspace, one build, and neither binary
              # links wgpu or the wasm runtime.
              #
              # `-p ronoctua-rescue` AND `--bin ronoctua-rescue`, and the `-p`
              # was missing. `--bin` names a target inside the packages `-p`
              # selected, so cargo answered "no bin target named
              # `ronoctua-rescue` in `ronoctua-shell`" and the WHOLE command
              # failed — every Rust edit of every session fell into the `else`
              # below, which says the compilation failed and keeps the old
              # footer running. Hot reload had stopped existing, and what it
              # looked like was a loop that had to be restarted for each change.
              #
              # `dev-rebuilds-what-sits-beside` did not catch it because it
              # greps for the NAME. It checks the packages now.
              #
              # AND THE PROFILE IS THE LOOP'S, not the one that is installed.
              # See `[profile.loop]` in the root `Cargo.toml` for the
              # measurement: the same edit is 148 s in `release`.
              if cargo build --profile loop -p ronoctua-shell --bin ronoctua-shell \
                   -p ronoctua-rescue --bin ronoctua-rescue \
                   -p ronoctua-transfers --bin ronoctua-transfers \
                   -p ronoctua-brain --bin ronoctua-brain 2>&1 | tail -20; then
                export RONOCTUA_FOOTER_BIN="$CARGO_TARGET_DIR/loop/ronoctua-shell"
                restart_footer
              else
                echo "  the compilation failed; the previous footer is still running"
              fi
              ;;
          esac
        }
        export -f restart_footer rebuild 2>/dev/null || true

        # THE NAME HAS TO BE THE FUNCTION'S, and for eight months it was not.
        #
        # It read `rebuild`, and no such command ever existed: the watcher
        # `eval`s the string, gets "command not found", and `ronoctua-reload`
        # swallows it with `|| true` — which is right, because a compile error is
        # the normal case in a loop. So hot reload never once fired, and what
        # hid it is that the store's footer kept working perfectly.
        # `reload-command-is-defined` is what fails now if the two drift again.
        RONOCTUA_RELOAD_CMD='rebuild' ${reloadApp}/bin/ronoctua-reload "$root_dir/crates" "$root_dir/buffers" &
        watcher_pid=$!
        echo "hot reload on: a .rs in crates/ recompiles the footer; in buffers/ it rebuilds the component"
      else
        echo "no hot reload: run it from inside the project's tree to switch it on"
      fi
      echo

      wait "$compositor_pid"
    '';

    meta = {
      description = "The nested compositor + footer, for the UI working loop";
      platforms = lib.platforms.linux;
    };
  };
in
# THE APP, with the CONFIG hanging off it.
#
# The checks that read the generated config need the file, and exposing it here is what
# stops the nested config from going back to being assembled only at run time,
# where nothing reaches it — that is how a `duplicate keybind` got through the
# whole `nix flake check` and only showed up as a red box on screen.
app.overrideAttrs (previous: {
  passthru = (previous.passthru or { }) // {
    config = nestedConfig;
  };
})
