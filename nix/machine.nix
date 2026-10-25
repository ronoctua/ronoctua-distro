# The machine's own flake — the one name of the machine directory, and the
# functions a machine's `flake.nix` calls (`lib.machine`, `lib.machineFlake`).
#
# # What a machine is
#
# Each installed machine has a flake of its own in the machine directory, and
# the distro is an INPUT of it (`templates/machine/flake.nix`):
#
#   flake.nix  flake.lock  host.nix  facter.json  [secrets.yaml]
#   person/    software/declared.json  software/declined.json
#              software/distro-pins.json  software/icons/<sha256>.png
#              brain/<my-id>/…
#   local/     held.json  verified.json  approved.json   (root only, never synced)
#
# The machine's code is the distro at the revision its lock names, so an update
# that moves the `ronoctua` input brings new code — the copy of the distro the
# machine used to carry froze it at the ISO's. What is the machine's own
# (`host.nix`, the hardware report, the person's declarations) is data beside
# it, and `machine` below turns the two into one `nixosSystem`.
#
# # Why this file is an attribute set and not a function
#
# Because the machine directory's NAME is read by three readers that have no
# NixOS configuration in hand — `nix/rust.nix` (the Rust gets it as
# `RONOCTUA_MACHINE_DIR`), the update's `steps.nix` and the option's own default
# in `nix/modules/machine.nix` — and `import ./machine.nix` must give it to them
# without asking for the distro's inputs. The functions are behind `lib`.
let
  # THE MACHINE DIRECTORY'S ONE NAME (`machine-dir-has-one-name`).
  #
  # NixOS's own convention, and the only place it is written: a module reads
  # `ronoctua.machine.dir`, a program `RONOCTUA_MACHINE_DIR`, and the check
  # refuses the literal anywhere else.
  machineDir = "/etc/nixos";

  # THE CONTRACT between the distro and a machine flake. A machine's
  # `flake.nix` says which one it was written for, and a distro that speaks
  # another refuses it with a sentence instead of building something
  # half-understood.
  contract = 1;

  # The machine directory's parts, relative to it. The same names
  # `ronoctua_person::names` holds for the Rust side (the person's half under
  # `person/`, root's under `local/`).
  parts = {
    host = "host.nix";
    secrets = "secrets.yaml";
    declared = "person/software/declared.json";
    declined = "person/software/declined.json";
    icons = "person/software/icons";
    brain = "person/brain";
    # The person's opt-ins (`ronoctua_person::names::OPTIONS`).
    options = "person/options.json";
    # The distro's AppImages the person follows past its pins
    # (`ronoctua_person::names::DISTRO_PINS`).
    distroPins = "person/software/distro-pins.json";
    held = "local/held.json";
    # The followed distro AppImages root's verify built
    # (`ronoctua_person::names::VERIFIED`).
    verified = "local/verified.json";
    approved = "local/approved.json";
    # The person's signing key root trusts, and what the last apply made of
    # the signature (`ronoctua_person::names::SIGNER`).
    signer = "local/signer.json";
    # ── anchor: L25 ──
    # The installer's marker: what nobody may redistribute waits for the
    # first network (`ronoctua_person::names::DEFERRED_HARDWARE`).
    deferred = "hardware-deferred.json";
  };
