# Root's half of the person's data: the narrow unit that validates the
# person's declarations and copies them into the machine directory, and the
# one that approves a person brain module's root part.
#
# WHY A ROOT COPY AT ALL
# ----------------------
# The machine's flake reads the person's data from `<machine dir>/person/`
# (`nix/modules/person.nix`). It cannot read the person's own files: a broken
# JSON makes `builtins.fromJSON` throw where nothing can catch it, and root
# must never build from a folder the person can change in the middle of the
# build. So the person STAGES (`/run/ronoctua-person/staged`, theirs, 0700)
# and a unit running as root validates every file, rewrites each document in
# its one canonical form, copies the brain modules byte for byte, and swaps
# the result in whole (`ronoctua-person apply`). Data, never Nix code — the
# one kind of code, a brain module's `module.nix`, is imported only at the
# hash the sudo door approved (below). A refused tree leaves the machine's
# data as it was: an older machine meeting a newer file keeps its old copy,
# and the update goes on with it.
#
# WHO STARTS IT
# -------------
# The person, through ONE polkit rule: verb `start`, this unit, the one user,
# `YES` — there is no polkit agent in the session, so anything that would
# ask would simply fail (the update's rule is the precedent). The software
# centre starts it after a declaration, and the update's person half after it
# stages (`person-stage`), before the root half verifies and builds; the
# installer runs the program directly. What buys the width back is that the
# unit takes no argument: it reads one folder the person owns, and writes one
# folder of root's.
#
# AND THE APPROVAL, WHICH IS NOT AUTOMATIC
# ----------------------------------------
# `ronoctua-person-approve.service` is started ONLY by the sudo door
# (`RootVerb::PersonModuleApprove`), after the person's own password. No
# polkit rule names it. The door's verb carries the module and the hash the
# person was shown; root's helper pins the two in root's folder
# (`approve-pin`) and starts the unit, which consumes the pin and the request
# the person staged in `/run/ronoctua-person/approve`, refuses when they
# disagree (any process of the person's may write their folder), asks
# `nix/person-hash.nix` — the single implementation — for the hash of the
# machine's copy of the module NOW, and records the approval only when that
# is the pinned one.
#
# AND THE PERSON'S SIGNING KEY, PINNED BY THEIR GESTURE
# -----------------------------------------------------
# What the person declares travels signed (the belongings' plain part,
# ML-DSA-65), and `apply` takes from a staging only what the key in
# `local/signer.json` signed. The installer pins that key, read from
# `key.age` with the passphrase. On a machine with no key pinned (or a
# repository made again), the pin is the person's gesture through the sudo
# door (`RootVerb::PersonSignerPin`): root's helper writes the key the door
# was given — the one the person's passphrase opened, never the repository's
# copy — in root's folder (`signer-pin`) and starts
# `ronoctua-person-pin.service`, which consumes it and writes the pin. No
# polkit rule names it.
#
# WHO MAY TOUCH THE MACHINE'S PERSON DATA NOW
# -------------------------------------------
# `apply`, `approve` and the update's `verify` and rebuild take one lock,
# `/run/ronoctua-person/machine.lock` — a `0600` file of root's in root's
# folder, made by the program. Not the machine directory: everybody may read
# that folder, `flock` needs only a read descriptor, and one process of the
# person's could hold it for ever. Whoever does not get the lock in
# `LOCK_WAIT_S` refuses as `busy`, and says so in its verdict; the start
# timeouts below are the belt to that brace.
#
# EACH SAYS WHAT IT DID in a file of root's the person may read:
# `verdict.json` (the apply: the staging's nonce, the outcome, the reason,
# what was left out) and `approve-verdict.json` (the approval: the module,
# the hash shown and the one computed, the reason). The door's own line and
# `systemctl start` only say that a unit failed.
#
# `person-apply-rule-is-narrow` holds the names together (this file, the
# crate's `root` module, the helper's arm) and the rule's width.
{
  config,
  lib,
  ...
}:

let
  user = config.ronoctua.user;
  group = config.users.users.${user}.group;
  dir = config.ronoctua.machine.dir;
  tool = "${config.ronoctua.person.tool}/bin/ronoctua-person";

  # THE NAMES, as `ronoctua_person::root` writes them.
  applyUnit = "ronoctua-person-apply.service";
  approveUnit = "ronoctua-person-approve.service";
  pinUnit = "ronoctua-person-pin.service";
  run = "/run/ronoctua-person";
  staged = "${run}/staged";
  approve = "${run}/approve";
  verdict = "${run}/verdict.json";
  approveVerdict = "${run}/approve-verdict.json";
  # What the sudo door was given for an approval, pinned by root's helper
  # (`ronoctua_person::root::APPROVE_PIN`): root's folder, so no process of
  # the person's can swap it while sudo reads the password.
  approvePin = "${run}/approve-pin";
  # The signing key the sudo door was given, pinned by root's helper
  # (`ronoctua_person::root::SIGNER_PIN`) for the pin's unit to consume.
  signerPin = "${run}/signer-pin";

  # WHAT BOTH UNITS ARE DENIED. They read a folder the person owns and write
  # the machine directory: no network, no devices, no new privileges, the
  # system read-only but for the two folders each names.
  hardening = {
    PrivateNetwork = true;
    RestrictAddressFamilies = "AF_UNIX";
    PrivateDevices = true;
    PrivateTmp = true;
    ProtectHome = true;
    ProtectSystem = "strict";
    ProtectHostname = true;
    ProtectClock = true;
    ProtectKernelTunables = true;
    ProtectKernelModules = true;
    ProtectKernelLogs = true;
    ProtectControlGroups = true;
    RestrictNamespaces = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    LockPersonality = true;
    MemoryDenyWriteExecute = true;
    NoNewPrivileges = true;
    SystemCallArchitectures = "native";
    UMask = "0022";
  };
