# the compositor's WINDOW RULES, generated from the shell — same reason as the binds.
#
# # What is in here
#
# One rule: the process monitor's window opens FLOATING. The shell does not
# place windows (invariant 4) — it sets the window's `app_id` and the compositor
# decides. The two ends of that agreement are one string, and it is written in
# `crates/ronoctua-shell/src/terminal.rs` (`APP_ID_MONITOR`).
#
# # Why generated, and not written in the two configs
#
# Because there are two of them: `nix/modules/scroll.nix` and `nix/dev.nix`. With
# the rule written by hand, the `app_id` would live in three places, and the day
# one of them drifts the symptom is a monitor that opens TILED — which looks
# exactly like the shortcut working. Generated, the config cannot disagree with
# the window: both come from the same constant.
#
# # Why a derivation
#
# Reading a derivation's output during evaluation is import-from-derivation:
# evaluation stops to wait for a build. This is only built when the config is.
{
  runCommand,
  ronoctua-shell,
}:

runCommand "ronoctua-window-rules.conf" { } ''
  ${ronoctua-shell}/bin/ronoctua-shell export-window-rules > $out

  # IT MUST NOT COME OUT EMPTY, and it must name the window it is about. An
  # export that failed in silence would give a config with no rule at all, and
  # the symptom would be "the monitor stopped floating" — with no error
  # anywhere, and looking like a compositor regression.
  if [ ! -s $out ]; then
    echo "ERROR: export-window-rules emitted nothing." >&2
    exit 1
  fi
  grep -q 'floating enable' $out || {
    echo "ERROR: the rule came out with no \`floating enable\`; the monitor's window" >&2
    echo "      would open side by side, which looks like the shortcut working." >&2
    exit 1; }
  # `grep -F`: the criterion is a regex with the app id's dots escaped, so it
  # is full of backslashes — reading it as a pattern would match something
  # else, or nothing.
  # AND IT MUST BE A CRITERION. A `for_window` with no criteria applies to
  # EVERY window, which would float the whole desktop — the loudest possible
  # way for a missing bracket to go unnoticed.
  grep -qF 'for_window [' $out || {
    echo "ERROR: the rule is not matched on anything." >&2
    exit 1; }
  grep -qF 'br\.ronoctua\.monitor' $out || {
    echo "ERROR: the rule does not match the monitor's app id." >&2
    exit 1; }
  # AND THE CAMERA APP, which a person opens over their work: a tile would
  # push every window in the strip aside to show a face.
  grep -qF 'br\.ronoctua\.camera' $out || {
    echo "ERROR: the rule does not match the camera app's id." >&2
    exit 1; }
''
