# THE NESTED LOOP'S KEYMAP — production's, with Super taken out of it.
#
# `nix run .#dev` runs niri as a WINDOW of another compositor, and that window
# shares one keyboard with the host. The host sees every key first. When it
# keeps a chord for itself — `Super+Ctrl+Space` opens the wallpaper manager on
# the machine this was found on — the nested niri is left holding half a
# gesture, and it never recovers. This file removes the half that can be
# removed.
#
# # The mechanism, read in the code and then measured
#
# Three facts, and it takes all three:
#
#   1. `winit` reports a lost focus as `Focused(false)` and NOTHING ELSE. Its
#      Wayland `Leave` handler synthesises no key releases (its X11 one does),
#      so the keys that were down when the host took the window away are never
#      reported up.
#   2. `smithay`'s winit backend drops every synthetic key event
#      (`if !is_synthetic && !event.repeat`) and never maps `ModifiersChanged`
#      at all — so even a host that reports the true modifier state on the way
#      back in cannot be heard. The nested compositor learns modifiers ONLY by
#      counting presses against releases.
#   3. niri's winit backend answers `WinitEvent::Focus(_) => ()`.
#
# So a modifier pressed inside the window and released outside it stays down.
# And it stays down FOR EVER: libxkbcommon keeps one filter per pressed key
# with a reference count, so pressing that modifier again and letting it go
# takes the count from 1 to 2 and back to 1. Measured, with the real state
# machine:
#
#     host chord down                    super=1 ctrl=1
#     after tapping Super once           super=1 ctrl=1
#     after tapping Super twice          super=1 ctrl=1
#
# Reloading the keymap does not help either: `update_xkb_state` builds a fresh
# state and REPLAYS the keys smithay still believes are down.
#
# What that costs is everything, because niri matches a bind's modifiers by
# EQUALITY (`bind_modifiers == modifiers`). One stray bit and no bind in the
# session matches — not the leader, not the chords, not the rescue. The only
# cure is restarting the nested compositor.
#
# # Why removing Super is the fix, and the only one available
#
# Prevention is the only route, and the single lever we own is the keymap: a
# key that maps to no modifier cannot leave one behind. Super is the one
# modifier the nested loop can spare — `nix/dev.nix` already says so in prose
# ("`Mod` belongs to the host's compositor when running nested"), and this
# makes it true in the keymap instead of only in the comment. Shift, Control,
# Alt and AltGr all have work to do in here and stay.
#
# It is worth exactly what it covers: every host chord built on Super alone
# — the launcher, the workspaces, the window moves — stops being able to poison
# the loop. A chord that also holds Control leaves Control behind, and Control
# is part of the leader, so the leader survives it; the `Alt` chords do not,
# and that residue is written down in `docs/PENDING.md`.
#
# PRODUCTION IS NOT TOUCHED, and must not be: there niri owns the seat, nothing
# gets between the keyboard and the compositor, and Super IS the leader
# (`nix/modules/keyboard.nix`). This file is imported by `nix/dev.nix` and by
# nothing else.
#
# # Why a compiled keymap and not an xkb option
#
# There is no option that takes Super away. The `altwin:` family MOVES it
# (`meta_win`, `hyper_win`) and moving it only changes which bit gets stuck;
# `compose:lwin` turns it into Compose, and then a stray press eats the two
# keystrokes after it. So the keymap is compiled from the SAME single source
# every other keyboard comes from (`nix/keyboard-xkb.nix`) and one statement is
# cut out of it.
{
  runCommand,
  lib,
  libxkbcommon,
  xkeyboard_config,
}:

let
  keyboard = import ./keyboard-xkb.nix;
in
runCommand "ronoctua-nested-keymap.xkb"
  {
    nativeBuildInputs = [ libxkbcommon ];
  }
  ''
    export XKB_CONFIG_ROOT=${xkeyboard_config}/share/X11/xkb

    # FORMAT 1, EXPLICITLY, on the way out and on the way in.
    #
    # The compositor parses this with `XKB_KEYMAP_FORMAT_TEXT_V1` and has no
    # other option; `xkbcli` defaults to reading format 2 and to "whatever the
    # input was" on the way out. A libxkbcommon that one day changes its
    # default would hand niri a file it cannot parse — and niri answers that
    # with a `warn` in a log nobody is reading and a silent fall back to the
    # layout below, which is the version WITH Super. The failure would look
    # exactly like the bug this file exists to remove.
    xkbcli compile-keymap \
      --output-format 1 \
      --layout ${keyboard.layout} \
      ${lib.optionalString (keyboard.variant != "") "--variant ${keyboard.variant} "}\
      --options "${keyboard.options}" > full.xkb 2> errors.txt || {
        echo "ERROR: the keymap did not compile:" >&2; cat errors.txt >&2; exit 1; }

    if grep -q "was ignored" errors.txt; then
      echo "ERROR: xkb IGNORED an option and carried on:" >&2
      cat errors.txt >&2
      exit 1
    fi

    # ONE STATEMENT, AND IT HAS TO BE THERE BEFORE IT CAN BE CUT.
    #
    # A sed that matches nothing is the whole danger here: the file would come
    # out correct-looking, the loop would go on being poisoned, and there would
    # be no error anywhere.
    before=$(grep -c "^[[:space:]]*modifier_map Mod4 {" full.xkb || true)
    if [ "$before" != 1 ]; then
      echo "ERROR: expected exactly one \`modifier_map Mod4\` in the compiled" >&2
      echo "      keymap and found $before. The statement that binds the Super" >&2
      echo "      keys to a modifier has moved, and cutting the wrong line would" >&2
      echo "      leave the nested loop poisonable with nothing saying so." >&2
      exit 1
    fi

    sed "/^[[:space:]]*modifier_map Mod4 {/d" full.xkb > $out

    # AND THE OTHER FOUR HAVE TO SURVIVE. A wider pattern that also took Shift
    # or Control with it would give a keyboard that cannot type a capital or a
    # `Ctrl+C`, and the leader would go on working — so the loop would look fine
    # until somebody typed.
    for real in Shift Control Mod1 Mod5; do
      grep -q "^[[:space:]]*modifier_map $real {" $out || {
        echo "ERROR: \`modifier_map $real\` did not survive the cut." >&2
        exit 1; }
    done

    # IT IS STILL A KEYMAP, and in the format the compositor reads.
    xkbcli compile-keymap --input-format 1 --keymap $out --test || {
      echo "ERROR: the keymap without Super does not parse as format 1," >&2
      echo "      which is the only format the compositor accepts." >&2
      exit 1; }

    # AND NO KEY SETS SUPER ANY MORE — asked of the compiled keymap rather
    # than of its spelling. `--modmaps` lists what every key contributes; the
    # section that matters is the one about keys, because the header above it
    # names every modifier the format knows, Mod4 included.
    xkbcli compile-keymap --input-format 1 --keymap $out --modmaps \
      | sed -n "/Keys modifier maps/,\$p" > modmaps.txt
    if grep -qE "Mod4|Super" modmaps.txt; then
      echo "ERROR: a key still carries Super in the nested keymap:" >&2
      grep -B 2 -E "Mod4|Super" modmaps.txt >&2
      exit 1
    fi

    echo "the nested keymap has no Super: $out" >&2
  ''
