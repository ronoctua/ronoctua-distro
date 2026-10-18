# Memory policy — the machine must not freeze when memory runs out.
#
# # The symptom this exists for
#
# It is not the killed process. It is the machine going away for minutes while
# the kernel reclaims, with the OOM killer arriving after the point where
# anything could still be done about it. What removes that is a userspace killer
# acting on PRESSURE — on stall — rather than on how many bytes are left.
#
# # Three things were true here before this file, and each is worse than nothing
#
# 1. **`systemd-oomd` was already running and watching nothing.** `systemd.oomd.enable`
#    is `true` by default in this nixpkgs and the three slice switches are `false`,
#    so the daemon's monitored list starts EMPTY. Writing `enable = true` "fixes"
#    it and changes nothing at all — the switch that arms it is
#    `ManagedOOMMemoryPressure=kill` on a slice, and nothing else.
# 2. **`user@.service` ships `OOMScoreAdjust=100`.** Everything in the session
#    inherits it, so the kernel preferred killing the compositor and the footer
#    over any system daemon.
# 3. **The compositor, the footer and every foreign application shared one
#    slice**, with no reservation for the first two and no ceiling on the third.
#
# # Why the desktop survives BY CONSTRUCTION
#
# `session.slice` (the compositor and the footer) is a SIBLING of `app.slice` (the
# applications), not a descendant. `systemd-oomd` only ever considers descendants
# of a monitored cgroup, so with `app.slice` monitored and nothing else in the
# session, the compositor and the footer are not candidates. No xattr, no
# `ManagedOOMPreference`, no ranking heuristic — they are outside the set.
#
# That is why `enableRootSlice` and `enableUserSlices` are `false` BY DECISION
# and not by omission. See the comment on them below; either one turns this
# guarantee into a coin toss.
#
# # Why every number is a percentage
#
# `systemd.resource-control(5)`: a percentage in `MemoryMin=`, `MemoryLow=`,
# `MemoryHigh=` or `MemoryMax=` is "taken relative to the installed physical
# memory on the system", and systemd resolves it WHEN IT LOADS THE UNIT — on the
# machine. The flake stores the string `3%`; the same closure gives 245 MiB on a
# machine with 8 GiB and 491 MiB on one with 16. Nothing here computes anything,
# and the policy travels to any machine without a second thought.
#
# **`ManagedOOMMemoryPressureLimit=` is NOT that kind of percentage.** It is a
# fraction of TIME — "the fraction of time in a 10 second window in which all
# tasks in the control group were delayed" (`oomd.conf(5)`). `MemoryHigh=70%` and
# a pressure limit of `50%` are unrelated quantities that happen to share a sign.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.memory;

  # THE MACHINE'S OWN FLAKE, by its one name (`machine-dir-has-one-name`).
  # `or` because a test machine imports this module without the machine's
  # options (`tests/oom-vm.nix`).
  machineDir = config.ronoctua.machine.dir or (import ../machine.nix).machineDir;

  # THE TWO PROFILES, and the numbers live HERE and nowhere else.
  #
  # `LEADER s m` can loosen the policy for a session, and to do that it needs
  # the numbers. It does not compute them: the table below is written to `/etc`
  # and the panel reads it — invariant 5 says no Nix logic gets rewritten in
  # Rust, and two copies of `70%` would be two copies to keep in step.
  profiles = {
    strict = {
      ceiling = cfg.appCeiling;
      pressure = cfg.appPressure;
      duration = cfg.appPressureFor;
    };
    tolerant = {
      ceiling = cfg.tolerantCeiling;
      pressure = cfg.tolerantPressure;
      duration = cfg.tolerantPressureFor;
    };
  };
  inForce = profiles.${cfg.profile};

  # WHAT WRITES THE PERMANENT OPTION.
  #
  # The machine's `host.nix` belongs to root and the session does not. So the panel
  # asks a oneshot to do it, and a NARROW polkit rule lets this user start those
  # two instances and nothing else — the same shape as `keyboard-restore`.
  #
  # It edits ONE line and refuses anything it does not recognise. A script that
  # guesses where to put a line in somebody's Nix file is a script that one day
  # writes into the middle of a string.
  profileWriter = pkgs.writeShellApplication {
    name = "ronoctua-memory-profile";
    runtimeInputs = [ pkgs.coreutils ];
    text = ''
      mode="$1"
      file=${machineDir}/host.nix
      case "$mode" in
        strict|tolerant) ;;
        *) echo "ERROR: unknown profile: $mode" >&2; exit 1 ;;
      esac
      if [ ! -f "$file" ]; then
        echo "ERROR: $file does not exist; this machine has no file of its own." >&2
        exit 1
      fi

      # The old line goes, whatever it said; then the new one, right before the
      # closing brace. Refusing when the last line is not that brace is the
      # point: this edits a file somebody may have written by hand.
      without=$(mktemp)
      grep -v '^[[:space:]]*ronoctua\.memory\.profile' "$file" > "$without"
      if [ "$(tail -n 1 "$without")" != "}" ]; then
        echo "ERROR: I do not recognise the shape of $file; I will not edit blind." >&2
        rm -f "$without"
        exit 1
      fi
      new=$(mktemp)
      head -n -1 "$without" > "$new"
      printf '  ronoctua.memory.profile = "%s";\n}\n' "$mode" >> "$new"
      install -m 0644 "$new" "$file"
      rm -f "$without" "$new"
      echo "written: ronoctua.memory.profile = \"$mode\" (a nixos-rebuild switch is still needed)"
    '';
  };

  # THE HOUSEKEEPING, and it is housekeeping — never configuration.
  #
  # None of the three changes what the machine IS: they remove what nothing
  # points at any more. The flake stays the truth, which is why these may exist
  # at all under invariant 6.
  sweeper = pkgs.writeShellApplication {
    name = "ronoctua-memory-sweep";
    runtimeInputs = [
      pkgs.nix
      pkgs.systemd
    ];
    text = ''
      case "$1" in
        # Garbage only: what no generation points at. Generations stay, so
        # rolling back is still possible afterwards.
        store) nix-collect-garbage ;;
        # And this one DOES remove old generations — which is why it is a
        # separate key and a separate confirmation. The SYSTEM's only, by the
        # one trim `nix/modules/store.nix` owns: the newest ones, the running
        # and the booted stay, and no other profile is touched. It was
        # `nix-collect-garbage --delete-older-than 30d`, which as root deleted
        # every profile's history, the person's own included. Then the garbage
        # that frees what the trimmed generations held.
        generations) systemctl start ${config.ronoctua.store.generationsUnit} && nix-collect-garbage ;;
        journal) journalctl --vacuum-size=256M ;;
        *) echo "ERROR: no such sweep: $1" >&2; exit 1 ;;
      esac
    '';
  };

  # The reservation that keeps the desktop's pages resident, applied at every
  # level of the chain. See `theChain` below for why one level is not enough.
  protection = {
    MemoryMin = cfg.desktopMin;
    MemoryLow = cfg.desktopLow;
  };

  # `asDropin` and not a full unit file: `user.slice` is not among the units
  # nixpkgs copies from systemd, so writing it whole would drop upstream's
  # `Before=slices.target`; the user's `session.slice` ships `CPUWeight=100`,
  # which a full file would discard in silence.
  chainLink = {
    overrideStrategy = "asDropin";
    sliceConfig = protection;
  };
