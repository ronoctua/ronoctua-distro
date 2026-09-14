# A declared option table, from Nix to Rust — the twin of `nix/audio-names.nix`.
#
# `assets/<domain>-options.nix` becomes two things here:
#
#   options.rs  the table `ronoctua_<domain>::options::SETTINGS` is
#   keys.txt    the text keys BOTH languages have to carry
#
# and a third that has no file: an entry with a duplicate id, a number with no
# range, a ring that cannot turn or two rows writing the same field takes the
# build down naming it. `keys.txt` is what makes `<domain>-options-have-text`
# possible.
#
# ONE DERIVATION, FOUR TABLES. This was `nix/syncthing-options.nix`, and it was
# never about Syncthing: Tor's bridges, ClipCascade's client and server, and
# Atuin's seventy settings are the same problem. What differs is the key prefix
# and the scopes, so they are arguments.
#
# THE SCOPES ARE ORDERED HERE AND NOT IN THE ASSET, because the order is the
# order the screen draws in and `builtins.toJSON` sorts an attribute set
# alphabetically. It is the same reason `assets/themes/themes.nix` carries an
# `order` beside its list.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  # The text-key prefix: `syncthing` gives `syncthing-opt-<id>`.
  domain,
  # The scopes, in the order the screen draws them.
  scopes,
  optionsFile,
}:

stdenvNoCC.mkDerivation {
  pname = "ronoctua-${domain}-options-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-options.py} \
      --domain ${domain} \
      --scopes ${lib.concatStringsSep "," scopes} \
      --options ${writeText "${domain}-options.json" (builtins.toJSON (import optionsFile))} \
      --out $out/options.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "${domain}'s options, generated from assets/${domain}-options.nix";
    platforms = lib.platforms.all;
  };
}
