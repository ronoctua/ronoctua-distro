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
# # Why `compose:ralt`, and what it costs
#
# The British keyboard has no accent key, and the plain `gb` layout only reaches
# dead `^` and `` ` `` — the acute, the tilde and the cedilla are missing, which
# is half of Portuguese.
#
# `compose:ralt` turns AltGr into the **Compose** key: `AltGr ' a` gives `á`,
# `AltGr ~ a` gives `ã`, `AltGr , c` gives `ç`, `AltGr ^ e` gives `ê`,
# `AltGr " u` gives `ü`. All accenting comes from a single rule, and **nothing
# changes in normal typing** — the backtick and the quote go on being
# themselves, which matters to whoever writes Markdown and shell.
#
# The alternatives were rejected with reason:
#
#   - `gb(intl)` and `gb(extd)` put a DEAD accent on level 1: the `` ` `` key
#     stops writing a backtick until you press space. On a keyboard that types
#     code all day, that is worse than the problem it solves.
#   - `lv3:ralt_switch` (`gb`'s default) keeps AltGr as level 3, which
#     gives `ê` and `è` and gives neither `á`, `ã` nor `ç`.
#
# The real cost: AltGr's level-3 symbols (`€`, `½`, `æ`, the arrows)
# stop being one keystroke away. Compose reaches all of them — `AltGr = e` gives
# `€` — so it is one extra gesture, not a loss.
#
# # Where this works
#
# In the apps (GTK/Qt do compose on their own) and **in our buffers too**:
# SCTK builds an `xkb::compose::State` and the text that reaches the footer and
# the terminal arrives already composed. It is not an assumption — it is in
# `init_compose`, in the `smithay-client-toolkit` the `Cargo.lock` pins.
{
  layout = "gb";
  # No variant: the two that exist for `gb` put a dead accent on level 1.
  variant = "";
  options = "compose:ralt";
  # The TTY console uses a `kbd` keymap, and not xkb. `uk` is `gb`'s equivalent
  # over there; without this the installer on a TTY would answer in US.
  console = "uk";
}
