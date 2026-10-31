# THE CONSOLE'S KEYBOARD, compiled from the session's own layouts.
#
# The kernel's console does not read xkb: `loadkeys` takes a `kbd` keymap,
# another database with other names (`uk` where xkb says `gb`), and most xkb
# answers have no `kbd` map at all (`us(euro)` is none). The console used to
# keep ONE `kbd` map, the default's, whatever the installer was answered — and
# the Wi-Fi's password, the user's and the disk's passphrase were typed on it,
# in the installer and again at the initrd's prompt. A password with
# `@ # " \ | ~` was stored with other characters than the session types (a US
# keyboard on the UK map: `|` came out `~`).
#
# So the console's map is COMPILED from the very answer the session reads, by
# `ckbcomp` (console-setup's, what NixOS's `console.useXkbConfig` runs), out of
# the same xkeyboard-config:
#
#   - `one <answer>`: a machine's console — one map, a file, which
#     `console.keyMap` takes and the initrd carries to the disk's prompt
#     (`nix/modules/console.nix`);
#   - `every`: the installer's — one map per answer it offers (each layout of
#     `base.lst` plain, and with each of its variants), `<answer>.map`, so the
#     ISO loads the map of an answer the moment it is given, with no
#     compiler on board (`crates/ronoctua-installer/src/keymap.rs`).
#
# An answer is written as xkb writes it (`notation`, `nix/keyboard-xkb.nix`):
# `gb`, `us(euro)`. The options are not compiled in — on the console Caps Lock
# stays Caps Lock, and no password needs Compose.
#
# # What is refused, and why it has to be
#
# `ckbcomp` FALLS BACK IN SILENCE: a variant the layout does not have is a
# warning and the PLAIN layout, exit 0 (measured: `-layout us -variant bogus`
# writes `us`'s map). So `one` refuses an answer `base.lst` does not offer,
# and both refuse a map whose compiler said it could not find what it was
# asked for. A layout whose symbols file does not exist — xkeyboard-config's
# `custom`, the placeholder for a person's own — has no map: `every` leaves it
# out and names it in `left-out.txt`, and the installer refuses the answer
# rather than load another one. Any other failure fails the build, and every
# map is read back by `loadkeys -b`, the console's own parser, before it is
# kept. `console-keymaps-cover-the-answers` holds all of it.
{ pkgs }:
let
  inherit (pkgs) lib;
  keyboard = import ./keyboard-xkb.nix;
  xkb = "${pkgs.xkeyboard_config}/share/X11/xkb";
  compile =
    name: what:
    pkgs.runCommand name
      {
        nativeBuildInputs = [ pkgs.python3 ];
        preferLocalBuild = true;
      }
      ''
        python3 ${./scripts/console-keymaps.py} \
          --xkb ${xkb} \
          --ckbcomp ${pkgs.ckbcomp}/bin/ckbcomp \
          --loadkeys ${pkgs.kbd}/bin/loadkeys \
          --out "$out" ${what}
      '';
in
{
  inherit (keyboard) notation;
  # Every answer's map, `$out/<answer>.map` — about 600, 70 MiB unpacked and
  # a few in the ISO's squashfs, where they are the same lines over and over.
  every = compile "console-keymaps" "--every";
  # One answer's, as a file. A name `base.lst` does not offer fails the build
  # with the sentence that says so — a `kbd` name (`uk`) among them.
  one =
    answer:
    compile "console-keymap-${
      lib.strings.sanitizeDerivationName (
        builtins.replaceStrings
          [
            "("
            ")"
          ]
          [
            "-"
            ""
          ]
          answer
      )
    }" "--one ${lib.escapeShellArg answer}";
}
