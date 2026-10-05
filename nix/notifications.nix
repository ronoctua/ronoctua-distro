# The notification catalogue and filters, from Nix to Rust — section 4.2.
#
# "Every notification and every pre-created filter of the distro, centralised."
# One file (`assets/notifications.nix`) becomes three things here, and the third
# is the one that matters most:
#
#   notifications.rs   the table `ronoctua_notify::catalogue` includes
#   keys.txt           the text keys BOTH languages have to carry
#   a refusal          an invalid field takes the build down, with the id
#
# `keys.txt` is what makes "all of the distro's notifications exist in English
# and Portuguese" stop depending on discipline: the `notifications-have-text`
# check reads it and demands every line of both `.ftl`s.
#
# And a SECOND source of filters, `assets/shared/notifications.json`: the ones
# somebody wrote on their own machine and decided everybody should have. They
# are already JSON — written by the shell, in the shape `Rule` serialises — so
# this derivation hands the file straight to the generator, which refuses
# anything malformed in it exactly as it refuses a bad field in the Nix.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  glyphdb,
  notificationsFile,
  sharedFile,
  # THE BRAIN MODULES' EVENTS that declared a notification (brain.md §3), as
  # entries of this same catalogue: `<module>-<event>`, recognised by that tag.
  # The flake's one loader's `notifications`.
  brainNotifications,
}:

let
  declared = import notificationsFile;
  # ONE ID, ONE ENTRY: a module's event named like an entry of the file would
  # be two recognitions of one tag, and the first in the table would win.
  clashes = builtins.filter (n: builtins.hasAttr n declared.entries) (
    builtins.attrNames brainNotifications
  );
  merged =
    assert lib.assertMsg (clashes == [ ])
      "a brain module's event is also an entry of assets/notifications.nix: ${lib.concatStringsSep ", " clashes}";
    declared // { entries = declared.entries // brainNotifications; };
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-notifications-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-notifications.py} \
      --notifications ${writeText "notifications.json" (builtins.toJSON merged)} \
      --shared ${sharedFile} \
      --icons ${glyphdb.db}/icons.json \
      --glyphs ${glyphdb.db}/glyphs.json \
      --out $out/notifications.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Notification catalogue and filters, generated from assets/notifications.nix";
    platforms = lib.platforms.all;
  };
}
