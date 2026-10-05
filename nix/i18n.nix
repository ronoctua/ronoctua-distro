# Registry of text keys, from Fluent to Rust — invariant 8.
#
# Same shape as the tokens (`nix/tokens.nix`): the data is defined ONCE, in
# `assets/i18n/*.ftl`, and the build generates the Rust table. Referencing a
# key that does not exist becomes a compile error; an incomplete `.ftl`
# becomes a build error HERE, before any test.
#
# AND THE BRAIN'S MODULES' TEXTS WITH THEM (brain.md §3). A module carries its
# own `i18n/<lang>.ftl` in its folder; each folder is handed to the generator
# with the prefixes its keys may begin with, and what comes out is ONE
# registry and ONE merged `<lang>.ftl` per language, the base first. The crate
# embeds the merged files (`face()` reads `RONOCTUA_I18N_DIR`), so the
# registry and the bundle cannot disagree; with no module the merged file is
# the base, byte for byte.
{
  lib,
  stdenvNoCC,
  python3,
  i18nDir,
  # The languages, from `assets/i18n/languages.nix`. Passed in rather than
  # imported here so there is exactly one `import` of that file in the flake.
  languages,
  # The modules' text folders, `[ { id; prefixes; dir; } ]`: the flake's one
  # loader's `i18nFragments`. A name nixpkgs has no package for: `callPackage`
  # fills an argument from `pkgs` first, and `fragments` is one.
  brainFragments,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-i18n-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    # The merged `<lang>.ftl` are written beside `keys.rs` by the generator
    # itself: the crate embeds them with `include_str!`, and a copy made here
    # separately would let the registry and the texts diverge.
    python3 ${./scripts/gen-i18n.py} ${i18nDir} $out/keys.rs ${lib.escapeShellArgs languages} ${
      lib.concatMapStringsSep " " (
        f:
        "--module ${lib.escapeShellArg "${f.id}=${f.dir}"} --prefixes ${lib.escapeShellArg "${f.id}=${lib.concatStringsSep "," f.prefixes}"}"
      ) brainFragments
    }
    runHook postBuild
  '';

  installPhase = "true";

  meta.description = "Text keys generated from the .ftl files, with the brain modules' merged in";
}
