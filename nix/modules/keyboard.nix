# The leader is the Super key ON ITS OWN — §7.4.
#
# # Why this is not niri config
#
# niri refuses a bind on a lone modifier: `Mod { … }` gives `invalid key:
# Mod`. And it has no trigger on RELEASE — the bind properties are
# `allow-when-locked`, `allow-inhibiting`, `cooldown-ms`, `repeat` and
# `hotkey-overlay-title`, and none of them is "on-release".
#
# This is not a limitation to work around: a modifier *is* a modifier. For
# the same physical key to be a modifier when HELD and a command when
# TAPPED, someone has to tell the two apart — and that distinction happens
# below the compositor, in the kernel's event stream.
#
# # What keyd does, exactly
#
# `overload(meta, f13)`: while the key is held it is the Meta modifier —
# so `Super+←` still reaches niri as `Mod+Left`. If it is released without
# any other key having been pressed, it emits `F13`, which niri binds to
# the leader.
#
# # The risk, and why it is acceptable
#
# keyd sits BETWEEN the keyboard and everything else: a bug in it is a
# keyboard that does not type. It is the most dangerous piece in the system
# per line of code.
#
# What bounds the damage: it remaps ONE key. Every other key passes
# straight through, `[ids] *` changes nothing beyond what the `[main]`
# section says, and the service does not block boot — if it fails to start,
# Super goes back to being just a modifier and the rest of the keyboard is
# unchanged. You lose the leader, not the keyboard.
#
# # The way out, when it does fall over
#
# Losing the leader means losing the path to every command in the system,
# and that is why binds live outside it
# (`crates/ronoctua-shell/src/actions.rs`, `emergency()`): `Ctrl+Alt+L`
# opens the sequence without depending on `F13`, and `Ctrl+Shift+L` runs
# `keyboard-restore`, which restarts this service. A recovery shortcut that
# depended on the leader would be the key inside the safe.
{ config, ... }:

{
  # DO NOT CREATE THE `keyd` GROUP.
  #
  # It warns "failed to set effective group to keyd" at every start, and the
  # obvious fix — `users.groups.keyd = { }` — TAKES THE SERVICE DOWN: the
  # NixOS unit runs without `CAP_SETGID`, so with the group existing the
  # `setgid` fails with "Operation not permitted" and the process exits 255.
  # Without the group, it warns and carries on as root, remapping just the same.
  #
  # The warning IS the intended path. Tested: creating the group cost one VM
  # round with a dead keyboard.
  services.keyd = {
    enable = true;
    keyboards.all = {
      # Every keyboard: the laptop's, the Bluetooth one, whatever gets plugged
      # in later. A specific `ids` would leave the leader out of precisely the
      # keyboard the person has just connected.
      ids = [ "*" ];
      settings.main = {
        # The physical key is `leftmeta`. `overload(meta, f13)` — see above.
        #
        # `f13` and not a real key: F13 exists on no keyboard, so nothing else can
        # emit it by accident. Using `esc` or `menu` would make the leader fire
        # when the person wanted the actual key.
        leftmeta = "overload(meta, f13)";
      };
    };
  };

  # WHO MAY RESTART KEYD, AND ONLY THAT.
  #
  # `keyd.service` is a SYSTEM unit: restarting it from inside the session
  # requires polkit. Without this rule, `keyboard-restore` would ask for a
  # password in a window that — precisely because the keyboard is broken —
  # might be impossible to answer.
  #
  # The rule is narrow on purpose: ONE verb, ONE unit, ONE user. A
  # `manage-units` opened to the `wheel` group would let any process in the
  # session stop any system service, which is a much bigger thing than
  # "fix the keyboard".
  #
  # The user comes from `config.ronoctua.user` (invariant 16), never a literal.
  security.polkit.extraConfig = ''
    polkit.addRule(function(action, subject) {
      if (action.id == "org.freedesktop.systemd1.manage-units" &&
          action.lookup("unit") == "keyd.service" &&
          action.lookup("verb") == "restart" &&
          subject.user == "${config.ronoctua.user}") {
        return polkit.Result.YES;
      }
    });
  '';
}
