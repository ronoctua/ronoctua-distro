# What reaches a machine after it is installed: the PARTS.
#
# WHY
# ---
# A NixOS system is one derivation, built whole or not at all. On 2026-10-23
# one download — the npm cache of the PO-token minter — failed during an
# installation, and the installation stopped there; every update would have
# stopped the same way. So whatever needs a download no public cache holds,
# whatever is not free, and whatever only the full profile carries is a
# PART: declared here, never in the system's closure, realized on its own by
# root (`ronoctua-parts`), and kept in a Nix profile of its own.
#
# THE CONTRACT
# ------------
# - The system names a part only by its stable path,
#   `/nix/var/nix/profiles/ronoctua/parts/<id>/v<interface>` (`path`, below),
#   which may not exist yet: every reader asks `ronoctua_parts::here` and the
#   screen says the part is arriving. The interface is in the path, so a
#   system never meets a part's shape it does not speak, and a rollback finds
#   the old one.
# - The plan (`/etc/ronoctua/parts-plan.json`) carries each wanted part's
#   derivation and output as TEXT, their context discarded: building the
#   system builds no part, and evaluating it instantiates every part's
#   derivation, which is why the realizer finds them in the store after a
#   switch without evaluating anything.
# - Each part is realized by its own unit instance, `ronoctua-part@<id>`: a
#   failure is that part's alone, retried by systemd ever later
#   (`RestartSteps=`, up to `RestartMaxDelaySec=`), and a part that fails
#   keeps its previous generation working.
# - The session sees the parts that are here through ONE folder, the view
#   (`/var/lib/ronoctua-parts/view`): `bin/` and a closed set of `share/`
#   folders, never `lib/` — a profile among `environment.profiles` would bring
#   `QT_PLUGIN_PATH` and its siblings, and a part's plugin would load into the
#   base's programs.
#
# WHO STARTS WHAT
# ---------------
# `ronoctua-parts-trigger` (every boot and every switch) starts the planner
# without waiting for it; NetworkManager's dispatcher starts it when a
# network comes up; the person starts it, or one part's realizer, through ONE
# narrow polkit rule. The planner takes no argument; a realizer's only
# argument is its instance, and the rule lists every instance by name.
#
# `parts-rule-is-narrow` holds the names here, in `ronoctua_parts::names` and
# in the exit codes together.
{ tool }:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.parts;
  user = config.ronoctua.user;

  # THE NAMES, as `ronoctua_parts::names` writes them.
  partsRoot = "/nix/var/nix/profiles/ronoctua/parts";
  stateDir = "/var/lib/ronoctua-parts";
  view = "${stateDir}/view";
  planFile = "ronoctua/parts-plan.json";
  gcroots = "/nix/var/nix/gcroots/ronoctua-parts";
  plannerUnit = "ronoctua-parts.service";
  realizerUnit = id: "ronoctua-part@${id}.service";

  # THE STATUSES A REALIZER ENDS WITH THAT ARE NOT RETRIED ON A CLOCK, as
  # `ronoctua_parts::exit::NOT_RETRIED` lists them: a command line that is
  # not one, a part not wanted, no link (the network coming up starts it
  # again), the records' lock not taken.
  notRetried = [
    2
    10
    11
    12
  ];

  program = "${tool}/bin/ronoctua-parts";
  # THROUGH THE DAEMON, as every other build on the machine: two realizers
  # each running root's `nix` on the local store would allocate build users
  # on their own, and one killed the other's builder (measured in
  # `parts-are-isolated`: SIGKILL twenty milliseconds into a build). And a
  # throwaway home in the unit's own runtime folder, for Nix's cache.
  unitEnvironment = home: {
    RONOCTUA_NIX = "${config.nix.package}/bin/nix";
    RONOCTUA_SYSTEMCTL = "${config.systemd.package}/bin/systemctl";
    NIX_REMOTE = "daemon";
    HOME = "/run/${home}";
  };

  wanted = lib.filterAttrs (_: p: p.wanted) cfg;

  # A PART'S ID becomes a file name, a path segment and a unit instance a
  # polkit rule compares as text: `ronoctua_parts::id::is_id`, the same shape.
  isId = id: builtins.match "[a-z]([a-z0-9]|-[a-z0-9])*" id != null && builtins.stringLength id <= 48;
  badIds = lib.filter (id: !isId id) (lib.attrNames cfg);

  plain = builtins.unsafeDiscardStringContext;
  plan = {
    format = 1;
    parts = lib.mapAttrs (_: p: {
      drv = plain p.package.drvPath;
      out = plain p.package.outPath;
      inherit (p)
        kind
        interface
        heavy
        unfree
        ;
    }) wanted;
  };

  part =
    { name, config, ... }:
    {
      options = {
        wanted = lib.mkOption {
          type = lib.types.bool;
          description = ''
            Whether this machine wants the part: the owning module's switch,
            the profile, and the person's declines decide.
          '';
        };
        kind = lib.mkOption {
          type = lib.types.enum [
            "program"
            "data"
          ];
          default = "program";
          description = ''
            `program`: its `bin/` and launchers reach the session through the
            view. `data`: read by a program at the part's path, and nothing
            in the view.
          '';
        };
        package = lib.mkOption {
          type = lib.types.package;
          description = "What the part's profile holds: one store path.";
        };
        interface = lib.mkOption {
          type = lib.types.ints.positive;
          default = 1;
          description = ''
            The shape the base speaks to the part in. Raised when what the
            base reads of it changes, so an older generation is never read as
            the new shape.
          '';
        };
        heavy = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = "Realized last and alone: a part whose build alone fills the machine.";
        };
        unfree = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = "Not free: never on the published ISO, never in a public cache.";
        };
        path = lib.mkOption {
          type = lib.types.str;
          readOnly = true;
          default = "${partsRoot}/${name}/v${toString config.interface}";
          description = ''
            The ONE path the system names the part by. It may not exist yet:
            every reader asks `ronoctua_parts::here` before using it.
          '';
        };
      };
    };
