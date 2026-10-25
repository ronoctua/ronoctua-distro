# One click updates everything — the privileged half, and the door.
#
# # What this module is
#
# `steps.nix`, beside this, declares the sequence; this file is what may RUN
# it. Two units for the root half — one that acts, one that rehearses — a
# polkit rule that names exactly those two and the verb `start`, two user
# units for the person's half, and `/etc/ronoctua/updates.conf`, which says
# which steps this machine has on.
#
# THE UNIT NAMES ARE THE MANIFEST'S: the four are declared there, the brain's
# Rust reads them from the table generated out of it (`unit::*`), and this
# file writes them into the units, the rule and the runners — so a rename is
# one line, and nothing can start a unit this file no longer declares.
#
# AND IT IS A BRAIN MODULE'S NIXOS HALF, so the loader (`nix/brain-modules.nix`)
# puts its `config` behind `ronoctua.brain.modules.<id>.enable` and the
# brain's own switch; its options stay declared either way, which is what
# lets `nix/modules/software.nix` and `nix/modules/emacs.nix` answer
# `ronoctua.update.steps` on any machine.
#
# # Why the halves are two units and not one
#
# What needs root runs as root, in a system unit. What does not runs as the
# person, in their own user manager — and that is not a workaround for polkit,
# it is the point: a tool somebody installed by hand lives on THEIR `PATH`, in
# THEIR home, and no process running as root can find it. It also means no
# language manager can ever run as root here, which is a guarantee by
# construction rather than by care.
#
# The person's half starts the root half and waits for it (`systemctl start` on
# a `Type=oneshot` returns when the work is done), so the whole thing is one
# sequence with one lock and one exit status.
#
# # There is no polkit agent in this session
#
# Only `pkttyagent`, in the rescue console, which has a TTY to ask on. So the
# rule returns `YES` and not `AUTH_SELF_KEEP`: `nix/modules/rescue.nix` may ask
# because it can, and this may not. What buys the width back is that the door is
# one millimetre wide — two unit names, one verb, one user, and a typo in any of
# the three is a refusal rather than a wider door.
#
# # What rehearsal proves, and what it does not
#
# Rehearsing runs every precondition FOR REAL and prints the program and its
# arguments, resolved: it answers "which steps would this machine run, which
# would it skip, and why". It does not answer anything that only appears once a
# command runs. `docs/PENDING.md` records the price of forgetting that — three
# defects of the installer that its own rehearsal mode could not see, because it
# prints commands instead of running them. The same gap is here, and it is the
# reason `--execute` exists rather than a reason to trust the run without it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.update;
  user = config.ronoctua.user;
  manifest = import ./manifest.nix;

  declared = import ./steps.nix;
  ids = map (s: s.id) declared.steps;

  # THE FOUR UNIT NAMES, from the manifest, which is the one place they are
  # written.
  #
  # They travel three ways from here — into the units, into the polkit rule and
  # into the generated runner, which is what tells the person's half which one to
  # start. Written three times, a rename would leave a rule naming a unit that no
  # longer exists: no error, no refusal, just a door that stops opening.
  service = unit: lib.removeSuffix ".service" unit;
  systemUnit = manifest.units.system;
  rehearsalUnit = manifest.units.system-rehearsal;
  personUnit = service manifest.units.run.name;
  personRehearsal = service manifest.units.rehearse.name;

  # Where the root half leaves its report and its lock. `/run` because none of
  # it survives a boot and none of it should.
  stateDir = "/run/ronoctua-update";
  configFile = "/etc/ronoctua/updates.conf";

  updates = pkgs.callPackage ./sequence.nix { };

  # THE PERSON'S DATA TOOL, told the machine's places (`record` reads the
  # machine's record) — the same table every unit that
  # runs `ronoctua-machine` gives it (`ronoctua.machine.environment`), by
  # `--set-default`, so nothing can disagree about which flake or which `nix`.
  personTool =
    pkgs.runCommand "ronoctua-person-for-the-update"
      {
        nativeBuildInputs = [ pkgs.makeWrapper ];
      }
      ''
        makeWrapper ${config.ronoctua.person.tool}/bin/ronoctua-person $out/bin/ronoctua-person ${
          lib.concatStringsSep " " (
            lib.mapAttrsToList (
              name: value: "--set-default ${name} ${lib.escapeShellArg value}"
            ) config.ronoctua.machine.environment
          )
        }
      '';

  runners = updates.mkScripts {
    # EVERY PACKAGE A STEP NAMES, resolved to a store path — never a bare name,
    # because a unit's `PATH` is minimal and the failure would appear far from
    # the cause (`AGENTS.md`). The generator refuses both directions: a package
    # no step names, and a step whose package is not here.
    programs = {
      # The machine's own Nix, not `pkgs.nix`: they can differ, and the one that
      # collects this store's garbage should be the one that filled it.
      nix = config.nix.package;
      systemd = pkgs.systemd;
      fontconfig = pkgs.fontconfig;
      tealdeer = pkgs.tealdeer;
      # The machine's own Flatpak, for the root half's `flatpak-system`.
      flatpak = config.services.flatpak.package;
      # THE SOFTWARE CLI, WRAPPED for this machine (`nix/modules/software.nix`):
      # a runner sees no unit environment, so the wrapper is the only thing
      # that tells the CLI where Flatpak, Waydroid and `appimage-run` are — and
      # it names only the ones this machine has, so a reduced profile carries
      # no Waydroid through this unit.
      software = config.ronoctua.software.package;
      # THE 2026-10-05 WAVE: the machine's life (`ronoctua-machine`, wrapped
      # with the machine's places: migrate, apply), the person's declarations
      # (`ronoctua-person`: record) and the belongings' plain part
      # (`ronoctua-belongings plain pull|push`) — each the EXTRACTED binary,
      # so a unit that runs as root carries none of the desktop.
      machine = config.ronoctua.machine.package;
      person = personTool;
      belongings = config.ronoctua.belongings.cli;
      # THE 2026-10-11 WAVE: the push to the person's binary cache
      # (`ronoctua-cachix`, EXTRACTED), wrapped by `nix/modules/binary-cache.nix`
      # with the machine's `nix`, its record and — where a cache is named —
      # `cachix`.
      binary-cache = config.ronoctua.binaryCache.pusher;
      # THE 2026-10-23 WAVE: the parts' root program (`ronoctua-parts`,
      # EXTRACTED), whose `plan` the update runs after the rebuild.
      parts = config.ronoctua.partsProgram;
    };
    wiring = {
      shell = pkgs.runtimeShell;
      systemctl = "${pkgs.systemd}/bin/systemctl";
      # NetworkManager is what this distro asks about the network
      # (`nix/modules/network.nix`), and `config.` rather than `pkgs.` so it is
      # the same one the machine is running.
      nmcli = "${config.networking.networkmanager.package}/bin/nmcli";
      flock = "${pkgs.util-linux}/bin/flock";
      # WHAT A STEP'S OWN REHEARSAL SAID, read back into this sequence's
      # output from the journal's end before it started — the trim's, today
      # (`steps.nix`).
      journalctl = "${pkgs.systemd}/bin/journalctl";
      state-dir = stateDir;
      config = configFile;
      system-unit = systemUnit;
      system-unit-rehearsal = rehearsalUnit;
      # THE 2026-10-05 PRECONDITIONS. Mains (`systemd-ac-power`, which says
      # yes on a machine with no battery), the kernel's power supplies, `df`
      # for the room a rebuild wants and the machine's own Nix collecting
      # garbage before refusing, `jq` over the machine's record for a hold,
      # and — for the root half, which has no session — the one person whose
      # yes to a run on battery counts, found under the runtime directories.
      ac-power = "${pkgs.systemd}/bin/systemd-ac-power";
      power-supplies = "/sys/class/power_supply";
      df = "${pkgs.coreutils}/bin/df";
      collect-garbage = "${config.nix.package}/bin/nix-collect-garbage";
      jq = "${pkgs.jq}/bin/jq";
      machine-state = "${config.ronoctua.machine.stateDir}/state.json";
      id = "${pkgs.coreutils}/bin/id";
      person = user;
      runtime-root = "/run/user";
    };
  };

  # WHICH STEPS THIS MACHINE HAS ON.
  #
  # The distro's default comes from the table; `ronoctua.update.steps` overrules
  # it per machine. A name that is not a step is a THROW and not a shrug: the
  # symptom of a typo would be a line that turns nothing on, and the only way to
  # find that out would be to run the sequence and notice an absence.
  unknown = lib.subtractLists ids (lib.attrNames cfg.steps);
  wanted =
    if unknown == [ ] then
      cfg.steps
    else
      throw (
        "ronoctua.update.steps cites a step that does not exist: "
        + lib.concatStringsSep ", " unknown
        + ". The steps are: "
        + lib.concatStringsSep ", " ids
        + "."
      );
  isOn = step: if wanted ? ${step.id} then wanted.${step.id} else step.enable;

  # THE UNITS A STEP STARTS INSTEAD OF RUNNING A PROGRAM.
  #
  # The trim is one: `nix/modules/store.nix` owns the one trim of the system's
  # generations, which the boot and the memory panel start too — and its
  # rehearsal, which this sequence's rehearsal starts in its place. (The
  # rebuild was the other until 2026-10-05; it is `ronoctua-machine apply`
  # now, run as a CHILD of the root half so root's lock of the person's data
  # reaches it — see the root half's unit below.)
  borrowed = lib.concatMap (
    s:
    lib.optional (s ? unit) {
      inherit (s) id;
      unit = s.unit;
    }
    ++ lib.optional (s ? rehearsal) {
      inherit (s) id;
      unit = s.rehearsal;
    }
  ) declared.steps;
