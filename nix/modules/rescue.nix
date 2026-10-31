# The rescue console — `Ctrl+Alt+Del`, and the door that does not need the compositor.
#
# # What this module is for
#
# `crates/ronoctua-rescue` is a terminal application. With the compositor alive,
# the footer opens it in a floating window and there is nothing for systemd to
# do. This file is the OTHER door: the one that answers when the compositor is
# what broke, when the session never came up, or when the screen is a TTY.
#
# # Taking `Ctrl+Alt+Del` over
#
# systemd ships `ctrl-alt-del.target` as an alias of `reboot.target`, and the
# kernel starts it when the key is pressed on a console the compositor is not
# holding. A unit of the same name in `/etc/systemd/system` — which is what
# `systemd.targets` writes — replaces the alias.
#
# So one press stops rebooting and opens this instead. That is a real loss and
# it is paid for twice: the reboot is a row INSIDE the console (which asks
# first), and systemd's hard way out is untouched — seven presses in two seconds
# still reboot by force, through `CtrlAltDelBurstAction`, without passing
# through anything of ours. `ctrl-alt-del-is-ours` asserts both halves.
#
# # Why it runs as the person and not as root
#
# Because `Ctrl+Alt+Del` needs no login. A root console one keypress away from
# any locked machine is a bigger hole than the one this file closes. It runs as
# `ronoctua.user`, and everything that needs more goes through polkit — which
# asks for that person's own password and remembers it for the session.
#
# `pkttyagent` is what makes asking possible on a VT (`crates/.../action.rs`).
{
  config,
  lib,
  pkgs,
  # The shell's package, which carries `bin/ronoctua-rescue`: one workspace, one
  # derivation (`nix/rust.nix`). The two ship together by construction.
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.rescue;
  user = config.ronoctua.user;

  # THE MACHINE'S LIFE (`nix/modules/machine.nix`), which this module needs
  # beside it — a test machine imports both: the rebuild is its apply, and the
  # rollback disarms its boot trial. The machine's own flake comes by its one
  # name (`machine-dir-has-one-name`): where the edited `host.nix` goes. The
  # rebuild's flake and attribute travel in `ronoctua.machine.environment`.
  machine = config.ronoctua.machine;
  machineDir = machine.dir;

  # WHERE THE EDITOR LEAVES WHAT IT WROTE.
  #
  # The same path the crate holds in `edit::STAGED`, and `rescue-staging-agrees`
  # is what keeps the two together: a file saved into a directory nobody reads
  # is an edit that silently did nothing.
  staged = "/run/ronoctua-rescue/staged";

  # THE ROLLBACK, without evaluating anything — the machine's own verb
  # (`ronoctua-machine rollback switch|boot`, `crates/ronoctua-machine/src/
  # rollback.rs`).
  #
  # `nixos-rebuild switch --rollback` would work, and it drags the whole
  # evaluation in; the verb runs with no flake, no network and no `nix`
  # evaluation at all — which matters, because the generation one is rolling
  # back FROM may be the reason evaluation is broken.
  #
  # NOT `nix-env --rollback`: updates set their system for the NEXT BOOT, so
  # the generation before the profile's may be one nobody ever booted (two
  # applies before a restart leave two) — on another nixpkgs, perhaps, and
  # with no boot trial behind it. While a system waits, the way back is the
  # system that RUNS, and nothing is switched; otherwise the generation
  # before, passing over the one whose trial failed. AND THE BOOT TRIAL IS
  # DISARMED by the same verb: an update arms one in two EFI variables a
  # rollback does not touch — the one-shot would take the next boot straight
  # back into the system rolled away from —, and the system left is recorded
  # as one that did not boot, so the next update does not switch into it
  # again. The instance (`switch` or `boot`) is the verb's word.
  rollback = "${machine.package}/bin/ronoctua-machine rollback";

  # WHAT INSTALLS WHAT THE EDITOR WROTE — and refuses what does not parse.
  #
  # A `host.nix` with a missing brace is a machine that cannot rebuild, and
  # finding that out at the next boot is finding it out too late. The parse is
  # the cheapest check that catches the whole class, and it costs no evaluation.
  installHost = pkgs.writeShellScript "ronoctua-rescue-host" ''
    set -eu
    test -s ${staged} || { echo "nothing was edited" >&2; exit 1; }
    ${pkgs.nix}/bin/nix-instantiate --parse ${staged} > /dev/null \
      || { echo "the edited file is not valid Nix; nothing was installed" >&2; exit 1; }
    ${pkgs.coreutils}/bin/install -m 0644 ${staged} ${machineDir}/host.nix
    echo "written; a nixos-rebuild switch is still needed"
  '';

  # The units the console may start or restart, and NOTHING else.
  #
  # It mirrors the `Tier::Password` rows of `crates/ronoctua-rescue/src/lib.rs`,
  # and `rescue-polkit-is-narrow` compares the two lists in both directions: a
  # row without its unit fails in front of the person, and a unit left here
  # after its row went away is privilege granted to nobody.
  allowed = [
    "ronoctua-rescue-rollback@switch.service"
    "ronoctua-rescue-rollback@boot.service"
    "ronoctua-rescue-rebuild.service"
    "ronoctua-rescue-host.service"
    "NetworkManager.service"
  ];
in
{
  options.ronoctua.rescue = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The rescue console on Ctrl+Alt+Del.";
    };
    vt = lib.mkOption {
      type = lib.types.int;
      default = 8;
      description = ''
        Which virtual console the rescue takes. The default is 8 because 1..6 belong to the
        gettys and 7 is where the graphical session usually is.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # `/run/ronoctua-rescue` belongs to the person, because the editor runs as
    # the person and the unit that installs the file runs as root. It is the
    # only place the two meet.
    # The GROUP comes from the user's own definition and is not the user's name.
    # Written by hand it was `noctua`, there is no such group, and the only
    # thing that said so was one line in the boot journal — `Failed to resolve
    # group 'noctua': Unknown group`. The directory then did not exist, and the
    # failure would have surfaced much later, as an edit that saved and
    # installed nothing.
    systemd.tmpfiles.rules = [
      "d /run/ronoctua-rescue 0750 ${user} ${config.users.users.${user}.group} -"
    ];

    # ---------------------------------------------------------------
    # THE DOOR
    # ---------------------------------------------------------------
    # WHAT `Ctrl+Alt+Del` STARTS — and NixOS has an option for exactly this.
    #
    # `systemd.ctrlAltDelUnit` is what `nixos/lib/systemd-lib.nix` links as
    # `ctrl-alt-del.target`; its default is `reboot.target`, which is the reboot
    # we are taking the key away from.
    #
    # Two other roads were tried first, and both are worth writing down because
    # both look right:
    #
    #   - `systemd.targets.ctrl-alt-del` fails the build with `ln: failed to
    #     create symbolic link … File exists`. That is the GOOD outcome — the
    #     alternative would be a silent race between two files for one name;
    #   - adding `suppressedSystemUnits` on top removes BOTH: it filters the
    #     upstream list AND `systemd.units`. The name then exists nowhere, the
    #     key starts nothing, and the only thing that says so is a booted
    #     machine — `systemctl show -p Wants` came back empty in the VM.
    #
    # A drop-in is not an option either: a drop-in on an ALIAS applies to what
    # it points at, so it would be configuring `reboot.target` itself.
    systemd.ctrlAltDelUnit = "ronoctua-rescue.target";

    systemd.targets.ronoctua-rescue = {
      description = "Ctrl+Alt+Del — opens the rescue console";
      # A target and not the service directly: `ctrl-alt-del.target` is a
      # symlink to whatever this names, and systemd expects a target's name to
      # end in `.target`.
      wants = [ "ronoctua-rescue.service" ];
      unitConfig.AllowIsolate = false;
    };

    systemd.services.ronoctua-rescue = {
      description = "Rescue console on virtual console ${toString cfg.vt}";
      # THE MACHINE'S THEME, as the session's programs get it (`scroll.nix`): a
      # system unit inherits none of the session's environment, and without
      # it the console painted the first theme of the chooser's order. What
      # the person chose (`theme.json`) is not read on a VT that may have no
      # session at all.
      environment.RONOCTUA_THEME = config.ronoctua.theme;
      # `Type = "idle"` and not `simple`: with jobs still running, a TTY service
      # that starts at once writes over whatever else is printing on the console.
      serviceConfig = {
        Type = "idle";
        User = user;
        # The console as a program of its own (`rust.rescue`, the palettes'
        # wrapper over its unit): the footer's bundle would put the whole
        # desktop in this unit's closure (`shell-bundle-is-only-the-shell`).
        ExecStart = "${ronoctua.rescue}/bin/ronoctua-rescue";
        # `+` runs as root: switching a VT needs a capability the person does
        # not have, and the rest of the unit stays unprivileged.
        ExecStartPre = "+${pkgs.writeShellScript "ronoctua-rescue-enter" ''
          ${pkgs.kbd}/bin/fgconsole > /run/ronoctua-rescue/came-from 2>/dev/null || true
          ${pkgs.kbd}/bin/chvt ${toString cfg.vt}
        ''}";
        # AND IT GIVES THE SCREEN BACK. Without this, leaving the console with
        # `Esc` leaves the person looking at an empty VT wondering what happened
        # — the desktop is fine, three consoles away.
        ExecStopPost = "+${pkgs.writeShellScript "ronoctua-rescue-leave" ''
          back=$(cat /run/ronoctua-rescue/came-from 2>/dev/null || echo 1)
          ${pkgs.kbd}/bin/chvt "$back" || true
        ''}";
        StandardInput = "tty-force";
        StandardOutput = "tty";
        StandardError = "journal";
        TTYPath = "/dev/tty${toString cfg.vt}";
        TTYReset = true;
        TTYVHangup = true;
        TTYVTDisallocate = true;
        # It does not come back on its own. A rescue console that restarts in a
        # loop is a machine whose VT cannot be left.
        Restart = "no";
      };
    };

    # ---------------------------------------------------------------
    # WHAT IT MAY ASK FOR
    # ---------------------------------------------------------------
    systemd.services."ronoctua-rescue-rollback@" = {
      description = "Goes back to the system that runs, or the previous generation (%i)";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${rollback} %i";
      };
    };

    # THE REBUILD IS THE MACHINE'S APPLY (`ronoctua-machine apply`): built
    # first, the profile and the switch only then, the boot trial armed, and
    # a build that fails puts back the lock the running system was built
    # from. The machine directory and the attribute come in the machine's
    # one table (`ronoctua.machine.environment`), which
    # `machine-flake-target-exists` reads off this unit. And the update's own
    # lock first, as the software centre's apply takes it: a rebuild beside a
    # running update would build the folder while the update rewrites its
    # lock — busy is the answer (exit 75), said in the console.
    systemd.services.ronoctua-rescue-rebuild = {
      description = "Applies the machine's own flake on this machine";
      environment = machine.environment;
      serviceConfig = {
        Type = "oneshot";
        # A build compiles; the default start timeout is 90 seconds.
        TimeoutStartSec = "infinity";
        ExecStart = machine.applyCommand;
        RuntimeDirectory = baseNameOf machine.runDir;
        RuntimeDirectoryPreserve = "yes";
      };
      # The switch it runs must not stop or restart the unit running it.
      restartIfChanged = false;
      stopIfChanged = false;
    };

    systemd.services.ronoctua-rescue-host = {
      description = "Installs the host.nix edited in the rescue console";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${installHost}";
      };
    };

    # THE NARROW RULE, in the shape `nix/modules/memory.nix` already uses.
    #
    # `auth_self_keep` and not `yes`: this is the line the owner drew — seeing is
    # free, acting proves who you are, and the proof is remembered for the
    # session so a rescue is not a password typed nine times.
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        // THE DISJUNCTION IS PARENTHESISED, and it was not for one commit.
        // `&&` binds tighter than `||` in JavaScript, so without these
        // brackets the condition parses as
        // `(id && user && unit == first) || unit == second || …` — every unit
        // after the first granted for ANY action id and ANY subject,
        // `manage-unit-files` included. The logind block below has always had
        // its brackets; this one was generated by a fold and lost them.
        if (
          action.id == "org.freedesktop.systemd1.manage-units" &&
          subject.user == "${user}" &&
          (${lib.concatMapStringsSep " " (u: ''action.lookup("unit") == "${u}" ||'') allowed} false)
        ) {
          return polkit.Result.AUTH_SELF_KEEP;
        }
        // Leaving the machine, which logind owns rather than the unit manager.
        if (
          subject.user == "${user}" &&
          (action.id == "org.freedesktop.login1.power-off" ||
           action.id == "org.freedesktop.login1.reboot" ||
           action.id == "org.freedesktop.login1.power-off-multiple-sessions" ||
           action.id == "org.freedesktop.login1.reboot-multiple-sessions")
        ) {
          return polkit.Result.AUTH_SELF_KEEP;
        }
      });
    '';
  };
}