in
{
  options.ronoctua.parts = lib.mkOption {
    type = lib.types.attrsOf (lib.types.submodule part);
    default = { };
    description = ''
      The parts this machine may want, each declared by the module that owns
      it and outside any `mkIf` (its presence is `wanted`).
    '';
  };

  config = {
    assertions = [
      {
        assertion = badIds == [ ];
        message = "ronoctua.parts: not a part id (lower-case kebab, at most 48): ${lib.concatStringsSep ", " badIds}";
      }
    ];

    environment.etc.${planFile}.text = builtins.toJSON plan;

    # THE VIEW, after the system's own profiles: a part never shadows a
    # program of the base.
    environment.profiles = lib.mkAfter [ view ];

    systemd.tmpfiles.rules = [
      "d ${stateDir} 0755 root root -"
      "d ${view} 0755 root root -"
      "d /nix/var/nix/profiles/ronoctua 0755 root root -"
      "d ${partsRoot} 0755 root root -"
      "d ${gcroots} 0755 root root -"
    ];

    # EVERY BOOT AND EVERY SWITCH: a oneshot with no `RemainAfterExit` that
    # a target wants runs again at each switch, which brings the next plan.
    # It starts the planner and does not wait — a switch never waits for a
    # download — and the `-` keeps a refusal from failing the switch.
    systemd.services.ronoctua-parts-trigger = {
      description = "Starts the realizer of the machine's parts";
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "-${config.systemd.package}/bin/systemctl start --no-block ${plannerUnit}";
      };
    };

    # THE PLANNER: the records brought to the plan, the derivations rooted,
    # the parts that are not here handed to their realizers two at a time.
    systemd.services.ronoctua-parts = {
      description = "Plans the machine's parts and starts what is missing";
      environment = unitEnvironment "ronoctua-parts";
      serviceConfig = {
        Type = "oneshot";
        RuntimeDirectory = "ronoctua-parts";
        ExecStart = "${program} plan --start";
        # Its workers wait for each realizer to end.
        TimeoutStartSec = "infinity";
        Nice = 10;
        IOSchedulingClass = "idle";
      };
      # Started by events and by the person: the default limit (five starts in
      # ten seconds) protects from no loop here and would refuse a gesture.
      startLimitIntervalSec = 0;
      restartIfChanged = false;
    };

    # ONE PART, ONE PROCESS. A failure is retried ever later — one minute, then
    # longer, at most twelve hours — and a status that must not be retried on
    # a clock (no link, not wanted) ends it until an event starts it again.
    systemd.services."ronoctua-part@" = {
      description = "Realizes the machine's part %i";
      environment = unitEnvironment "ronoctua-part-%i";
      serviceConfig = {
        Type = "oneshot";
        RuntimeDirectory = "ronoctua-part-%i";
        ExecStart = "${program} realize %i";
        TimeoutStartSec = "infinity";
        Restart = "on-failure";
        RestartSec = "1min";
        RestartSteps = 8;
        RestartMaxDelaySec = "12h";
        RestartPreventExitStatus = lib.concatMapStringsSep " " toString notRetried;
        Nice = 10;
        IOSchedulingClass = "idle";
      };
      startLimitIntervalSec = 0;
      # A switch never restarts a download in the middle.
      restartIfChanged = false;
    };

    # A NETWORK COMING UP starts the planner, which tries again the parts that
    # waited for one. It never evaluates anything: the plan is the system's.
    networking.networkmanager.dispatcherScripts = lib.mkIf config.networking.networkmanager.enable [
      {
        type = "basic";
        source = pkgs.writeShellScript "ronoctua-parts-network" ''
          case "$2" in
            up | connectivity-change)
              exec ${config.systemd.package}/bin/systemctl start --no-block ${plannerUnit}
              ;;
          esac
        '';
      }
    ];

    # WHO MAY START THE PLANNER AND EACH REALIZER: the person, verb `start`,
    # each unit by its whole name. The alternatives are BRACKETED: `&&` binds
    # tighter than `||` (`polkit-rules-are-the-ones-we-know`).
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            action.lookup("verb") == "start" &&
            (${
              lib.concatMapStringsSep " ||\n       " (u: ''action.lookup("unit") == "${u}"'') (
                [ plannerUnit ] ++ map realizerUnit (lib.attrNames wanted)
              )
            }) &&
            subject.user == "${user}") {
          return polkit.Result.YES;
        }
      });
    '';
  };
}
