# ONE OF THE PERSON'S BRAIN MODULES, EVALUATED ALONE — what root asks of
# each folder a machine would compile, before any evaluation of the machine
# reads it (`ronoctua-person check-brain`, which `apply` and `approve` run
# too; `ronoctua_person::local::BrainChecked`).
#
# A manifest that does not parse, aborts or recurses for ever is an error
# `tryEval` cannot catch: read by the machine's evaluation, it stops all of
# it, and the person's data would break the machine's update. Read here, in a
# `nix eval` of its own, it stops that process alone; root records the folder
# as not evaluating, and `lib.machine` leaves it out and says why
# (`nix/machine.nix`, `approvals`).
#
# Two questions, the folder and the id in the environment, never spliced
# into the expression:
#
#   (import <this>).hashes  "<brain folder>"         → { <id> = "sha256-…" or null; }
#   (import <this>).verdict "<brain folder>" "<id>"  → [ "<the loader's sentence>" … ]
#
# `hashes` reads no manifest: a folder whose NixOS half waits for an
# approval is hashed, and never evaluated. `verdict` is the brain's loader
# reading that one folder all the way down (`personVerdictOf`): an empty list
# is a module, sentences are one the loader leaves out by itself — either way
# it evaluated.
#
# It runs from a bundle of its own (`nix/person-brain-check-bundle.nix`):
# this file at its top, the loader and the hash beside it under `nix/`, the
# languages under `assets/`, and nixpkgs' `lib`.
let
  lib = import ./lib;
  hash = import ./nix/person-hash.nix;
  # THE LOADER TOLD TO READ THE ONE FOLDER: what it checks across modules
  # (two that generate one name) reads every folder it was told to, and a
  # neighbour that aborts would stop this one's process.
  loader =
    dir: id:
    import ./nix/brain-modules.nix {
      inherit lib;
      languages = import ./assets/i18n/languages.nix;
      # The person's folder alone: no distro module is read.
      modulesDir = null;
      personModulesDir = /. + dir;
      personModulesOnly = [ id ];
      restingApproved = [ ];
    };
in
{
  hashes =
    dir:
    let
      at = /. + dir;
      listing = builtins.readDir at;
    in
    builtins.mapAttrs (
      id: type:
      let
        hashed = builtins.tryEval (hash (at + "/${id}"));
      in
      if type == "directory" && hashed.success then hashed.value else null
    ) listing;

  verdict = dir: id: (loader dir id).personVerdictOf.${id};
}