in
{
  # THE STORE'S MODULE, because the panel's `generations` starts its unit and
  # names it through its handle. Imported here too so a machine (or a test)
  # that takes this module alone still has what it starts; the module system
  # loads a file once however many times it is imported.
  imports = [ ./store.nix ];

  options.ronoctua.memory = {
    desktopMin = lib.mkOption {
      type = lib.types.str;
      default = "3%";
      description = ''
        `MemoryMin` for the compositor and the footer — memory the kernel may
        never reclaim from them. A percentage, resolved against this machine's
        physical memory when systemd loads the unit.
      '';
    };
    desktopLow = lib.mkOption {
      type = lib.types.str;
      default = "8%";
      description = ''
        `MemoryLow` for the same: reclaimed only when there is nothing else
        left. Above `desktopMin` on purpose — the first is a floor, this is a
        preference.
      '';
    };
    appCeiling = lib.mkOption {
      type = lib.types.str;
      default = "70%";
      description = ''
        `MemoryHigh` on `app.slice`. NOT a kill: past it the kernel throttles
        the applications and reclaims from them harder. That throttling is what
        raises the pressure that makes `systemd-oomd` act, so this value and
        `appPressure` are two halves of one mechanism.
      '';
    };
    appPressure = lib.mkOption {
      type = lib.types.str;
      default = "50%";
      description = ''
        `ManagedOOMMemoryPressureLimit` on `app.slice`. A fraction of TIME
        STALLED, not of memory: how much of a ten-second window every task in
        the slice spent waiting on memory.
      '';
    };
    appPressureFor = lib.mkOption {
      type = lib.types.str;
      default = "15s";
      description = ''
        How long the pressure has to stay above the limit before anything is
        killed. Without it a momentary spike — opening a large file — would be
        enough.
      '';
    };
    systemPressure = lib.mkOption {
      type = lib.types.str;
      default = "80%";
      description = "The same, for `system.slice`: daemons and builds.";
    };
    profile = lib.mkOption {
      type = lib.types.enum [
        "strict"
        "tolerant"
      ];
      default = "strict";
      description = ''
        Which of the two policies this machine boots with.

        `strict` is the one the whole file argues for. `tolerant` gives the
        applications more room and more patience before anything is closed —
        for a machine that is about to run something heavy on purpose. The
        DESKTOP's reservation does not change in either: it is what keeps the
        machine answering, and loosening it would be turning the protection
        off rather than loosening it.

        `LEADER s m` can switch for the current session without touching this;
        what is written here is what comes back after a reboot.
      '';
    };
    tolerantCeiling = lib.mkOption {
      type = lib.types.str;
      default = "90%";
      description = ''
        `MemoryHigh` on `app.slice` under the tolerant profile. Throttling
        starts later, so whoever legitimately needs a lot gets it before the
        kernel starts squeezing.
      '';
    };
    tolerantPressure = lib.mkOption {
      type = lib.types.str;
      default = "70%";
      description = ''
        The tolerant `ManagedOOMMemoryPressureLimit`. 70% of a ten-second
        window stalled is still a machine clearly in trouble; 50% also catches
        a compile that merely touched swap.
      '';
    };
    tolerantPressureFor = lib.mkOption {
      type = lib.types.str;
      default = "60s";
      description = ''
        And for how long. A heavy job stalls for tens of seconds legitimately;
        a minute is a decision rather than a fright. The cost is said out loud
        on screen: in this mode the machine may be slow for up to a minute
        before anything is closed.
      '';
    };
    buildCeiling = lib.mkOption {
      type = lib.types.str;
      default = "50%";
      description = ''
        `MemoryHigh` on `nix-daemon`. A `nixos-rebuild` on this machine compiles
        the whole workspace, and `nix.settings.max-jobs` takes no percentage —
        the cgroup is the only lever that adapts to the machine.
      '';
    };
  };

  config = {
    # -----------------------------------------------------------------
    # 1. The actor
    # -----------------------------------------------------------------
    systemd.oomd = {
      # Already the default in this nixpkgs. Restated so this file says what it
      # depends on rather than inheriting it silently — if the default ever
      # flips, the policy below would arm nothing and nothing would say so.
      enable = true;

      # Daemons and builds: killable.
      enableSystemSlice = true;

      # BOTH OF THESE ARE FALSE BY DECISION.
      #
      # `enableRootSlice` monitors `-.slice`, whose candidate set includes
      # `scroll.service` and `ronoctua-footer.service`. And their
      # `ManagedOOMPreference=avoid` would be IGNORED there:
      # `systemd.resource-control(5)` only honours that attribute when the
      # unit's cgroup owner and the monitored ancestor's owner are the same, and
      # `-.slice` belongs to root while the session belongs to the user.
      #
      # `enableUserSlices` installs `/etc/systemd/user/slice.d/overrides.conf`, a
      # drop-in that applies to EVERY slice of that type — including
      # `session.slice`, whose only two candidates are the compositor and the
      # footer.
      #
      # Either one makes the desktop a candidate. The check
      # `oom-policy-shape` refuses both.
      enableRootSlice = false;
      enableUserSlices = false;
    };

    systemd.slices.system.sliceConfig.ManagedOOMMemoryPressureLimit = cfg.systemPressure;

    # -----------------------------------------------------------------
    # 2. What it may kill inside the session
    # -----------------------------------------------------------------
    # `app.slice` is where `systemd-run --user --scope` puts every foreign
    # buffer (see `crates/ronoctua-shell-kit/src/foreign.rs`, which now names it
    # explicitly). Its candidates are the leaves below it — one scope per
    # application — so what dies is an application, whole, and never a piece of
    # one.
    systemd.user.slices.app = {
      overrideStrategy = "asDropin";
      sliceConfig = {
        ManagedOOMMemoryPressure = "kill";
        # THE PROFILE'S NUMBERS, and not the options directly: `strict` reads
        # exactly the same as before, and `tolerant` is the same three keys
        # with the other three values. One place decides which.
        ManagedOOMMemoryPressureLimit = inForce.pressure;
        ManagedOOMMemoryPressureDurationSec = inForce.duration;
        MemoryHigh = inForce.ceiling;
      };
    };

    # THE TABLE THE PANEL READS.
    #
    # `LEADER s m` switches profile for a session, and to do that it needs the
    # numbers systemd takes — `70%`, `60s`, verbatim. They cross as TEXT and are
    # applied as text: the panel never parses a percentage, never recomputes
    # one, and cannot disagree with this file (invariant 5).
    #
    # `declared` is what survives a reboot, which is the one thing a transient
    # change cannot tell you about itself.
    environment.etc."ronoctua/memory-profiles.conf".text = ''
      # GENERATED by nix/modules/memory.nix. Do not edit by hand.
      declared ${cfg.profile}
      strict ceiling=${profiles.strict.ceiling} pressure=${profiles.strict.pressure} duration=${profiles.strict.duration}
      tolerant ceiling=${profiles.tolerant.ceiling} pressure=${profiles.tolerant.pressure} duration=${profiles.tolerant.duration}
    '';

    # `ManagedOOMSwap` is deliberately absent, here and everywhere. Its trigger
    # is a system-wide swap fraction, and with a 16 GiB hibernation swapfile plus
    # zram it would fire long after the machine had already become unusable —
    # which is the failure this file exists to remove. Stalling is the signal,
    # because thrashing IS stalling.

    # -----------------------------------------------------------------
    # 3. What it may never reach — and the chain that makes that true
    # -----------------------------------------------------------------
    # `memory.min` and `memory.low` are capped by EVERY ancestor: cgroup-v2's
    # effective floor is the minimum along the path. Setting this on
    # `session.slice` alone gives an effective value of ZERO, and it would look
    # configured. Four levels, or nothing.
    systemd.slices.user = chainLink; # user.slice
    systemd.slices."user-" = chainLink; # reaches user-1000.slice by dash truncation
    systemd.services."user@" = {
      serviceConfig = protection; # user@1000.service
    };
    systemd.user.slices.session = {
      overrideStrategy = "asDropin";
      sliceConfig = protection;
    };

    # -----------------------------------------------------------------
    # 4. Builds do not take the desktop down
    # -----------------------------------------------------------------
    # A `nixos-rebuild` here compiles the workspace, and `nix.settings.max-jobs`
    # accepts no percentage — the cgroup is the lever that adapts by itself.
    # `idle` scheduling on top, so a build yields the CPU and the disk to
    # whatever the person is actually doing.
    nix.daemonCPUSchedPolicy = "idle";
    nix.daemonIOSchedClass = "idle";
    systemd.services.nix-daemon.serviceConfig.MemoryHigh = cfg.buildCeiling;

    # -----------------------------------------------------------------
    # 5. What must not be the victim inside system.slice
    # -----------------------------------------------------------------
    # These attributes ARE honoured: root-owned cgroups under a root-owned
    # monitor, which is the ownership rule quoted above.
    #
    # The journal is first because killing it destroys the record of why
    # anything else was killed. NetworkManager is second because a machine that
    # comes back without a network is a machine nobody can fix remotely.
    systemd.services.systemd-journald.serviceConfig.ManagedOOMPreference = "omit";
    systemd.services.NetworkManager.serviceConfig.ManagedOOMPreference = "omit";

    # D-BUS IS DELIBERATELY NOT ON THAT LIST. nixpkgs defaults to
    # `dbus-broker`, so the unit is `dbus-broker.service`; writing
    # `systemd.services.dbus.*` would create a PHANTOM unit that configures
    # nothing and reads as done. It is the same class as the switches that did
    # not switch, recorded in `AGENTS.md`.

    # -----------------------------------------------------------------
    # 6. Swap that does not thrash the disk
    # -----------------------------------------------------------------
    # Compressed swap in RAM: the first place a cold page goes, at memory speed
    # instead of disk speed. It is what turns "the machine stopped" into "the
    # machine got slower".
    #
    # IT DOES NOT ENDANGER HIBERNATION. systemd's hibernation-location picker
    # skips zram outright ("Swap partition '%s' is a zram device, ignoring"), so
    # a higher priority here does not steal the target: the image still goes to
    # the btrfs swapfile, which `nix/disko/default.nix` sizes at >= RAM. And the
    # compressed pages live IN RAM, so they cannot make the image outgrow it.
    #
    # The priority is written out rather than left to the default so that the
    # order is a statement instead of an accident: zram first, the file as
    # overflow and as the hibernation store.
    zramSwap = {
      enable = true;
      algorithm = "zstd";
      memoryPercent = 50;
      priority = 100;
    };

    # -----------------------------------------------------------------
    # 7. What the panel may ask for — and only that
    # -----------------------------------------------------------------
    # Two templates, five instances between them, and a polkit rule that names
    # every one. `manage-units` opened to the `wheel` group would let anything
    # in the session stop any service, which is a much bigger thing than "write
    # one line" and "collect garbage".
    systemd.services."ronoctua-memory-profile@" = {
      description = "Writes ronoctua.memory.profile = %i into host.nix";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${profileWriter}/bin/ronoctua-memory-profile %i";
      };
    };

    systemd.services."ronoctua-memory-sweep@" = {
      description = "Disk sweep: %i";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${sweeper}/bin/ronoctua-memory-sweep %i";
        # A garbage collection competes with everything for disk. `idle` on both
        # so it yields to whatever the person is actually doing — the same
        # reasoning as the `nix-daemon`'s scheduling above.
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
      };
    };

    # WHO MAY START THOSE FIVE, AND ONLY THOSE.
    #
    # The user comes from `config.ronoctua.user` (invariant 16), never a
    # literal. The verb is `start` and nothing else: this rule cannot stop a
    # service, cannot restart one, and cannot name a unit that is not on the
    # list — a typo in an instance name is a refusal, not a wider door.
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        var allowed = [
          "ronoctua-memory-profile@strict.service",
          "ronoctua-memory-profile@tolerant.service",
          "ronoctua-memory-sweep@store.service",
          "ronoctua-memory-sweep@generations.service",
          "ronoctua-memory-sweep@journal.service"
        ];
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            action.lookup("verb") == "start" &&
            allowed.indexOf(action.lookup("unit")) >= 0 &&
            subject.user == "${config.ronoctua.user}") {
          return polkit.Result.YES;
        }
      });
    '';
  };
}