in
{
  config = {
    # WHERE THE PERSON STAGES. The folder above the two is ROOT's, so the
    # person can fill them but never swap one for a link; each is `0700` and
    # theirs, so nobody else can leave something a root unit will read.
    systemd.tmpfiles.rules = [
      "d ${run} 0755 root root -"
      "d ${staged} 0700 ${user} ${group} -"
      "d ${approve} 0700 ${user} ${group} -"
    ];

    systemd.services.${lib.removeSuffix ".service" applyUnit} = {
      description = "Validates the person's staged declarations and swaps them into the machine directory";
      # NO START LIMIT: each start is one gesture of the person (a save, a
      # sync that brought declarations), and the unit never restarts by
      # itself — systemd's default of five starts in ten seconds refused a
      # sixth gesture as `start-limit-hit`.
      startLimitIntervalSec = 0;
      serviceConfig = hardening // {
        Type = "oneshot";
        ExecStart = "${tool} apply --staged ${staged} --machine ${dir} --owner ${user} --verdict ${verdict}";
        # A walk of a few megabytes, after at most `LOCK_WAIT_S` for the lock.
        TimeoutStartSec = "2min";
        # Reading files the person owns (0600 under 0700) is what the one
        # capability is for; writing the machine directory needs none.
        CapabilityBoundingSet = [ "CAP_DAC_READ_SEARCH" ];
        # `-`: a machine without the directory is refused by the program,
        # with a sentence, instead of failing in the namespace setup.
        ReadWritePaths = [
          "-${dir}"
          run
        ];
      };
    };

    systemd.services.${lib.removeSuffix ".service" approveUnit} = {
      description = "Approves a person brain module's root part at the hash it has now";
      environment = {
        # The machine's own Nix, which is the one that knows this store, and
        # the one implementation of the hash — both by store path.
        RONOCTUA_NIX = "${config.nix.package}/bin/nix";
        RONOCTUA_PERSON_HASH = "${../person-hash.nix}";
        # Through the daemon: the store is read-only in here.
        NIX_REMOTE = "daemon";
        # A throwaway home in the unit's own runtime folder; Nix's cache
        # follows it (`$HOME/.cache`), so no XDG_CACHE_HOME is pointed
        # anywhere (`snapshots-skip-the-store`).
        HOME = "/run/ronoctua-person-approve";
      };
      serviceConfig = hardening // {
        Type = "oneshot";
        ExecStart = "${tool} approve --request ${approve} --pin ${approvePin} --machine ${dir} --owner ${user} --verdict ${approveVerdict}";
        # One `nix eval` of a folder, after at most `LOCK_WAIT_S` for the lock.
        TimeoutStartSec = "5min";
        RuntimeDirectory = "ronoctua-person-approve";
        # Reading the request, and REMOVING it from the person's folder (a
        # request left behind would be approved again by the next start).
        CapabilityBoundingSet = [
          "CAP_DAC_READ_SEARCH"
          "CAP_DAC_OVERRIDE"
        ];
        ReadWritePaths = [
          "-${dir}"
          run
        ];
      };
    };

    systemd.services.${lib.removeSuffix ".service" pinUnit} = {
      description = "Pins the person's signing key in the machine directory";
      # ONE GESTURE OF THE PERSON per start, as the apply.
      startLimitIntervalSec = 0;
      serviceConfig = hardening // {
        Type = "oneshot";
        ExecStart = "${tool} pin --pin ${signerPin} --machine ${dir}";
        # One small file, after at most `LOCK_WAIT_S` for the lock.
        TimeoutStartSec = "2min";
        # Root's own pin, in root's folder, and root's machine directory:
        # no capability at all.
        CapabilityBoundingSet = [ "" ];
        ReadWritePaths = [
          "-${dir}"
          run
        ];
      };
    };

    # WHO MAY START THE APPLY: the person, verb `start`, this unit, `YES`.
    # The approval and the pin have no rule: the sudo door starts them, as
    # root.
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            action.lookup("unit") == "${applyUnit}" &&
            action.lookup("verb") == "start" &&
            subject.user == "${user}") {
          return polkit.Result.YES;
        }
      });
    '';

    # The person's own tool: `ronoctua-person request-approval` and `read`.
    environment.systemPackages = [ config.ronoctua.person.tool ];
  };
}