in
{
  inherit machineDir contract parts;

  # The functions, given the distro they belong to. `flake.nix` calls this once
  # with its own `self`, inputs and module list; a check calls it again with
  # inputs it tampered with (`machine-uses-the-distro-pins`).
  lib =
    {
      # The distro flake itself: its revision goes into the person's report.
      self,
      # The distro's inputs, exactly as its outputs function received them.
      inputs,
      # Every module of the distro (`distroModules` in `flake.nix`).
      distroModules,
      # The one platform the distro targets.
      system,
      # The distro's lock, which the guard compares the inputs against.
      lockFile ? ../flake.lock,
    }:
    let
      lib = inputs.nixpkgs.lib;
      personHash = import ./person-hash.nix;
      # The contract this distro speaks, named apart from `machineFlake`'s
      # argument of the same meaning.
      speaks = contract;
      # The format of the person's files (`ronoctua_person::FORMAT`).
      personFormat = 1;

      # THE REVISION OF THE DISTRO this machine is built from, for
      # `person-applied.json`. A dirty tree says so (`<rev>-dirty`).
      distroRev = self.rev or self.dirtyRev or "unknown";

      # THE MACHINE USES EXACTLY THE PINS THE GATE TESTED.
      #
      # When a machine flake locks or updates its `ronoctua` input, Nix copies
      # the distro's own inputs from the distro's `flake.lock`; only a
      # `follows`, an `--override-input` or `nix flake update ronoctua/<x>`
      # would change them, and the template has none. This is the belt over
      # those braces: every input the distro received — and every input of
      # theirs, all the way down — must carry the `narHash` the distro's own
      # lock names, or evaluation stops here with the path that differs.
      #
      # By the LOCK GRAPH and not by the direct inputs alone: a machine flake
      # that rewires `ronoctua/lanzaboote/nixpkgs` changes nothing the root
      # names, and would still build against an untested nixpkgs.
      pinGuard =
        {
          lockFile,
          inputs,
        }:
        let
          lock = builtins.fromJSON (builtins.readFile lockFile);
          nodes = lock.nodes;
          # A `follows` is a path of input names from the root; anything
          # else is a node's key.
          keyOf =
            ref:
            if builtins.isList ref then
              builtins.foldl' (key: name: keyOf nodes.${key}.inputs.${name}) lock.root ref
            else
              ref;
          # Every input the distro evaluated, beside the node its lock names.
          visit =
            path: given: key:
            let
              node = nodes.${key};
              # The root's inputs ARE `given`; an input's are `given.inputs`.
              below = if key == lock.root then given else given.inputs or { };
              mine =
                if key == lock.root then
                  [ ]
                else
                  [
                    {
                      path = lib.concatStringsSep "/" path;
                      got = given.narHash or null;
                      want = node.locked.narHash or null;
                    }
                  ];
            in
            mine
            ++ lib.concatLists (
              lib.mapAttrsToList (name: ref: visit (path ++ [ name ]) (below.${name} or { }) (keyOf ref)) (
                node.inputs or { }
              )
            );
          wrong = builtins.filter (p: p.want != null && p.got != p.want) (visit [ ] inputs lock.root);
        in
        if wrong == [ ] then
          true
        else
          throw (
            "lib.machine: this machine does not build with the distro's pins. "
            + "The distro was tested with what its flake.lock names, and the machine flake "
            + "changed "
            + lib.concatMapStringsSep ", " (p: "`${p.path}` (${toString p.got} instead of ${p.want})") wrong
            + ". Remove the `follows` or the `--override-input` that did it "
            + "(machine-uses-the-distro-pins)."
          );

      # THE PERSON'S BRAIN MODULES, and which of them may reach root.
      #
      # A module is a folder `person/brain/<my-id>/`. Without a `module.nix` it
      # runs as the person only, inside their brain, and it is included. WITH
      # one it reaches root — a NixOS module is the whole machine — so it is
      # included only when `local/approved.json` names it with the hash
      # `nix/person-hash.nix` computes of the folder AS IT IS NOW. An edit after
      # the approval changes the hash, and the module waits for a new one; the
      # report lists it with the hash an approval must name
      # (`person-root-module-needs-approval`).
      #
      # THE ROOT HALF IS IMPORTED FROM A COPY OF ITS OWN FOLDER, alone in the
      # store, and that copy is what is hashed. In place, inside the machine
      # flake's source, `imports = [ ../my-common/lib.nix ]` or
      # `builtins.readFile ../my-common/run.sh` would reach a sibling — and a
      # sibling with no `module.nix` needs no approval, so whoever can change
      # the person's data would change root code the approval never saw.
      # From the copy, `../` is the store's root: nothing there is a sibling,
      # and a pure evaluation finds nothing (`person-root-module-needs-approval`).
      #
      # What isolation cannot stop is a module that NAMES a path through the
      # configuration (`config.ronoctua.facterReport` is a file of the
      # machine's source): that is written in the approved text, where whoever
      # approves reads it.
      #
      # Decided HERE and not in a NixOS module because it decides `imports`,
      # and a module's `imports` cannot depend on the configuration.
      approvals =
        dir:
        let
          brainDir = dir + "/${parts.brain}";
          approvedFile = dir + "/${parts.approved}";
          approvedDoc =
            if builtins.pathExists approvedFile then
              builtins.fromJSON (builtins.readFile approvedFile)
            else
              { };
          # A format this distro does not read approves nothing.
          approved =
            if
              (approvedDoc.format or null) == personFormat && builtins.isAttrs (approvedDoc.modules or null)
            then
              approvedDoc.modules
            else
              { };
          listing = if builtins.pathExists brainDir then builtins.readDir brainDir else { };
          ids = builtins.filter (
            name: listing.${name} == "directory" && builtins.match "my-[a-z0-9][a-z0-9-]{0,63}" name != null
          ) (builtins.attrNames listing);
          one =
            id:
            let
              folder = brainDir + "/${id}";
              rootHalf = builtins.pathExists (folder + "/module.nix");
              # A folder the hash refuses (a link, a name it cannot write)
              # is never copied, never imported, and says so.
              hashable = (builtins.tryEval (personHash folder)).success;
              isolated = builtins.path {
                path = folder;
                name = "person-brain-${id}";
              };
              hash = if hashable then personHash isolated else null;
              given = approved.${id}.hash or null;
            in
            {
              inherit id folder rootHalf;
              # Only the root half needs the copy (and the hash).
              rootModule = isolated + "/module.nix";
              hash = if hash == null then "unhashable" else hash;
              included = !rootHalf || (hash != null && given == hash);
            };
          every = map one ids;
        in
        {
          included = map (m: m.folder) (builtins.filter (m: m.included) every);
          awaiting = map (m: { inherit (m) id hash; }) (builtins.filter (m: !m.included) every);
          rootModules = map (m: m.rootModule) (builtins.filter (m: m.included && m.rootHalf) every);
        };

      # THE MACHINE: the distro's modules, `host.nix`, the hardware report's
      # module, the person's data and the approved root halves.
      machine =
        {
          # The machine flake's own source (`./.` in its `flake.nix`).
          dir,
          system ? defaultSystem,
          # What a check or a reference system adds.
          extraModules ? [ ],
        }:
        let
          at = name: dir + "/${name}";
          present = name: if builtins.pathExists (at name) then at name else null;
          brain = approvals dir;
        in
        assert pinGuard { inherit lockFile inputs; };
        if !builtins.pathExists (at parts.host) then
          throw (
            "lib.machine: ${toString dir} has no ${parts.host}. A machine flake is the "
            + "distro plus this machine's own file; the installer writes it, and "
            + "`nix flake init -t <distro>#machine` gives only the flake."
          )
        else
          inputs.nixpkgs.lib.nixosSystem {
            inherit system;
            specialArgs = { inherit inputs; };
            modules =
              distroModules
              ++ [
                ./modules/host.nix
                inputs.nixos-facter-modules.nixosModules.facter
                (at parts.host)
                {
                  ronoctua.machine = {
                    source = dir;
                    inherit distroRev;
                  };
                  ronoctua.hardware.deferUndistributable = builtins.pathExists (at parts.deferred);
                  ronoctua.person = {
                    data = present parts.declared;
                    declined = present parts.declined;
                    held = present parts.held;
                    verified = present parts.verified;
                    icons = present parts.icons;
                    options = present parts.options;
                    distroPins = present parts.distroPins;
                    signer = present parts.signer;
                    brainModules = brain.included;
                    awaitingApproval = brain.awaiting;
                  };
                }
              ]
              ++ brain.rootModules
              ++ extraModules;
          };

      defaultSystem = system;

      # WHAT THE UPDATE'S VERIFICATION EVALUATES, one entry at a time: each
      # entry in the machine as `ronoctua.person.alone` makes it — its own
      # unfree consent and no other, held or not — so the entry is judged by
      # itself. Through the machine as it is, a held sibling whose reading
      # aborts would fail every consented entry that reads the predicate,
      # and hold them all. The names come from the fingerprints, which read
      # JSON only.
      entriesOf =
        built:
        lib.mapAttrs (
          id: _:
          (built.extendModules {
            modules = [ { ronoctua.person.alone = id; } ];
          }).config.ronoctua.person.entries.${id}
        ) built.config.ronoctua.person.fingerprints;

      # WHAT A MACHINE'S `flake.nix` RETURNS.
      #
      # `nixosConfigurations.<attr>` is named after the person
      # (`ronoctua.machine.attr`, which is `ronoctua.user`): the same name the
      # rescue's rebuild and `ronoctua-machine apply` ask for
      # (`machine-flake-target-exists`). `personEntries` and
      # `personFingerprints` are what the update's isolated verification
      # evaluates one entry at a time — never through the graceful consumer,
      # whose job is to skip what that verification has to see fail.
      machineFlake =
        {
          dir,
          contract ? 1,
          system ? defaultSystem,
          extraModules ? [ ],
        }:
        let
          built = machine { inherit dir system extraModules; };
        in
        if contract != speaks then
          throw (
            "lib.machineFlake: this machine's flake.nix was written for contract ${toString contract}, "
            + "and this distro speaks ${toString speaks}. Make it again from the template "
            + "(`nix flake init -t <distro>#machine`) and keep host.nix, facter.json and person/."
          )
        else
          {
            nixosConfigurations.${built.config.ronoctua.machine.attr} = built;
            personEntries = entriesOf built;
            personFingerprints = built.config.ronoctua.person.fingerprints;
          };
    in
    {
      inherit
        machine
        machineFlake
        pinGuard
        approvals
        distroRev
        entriesOf
        ;
      # The same functions over other inputs: what a machine flake with a
      # `follows` would hand the distro. Only a check asks for it.
      withInputs =
        other:
        (import ./machine.nix).lib {
          inherit
            self
            distroModules
            system
            lockFile
            ;
          inputs = other;
        };
    };
}
