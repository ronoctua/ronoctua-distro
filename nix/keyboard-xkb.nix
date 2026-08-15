# THE KEYBOARD LAYOUT, in a single place.
#
# It is read by the production config (`nix/modules/niri.nix`), by the nested
# loop (`nix/dev.nix`) and by the TTY console. Written in each of them, one day
# they would diverge — and the symptom would be the right key producing a
# different character depending on where you type.
#
# Not to be confused with `nix/modules/keyboard.nix`: that one is the daemon
# that makes Super the leader when tapped. This one is the symbol map.
#
# # Why `gb`
#
# The owner's decision, and temporary: the Bluetooth keyboard in use is a UK one.
# The installer's prompt will ask about this one day (§7.1 already asks about
# language); until then, the value is here and changes in one place.
#
# # Why Compose on CAPS LOCK, and what it costs
#
# The British keyboard has no accent key, and the plain `gb` layout only reaches
# dead `^` and `` ` `` — the acute, the tilde and the cedilla are missing, which
# is half of Portuguese. Compose fills that gap: `CAPS ' a` gives `á`,
# `CAPS ~ a` gives `ã`, `CAPS , c` gives `ç`, `CAPS ^ e` gives `ê`, `CAPS " u`
# gives `ü`. **Nothing changes in normal typing** — the backtick and the quote
# go on being themselves, which matters to whoever writes Markdown and shell.
#
# It used to be AltGr (`compose:ralt`), and Caps Lock is the owner's decision.
# It is also the better key on the merits: Caps Lock is the largest key on the
# home row, under the left little finger, and what it does by default —
# shouting — is something nobody asked for. AltGr goes back to being AltGr, and
# with it the level-3 symbols it always had (`€`, `½`, `æ`, the arrows) come
# back to being one keystroke instead of a Compose sequence.
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
#   - `gb(intl)` and `gb(extd)` put a DEAD accent on level 1: the `` ` `` key
#     stops writing a backtick until you press space. On a keyboard that types
#     code all day, that is worse than the problem it solves.
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

  layout = "gb";
  # No variant: the two that exist for `gb` put a dead accent on level 1.
  variant = "";
  # What xkb actually receives. `nix/modules/base.nix` builds the same string
  # from the option the installer fills in — the two agree because both come
  # from the two lists above.
  options = builtins.concatStringsSep "," ((if compose == "" then [ ] else [ compose ]) ++ always);
  # The TTY console uses a `kbd` keymap, and not xkb. `uk` is `gb`'s equivalent
  # over there; without this the installer on a TTY would answer in US.
  console = "uk";
}
