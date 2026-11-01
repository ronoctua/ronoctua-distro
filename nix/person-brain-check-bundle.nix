# THE BUNDLE `nix/person-brain-check.nix` RUNS FROM: that file at its top,
# the brain's loader and the person's hash beside it under `nix/` — the same
# files the machine's evaluation reads, copied, never rewritten — the
# languages under `assets/`, and nixpkgs' `lib`. A folder of its own in the
# store, so root's `nix eval` of one brain module imports nothing of the
# machine's flake and fetches nothing (`nix/modules/person-apply.nix`).
{
  runCommand,
  # Nixpkgs' source: only its `lib` is copied.
  path,
}:

runCommand "ronoctua-person-brain-check" { } ''
  mkdir -p $out/nix $out/assets/i18n
  cp ${./person-brain-check.nix} $out/check.nix
  cp ${./brain-modules.nix} $out/nix/brain-modules.nix
  cp ${./text-names.nix} $out/nix/text-names.nix
  cp ${./person-hash.nix} $out/nix/person-hash.nix
  cp ${../assets/i18n/languages.nix} $out/assets/i18n/languages.nix
  cp -r ${path + "/lib"} $out/lib
''
