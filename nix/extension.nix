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
  # The DEFAULT theme's palette as a flat `key -> 0xRRGGBBAA` JSON, out of
  # `nix/themes.nix`. A store PATH, read by the generator at build time — the
  # extension cannot see the person's choice, so it wears the default.
  colorsJson,
  spaceFile,
  i18nDir,
  # The languages, from `assets/i18n/languages.nix` — the same list the shell
  # compiles in. The popup used to choose between exactly two.
  languages,
  source,
  # The native-messaging hosts' names. ONE PLACE (`nix/browser-hosts.nix`):
  # the manifest of each host, the extension's `nativeMessaging` calls and the
  # Nix module that writes the hosts' own manifests all have to agree, and a
  # literal in each is a chance not to.
  hosts ? import ./browser-hosts.nix,
  # The version the manifest is built with. The packing unit adds the fourth
  # number that makes every new build a HIGHER one — see the file.
  version ? (import ./extension-version.nix).base,
}:

let
  toJson = name: file: writeText name (builtins.toJSON (import file));
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
      --languages ${lib.escapeShellArgs languages} \
      --host '${hosts.transfers}' \
      --browser-host '${hosts.browser}' \
      --version '${version}' \
      --out .
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out/share/ronoctua-extension
    # THE LIST IS WRITTEN OUT, not globbed: a file dropped into `extension/`
    # by accident would otherwise ship, signed, to every browser.
    cp manifest.json generated.js worker.js popup.html popup.css popup.js find-media.js \
      bridge.js address.js kiosk.html kiosk.js \
      $out/share/ronoctua-extension/
    runHook postInstall
  '';

  meta = {
    description = "The browser extension: sends a download to the queue, bridges the kiosk windows, and does not enlarge the fingerprint";
    platforms = lib.platforms.all;
  };
}
