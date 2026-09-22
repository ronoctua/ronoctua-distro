# The compositor's CHORD binds, GENERATED from the shell's command table.
#
# # Why generated
#
# A chord — the leader HELD while an arrow is pressed — never reaches the
# footer's keymap: it only receives the keyboard after the leader is released
# (§7.4). What resolves a chord is the compositor, and that is why the bind
# lives in its config.
#
# Writing those binds by hand would bring back exactly what this work has just
# eliminated: two shortcut tables, two notations, and a which-key
# that knows only half. Here the text comes out of the SAME `actions.rs` that
# `run_command` uses — remove an id there, the bind disappears from here.
#
# # Why a derivation, and not `builtins.readFile`
#
# Reading a derivation's output during evaluation is import-from-derivation:
# evaluation stops and waits for a build. The whole config is assembled as a
# derivation and delivered through `source`, so none of this enters the flake's
# evaluation path.
{
  runCommand,
  ronoctua-shell,
}:

{
  # Returns a file with the binds, ready to go inside the config's
  # `binds { … }` block.
  #
  # `prefix` is `Super` in production and `Alt` in the nested loop — Super
  # belongs to the host's compositor when running inside another one.
  forPrefix =
    prefix:
    runCommand "ronoctua-binds-${prefix}.conf" { } ''
      # THE PREFIX HAS TO BE A NAME THE COMPOSITOR KNOWS, and this is the guard
      # that would have caught the one defect this whole file exists to make
      # impossible.
      #
      # `Mod` is not a modifier here. The table is
      # `Shift Lock Control Ctrl Mod1 Alt Mod2 Mod3 Mod4 Super Mod5`
      # (`get_modifier_mask_by_name`, `sway/input/keyboard.c`), and anything
      # else returns 0 — so `bindsym Mod+Left …` is refused line by line with
      # nothing but a log entry, and EVERY chord in the session is dead while
      # the leader, the emergency binds and the media keys all still work. That
      # is a defect with no symptom anybody would connect to a config: the
      # arrows simply stop moving windows.
      #
      # It was the previous compositor's spelling, and it survived the rename
      # because nothing here read it.
      case "${prefix}" in
        Shift | Lock | Control | Ctrl | Mod1 | Alt | Mod2 | Mod3 | Mod4 | Super | Mod5) ;;
        *)
          echo "ERROR: \`${prefix}\` is not a modifier this compositor knows." >&2
          echo "       It takes Shift, Lock, Control, Ctrl, Mod1, Alt, Mod2," >&2
          echo "       Mod3, Mod4, Super or Mod5. Every chord would be refused" >&2
          echo "       one by one, with only a log line to say so." >&2
          exit 1
          ;;
      esac

      ${ronoctua-shell}/bin/ronoctua-shell export-binds ${prefix} > $out

      # IT MUST NOT COME OUT EMPTY. An `export-binds` that failed in
      # silence would give a config with no chords — and the symptom would be "the
      # arrows stopped working", with no error anywhere.
      if [ ! -s $out ]; then
        echo "ERROR: export-binds emitted nothing." >&2
        exit 1
      fi

      # THE LEADER ALONE MUST NOT BECOME A BIND.
      #
      # `${prefix} {` — the modifier with no key at all — would make HOLDING the
      # leader fire something, and the requirement is the opposite: holding has to be
      # inert until the second key arrives. The sequence's trigger is
      # `${prefix}+Space`, a chord like the others.
      if grep -qE '^bindsym( --[a-z-]+)* ${prefix} ' $out; then
        echo "ERROR: there is a bind on the leader ALONE; holding the leader would stop" >&2
        echo "      be inert and would steal the window's focus." >&2
        exit 1
      fi

      # THE EMERGENCY BINDS HAVE TO BE HERE, AND WITHOUT THE PREFIX.
      #
      # They exist for when the leader does NOT respond. One of them coming out with
      # `${prefix}+` in front would be the lock inside the safe: to
      # fix the keyboard, press the key the broken keyboard does not emit.
      #
      # `grep -F`, and not `-E`: the `+` in `Ctrl+Alt+L` is a metacharacter in ERE
      # (`Ctr` followed by one or more `l`), so the guard matched nothing and
      # always passed. A guard that never fires is worse than none —
      # it claims to have checked.
      # AND LOWERCASE FOR A SINGLE LETTER, which is not cosmetic: a bind
      # matches a KEYSYM, so `Ctrl+Alt+L` is the keysym `L` — which needs
      # Shift to be produced at all, so the bind would simply never fire.
      for press_key in Ctrl+Alt+l Ctrl+Shift+l Ctrl+Alt+Escape Ctrl+Alt+Delete; do
        if ! grep -qF "bindsym" $out || ! grep -qF " $press_key " $out; then
          echo "ERROR: the emergency bind $press_key did not come out." >&2
          exit 1
        fi
        if grep -qF "${prefix}+$press_key " $out; then
          echo "ERROR: $press_key came out with the leader's prefix, and it exists" >&2
          echo "      exactly so as to work when the leader does not." >&2
          exit 1
        fi
      done

      # AND THE RESTORER HAS TO REACH THE RIGHT COMMAND. A wrong id here
      # would give a `warn` in the footer's log and nothing on screen — at the moment
      # when one can least afford to guess what happened.
      grep -q 'keyboard-restore' $out || {
        echo "ERROR: the emergency bind does not call \`keyboard-restore\`." >&2
        exit 1; }
    '';
}
