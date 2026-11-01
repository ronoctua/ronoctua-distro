# AN APP THAT FAILS TO INSTALL NEVER BREAKS THE DISTRO'S INSTALLATION OR BOOT
# (2026-11-01): the two checks the 2026-10-23 plan of the parts named and
# never wrote.
#
#   - `parts-build`: building a system never builds one of its parts, so a
#     part whose build fails can stop neither an installation
#     (`nixos-install` builds the system) nor an update (`ronoctua-machine
#     apply` does). Asked of the BUILD GRAPH, with nothing built: no wanted
#     part's derivation is in the closure of the system's derivation, on
#     each reference machine — and on a machine planted with a part whose
#     builder fails, whose plan wants it. The same broken package put in the
#     system's packages IS found, so the probe is not blind. What it catches
#     is a list MADE from a part at build time: the software centre's icon
#     names were, and every system built the 0,61 GiB icon theme to read
#     them (they arrive in the part now, `ronoctua.icons.names`).
#   - `parts-absent-vm`: a machine with no part realised yet works — it
#     boots with nothing failed, an update switches, the person's session
#     runs — while one part's build fails, said in its record as the error
#     itself (`tests/parts-absent-vm.nix`).
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  self,
  mkCheck,
  pyPrelude,
  rust,
}:
let
  plain = builtins.unsafeDiscardStringContext;
  system = pkgs.stdenv.hostPlatform.system;

  # THE PLANTED PART: its builder fails, under a step that builds.
  step = pkgs.runCommand "parts-build-planted-step" { } "echo built > $out";
  broken = pkgs.runCommand "parts-build-planted-broken" { } ''
    echo "${step}"
    echo "the planted failure: this part does not build" >&2
    exit 1
  '';

  # A MACHINE OF THE PARTS' MODULE ALONE, wanting the broken part — and its
  # twin with the same package in the system's packages, which a build of
  # the system has to build.
  planted =
    extra:
    (import (pkgs.path + "/nixos/lib/eval-config.nix") {
      inherit system;
      modules = [
        (import ../nix/modules/parts.nix { tool = rust.parts; })
        {
          # The one option of the distro's `base.nix` the parts' module reads.
          options.ronoctua.user = lib.mkOption {
            type = lib.types.str;
            default = "noctua";
          };
          config = {
            boot.isContainer = true;
            users.users.noctua.isNormalUser = true;
            system.stateVersion = lib.trivial.release;
            ronoctua.parts.broken = {
              wanted = true;
              package = broken;
            };
          };
        }
        extra
      ];
    }).config;

  # A SYSTEM'S DERIVATION as a path, never as its outputs: the check reads
  # the `.drv` files, and building them is what it must never need.
  machineOf =
    c:
    let
      parts = lib.filterAttrs (_: p: p.wanted && p.package != null) c.ronoctua.parts;
    in
    {
      system = builtins.unsafeDiscardOutputDependency c.system.build.toplevel.drvPath;
      plan = c.environment.etc."ronoctua/parts-plan.json".text;
      parts = lib.mapAttrs (_: p: plain p.package.drvPath) parts;
    };
  facts = pkgs.writeText "parts-build-facts.json" (
    builtins.toJSON {
      machines = {
        vm = machineOf self.nixosConfigurations.vm.config;
        noctua = machineOf self.nixosConfigurations.noctua.config;
        planted = machineOf (planted { });
      };
      # The probe's own planted failure.
      leaking = machineOf (planted {
        environment.systemPackages = [ broken ];
      });
      broken = plain broken.drvPath;
      step = plain step.drvPath;
    }
  );
in
{
  parts-build = mkCheck "parts-build" [ pkgs.python3 ] ''
    python3 - <<'END'
    import json, re, sys
    exec(open("${pyPrelude}").read())

    # PARSED: the systems' derivations, their parts' and the planted
    # failure's arrive as JSON; the derivations themselves are read as the
    # store keeps them, every input derivation named in its file.
    facts = json.load(open("${facts}"))
    drv = re.compile(r'"(/nix/store/[0-9a-z]{32}-[^"/]+\.drv)"')

    def closure(root):
        seen, todo = set(), [root]
        while todo:
            here = todo.pop()
            if here in seen:
                continue
            seen.add(here)
            todo.extend(drv.findall(open(here).read()))
        return seen

    wrong = []
    leaking = closure(facts["leaking"]["system"])
    floor(len(leaking), 50, "derivations a planted system's build needs")
    if facts["broken"] not in leaking or facts["step"] not in leaking:
        wrong.append("the probe is blind: a system with the broken package in its packages does not need its build")
    parts = 0
    for name, machine in facts["machines"].items():
        needed = closure(machine["system"])
        floor(len(needed), 50, f"derivations {name}'s build needs")
        for part, path in sorted(machine["parts"].items()):
            parts += 1
            if path in needed:
                wrong.append(f"{name}: building the system builds the part {part} ({path}): a failure of its build stops the installation")
    planted = facts["machines"]["planted"]
    if planted["parts"].get("broken") != facts["broken"] or facts["broken"] not in planted["plan"]:
        wrong.append("the planted machine does not want the broken part: this proves nothing of it")
    if facts["step"] in closure(planted["system"]):
        wrong.append("the planted machine's build needs the broken part's own step")
    floor(parts, 10, "wanted parts of the three machines")
    if wrong:
        print("ERROR: a part's build is part of a system's build.", file=sys.stderr)
        for w in wrong:
            print("       " + w, file=sys.stderr)
        sys.exit(1)
    print(f"OK: no system's build needs any of its {parts} wanted parts, a planted broken one included")
    END
  '';

  parts-absent-vm = import ./parts-absent-vm.nix {
    inherit pkgs lib;
    partsTool = rust.parts;
  };
}
