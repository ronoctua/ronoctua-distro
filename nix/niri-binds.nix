# niri's CHORD binds, GENERATED from the shell's command table.
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
# that knows only half. Here the KDL comes out of the SAME `actions.rs` that
# `executar` uses — remove an id there, the bind disappears from here.
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
  # `prefixo` is `Mod` in production and `Alt` in the nested loop — `Mod` belongs
  # to the host's compositor when running inside another one.
  forPrefix =
    prefix:
    runCommand "ronoctua-binds-${prefix}.kdl" { } ''
      ${ronoctua-shell}/bin/ronoctua-shell export-niri-binds ${prefix} > $out

      # IT MUST NOT COME OUT EMPTY. An `export-niri-binds` that failed in
      # silence would give a config with no chords — and the symptom would be "the
      # arrows stopped working", with no error anywhere.
      if [ ! -s $out ]; then
        echo "ERRO: export-niri-binds nao emitiu nada." >&2
        exit 1
      fi

      # THE LEADER ALONE MUST NOT BECOME A BIND.
      #
      # `${prefix} {` — the modifier with no key at all — would make HOLDING the
      # leader fire something, and the requirement is the opposite: holding has to be
      # inert until the second key arrives. The sequence's trigger is
      # `${prefix}+Space`, a chord like the others.
      if grep -qE '^ *${prefix} *(repeat=[a-z]+ *)?\{' $out; then
        echo "ERRO: ha bind na leader SOZINHA; segurar a leader deixaria de" >&2
        echo "      ser inerte e roubaria o foco da janela." >&2
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
      for press_key in Ctrl+Alt+L Ctrl+Shift+L; do
        if ! grep -qF "    $press_key " $out; then
          echo "ERRO: o bind de emergencia $press_key nao saiu." >&2
          exit 1
        fi
        if grep -qF "${prefix}+$press_key " $out; then
          echo "ERRO: $press_key saiu com o prefixo da leader, e ele existe" >&2
          echo "      exatamente para funcionar quando a leader nao funciona." >&2
          exit 1
        fi
      done

      # AND THE RESTORER HAS TO REACH THE RIGHT COMMAND. A wrong id here
      # would give a `warn` in the footer's log and nothing on screen — at the moment
      # when one can least afford to guess what happened.
      grep -q 'keyboard-restore' $out || {
        echo "ERRO: o bind de emergencia nao chama \`keyboard-restore\`." >&2
        exit 1; }
    '';
}
