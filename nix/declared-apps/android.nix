# A DECLARED ANDROID APP — `{ source = "android"; package; pin; from; image; }`.
#
# The builder's contract is `nix/declared-apps/nix.nix`'s. Nix cannot install
# anything inside Android: the image is a Waydroid container that exists only
# while the person's session runs, and `adb install` is not a build step. So
# the system FETCHES the apk by its pin — into the store, where the next
# collection cannot take it while the generation names it — and lists it in
# `/etc/ronoctua/declared-android.json`; the software centre's reconciler
# installs from there, as the person, while the session runs (and says
# "pending" otherwise).
#
# `declarer` is who declared it (`person` here; the distro's registry passes
# `distro`), which the reconciler shows and the person's half may not remove.
# `lib` is the contract's argument; nothing here needs more than builtins.
{ pkgs, ... }:

let
  done =
    what:
    {
      ok = true;
      packages = [ ];
      flatpaks = [ ];
      desktopEntries = { };
      apks = [ ];
    }
    // what;

  isPackage =
    name:
    builtins.isString name
    && builtins.stringLength name <= 255
    && builtins.match "[A-Za-z][A-Za-z0-9_]*([.][A-Za-z][A-Za-z0-9_]*)+" name != null;
  isSri =
    hash:
    builtins.isString hash
    && builtins.match "sha256-[A-Za-z0-9+/=]{44}|sha512-[A-Za-z0-9+/=]{88}" hash != null;
  isHttps =
    url:
    builtins.isString url
    && builtins.stringLength url <= 2048
    && builtins.match "https://[^/?#@[:space:][:cntrl:]]+([/?#][^[:space:][:cntrl:]]*)?" url != null;

  apk =
    entry:
    pkgs.fetchurl {
      inherit (entry.pin) url hash;
      name = "${entry.package}_${toString entry.pin."version-code"}.apk";
    };
in
{
  shape =
    entry:
    if !isPackage (entry.package or null) then
      "`package` is not an Android package"
    else if !builtins.isAttrs (entry.pin or null) then
      "`pin` is missing"
    else if !isHttps (entry.pin.url or null) then
      "`pin.url` is not an https address"
    else if !isSri (entry.pin.hash or null) then
      "`pin.hash` is not an SRI hash"
    else if !builtins.isInt (entry.pin."version-code" or null) || entry.pin."version-code" < 0 then
      "`pin.version-code` is not a number"
    else if
      !builtins.elem (entry.image or null) [
        "gapps"
        "vanilla"
      ]
    then
      "`image` is neither gapps nor vanilla"
    else
      null;

  build =
    {
      entry,
      declarer ? "person",
      ...
    }:
    done {
      apks = [
        {
          id = "android:${entry.package}";
          inherit (entry) package image;
          apk = "${apk entry}";
          version-code = entry.pin."version-code";
          from = declarer;
        }
      ];
    };
}