in
{
  options.ronoctua.update = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The sequence that updates everything at once.";
    };

    steps = lib.mkOption {
      type = lib.types.attrsOf lib.types.bool;
      default = { };
      example = {
        old-generations = false;
        rust-toolchains = true;
      };
      description = ''
        Which steps this machine switches on or off, by id. Whatever is not there
        here stays as the update's `steps.nix` declares.

        This is where the language managers come in. They are born
        SWITCHED OFF because what the flake does not put on the machine cannot be a distro
        default (invariant 6) — but the machine where the owner put the tool by
        hand switches it on in one line, the same way `~/.shellrc.local` is the
        declared place for a toolchain that came from outside.

        An id that does not exist takes the build down saying which ones do: a line
        that switches nothing on would have no way of announcing itself.
      '';
    };

    # THE DECLARED SEQUENCE, as data, so a check about something else — the
    # software centre's, which asks which step moves the system's Flatpaks —
    # reads the steps from the evaluated machine and never from this folder's
    # file by path. Read-only, like the runners below: a handle, not a knob.
    sequence = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      internal = true;
      default = declared;
      defaultText = "the update's steps.nix";
      description = "The update's declared steps, as the table the runners are generated from.";
    };

    # THE BUILT RUNNERS, so something outside this module can reach them.
    #
    # `readOnly` and `internal`: it is not a knob, it is a handle. Without it
    # the only way to get at the two scripts is to dig them out of
    # `environment.systemPackages` by index, and a check written that way
    # breaks the day another package is added to the list.
    package = lib.mkOption {
      type = lib.types.package;
      readOnly = true;
      internal = true;
      default = runners;
      defaultText = "the two scripts, generated from the update's steps.nix";
      description = "The sequence's scripts, with the paths already resolved.";
    };
  };

  config = lib.mkIf cfg.enable {
    # A STEP MAY ONLY BORROW A UNIT THAT EXISTS.
    #
    # `ronoctua-rescue-rebuild.service` lives under `ronoctua.rescue.enable`,
    # which defaults on. With the rescue console off, this step would start a
    # unit that is not there — and `systemctl start` on a missing unit fails
    # inside the sequence, minutes in, saying nothing about why.
    assertions = map (step: {
      assertion = config.systemd.services ? "${lib.removeSuffix ".service" step.unit}";
      message =
        "ronoctua.update: the step `${step.id}` starts `${step.unit}`, which this "
        + "machine does not have. Switch on whoever declares it (`nix/modules/store.nix` for "
        + "the trim), or switch the step off: `ronoctua.update.steps.${step.id} = false;`.";
    }) borrowed;

    # WHERE THE ROOT HALF LEAVES ITS REPORT.
    #
    # `0755` and root's: the person READS it — the view that draws the sequence
    # has to be able to — and nobody else writes where a unit running as root
    # will later look.
    systemd.tmpfiles.rules = [ "d ${stateDir} 0755 root root -" ];

    # THE TABLE THE RUNNERS READ, in the shape `nix/modules/memory.nix` already
    # uses for its profiles: the decision lives in Nix, is written to `/etc`, and
    # what runs only reads it (invariant 5). One line per step, so a person can
    # answer "is it on?" with `cat`.
    environment.etc."ronoctua/updates.conf".text = ''
      # GENERATED from the system-update brain module's steps.nix. Do not edit by hand.
      # Switch a step on or off in ronoctua.update.steps.
      ${lib.concatMapStringsSep "\n" (s: "${s.id} ${if isOn s then "on" else "off"}") declared.steps}
    '';

    # THE DOOR ON THE PATH. `ronoctua-update` rehearses; `ronoctua-update
    # --execute` runs. It is the same sequence the footer's command starts, so
    # the rescue console and a plain terminal reach it when the footer cannot.
    #
    # The root half is in `libexec/` and not here: a person who typed it would
    # get a permission error instead of the sequence.
    environment.systemPackages = [ runners ];

    # -----------------------------------------------------------------
    # THE ROOT HALF — two units, and one of them only prints
    # -----------------------------------------------------------------
    systemd.services.${lib.removeSuffix ".service" systemUnit} = {
      description = "Updates this machine: inputs, system and sweep";
      serviceConfig = {
        # `Type = "oneshot"`, which is also what turns the start timeout OFF —
        # restated here because the whole sequence depends on it: a rebuild
        # compiles the workspace and `DefaultTimeoutStartSec` is 90 seconds.
        Type = "oneshot";
        TimeoutStartSec = "infinity";
        # THIS RUN'S REPORT STARTS EMPTY, before anything waits: the doors
        # read it, and the root half may wait a while for the lock below —
        # with the last run's lines still there, they would draw that run's
        # verdicts as this one's. `-`: a report that cannot be emptied stops
        # nothing; the runner empties it again when it starts.
        ExecStartPre = "-${pkgs.coreutils}/bin/truncate --size=0 ${stateDir}/system.report";
        # UNDER ROOT'S LOCK OF THE PERSON'S DATA, for the whole half: taken
        # by `ronoctua-person hold` and handed to the runner — and so to the
        # verification and the apply it runs — as the standard input. The
        # declarations the verification tried are then the ones the rebuild
        # builds: a person's apply started meanwhile (the software centre's)
        # waits, or hears `busy` and lands at the next apply, instead of
        # slipping between the two.
        #
        # `--patient`: an apply already building when the update starts
        # (`ronoctua-update-apply.service` holds the lock from build to
        # switch) is waited for, said in this unit's journal, and the update
        # runs after it. Refused as `busy` after half a minute, the whole
        # update counted as failed while the machine was only applying.
        ExecStart = "${personTool}/bin/ronoctua-person hold --patient -- ${runners}/libexec/ronoctua-update-system --execute";
        # The same scheduling `nix/modules/memory.nix` gives its housekeeping:
        # this competes with everything for CPU and disk, and it yields to
        # whatever the person is actually doing.
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
      };
      # A REBUILD RESTARTS CHANGED UNITS — INCLUDING THIS ONE.
      #
      # Step two is `nixos-rebuild switch`, and `switch-to-configuration`
      # restarts every service whose definition moved. If the update that just
      # landed changed this unit, systemd would stop the sequence in the middle
      # of itself and start it again from the top. Not a hypothetical: it is
      # exactly what the flake update makes likely.
      restartIfChanged = false;
      stopIfChanged = false;
    };

    systemd.services.${lib.removeSuffix ".service" rehearsalUnit} = {
      description = "Rehearses this machine's update, running nothing";
      serviceConfig = {
        Type = "oneshot";
        TimeoutStartSec = "infinity";
        ExecStart = "${runners}/libexec/ronoctua-update-system";
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
      };
      restartIfChanged = false;
      stopIfChanged = false;
    };

    # WHO MAY START THOSE TWO, AND ONLY THOSE.
    #
    # The user comes from `config.ronoctua.user` (invariant 16), never a
    # literal. The verb is `start` and nothing else: this rule cannot stop a
    # service, cannot restart one and cannot name a unit that is not on the list
    # — a typo in a name is a refusal, not a wider door.
    #
    # `YES` and not `AUTH_SELF_KEEP`, and the difference is not a relaxation:
    # there is no polkit agent in this session, so anything that would ask is
    # something that would simply fail. `nix/modules/rescue.nix` may ask because
    # it runs on a VT with `pkttyagent`; nothing here does.
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        var allowed = [
          "${systemUnit}",
          "${rehearsalUnit}"
        ];
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            action.lookup("verb") == "start" &&
            allowed.indexOf(action.lookup("unit")) >= 0 &&
            subject.user == "${user}") {
          return polkit.Result.YES;
        }
      });
    '';

    # -----------------------------------------------------------------
    # THE PERSON'S HALF — no privilege at all, so no rule at all
    # -----------------------------------------------------------------
    # A user unit needs no polkit: the manager belongs to the person already.
    # This is where the whole sequence is started from, and the root half is one
    # step inside it.
    #
    # AND WITH THE SESSION'S `PATH`, not a unit's. NixOS gives every unit five
    # packages on `PATH` unless told otherwise, and the person's half exists
    # precisely to find what the PERSON has: every `binary` step (the
    # language managers) was skipped from here, always, because `command
    # -v` asked those five. Off and with no `path`, the unit writes no
    # `Environment=PATH` (`nixos/lib/systemd-lib.nix`) and inherits the user
    # manager's, built from `environment.profiles`. The consequence is
    # deliberate and worth saying: a `binary` step that is switched on now
    # really runs.
    systemd.user.services.${personUnit} = {
      description = "Updates everything: the privileged half and then the caches";
      enableDefaultPath = false;
      serviceConfig = {
        Type = "oneshot";
        TimeoutStartSec = "infinity";
        ExecStart = "${runners}/bin/ronoctua-update --execute";
      };
      restartIfChanged = false;
      stopIfChanged = false;
    };

    systemd.user.services.${personRehearsal} = {
      description = "Rehearses the whole update, running nothing";
      # The same `PATH` as the run it rehearses, or its skips would be lies.
      enableDefaultPath = false;
      serviceConfig = {
        Type = "oneshot";
        TimeoutStartSec = "infinity";
        ExecStart = "${runners}/bin/ronoctua-update";
      };
      restartIfChanged = false;
      stopIfChanged = false;
    };

    # NO TIMER, AND THAT IS A DECISION.
    #
    # The store's garbage is collected by Nix itself when a build finds the
    # disk short (`nix/modules/store.nix`), and `nix/modules/cli.nix` refreshes
    # the tldr cache on a schedule. Moving the pinned inputs and rebuilding
    # the system do not belong on a clock: this sequence can bring a kernel
    # that does not boot, and the machine choosing the moment for that on its
    # own is the thing `system.autoUpgrade` does and this distro deliberately
    # does not have.
    #
    # The moment is chosen by the person, or later by `ronoctua-brain`, which is
    # a different thing from a calendar: it knows whether anything is running.
  };
}
