# The leader is the Super key ON ITS OWN — §7.4.
#
# # Why this is not compositor config
#
# The compositor CAN express the shape — `bindsym --release Super_L …` binds a
# lone modifier keysym and fires when it goes up — and it still cannot do the
# job, because `--release` does not tell a TAP from a HOLD.
#
# Read in `sway/input/keyboard.c`: a release binding that matches at PRESS time
# is stored in `held_binding`, and it executes as soon as every key in the
# history has been released (`key_history_is_released`). Nothing clears it when
# another key is pressed in between. So `Super+d` sets the binding on Super's
# press, runs `d`'s own binding, and then fires the leader as well when the
# hand comes off — every chord in the system would open the list behind itself.
#
# This is not a limitation to work around: a modifier *is* a modifier. For
# the same physical key to be a modifier when HELD and a command when
# TAPPED, someone has to tell the two apart — and that distinction happens
# below the compositor, in the kernel's event stream.
#
# # What keyd does, exactly
#
# `overload(meta, f13)`: while the key is held it is the Meta modifier —
# so `Super+←` still reaches the compositor as `Mod4+Left`. If it is released
# without any other key having been pressed, it emits `F13`, which the
# compositor binds to the leader.
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
#
# # AND WHY THE DIRECTORY IS OURS RATHER THAN THE STORE'S
#
# Since 2026-09-08 the shell can give ONE keyboard rules of its own
# (`LEADER s k`), and keyd's own manual is what shapes this: *a device id may
# only be listed in a single config file*. The file below claims `*`, so a
# keyboard with rules of its own has to be taken OUT of that claim — and that
# subtraction changes whenever somebody configures a keyboard, which a store
# path cannot do.
#
# So `/etc/keyd` is a symlink to a directory this machine owns, seeded here
# with the same leader rule and written afterwards by the shell. What the
# flake still declares is the RULE (`assets/keyboards.nix`), which is the part
# that must never drift: every file the shell renders repeats it, because a
# keyboard outside the catch-all with no leader is a keyboard that cannot open
# anything.
{
  config,
  pkgs,
  ...
}:

let
  keyboards = import ../../assets/keyboards.nix;
  inherit (keyboards.leader) key action;

  # WHERE THE RULES LIVE, and the shell has the same path baked in
  # (`ronoctua_keyboards::DIRECTORY`). Under `/var/lib` and not
  # `$XDG_CONFIG_HOME`: keyd is a SYSTEM daemon and starts before anybody logs
  # in, so a file in a home directory would be a leader that arrives late.
  directory = "/var/lib/ronoctua/keyd";

  # THE SEED, and it is what `ronoctua_keyboards::render` produces for a
  # machine where nobody has configured anything. It exists so the leader
  # works on the FIRST boot, before the shell has ever written a file — and
  # `keyd-rules-carry-the-leader` compares the two.
  seed = pkgs.writeText "keyd-default.conf" ''
    [ids]
    *

    [main]
    ${key} = ${action}
  '';
in
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
    # NO `keyboards` BLOCK. The upstream module turns one into
    # `environment.etc."keyd/<name>.conf"`, which is a read-only store path —
    # and the whole point is that the subtraction below its `[ids]` changes
    # when somebody configures a keyboard. The rule itself is still declared,
    # in `assets/keyboards.nix`, and seeded into the directory below.
    keyboards = { };
  };

  # THE DIRECTORY, ITS SEED, AND THE SYMLINK.
  #
  # `d` makes it belong to the session's user, so the shell writes there with
  # no privilege at all. `C` copies the seed only when the file is not there,
  # so a machine that has been configured keeps its rules across a rebuild —
  # and one that has never been configured has a leader on first boot. `L+`
  # replaces whatever is at `/etc/keyd`, which on this system is nothing:
  # `keyboards = { }` above means the upstream module declares no file.
  systemd.tmpfiles.rules = [
    "d ${directory} 0755 ${config.ronoctua.user} users -"
    "C ${directory}/default.conf 0644 ${config.ronoctua.user} users - ${seed}"
    "L+ /etc/keyd - - - - ${directory}"
  ];

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
