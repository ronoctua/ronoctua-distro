# THE NESTED LOOP'S KEYMAP — production's, with Super taken out of it.
#
# `nix run .#dev` runs the compositor as a WINDOW of another compositor, and
# that window shares one keyboard with the host. The host sees every key first.
# When it keeps a chord for itself — `Super+Ctrl+Space` opens the wallpaper
# manager on the machine this was found on — the nested compositor used to be
# left holding half a gesture, and it never recovered. This file removes the
# half that can be removed.
#
# # WHY IT PROBABLY NO LONGER APPLIES, on paper
#
# A bit gets stuck when the nested compositor DERIVES its modifier state by
# counting presses against releases: a key pressed inside the window and
# released outside it is counted down nowhere, and the bit stays up for the
# rest of the session.
#
# wlroots does not derive it. Its Wayland backend implements `.modifiers` and
# hands the host's mask straight through (`keyboard_handle_modifiers` →
# `wlr_keyboard_notify_modifiers`, `backend/wayland/seat.c`), and its key
# handler passes `update_state = false` — so the nested state is a MIRROR of
# the host's. A key released outside the window cannot leave a bit behind,
# because the next `modifiers` event from the host overwrites the whole mask.
#
# What it cost when it did happen was everything, and that part is unchanged:
# a bind is matched by EQUALITY here too (`modifiers ^ binding->modifiers`,
# `sway/input/keyboard.c`), so one stray bit means no bind in the session
# matches — not the leader, not the chords, not the rescue.
#
# # So why is this file still here
#
# BECAUSE IT HAS NOT BEEN MEASURED on this compositor, and the reasoning above
# is reading, not evidence. The measurement is one gesture: in `nix run .#dev`,
# press a host chord built on Super, let the host take the window, come back,
# and try the leader. If it opens, all of this goes — this file, the
# `nested-survives-a-stray-modifier` check, and the second leader bind with a
# Shift nobody pressed. `docs/PENDING.md` carries it.
#
# Until then it costs one build of a keymap and takes away a modifier the
# nested loop does not use anyway.
#
# # Why removing Super is what it removes
#
# Prevention was the only route, and the single lever we own is the keymap: a
# key that maps to no modifier cannot leave one behind. Super is the one
# modifier the nested loop can spare — `nix/dev.nix` already says so in prose
# ("`Mod` belongs to the host's compositor when running nested"), and this
# makes it true in the keymap instead of only in the comment. Shift, Control,
# Alt and AltGr all have work to do in here and stay.
#
# PRODUCTION IS NOT TOUCHED, and must not be: there the compositor owns the
# seat, nothing gets between the keyboard and it, and Super IS the leader
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
    # default would hand the compositor a file it cannot parse — and it answers
    # that with a log line nobody is reading and a silent fall back to the
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
