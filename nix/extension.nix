# The browser extension, assembled — and its texts and colours GENERATED.
#
# Invariants 8 and 9 do not stop at the browser's edge. Every word the popup
# shows comes from `assets/i18n/*.ftl` and every colour from
# `assets/tokens/colors.nix`, both of them arriving here as JSON — the same
# Nix-to-Python bridge `nix/tokens.nix` uses, and for the same reason: there is
# no Nix parser in Python and writing one would be reimplementing Nix logic
# (invariant 5).
#
# `extension-has-no-literal` fails the build if a word or a colour ever appears
# in the extension's own sources, and `extension-has-no-fingerprint` fails it if
# the manifest ever grows one of the five things that make an extension visible
# to a website.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  colorsFile,
  spaceFile,
  i18nDir,
  source,
  # The native-messaging host's name. ONE PLACE: the manifest of the host, the
  # extension's `nativeMessaging` call and the Nix module that writes the host's
  # own manifest all have to agree, and three literals is three chances not to.
  hostName ? "br.ronoctua.transfers",
  version ? "1.0.0",
}:

let
  toJson = name: file: writeText name (builtins.toJSON (import file));
  colorsJson = toJson "colors.json" colorsFile;
  spaceJson = toJson "space.json" spaceFile;
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-extension";
  inherit version;

  src = source;

  nativeBuildInputs = [ python3 ];

  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    python3 ${./scripts/gen-extension.py} \
      --colors ${colorsJson} \
      --space ${spaceJson} \
      --i18n ${i18nDir} \
      --host '${hostName}' \
      --version '${version}' \
      --out .
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out/share/ronoctua-extension
    cp manifest.json generated.js worker.js popup.html popup.css popup.js find-media.js \
      $out/share/ronoctua-extension/
    runHook postInstall
  '';

  meta = {
    description = "The browser extension: sends a download to the queue, and does not enlarge the fingerprint";
    platforms = lib.platforms.all;
  };
}
