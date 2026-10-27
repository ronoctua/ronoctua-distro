# THE KEYBOARD LAYOUT, in a single place.
#
# It is read by the production config (`nix/modules/scroll.nix`), by the nested
# loop (`nix/dev.nix`) and by the TTY console. Written in each of them, one day
# they would diverge — and the symptom would be the right key producing a
# different character depending on where you type.
#
# Not to be confused with `nix/modules/keyboard.nix`: that one is the daemon
# that makes Super the leader when tapped. This one is the symbol map.
#
# # Why `us` with the `euro` variant
#
# The layout of the machine this was written on. US QWERTY keeps `` ` ``, `'`,
# `~`, `^` and `"` as ordinary characters — what a keyboard that types code and
# Markdown all day wants — and the `euro` variant adds `€` on AltGr and moves no
# key. A Bluetooth keyboard with a different layout (a UK one, say) is a
# PER-DEVICE setting, not this machine-wide default: it carries its own layout
# in `keyboards.json`, so unplugging it leaves the built-in keyboard on `us`.
# This used to read `gb` for the owner's UK Bluetooth keyboard, which put that
# peripheral's layout on the laptop too — the thing per-device layouts now fix.
# A different machine answers the installer and overrides this in `host.nix`.
#
# # Why Compose on CAPS LOCK, and what it costs
#
# The US layout has no accent key and no dead keys at all — the acute, the
# tilde, the circumflex and the cedilla are all missing, which is half of
# Portuguese. Compose fills that gap: `CAPS ' a` gives `á`,
# `CAPS ~ a` gives `ã`, `CAPS , c` gives `ç`, `CAPS ^ e` gives `ê`, `CAPS " u`
# gives `ü`. **Nothing changes in normal typing** — the backtick and the quote
# go on being themselves, which matters to whoever writes Markdown and shell.
#
# It used to be AltGr (`compose:ralt`), and Caps Lock is the owner's decision.
# It is also the better key on the merits: Caps Lock is the largest key on the
# home row, under the left little finger, and what it does by default —
# shouting — is something nobody asked for. AltGr goes back to being AltGr, and
# the `euro` variant's `€` is one keystroke again (AltGr) instead of a Compose
# sequence.
#
# COMPOSE IS NOT A MODIFIER, and the difference shows here more than it did on
# AltGr: it is not held down. Press and release Caps, then `'`, then `a`. That
# is also why it does not collide with the leader or with any chord — a key that
# sets no modifier cannot be stuck under one.
#
# # And Caps Lock itself, which did not stop existing
#
# `shift:both_capslock`: the two Shifts pressed together toggle Caps Lock, which
# is where it went. It costs nothing — no one presses both Shifts by accident —
# and it keeps the function reachable for whoever writes an acronym.
#
# The alternatives were rejected with reason:
#
#   - `us(intl)` puts DEAD accents on level 1: the `'`, `"`, `` ` `` and `~`
#     keys stop writing themselves until you press space. On a keyboard that
#     types code all day, that is worse than the problem it solves.
#   - `lv3:caps_switch` makes Caps a third-level chooser instead of Compose,
#     which gives `ê` and `è` and gives neither `á`, `ã` nor `ç`.
#
# # Where this works
#
# In the apps (GTK/Qt do compose on their own) and **in our buffers too**:
# SCTK builds an `xkb::compose::State` and the text that reaches the footer and
# the terminal arrives already composed. It is not an assumption — it is in
# `init_compose`, in the `smithay-client-toolkit` the `Cargo.lock` pins.
#
# The TTY is the exception, and it is another database entirely (`kbd`, below):
# there Caps Lock is still Caps Lock. The console is where the installer asks
# for the disk's passphrase, and a passphrase needs no accent.
let
  # WHAT THE PERSON CHOOSES — the installer's seventh question, whose default
  # this is. Empty is a valid answer: a `br(abnt2)` carries the accents on its
  # own keys and wants no Compose key stealing one.
  compose = "compose:caps";
  # AND WHAT THE SYSTEM ADDS EITHER WAY.
  #
  # It is separate from `compose` because the installer overrides that one and
  # not this one. Written into a single string, whoever picked another Compose
  # key would silently lose the Caps Lock toggle along with it — the answer to a
  # question taking away something nobody was asked about.
  always = [ "shift:both_capslock" ];
in
{
  inherit compose always;

  layout = "us";
  # The `euro` variant adds `€` on AltGr and no dead keys; `us(intl)`, which
  # would put dead accents on level 1, is rejected above.
  variant = "euro";
  # What xkb actually receives. `nix/modules/base.nix` builds the same string
  # from the option the installer fills in — the two agree because both come
  # from the two lists above.
  options = builtins.concatStringsSep "," ((if compose == "" then [ ] else [ compose ]) ++ always);
  # The TTY console uses a `kbd` keymap, and not xkb. `us` is this layout's
  # name over there; the console has no Compose and no `euro` variant, and the
  # disk's passphrase needs neither.
  console = "us";
}
