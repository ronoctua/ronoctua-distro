# Registry of text keys, from Fluent to Rust — invariant 8.
#
# Same shape as the tokens (`nix/tokens.nix`): the data is defined ONCE, in
# `assets/i18n/*.ftl`, and the build generates the Rust table. Referencing a
# key that does not exist becomes a compile error; an incomplete `.ftl`
# becomes a build error HERE, before any test.
{
  lib,
  stdenvNoCC,
  python3,
  i18nDir,
  # The languages, from `assets/i18n/languages.nix`. Passed in rather than
  # imported here so there is exactly one `import` of that file in the flake.
  languages,
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
    python3 ${./scripts/gen-i18n.py} ${i18nDir} $out/keys.rs ${lib.escapeShellArgs languages}
    # The .ftl files themselves come along: the crate embeds them with
    # `include_str!`, and separating the registry from the texts would let the two
    # diverge.
    cp ${i18nDir}/*.ftl $out/
    runHook postBuild
  '';

  installPhase = "true";

  meta.description = "Text keys generated from the .ftl files";
}
