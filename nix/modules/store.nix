# The Nix store looks after itself.
#
# # What the owner asked for
#
# That the distro handles the disk "sem precisar de intervenções minhas", on
# every machine that installs it and not only on his; and that nothing may
# remove what the person's Nix relies on — with ONE exception he granted by
# name: a count limit on the SYSTEM's generations that always keeps the
# current, the running and the booted one.
#
# # The four things this module does
#
# 1. **A floor.** At boot a unit measures the filesystem under `/nix/store` and
#    writes Nix's own `min-free` and `max-free` into `/run/ronoctua-store`, as a
#    percentage of THAT disk, capped at a size (`assets/cache.nix`). Every build
#    and every download then collects garbage by itself when free space runs
#    short. Only garbage: Nix's automatic collection never deletes a generation
#    or anything a root reaches.
# 2. **The system's generations, by count.** `ronoctua-system-generations`
#    keeps the newest N, the profile's current one, the running system, the
#    booted system, the one the last switch went away from, and every one the
#    boot menu lists — each of them also when it is a SPECIALISATION of a
#    generation that was booted or switched to. It deletes the other generation
#    LINKS of the system profile and of no other profile. The garbage
#    collection that follows is what frees what they held.
# 3. **The boot menu fits inside the count.** Every loader lists at most N
#    generations, so no boot entry points at a generation the count would
#    delete — and nothing here ever has to rewrite the boot menu. A loader this
#    does not know how to cap refuses evaluation. And where an install of the
#    loader failed and the menu fell behind the profile, the trim reads the
#    menu and keeps what it lists.
# 4. **Build logs age out** after `buildLogDays`, through the `systemd-tmpfiles`
#    clean that already runs every day. No clock is armed here.
#
# # What is gone, and why
#
# The weekly `nix.gc` with `--delete-older-than 30d` (it lived in `base.nix`).
# Run as root, that flag deletes the old generations of EVERY profile under
# `/nix/var/nix/profiles`, `per-user` included — the person's own Nix history.
# That was the one standing break of the guarantee. The collection itself is
# not lost: the update's `store-collect-garbage` step runs every week, and the
# floor above covers the days in between. A timer that duplicated both would be
# a clock doing an event's job.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.store;

  # THE NUMBERS ARE THE CATALOGUE'S. Floor and goal have no option on purpose:
  # the brain reads the same two from the generated Rust table, and a machine
  # that overrode one side only would have two floors.
  declared = import ../../assets/cache.nix;

  # WHERE THE FLOOR LIVES. `/run` because it is measured at every boot — a disk
  # that was resized is a disk with another floor — and never written by hand.
  floorDir = "ronoctua-store";
  floorFile = "/run/${floorDir}/free.conf";

  # THE ONE PROFILE THAT MAY LOSE GENERATIONS, and the one place it is written.
  systemProfile = "/nix/var/nix/profiles/system";

  generationsUnit = "ronoctua-system-generations.service";

  # THE SAME TRIM AT EVERY ACTIVATION — boot and every switch — which may
  # not fail the switch (see the unit below).
  activationUnit = "ronoctua-system-generations-at-activation";

  # WHERE THE LAST SWITCH WROTE WHAT IT WENT AWAY FROM, and the one place
  # it is spelled: the activation writes it, the trim reads it.
  previousDir = "/run/ronoctua-generations";
  previousSystem = "${previousDir}/previous-system";

  # THE FLOOR'S WRITER.
  #
  # Written 0644 and by a rename, and both matter more than they look: Nix
  # SKIPS an `!include` it cannot read, without a word (`configuration.cc`
  # catches the read error and ignores it). A file left 0600 by `mktemp`, or
  # half written when a client read it, would not be an error anywhere — it
  # would be a machine with no floor.
  floorWriter = pkgs.writeShellApplication {
    name = "ronoctua-store-floor";
    runtimeInputs = [ pkgs.coreutils ];
    text = ''
      store="" floor="" goal="" floor_cap="" goal_cap="" into=""
      while [ $# -gt 0 ]; do
        case "$1" in
          --store) store=$2; shift 2 ;;
          --floor-percent) floor=$2; shift 2 ;;
          --goal-percent) goal=$2; shift 2 ;;
          --floor-cap-gib) floor_cap=$2; shift 2 ;;
          --goal-cap-gib) goal_cap=$2; shift 2 ;;
          --into) into=$2; shift 2 ;;
          *) echo "ERROR: unknown argument: $1" >&2; exit 2 ;;
        esac
      done
      for pair in "store:$store" "floor-percent:$floor" "goal-percent:$goal" \
                  "floor-cap-gib:$floor_cap" "goal-cap-gib:$goal_cap" "into:$into"; do
        if [ -z "''${pair#*:}" ]; then
          echo "ERROR: --''${pair%%:*} is required" >&2
          exit 2
        fi
      done
      whole='^[1-9][0-9]?$'
      if ! [[ $floor =~ $whole && $goal =~ $whole ]] || [ "$floor" -ge "$goal" ]; then
        echo "ERROR: the floor ($floor) and the goal ($goal) must be percentages from 1 to 99, the floor below the goal" >&2
        exit 2
      fi
      size='^[1-9][0-9]{0,5}$'
      if ! [[ $floor_cap =~ $size && $goal_cap =~ $size ]] || [ "$floor_cap" -ge "$goal_cap" ]; then
        echo "ERROR: the caps ($floor_cap and $goal_cap GiB) must be whole sizes, the floor's below the goal's" >&2
        exit 2
      fi

      # `%S` is the fundamental block size and `%b` the total blocks in it:
      # their product is the size of the filesystem, which is what the two
      # percentages are OF.
      sizes=$(stat --file-system --format='%S %b' -- "$store")
      read -r block blocks <<< "$sizes"
      total=$((block * blocks))
      if [ "$total" -le 0 ]; then
        echo "ERROR: $store reports a filesystem of $total bytes" >&2
        exit 1
      fi
      # A SHARE, AND NEVER MORE THAN A SIZE: ten per cent of a disk that is
      # mostly the person's own files is not a floor, it is a collection at
      # every build (`assets/cache.nix`).
      min=$((total * floor / 100))
      max=$((total * goal / 100))
      gib=$((1024 * 1024 * 1024))
      if [ "$min" -gt $((floor_cap * gib)) ]; then min=$((floor_cap * gib)); fi
      if [ "$max" -gt $((goal_cap * gib)) ]; then max=$((goal_cap * gib)); fi

      dir=$(dirname -- "$into")
      draft=$(mktemp --tmpdir="$dir" .free.conf.XXXXXX)
      {
        echo "# Written at boot by ronoctua-store-floor (nix/modules/store.nix). Do not edit:"
        echo "# $floor% and $goal% of the $total bytes of the filesystem holding $store,"
        echo "# at most $floor_cap GiB and $goal_cap GiB."
        echo "min-free = $min"
        echo "max-free = $max"
      } > "$draft"
      chmod 0644 "$draft"
      mv -f -- "$draft" "$into"
      echo "floor: min-free = $min bytes ($floor%, at most $floor_cap GiB), max-free = $max bytes ($goal%, at most $goal_cap GiB) of $total bytes under $store"
    '';
  };

  floorCommand = lib.concatStringsSep " " [
    "${floorWriter}/bin/ronoctua-store-floor"
    "--store /nix/store"
    "--floor-percent ${toString declared.diskFloorPercent}"
    "--goal-percent ${toString declared.diskGoalPercent}"
    "--floor-cap-gib ${toString declared.diskFloorCapGiB}"
    "--goal-cap-gib ${toString declared.diskGoalCapGiB}"
    "--into ${floorFile}"
  ];

  # THE TRIM, and everything it refuses.
  #
  # It rehearses unless told `--execute`, and it never runs
  # `switch-to-configuration`: that takes `/run/nixos/switch-to-configuration.lock`
  # without waiting, so an automatic unit that ran it would kill a person's
  # `nixos-rebuild` in the middle. It has no need to — the boot menu lists at
  # most N generations (the assertions below), and those are always among the
  # kept ones.
  #
  # WHAT IT KEEPS, and each one is a way back:
  #
  # - the newest N, and the profile's current one;
  # - the RUNNING system and the BOOTED one — each compared with a generation
  #   and with every specialisation of it, since booting or switching to a
  #   specialisation leaves `/run/booted-system` pointing at the
  #   specialisation's own toplevel, which no `system-N-link` resolves to;
  # - the system the last switch went AWAY from (`/run/ronoctua-generations/
  #   previous-system`, written by the activation below). A person who rolled
  #   back to generation 3 because 4 to 8 are broken, and whose weekly update
  #   then built 9, would otherwise lose 3 in the very switch that left it;
  # - every generation the BOOT MENU lists (`--menu`, each boot mount): by
  #   the generation number in an entry's file name (`nixos-generation-7.conf`,
  #   a lanzaboote `nixos-generation-7-….efi`) or by its toplevel's store path
  #   in a menu's text (grub, extlinux, limine, rEFInd). Normally the menu is
  #   inside the newest N; when an install of the loader failed while the
  #   profile moved on, it is not, and its entries must still boot.
  #
  # The profile is an argument so it can be proved on a fixture, and the
  # refusals are what keep that argument from reaching anybody else's
  # history: the name has to be `system`, and nothing under `per-user` or
  # `system-profiles` is accepted, whatever it is called.
  generationsProgram = pkgs.writeShellApplication {
    name = "ronoctua-system-generations";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.findutils
      pkgs.gnugrep
    ];
    text = ''
      profile=${systemProfile} running=/run/current-system booted=/run/booted-system
      previous=${previousSystem}
      keep="" execute=0 menus=()
      while [ $# -gt 0 ]; do
        case "$1" in
          --profile) profile=$2; shift 2 ;;
          --keep) keep=$2; shift 2 ;;
          --running) running=$2; shift 2 ;;
          --booted) booted=$2; shift 2 ;;
          --previous) previous=$2; shift 2 ;;
          --menu) menus+=("$2"); shift 2 ;;
          --execute) execute=1; shift ;;
          *) echo "ERROR: unknown argument: $1" >&2; exit 2 ;;
        esac
      done
      if ! [[ $keep =~ ^[0-9]+$ ]] || [ "$keep" -lt 2 ]; then
        echo "ERROR: --keep must be a count of at least 2 (the rescue rollback needs the previous generation), not '$keep'" >&2
        exit 2
      fi
      case "$profile" in
        */per-user/* | */system-profiles/*)
          echo "ERROR: $profile is not the system profile; only the system's generations are trimmed here" >&2
          exit 2 ;;
      esac
      if [ "$(basename -- "$profile")" != system ]; then
        echo "ERROR: $profile is not the system profile; only the system's generations are trimmed here" >&2
        exit 2
      fi
      if [ ! -L "$profile" ]; then
        echo "$profile does not exist; nothing to trim"
        exit 0
      fi

      dir=$(dirname -- "$profile")
      current=$(readlink -- "$profile")
      if ! [[ $current =~ ^system-([0-9]+)-link$ ]]; then
        echo "ERROR: $profile points at '$current', which is not one of its generations; nothing was touched" >&2
        exit 1
      fi
      current=''${BASH_REMATCH[1]}
      running_to=$(readlink -f -- "$running" || true)
      booted_to=$(readlink -f -- "$booted" || true)
      previous_to=""
      if [ -L "$previous" ]; then
        previous_to=$(readlink -f -- "$previous" || true)
      fi

      shopt -s nullglob

      # WHAT THE BOOT MENU LISTS. A file that cannot be read says nothing, and
      # a menu that is not there lists nothing: the count and the rest still
      # hold, as they did before this looked.
      menu_numbers=() menu_paths=()
      for place in "''${menus[@]}"; do
        if [ ! -d "$place" ]; then
          continue
        fi
        while IFS= read -r -d "" entry; do
          name=$(basename -- "$entry")
          if [[ $name =~ nixos-generation-([0-9]+) ]]; then
            menu_numbers+=("''${BASH_REMATCH[1]}")
          fi
          case "$name" in
            *.conf | *.cfg)
              while IFS= read -r named; do
                menu_paths+=("$named")
              done < <(grep -oE '/nix/store/[0-9a-z]{32}-[^/[:space:]"]+' -- "$entry" 2> /dev/null || true)
              ;;
          esac
        done < <(find "$place" -xdev -maxdepth 4 -type f -print0 2> /dev/null || true)
      done

      # A GENERATION'S TOPLEVEL, and each of its specialisations'.
      tops_of() {
        readlink -f -- "$1" || true
        local s
        for s in "$1"/specialisation/*; do
          readlink -f -- "$s" || true
        done
      }
      # Whether `$1` is one of the rest — never when it is empty.
      among() {
        local want=$1 x
        shift
        if [ -z "$want" ]; then
          return 1
        fi
        for x in "$@"; do
          if [ "$x" = "$want" ]; then
            return 0
          fi
        done
        return 1
      }
      # Whether the menu lists generation `$1`, whose toplevels follow.
      listed() {
        local n=$1 top
        shift
        if among "$n" "''${menu_numbers[@]}"; then
          return 0
        fi
        for top in "$@"; do
          if among "$top" "''${menu_paths[@]}"; then
            return 0
          fi
        done
        return 1
      }

      numbers=()
      for link in "$dir"/system-*-link; do
        name=$(basename -- "$link")
        if [[ $name =~ ^system-([0-9]+)-link$ ]]; then
          numbers+=("''${BASH_REMATCH[1]}")
        fi
      done
      if [ ''${#numbers[@]} -gt 0 ]; then
        mapfile -t numbers < <(printf '%s\n' "''${numbers[@]}" | sort -n)
      fi
      total=''${#numbers[@]}
      newest_from=$((total > keep ? total - keep : 0))

      doomed=()
      for at in "''${!numbers[@]}"; do
        n=''${numbers[$at]}
        mapfile -t tops < <(tops_of "$dir/system-$n-link")
        why=""
        if [ "$at" -ge "$newest_from" ]; then
          why="one of the newest $keep"
        elif [ "$n" = "$current" ]; then
          why="the profile's current one"
        elif among "$running_to" "''${tops[@]}"; then
          why="the running system"
        elif among "$booted_to" "''${tops[@]}"; then
          why="the booted system"
        elif among "$previous_to" "''${tops[@]}"; then
          why="the system the last switch went away from"
        elif listed "$n" "''${tops[@]}"; then
          why="the boot menu lists it"
        fi
        if [ -n "$why" ]; then
          echo "keeps generation $n: $why"
        else
          doomed+=("$n")
        fi
      done

      if [ ''${#doomed[@]} -eq 0 ]; then
        echo "nothing to trim: all $total system generations are kept"
        exit 0
      fi
      if [ "$execute" -eq 0 ]; then
        echo "would remove system generations ''${doomed[*]} (rehearsal: nothing was touched)"
        exit 0
      fi
      ${config.nix.package}/bin/nix-env --profile "$profile" --delete-generations "''${doomed[@]}"
      echo "removed system generations ''${doomed[*]}; kept $((total - ''${#doomed[@]}))"
    '';
  };

  # EVERY LOADER THIS MACHINE MIGHT BOOT FROM, how many generations its menu
  # lists, and the option that says so. `lanzaboote` only when its module is
  # imported (a host that enables Secure Boot); it defaults to systemd-boot's
  # limit.
  loaders = [
    {
      name = "boot.loader.systemd-boot.configurationLimit";
      on = config.boot.loader.systemd-boot.enable;
      limit = config.boot.loader.systemd-boot.configurationLimit;
    }
    {
      name = "boot.loader.grub.configurationLimit";
      on = config.boot.loader.grub.enable;
      limit = config.boot.loader.grub.configurationLimit;
    }
    {
      name = "boot.loader.generic-extlinux-compatible.configurationLimit";
      on = config.boot.loader.generic-extlinux-compatible.enable;
      limit = config.boot.loader.generic-extlinux-compatible.configurationLimit;
    }
    {
      name = "boot.loader.limine.maxGenerations";
      on = config.boot.loader.limine.enable;
      limit = config.boot.loader.limine.maxGenerations;
    }
    {
      name = "boot.loader.refind.maxGenerations";
      on = config.boot.loader.refind.enable;
      limit = config.boot.loader.refind.maxGenerations;
    }
  ]
  ++ lib.optional lanzaboote {
    name = "boot.lanzaboote.configurationLimit";
    on = config.boot.lanzaboote.enable;
    limit = config.boot.lanzaboote.configurationLimit;
  };

  lanzaboote = config.boot ? lanzaboote;

  # AND THE LOADERS NOBODY CAN CAP: they list every generation there is, so a
  # trim by count would leave entries pointing at deleted ones. A machine on
  # one of these is refused at evaluation rather than trimmed wrong. The
  # external hook is lanzaboote's own when lanzaboote is on — capped above.
  uncapped = [
    {
      name = "boot.loader.generationsDir";
      on = config.boot.loader.generationsDir.enable;
    }
    {
      name = "boot.loader.initScript";
      on = config.boot.loader.initScript.enable;
    }
    {
      name = "boot.loader.external";
      on = config.boot.loader.external.enable && !(lanzaboote && config.boot.lanzaboote.enable);
    }
  ];

  # WHERE A MENU CAN BE: every boot mount any loader here writes to. The trim
  # reads what is there and keeps what it lists; a place that is not there
  # lists nothing.
  menus = lib.unique (
    [
      "/boot"
      config.boot.loader.efi.efiSysMountPoint
    ]
    ++ lib.optional (
      config.boot.loader.systemd-boot.xbootldrMountPoint != null
    ) config.boot.loader.systemd-boot.xbootldrMountPoint
    ++ map (b: b.path) config.boot.loader.grub.mirroredBoots
    ++ map (b: b.path) config.boot.loader.generic-extlinux-compatible.mirroredBoots
  );

  # THE TRIM, as both units run it.
  trimCommand = lib.concatStringsSep " " (
    [
      "${generationsProgram}/bin/ronoctua-system-generations"
      "--profile ${systemProfile}"
      "--keep ${toString cfg.systemGenerations}"
    ]
    ++ map (m: "--menu ${lib.escapeShellArg m}") menus
    ++ [ "--execute" ]
  );

  # WHAT BOTH UNITS SHARE: idle, and never re-run half way through the switch
  # that changed them.
  trimUnit = {
    unitConfig.RequiresMountsFor = "/nix/var/nix/profiles";
    serviceConfig = {
      # No `RemainAfterExit`: `systemctl start` on a unit that stayed active
      # does nothing, and whoever starts these starts them to RUN them.
      Type = "oneshot";
      CPUSchedulingPolicy = "idle";
      IOSchedulingClass = "idle";
    };
    restartIfChanged = false;
    stopIfChanged = false;
  };
in
{
  options.ronoctua.store = {
    systemGenerations = lib.mkOption {
      type = lib.types.ints.positive;
      default = declared.systemGenerations;
      description = ''
        How many of the system's generations stay, counted from the newest. The
        profile's current one, the running system and the booted system always
        stay on top of these, and no other profile is ever touched.

        The default comes from `assets/cache.nix`; the option exists so a
        machine can differ, not so the number can be written twice. Every boot
        loader's menu is capped at this, so no entry outlives its generation.
      '';
    };

    buildLogDays = lib.mkOption {
      type = lib.types.ints.positive;
      default = declared.buildLogDays;
      description = "How long a build log in /nix/var/log/nix/drvs stays, in days.";
    };

    # THE HANDLES, so the memory panel and the checks name the unit and reach
    # the programs from here. The update's table and the shell's `ngc` are
    # plain text and spell the unit again; `nix-gc-keeps-generations` holds
    # both to this one.
    generationsUnit = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      internal = true;
      default = generationsUnit;
      description = "The unit that trims the system's generations.";
    };

    activationUnit = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      internal = true;
      default = "${activationUnit}.service";
      description = "The unit that trims them after every activation, and never fails it.";
    };

    programs = lib.mkOption {
      type = lib.types.attrsOf lib.types.package;
      readOnly = true;
      internal = true;
      default = {
        floor = floorWriter;
        generations = generationsProgram;
      };
      description = "The floor's writer and the generations' trim, already built.";
    };
  };

  config = {
    assertions = [
      {
        assertion = cfg.systemGenerations >= 2;
        message =
          "ronoctua.store.systemGenerations is ${toString cfg.systemGenerations}; it must be at "
          + "least 2, because the rescue console's rollback goes to the previous generation.";
      }
    ]
    # NO MENU ENTRY OUTLIVES ITS GENERATION. A loader's builder lists the
    # newest L generations and removes every other entry; with L <= N those L
    # are always among the ones the trim keeps. Null or 0 means "no limit" to
    # the builders, and a menu with no limit would list generations the trim
    # has already deleted.
    ++ map (loader: {
      assertion =
        !loader.on || (loader.limit != null && loader.limit >= 1 && loader.limit <= cfg.systemGenerations);
      message =
        "${loader.name} is ${builtins.toJSON loader.limit}, and it must be "
        + "between 1 and ronoctua.store.systemGenerations (${toString cfg.systemGenerations}): "
        + "a boot entry beyond that count would point at a generation that is trimmed.";
    }) loaders
    ++ map (loader: {
      assertion = !loader.on;
      message =
        "${loader.name} is on, and it lists every generation with no limit this can set: "
        + "the system's generations are trimmed by count (ronoctua.store.systemGenerations), "
        + "and its entries beyond the count would point at generations that are gone.";
    }) uncapped;

    boot.loader.systemd-boot.configurationLimit = lib.mkDefault cfg.systemGenerations;
    boot.loader.grub.configurationLimit = lib.mkDefault cfg.systemGenerations;
    boot.loader.generic-extlinux-compatible.configurationLimit = lib.mkDefault cfg.systemGenerations;
    boot.loader.limine.maxGenerations = lib.mkDefault cfg.systemGenerations;
    boot.loader.refind.maxGenerations = lib.mkDefault cfg.systemGenerations;

    # -----------------------------------------------------------------
    # 1. The floor
    # -----------------------------------------------------------------
    # LAST in `nix.conf`: nixpkgs writes `nix.settings` first and `extraOptions`
    # after, and a later setting wins. A missing file is no floor and no error —
    # which is also what keeps nixpkgs' own `nix config show` check of this file
    # quiet, since `/run` does not exist in its sandbox.
    nix.extraOptions = "!include ${floorFile}";

    systemd.services.ronoctua-store-floor = {
      description = "Measures the disk under /nix/store and writes Nix's free-space floor";
      # Before the daemon, which reads `nix.conf` once when it starts; and
      # wanted by it, so a daemon started by its socket brings the floor along.
      # NOT before `nix-daemon.socket`: sockets come before `basic.target`,
      # and this unit's default dependencies come after it — a cycle.
      wantedBy = [
        "multi-user.target"
        "nix-daemon.service"
      ];
      before = [ "nix-daemon.service" ];
      unitConfig.RequiresMountsFor = "/nix/store";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = floorCommand;
        RuntimeDirectory = floorDir;
        RuntimeDirectoryMode = "0755";
        # Kept when the unit is restarted by a switch: removed on the stop, the
        # file would be missing for a moment, and a build in that moment has
        # no floor.
        RuntimeDirectoryPreserve = "yes";
      };
    };

    # A NEW FLOOR IS A NEW DAEMON. The daemon reads the file once, at start;
    # changed percentages restart it at the next switch, after the unit above
    # rewrote the file (the ordering holds within the switch's transaction).
    systemd.services.nix-daemon.restartTriggers = [ floorCommand ];

    # -----------------------------------------------------------------
    # 2. The system's generations
    # -----------------------------------------------------------------
    # THE ONE THE UPDATE, THE MEMORY PANEL AND `ngc` START, and whose failure
    # they see: `systemctl start` on a oneshot waits for it and says how it
    # ended.
    systemd.services.${lib.removeSuffix ".service" generationsUnit} = lib.recursiveUpdate trimUnit {
      description = "Trims the system's generations to the newest ones, the running and the booted";
      serviceConfig.ExecStart = trimCommand;
    };

    # AND THE SAME TRIM AT EVERY ACTIVATION — boot, the event, and every
    # switch, since a switch re-starts `multi-user.target` and so every
    # oneshot it wants: after the boot loader was installed, so every entry
    # the menu lists is among the kept ones.
    #
    # A UNIT OF ITS OWN, because of what a switch does with a failure: it
    # exits 4 when any unit it started failed, and `nixos-rebuild` calls the
    # whole switch failed — the update's `system-rebuild` step would stop the
    # sequence over a trim, week after week, while the new system was in fact
    # running. So this one never fails (`-`): a trim that could not run says
    # so in the journal and is tried again at the next activation, the next
    # update and the panel, where the other unit does fail loudly.
    #
    # And no default dependencies: a target waits for every unit it wants that
    # has them, and boot would wait for a trim at idle priority. It needs only
    # the profiles' mount, and it stops for a shutdown like anything else.
    systemd.services.${activationUnit} = lib.recursiveUpdate trimUnit {
      description = "Trims the system's generations after every activation, never failing it";
      wantedBy = [ "multi-user.target" ];
      after = [ "basic.target" ];
      before = [ "shutdown.target" ];
      conflicts = [ "shutdown.target" ];
      unitConfig.DefaultDependencies = false;
      serviceConfig.ExecStart = "-${trimCommand}";
    };

    # WHAT THE SWITCH WENT AWAY FROM, written while `/run/current-system` still
    # names it: the activation replaces that link only as its very last step.
    # At boot there is nothing to write — `/run` is new — and the trim reads
    # no such link then.
    system.activationScripts.ronoctua-previous-system = {
      deps = [ "specialfs" ];
      text = ''
        if [ -L /run/current-system ]; then
          mkdir -p ${previousDir}
          ln -sfn "$(readlink -f /run/current-system)" ${previousSystem}
        fi
      '';
    };

    # -----------------------------------------------------------------
    # 3. Build logs
    # -----------------------------------------------------------------
    # `e` never creates the directory, and the clean is the daily
    # `systemd-tmpfiles-clean.timer` nixpkgs already runs. Under `noatime` a
    # log's age is its write time, which is the age that means something here.
    systemd.tmpfiles.rules = [
      "e /nix/var/log/nix/drvs - - - ${toString cfg.buildLogDays}d"
    ];
  };
}
