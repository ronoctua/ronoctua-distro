# What the ISO carries so an installation needs NO download — computed, never
# listed by hand. See docs/ARCHITECTURE.md, "Instalação offline".
#
# An installed system is the reference's store paths plus a few dozen
# derivations that depend on the answers and the hardware (`etc`, the
# initrd, the units that name the host, the locale, the keyboard…). Those
# are BUILT on the target, and building them needs their inputs: tools no
# runtime closure holds (`make-initrd-ng`, `cpio`, the stdenv, perl's
# environment…). Without them, an offline `nixos-install` asked for 4684
# derivations, starting from the bootstrap seed.
#
# So the ISO carries, for every system below:
#
#   1. its runtime closure — the references, and the probes, whose hardware
#      brings packages no reference has (microcode, the GRUB of a BIOS
#      machine, a guest's modules);
#   2. THE GLUE: the outputs every derivation a probe builds differently
#      from the references takes from derivations that do NOT differ. Found
#      by walking the `.drv` graphs (`nix/scripts/offline-glue.py`): the
#      references' whole, a probe's only below what differs;
#   3. every node of the lock (`lockSources`), which evaluating the machine's
#      flake fetches;
#   4. the parts it seeds (`seeded`): their packages, so a part the plan
#      wants arrives with no network on the first boot.
{
  lib,
  runCommand,
  python3,
  # The systems an installation builds the same store paths as.
  references,
  # Systems that answer and look unlike the references (`tests/fixtures/`).
  probes,
  # Every source the flake's lock names, all the way down.
  lockSources,
  # "free-minimal": the free parts the minimal reference wants (the
  # published ISO). "every-part": every part of every reference, unfree
  # included (`.#iso-complete`, never published).
  seeded,
}:

assert lib.assertMsg
  (builtins.elem seeded [
    "free-minimal"
    "every-part"
  ])
  "nix/installer/offline.nix: seeded is \"free-minimal\" or \"every-part\", not ${builtins.toJSON seeded}";

let
  # WHAT AN INSTALLATION BUILDS of a system: its top, and the script `disko`
  # partitions with (`disko --mode destroy,format,mount --flake`, which
  # builds `destroyFormatMount`), which names the disk.
  #
  # Each `.drv` with no output dependency: as an input it brings the `.drv`
  # files and the sources of its graph into the sandbox, and builds nothing
  # (the plain `drvPath`, a deep context, would make every output of the
  # graph an input).
  graphsOf =
    system:
    map (d: builtins.unsafeDiscardOutputDependency d.drvPath) [
      system.config.system.build.toplevel
      system.config.system.build.destroyFormatMount
    ];

  # THE GLUE, read from the `.drv` files at BUILD time. A pure evaluation
  # cannot read a `.drv` (`readFile` of one is refused), so the walk is a
  # derivation, and its answer is read back (import-from-derivation) — the
  # one this tree makes, and only for the ISO's contents: it builds in well
  # under a second from files already in the store, and fetches nothing.
  #
  # The answer names store paths, so its text carries them as context; the
  # context goes, and each output gets its own back below.
  walk = runCommand "ronoctua-offline-glue.json" { nativeBuildInputs = [ python3 ]; } ''
    python3 ${../scripts/offline-glue.py} \
      --references ${lib.escapeShellArgs (lib.concatMap graphsOf references)} \
      --probes ${lib.escapeShellArgs (lib.concatMap graphsOf probes)} > $out
  '';
  walked = builtins.fromJSON (builtins.unsafeDiscardStringContext (builtins.readFile walk));
  # Each output as a store path WITH its derivation's context, so the ISO's
  # closure builds it where the store does not hold it yet.
  glue = map (g: builtins.appendContext g.path { ${g.drv}.outputs = [ g.output ]; }) walked.glue;

  partsOf =
    system:
    lib.filter (p: p.wanted && (if seeded == "every-part" then true else !p.unfree && !p.heavy)) (
      lib.attrValues system.config.ronoctua.parts
    );
  seededParts =
    if seeded == "every-part" then
      lib.concatMap partsOf references
    else
      partsOf (
        lib.findFirst (
          s: s.config.ronoctua.profile == "minimal"
        ) (throw "nix/installer/offline.nix: no reference is the minimal profile") references
      );

in
{
  contents =
    map (s: s.config.system.build.toplevel) (references ++ probes)
    ++ glue
    ++ lockSources
    ++ map (p: p.package) seededParts;

  # The walk's answer, for whoever wants to read it whole.
  inherit walk;

  # What a check reads: counts, beside the walk (built in under a second).
  summary = {
    inherit (walked) differing;
    glue = builtins.length glue;
    lockSources = builtins.length lockSources;
    seeded = map (p: p.name) seededParts;
  };
}
