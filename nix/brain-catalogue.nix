# THE BRAIN'S MODULES AS DATA — `brain-catalogue.json`, the 2026-10-12 wave (B2).
#
# What the footer draws of each module — its marks and the glyphs they are
# drawn with, its notices, the actions the leader registers by name, the
# events it tells and the notification each raises, its reasons, its text
# prefixes and every key it needs — written ONCE here, out of the folders the
# loader read (`nix/brain-modules.nix`, `forCatalogue`), and read by every
# program that draws at RUN time: `RONOCTUA_BRAIN_CATALOGUE`, which the
# wrappers set (`nix/rust.nix`, `keyed`). `ronoctua_brain::catalogue` reads it,
# and `ronoctua_notify` merges its notifications into the book.
#
# WHY DATA. When this lived in the compiled tables — the glyphs in the icon
# table (`tokens.rs`), the notifications in `notifications.rs`, the keys in
# the registry, the actions in the brain library — a module of the person's
# rebuilt every crate that draws or speaks, on every machine of theirs. Now a
# person's module moves this file, the texts (also data) and the daemon that
# compiles its code, and nothing else (`person-module-rebuilds-only-the-daemon`).
#
# THE GLYPHS ARE MEASURED, not looked up: the modules' tokens alone, through
# the glyph generator's own code (`glyphdb.measure`), so a module's mark
# answers the same band beside the digit as a chrome glyph does — and the
# chrome's table (`glyphdb.db`) is the distro's whatever the person wrote.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  # The loader's view, `import ./brain-modules.nix { … }`: the flake's, or a
  # machine's with the person's folder beside the distro's.
  brainModules,
  # The glyph generator (`nix/glyphgen.nix`): `measure` for the modules'
  # tokens, `db` for the chrome's table and the Nerd Fonts names a
  # notification's `icon` may say.
  glyphdb,
  # `assets/notifications.nix` and the filters somebody shared: the ids a
  # module's event may not take.
  notificationsFile,
  sharedFile,
  # EVERY KEY NAME THE BUILD'S TEXTS HOLD — the base's and every module's
  # (`texts.allNames`, read at evaluation from the names alone): a key a
  # module needs and no text has is refused here, by the module's name.
  keyNames,
}:

let
  records = writeText "brain-catalogue-records.json" (builtins.toJSON brainModules.forCatalogue);
  measured = glyphdb.measure brainModules.glyphs;
  declared = writeText "notifications.json" (builtins.toJSON (import notificationsFile));
  keys = writeText "text-key-names.json" (builtins.toJSON (lib.unique keyNames));
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-brain-catalogue";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    python3 ${./scripts/gen-brain-catalogue.py} \
      --modules ${records} \
      --measured ${measured}/icons.json \
      --icons ${glyphdb.db}/icons.json \
      --glyph-names ${glyphdb.db}/glyphs.json \
      --notifier ${./scripts/gen-notifications.py} \
      --notifications ${declared} \
      --shared ${sharedFile} \
      --keys ${keys} \
      --out $out/brain-catalogue.json
    runHook postBuild
  '';

  dontInstall = true;

  passthru = {
    # The file the wrappers point at.
    file = "brain-catalogue.json";
    # What the generator was handed — the records, the modules' glyphs as the
    # glyph generator measured them, the distro's notification table and the
    # key names — for a check to hand it again, changed in one place.
    inherit
      records
      measured
      declared
      keys
      ;
  };

  meta = {
    description = "The brain modules' marks, actions, events, notifications and glyphs, read by the footer at run time";
    platforms = lib.platforms.all;
  };
}
