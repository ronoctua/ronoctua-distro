# The machine's own flake — the options the machine directory is read by.
#
# Each installed machine has its own flake in the machine directory, with the
# distro as an input that follows the `release` branch (`nix/machine.nix`,
# `templates/machine`). This module hands the configuration that directory's
# name — spelled once, in `nix/machine.nix` (`machine-dir-has-one-name`) —,
# the attribute the machine flake builds, the contract between the two, and
# what `lib.machine` knows of the build: the flake's own source and the
# distro's revision. The units of the machine's life (the boot trial's bless,
# the update's apply) join it with their package.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  machine = import ../machine.nix;
  cfg = config.ronoctua.machine;
  # HOW MANY BUILDS FIT, and the plan of the units, each by its measured
  # class (`nix/build-sizing.nix`): the same arithmetic the batteries build
  # by.
  sizing = pkgs.callPackage ../build-sizing.nix { };
  # What roots a build's units: over the machine's own Nix, whose store it is.
  keepUnits = pkgs.callPackage ../keep-units.nix { nix = config.nix.package; };
  tool = "${cfg.package}/bin/ronoctua-machine";
  # THE UPDATE'S OWN LOCK, which the root half of the update holds for its
  # whole run (`gen-updates.py`): an apply started beside it — by the software
  # centre, by the rescue console — is refused at once instead of building
  # beside it. `update-apply-rule-is-narrow` reads the update's runner and
  # holds the two names together.
  updateLock = "${cfg.runDir}/system.lock";
  # THE DISTRO CHECKOUT THIS MACHINE'S FLAKE NAMES as a LOCAL input, echoed —
  # `inputs.ronoctua.url = "git+file:///…"`, or a bare `path:/…` that is not in
  # the store. Read the way the glyph's Rust reads it
  # (`build_switch::checkout_of` — keep the two in step), off the one
  # uncommented line that carries both `ronoctua` and `url`. The build switch's
  # two services share it: the reference direction builds it, the own direction
  # overrides `ronoctua` with it. A machine whose input is a published distro
  # has none, the script refuses, and the glyph is away there in the first place.
  distroCheckout = pkgs.writeShellScript "ronoctua-distro-checkout" ''
    set -eu
    url=$(${pkgs.gawk}/bin/awk \
      '!/^[[:space:]]*#/ && /ronoctua/ && /url/ { if (match($0, /"[^"]*"/)) { print substr($0, RSTART + 1, RLENGTH - 2); exit } }' \
      "${cfg.dir}/flake.nix")
    case "$url" in
      git+file://*) dir=''${url#git+file://} ;;
      path:*)       dir=''${url#path:} ;;
      *) echo "the machine flake names no local checkout to switch to" >&2; exit 1 ;;
    esac
    dir=''${dir%%\?*}
    case "$dir" in
      /nix/store/*) echo "the ronoctua input is a store copy, not a checkout" >&2; exit 1 ;;
      /*) ;;
      *) echo "the checkout path is not absolute: $dir" >&2; exit 1 ;;
    esac
    printf '%s\n' "$dir"
  '';
  # WHERE A SYSTEM CARRIES THE LOCK ITS MACHINE FLAKE WAS BUILT FROM, under
  # `/etc` (and so under its toplevel's `etc/`): `ronoctua-machine` reads it
  # there to put the folder back on the system that runs.
  systemLock = "ronoctua/machine.lock";
in
{
  options.ronoctua.machine = {
    dir = lib.mkOption {
      type = lib.types.str;
      default = machine.machineDir;
      defaultText = lib.literalExpression "(import nix/machine.nix).machineDir";
      readOnly = true;
      description = ''
        The machine directory: the machine's own flake (`flake.nix`,
        `flake.lock`, `host.nix`, `facter.json`, `person/`, `local/`). The one
        place this path is written; programs receive it as
        `RONOCTUA_MACHINE_DIR`.
      '';
    };

    attr = lib.mkOption {
      type = lib.types.str;
      default = config.ronoctua.user;
      defaultText = lib.literalExpression "config.ronoctua.user";
      readOnly = true;
      description = ''
        The `nixosConfigurations` attribute the machine flake builds — the
        same name the rescue's rebuild and `ronoctua-machine apply` ask for.
      '';
    };

    contract = lib.mkOption {
      type = lib.types.ints.positive;
      default = machine.contract;
      defaultText = lib.literalExpression "(import nix/machine.nix).contract";
      readOnly = true;
      description = ''
        The version of the contract between the distro and a machine flake
        (`lib.machine`): a machine flake written for another is refused at
        evaluation with a sentence.
      '';
    };

    # ── anchor: L03 ──
    source = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The machine flake's own source, as `lib.machine` received it (`./.` in
        the machine's `flake.nix`, a store path once evaluated). `null` for a
        system the distro builds without one (the VM, a test).
      '';
    };

    distroRev = lib.mkOption {
      type = lib.types.str;
      default = "unknown";
      description = ''
        The distro's revision this system is built from — `lib.machine` sets
        it from the distro flake (`<rev>-dirty` for a tree with changes).
        `unknown` is the VM's and a test's: they are built from no machine
        flake. It goes into `/etc/ronoctua/person-applied.json`.
      '';
    };

    # ── anchor: L05 ──
    # THE MACHINE'S LIFE (`ronoctua-machine`): migrating the old layout,
    # building before switching, the boot trial and its bless, a hold.
    tool = lib.mkOption {
      type = lib.types.package;
      description = ''
        `ronoctua-machine`, EXTRACTED from the workspace (`rust.machine`): it
        runs as root on every update, every rescue rebuild and every boot,
        and must not carry the desktop. The flake sets it next to the import
        of this module.
      '';
    };

    package = lib.mkOption {
      type = lib.types.package;
      readOnly = true;
      internal = true;
      default = pkgs.runCommand "ronoctua-machine" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
        makeWrapper ${config.ronoctua.machine.tool}/bin/ronoctua-machine $out/bin/ronoctua-machine ${
          lib.concatStringsSep " " (
            lib.mapAttrsToList (
              name: value: "--set-default ${name} ${lib.escapeShellArg value}"
            ) config.ronoctua.machine.environment
          )
        }
      '';
      defaultText = "the tool, wrapped with ronoctua.machine.environment";
      description = ''
        `ronoctua-machine` with the machine's environment already set: what
        the units run and what root finds on the `PATH` (`sudo
        ronoctua-machine hold <rev>`). `--set-default`, so a unit's own
        `Environment=` (the same table) says the same thing twice and a test
        can point it elsewhere.
      '';
    };

    applyCommand = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      internal = true;
      default = "${pkgs.util-linux}/bin/flock --nonblock --conflict-exit-code 75 ${updateLock} ${tool} apply";
      defaultText = "flock --nonblock on the update's lock, then ronoctua-machine apply";
      description = ''
        The command line of an apply that does not run inside the update: the
        update's own lock first, not waited for (busy is an answer, exit 75),
        then `ronoctua-machine apply`. The software centre's
        `ronoctua-update-apply.service` and the rescue's rebuild run it.
      '';
    };

    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      readOnly = true;
      internal = true;
      default = {
        RONOCTUA_MACHINE_DIR = config.ronoctua.machine.dir;
        RONOCTUA_MACHINE_ATTR = config.ronoctua.machine.attr;
        RONOCTUA_MACHINE_STATE = config.ronoctua.machine.stateDir;
        RONOCTUA_MACHINE_RUN = config.ronoctua.machine.runDir;
        # Where a toplevel carries its lock (`etc/…`, below).
        RONOCTUA_MACHINE_SYSTEM_LOCK = "etc/${systemLock}";
        RONOCTUA_MACHINE_BLESS_WAIT_S = toString config.ronoctua.machine.blessWaitSeconds;
        # HOW MANY GENERATIONS THE BOOT MENU LISTS: `apply` refuses the
        # generation that would push the last good one out of it. The store
        # module caps every loader at the count the trim keeps; the loader's
        # own limit is read first, so a machine that imports this module
        # alone (a test) still has one.
        RONOCTUA_MACHINE_MENU_LIMIT = toString (
          if config.boot.loader.systemd-boot.enable then
            config.boot.loader.systemd-boot.configurationLimit
          else if config.boot.loader.grub.enable then
            config.boot.loader.grub.configurationLimit
          else
            config.ronoctua.store.systemGenerations
        );
        # The template a migrated machine's `flake.nix` is written from.
        RONOCTUA_MACHINE_TEMPLATE = "${../../templates/machine}";
        # By store path: a unit's `PATH` is minimal, and the machine's own
        # Nix is the one that filled this store.
        RONOCTUA_NIX = "${config.nix.package}/bin/nix";
        # HOW MANY BUILDS FIT HERE, what builds the units, each by its
        # class, and what roots a build's units.
        RONOCTUA_BUILD_SIZING = "${sizing.program}/bin/ronoctua-build-sizing";
        RONOCTUA_BUILD_UNITS = "${sizing.unitsProgram}/bin/ronoctua-build-units";
        RONOCTUA_KEEP_UNITS = "${keepUnits}/bin/ronoctua-keep-units";
      }
      # THE BOOT LOADER'S OWN TOOL, for the boot trial: systemd-boot's
      # `bootctl` (EFI variables), or, on a BIOS machine's GRUB
      # (`ronoctua.firmware`), `grub-editenv` (GRUB's environment block) —
      # by the loader the system has, so a machine that imports this module
      # alone (a test) still names the right one.
      // (
        if config.boot.loader.grub.enable && !config.boot.loader.systemd-boot.enable then
          # NixOS's GRUB is `grub2` (it has no `package` option), and the
          # environment block is the same on every platform GRUB has.
          { RONOCTUA_GRUB_EDITENV = "${pkgs.grub2}/bin/grub-editenv"; }
        else
          { RONOCTUA_BOOTCTL = "${config.systemd.package}/bin/bootctl"; }
      );
      defaultText = "the machine directory, its attribute, the state and run folders, the menu's limit, the template, nix, the boot loader's tool (bootctl, or grub-editenv), the build sizing, what builds the units and what keeps them";
      description = ''
        What `ronoctua-machine` is told, by every unit that runs it and by
        its wrapper: one table, so the rescue's rebuild, the update and the
        boot's bless cannot disagree about which flake or which `nix`.
      '';
    };

    # HOW THE SYSTEM IS BUILT (`ronoctua-machine apply`, `crate::build`): its
    # units first, each by its class, and the compiled units kept after a
    # build that worked. The flake sets the two inputs next to the import of
    # this module, from the same `rust` the system is built from.
    units = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      internal = true;
      description = ''
        The release units the system is built from, as data
        (`rust.unitIndex` of the release graph: `kind`, `drv` and `deps` by
        id): their plan (`unitPlan`, each with its class as measured) is
        what an apply builds, unit by unit, before the system.
      '';
    };

    keptUnits = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      internal = true;
      description = ''
        The compiled units of the version this system is built from
        (`rust.release.keep`, as store paths): after a build that worked,
        `ronoctua-machine apply` roots the ones the store holds, under the
        machine's record, and releases the last build's — so the next update
        compiles only what changed, and a weekly collection does not take the
        rest. Never part of the system's closure.
      '';
    };

    phases = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      internal = true;
      default = {
        # `ronoctua_machine::build::PHASES_FORMAT`.
        format = 3;
        # One line a unit: its id, its class, how the class was found, its
        # COST — the slot in MiB, or `alone` with a heavy unit's numbers —,
        # its `.drv` and the lines it is built from: what
        # `ronoctua-build-units` reads. The cost is in the line because the
        # program that reads it is the RUNNING system's, and this plan the
        # new distro's: a class table in the program would be the old one.
        units = (sizing.unitPlan { inherit (cfg) units; }).lines;
        system = builtins.unsafeDiscardStringContext config.system.build.toplevel.drvPath;
        kept = cfg.keptUnits;
      };
      defaultText = "the plan of the units, the system's derivation and the units to keep";
      description = ''
        What `ronoctua-machine apply` evaluates, once, before it builds: the
        plan of the units, each with its class, its cost and the `.drv` the
        evaluation writes (`nix eval --json`), the system's and the units to
        keep. Of the plan it builds only what Nix's own plan for the system
        lists (a dry run): a unit whose consumer the store already holds,
        the seed's or the person's cache's, is in the plan and never built.
        Read by the apply and nothing else; no part of the system refers to
        it.
      '';
    };

    stateDir = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      default = "/var/lib/ronoctua-machine";
      description = ''
        The machine's record (`state.json`: the last good generation, the
        boot trial, a hold, the last apply), the lock snapshots, the old
        layout a migration set aside, the roots of the units the last build
        was compiled from (`kept-units/`) and of the sources it was evaluated
        from (`kept-sources/`). Persisted; the generations' trim reads the
        record to keep the last good system and the one on trial.
      '';
    };

    runDir = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      default = "/run/ronoctua-update";
      description = ''
        The update's folder under `/run`, shared with it: the out-link of a
        built system lives here while it is applied, and the update's own
        lock — which `ronoctua-update-apply.service` and the rescue's rebuild
        take too (`applyCommand`).
      '';
    };

    blessAfterSeconds = lib.mkOption {
      type = lib.types.ints.positive;
      default = 60;
      description = ''
        How long after the system's default target (`systemd.defaultUnit`) the
        boot is judged: a system that reached it and stayed up this long
        booted well, and the trial is blessed.
      '';
    };

    blessWaitSeconds = lib.mkOption {
      type = lib.types.ints.positive;
      default = 4 * 60 * 60;
      description = ''
        How long the bless waits for root's lock of the person's data, which
        an update holds for its whole root half. What booted is written down
        before the wait, so a bless that never gets the lock loses nothing:
        the next one judges by that boot.
      '';
    };

    trialBootSeconds = lib.mkOption {
      type = lib.types.ints.positive;
      default = 600;
      description = ''
        How long a boot TRYING a new system has, from the start of stage 2, to
        reach the system's default target. A trial that does not — a mount
        that failed into emergency mode, a unit that never ends — is
        restarted, and the boot loader takes the last good system.
      '';
    };
  };

  # ── anchor: L03 ──
  # ── anchor: L05 ──
  config =
    let
      user = config.ronoctua.user;
      # The target a boot comes up to — `multi-user.target` here: the session
      # is a console login and a user unit, with no display manager, so
      # `graphical.target` is never reached.
      target = config.systemd.defaultUnit;
      # THE DEADLINE OF A TRIAL BOOT. Only a boot that TRIES a new system is
      # bounded: the boot loader's default names the last good entry and it
      # booted another (`LoaderEntrySelected`, which systemd-boot writes at
      # every boot) — no other boot has a default set, since every judgement
      # unsets it. GRUB, on a BIOS machine, leaves Linux no such variable: its
      # menu marks the boot of once of an armed trial on the kernel's command
      # line instead (`nix/boot-trial-mark.nix`, `nix/modules/boot-loader.nix`).
      # Then: the default target, in time, or a restart, and the one-shot is
      # spent, so the boot loader takes the last good system.
      grub = config.boot.loader.grub.enable && !config.boot.loader.systemd-boot.enable;
      tries =
        if grub then
          ''
            read -r cmdline < /proc/cmdline
            case " $cmdline " in
              *" ${import ../boot-trial-mark.nix} "*) ;;
              *) exit 0 ;;
            esac
            if up; then
              exit 0
            fi
            selected=$(${pkgs.coreutils}/bin/readlink -f /run/booted-system) default="the last good system"
          ''
        else
          ''
            vars=/sys/firmware/efi/efivars loader=4a67b082-0a4c-41cf-b6c7-440b29bb8c4f
            # An entry id, from the boot loader's UTF-16 variable: the four bytes
            # of attributes off, the zero half of every character off.
            value() {
              if [ -r "$vars/$1-$loader" ]; then
                ${pkgs.coreutils}/bin/tail -c +5 "$vars/$1-$loader" | ${pkgs.coreutils}/bin/tr -d '\000'
              fi
            }
            default=$(value LoaderEntryDefault) selected=$(value LoaderEntrySelected)
            if [ -z "$default" ] || [ "$default" = "$selected" ] || up; then
              exit 0
            fi
          '';
      # UP is REACHED IN THIS BOOT, not active now: a `systemctl rescue` later
      # in a boot that came up (which wants this unit again) is no stage 2
      # that never did — and on GRUB the trial's mark stays on the command
      # line for the whole boot. systemd keeps when the target last became
      # active, across its own deactivation.
      deadline = pkgs.writeShellScript "ronoctua-boot-trial-deadline" ''
        up() {
          entered=$(${config.systemd.package}/bin/systemctl show --property=ActiveEnterTimestampMonotonic --value ${target})
          [ "''${entered:-0}" -gt 0 ]
        }
        ${tries}
        echo "<5>this boot tries $selected: ${target} within ${toString cfg.trialBootSeconds} s, or back to $default"
        ${pkgs.coreutils}/bin/sleep ${toString cfg.trialBootSeconds}
        if up; then
          exit 0
        fi
        # Said to the kernel's log too: on every console, where somebody
        # looking at a stuck boot is looking.
        said="the boot trial did not reach ${target} in time: restarting into the last good system"
        echo "<2>$said"
        echo "<2>ronoctua: $said" > /dev/kmsg || true
        exec ${config.systemd.package}/bin/systemctl reboot --force
      '';
    in
    {
      # THE BOOT TRIAL'S WAY BACK, which has to happen without anybody:
      #
      # - `panic=10`: a kernel that panics reboots ten seconds later instead
      #   of hanging on the message;
      # - `boot.panic_on_fail`: a stage 1 that fails (no `init=` to switch
      #   to, a root that does not mount) panics instead of waiting in an
      #   emergency shell whose root account is locked anyway — nixpkgs'
      #   systemd initrd turns it into a crash (`panic-on-fail.service`);
      # - the hardware watchdog: a system that hangs with the kernel alive
      #   stops being petted and is reset;
      # - a stage 2 that does not reach the default target in time (a mount
      #   that failed into emergency mode, whose root account is locked; a
      #   unit that never ends): `ronoctua-boot-trial-deadline.service`,
      #   below, restarts a TRIAL boot that does.
      #
      # Each of those boots again, and the one-shot entry was spent: the boot
      # loader takes the default, which during a trial is the last good one.
      # Three wrong passphrases at the disk's prompt are such a failure too,
      # and reboot to try again.
      boot.kernelParams = [
        "panic=10"
        "boot.panic_on_fail"
      ];
      systemd.settings.Manager.RuntimeWatchdogSec = lib.mkDefault "30s";

      # Started at the very beginning of stage 2, before anything it waits
      # for, and kept through `emergency.target` and `rescue.target` (which
      # isolate, and would stop it — and which want it, in case they came
      # first). It does nothing on a boot that tries nothing, and nothing once
      # the default target is up: a switch that starts it again is harmless.
      systemd.services.ronoctua-boot-trial-deadline = {
        description = "Restarts a boot trial that does not reach the default target in time";
        wantedBy = [
          "sysinit.target"
          "emergency.target"
          "rescue.target"
        ];
        conflicts = [ "shutdown.target" ];
        before = [ "shutdown.target" ];
        unitConfig = {
          DefaultDependencies = false;
          IgnoreOnIsolate = true;
        };
        serviceConfig = {
          Type = "simple";
          ExecStart = deadline;
        };
        restartIfChanged = false;
        stopIfChanged = false;
      };

      # THE LOCK THE SYSTEM WAS BUILT FROM, carried by the system: the folder
      # can always be put back on the system that runs — the first apply after
      # an installation, the first trial that fails, a rollback by hand —
      # with no snapshot taken before. The text, not a link: a link would put
      # the whole machine flake (its secrets file included) in the closure.
      environment.etc = lib.mkIf (
        cfg.source != null && builtins.pathExists (cfg.source + "/flake.lock")
      ) { ${systemLock}.text = builtins.readFile (cfg.source + "/flake.lock"); };

      # The record's folder: root's, readable by everybody — the person's
      # half publishes a copy of `state.json`, and the update's view reads it.
      #
      # AND THE UPDATE'S LOCK, root's `0600` before anybody opens it. Every
      # system change takes it (the update's root half, `applyCommand`,
      # apply-here), and `flock` needs only a READ descriptor: the file their
      # shells would create is `0644` in a `0755` folder, and any local uid
      # could then hold it for ever — every update refused as "already
      # running", every apply busy, until a reboot. Made at boot (`f`, the
      # folder is a tmpfs) and brought back to `0600` at every switch (`z`)
      # on a machine that ran with the old mode; the folder is the same line
      # the update module writes, and identical lines are one.
      systemd.tmpfiles.rules = [
        "d ${cfg.stateDir} 0755 root root -"
        "d ${cfg.runDir} 0755 root root -"
        "f ${updateLock} 0600 root root -"
        "z ${updateLock} 0600 root root -"
      ];

      # `sudo ronoctua-machine hold <rev>` from a terminal; anybody else is
      # refused before anything runs (`not-root`).
      environment.systemPackages = [ cfg.package ];

      # THE BLESS, ONCE PER BOOT, a while after the boot came up.
      #
      # Wanted by the system's DEFAULT target and ordered after it, with the
      # wait in front: the boot is judged good when it reached that target
      # and stayed up. `RemainAfterExit`, so the switches of this boot (which
      # start the targets again) do not run it again; and `bless` itself
      # never judges a trial armed during this same boot — the first switch
      # that brings this unit starts it right away, and that boot tried
      # nothing. It writes down what booted before it waits for root's lock
      # of the person's data, and waits long (`blessWaitSeconds`).
      # `Before=boot-complete.target`: whatever waits for a complete boot
      # waits for the judgement.
      systemd.services.${lib.removeSuffix ".service" "ronoctua-boot-bless.service"} = {
        description = "Blesses the boot trial, or puts the machine back on the last good system";
        wantedBy = [ target ];
        after = [ target ];
        before = [ "boot-complete.target" ];
        environment = cfg.environment;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecStartPre = "${pkgs.coreutils}/bin/sleep ${toString cfg.blessAfterSeconds}";
          ExecStart = "${tool} bless";
          # The wait, the lock's wait, and a quarter of an hour for the way
          # back to write the boot menu again; a unit that never ends is not.
          TimeoutStartSec = "${toString (cfg.blessAfterSeconds + cfg.blessWaitSeconds + 900)}s";
        };
        restartIfChanged = false;
        stopIfChanged = false;
      };

      # THE SOURCES, ONCE PER BOOT, after the bless.
      #
      # The apply roots the sources each build was evaluated from — the
      # machine's flake, the distro, every input (`build::keep_sources`) —, so
      # an update fetches none of them again unchanged. An installation runs
      # no apply: from the first boot to the first update's build nothing held
      # them, and a collection in between took them. So they are rooted at
      # every boot too, after the default target and the bless: the boot
      # waits for none of it. (`After=` the target is what keeps it off the
      # boot's path: without it, the target would order itself after a unit
      # it wants, and with the bless after the target, that is a cycle.) Only
      # on a machine with its own flake — the VM's and a test's have none, and
      # a unit run without one would be a failed unit —, and a machine an
      # apply holds is answered at once: the apply keeps its own build's.
      systemd.services.ronoctua-keep-sources = {
        description = "Keeps the sources the machine's flake was evaluated from";
        wantedBy = [ target ];
        after = [
          target
          "ronoctua-boot-bless.service"
        ];
        unitConfig.ConditionPathExists = "${cfg.dir}/flake.lock";
        environment = cfg.environment;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecStart = "${tool} keep-sources";
          CPUSchedulingPolicy = "idle";
          IOSchedulingClass = "idle";
        };
        restartIfChanged = false;
        stopIfChanged = false;
      };

      # APPLY WITHOUT MOVING THE INPUTS — what the software centre starts after
      # the person declared or forgot a program, so it lands now and not at
      # the next weekly update. The update's lock first (`flock -n`: busy is
      # an answer, said with its own code), then root's lock of the person's
      # data, which `ronoctua-machine` takes itself.
      systemd.services.${lib.removeSuffix ".service" "ronoctua-update-apply.service"} = {
        description = "Applies the machine directory without moving its inputs";
        environment = cfg.environment;
        serviceConfig = {
          Type = "oneshot";
          # A build compiles; the default start timeout is 90 seconds.
          TimeoutStartSec = "infinity";
          ExecStart = cfg.applyCommand;
          RuntimeDirectory = baseNameOf cfg.runDir;
          RuntimeDirectoryPreserve = "yes";
          CPUSchedulingPolicy = "idle";
          IOSchedulingClass = "idle";
        };
        # The switch it runs must not stop or restart the unit running it.
        restartIfChanged = false;
        stopIfChanged = false;
      };

      # THE BUILD SWITCH — what the modeline's glyph and `LEADER / b` start to
      # move between the machine's OWN build and a distro checkout's REFERENCE
      # build (`ronoctua_shell_footer::build_switch`). Both run the same apply
      # the update does, so the boot trial is the net either way: a build that
      # will not come up is rolled back on the next boot, as any trial is.
      #
      # THE OWN DIRECTION builds `/etc/nixos`'s own system (its `host.nix`) but
      # with `ronoctua` OVERRIDDEN to a `path:` of the checkout
      # (`RONOCTUA_MACHINE_OVERRIDE_DIR`, `argv::with_override`). Without that, a
      # root apply would fetch the machine flake's `git+file` input — a
      # user-owned repo git refuses root, and a branch `--no-update-lock-file`
      # will not re-resolve. The override is the same checkout the reference
      # direction builds, so neither fetches git as root.
      #
      # TWO NAMED UNITS, not one templated by the directory: the polkit rule
      # then names exactly the two the person may start, and no instance can
      # smuggle an arbitrary flake to apply as root — the parts module's reason
      # (`polkit-rules-are-the-ones-we-know`), applied here.
      systemd.services.ronoctua-build-switch-own = {
        description = "Builds and switches to this machine's own build";
        environment = cfg.environment;
        serviceConfig = {
          Type = "oneshot";
          TimeoutStartSec = "infinity";
          ExecStart = pkgs.writeShellScript "ronoctua-build-switch-own" ''
            set -eu
            dir=$(${distroCheckout}) || exit 1
            # Keep RONOCTUA_MACHINE_DIR (`/etc/nixos`, the machine's own system);
            # only override the distro input with the checkout.
            export RONOCTUA_MACHINE_OVERRIDE_DIR="$dir"
            exec ${cfg.applyCommand}
          '';
          RuntimeDirectory = baseNameOf cfg.runDir;
          RuntimeDirectoryPreserve = "yes";
          CPUSchedulingPolicy = "idle";
          IOSchedulingClass = "idle";
        };
        restartIfChanged = false;
        stopIfChanged = false;
      };
      # THE REFERENCE DIRECTION builds the checkout itself (its fixture host),
      # so it points the machine directory at it — no override, no git.
      systemd.services.ronoctua-build-switch-reference = {
        description = "Builds and switches to a distro checkout's reference build";
        environment = cfg.environment;
        serviceConfig = {
          Type = "oneshot";
          TimeoutStartSec = "infinity";
          ExecStart = pkgs.writeShellScript "ronoctua-build-switch-reference" ''
            set -eu
            dir=$(${distroCheckout}) || exit 1
            export RONOCTUA_MACHINE_DIR="$dir"
            exec ${cfg.applyCommand}
          '';
          RuntimeDirectory = baseNameOf cfg.runDir;
          RuntimeDirectoryPreserve = "yes";
          CPUSchedulingPolicy = "idle";
          IOSchedulingClass = "idle";
        };
        restartIfChanged = false;
        stopIfChanged = false;
      };

      # WHO MAY START THEM: the person, verb `start`, these units and no other,
      # `YES` — there is no polkit agent in this session, and the software
      # centre starts the update's when nobody is looking at a prompt. The width
      # is bought back the way the update's rule buys it: named units, one verb,
      # one user, no argument (`update-apply-rule-is-narrow`). The build
      # switch's two join the update's apply in the one rule.
      security.polkit.extraConfig = ''
        polkit.addRule(function (action, subject) {
          if (action.id == "org.freedesktop.systemd1.manage-units" &&
              (action.lookup("unit") == "ronoctua-update-apply.service" ||
               action.lookup("unit") == "ronoctua-build-switch-own.service" ||
               action.lookup("unit") == "ronoctua-build-switch-reference.service") &&
              action.lookup("verb") == "start" &&
              subject.user == "${user}") {
            return polkit.Result.YES;
          }
        });
      '';
    };
}
