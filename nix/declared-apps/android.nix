# A DECLARED ANDROID APP — `{ source = "android"; package; pin; from; image; }`.
#
# The builder's contract is `nix/declared-apps/nix.nix`'s. Nix cannot install
# anything inside Android: the image is a Waydroid container that exists only
# while the person's session runs, and `adb install` is not a build step. So
# the apk is a DATA PART of the person's (`nix/modules/person.nix`): its
# realizer fetches it by its pin after the system, on its own — an apk that
# does not download fails that part alone, never the system — and the system
# lists it in `/etc/ronoctua/declared-android.json` by its part and its file,
# never by a store path. The software centre's reconciler installs it from
# the part's path, as the person, while the session runs and once it arrived
# (and says "pending" otherwise).
#
# `inPart` and `record` are the ONE shape of an apk's part and of its line in
# the list, the distro's registry's too (`nix/modules/distro-apps.nix`).
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

  # THE APK'S FILE in its part: F-Droid's own name, `<package>_<version
  # code>.apk`. The version is in the name, so a part still at its previous
  # generation — the new pin not arrived yet — is never read as the new
  # version (`ronoctua_person::reports::DeclaredApk`).
  fileOf = package: versionCode: "${package}_${toString versionCode}.apk";
in
rec {
  # AN APK'S PART: the folder its profile holds, the apk under its file name.
  inPart =
    {
      package,
      versionCode,
      src,
    }:
    pkgs.linkFarm "android-${package}" [
      {
        name = fileOf package versionCode;
        path = src;
      }
    ];

  # ITS LINE in `declared-android.json`: the part it arrives in and its file
  # there — text, with no store path, so the system never holds the apk.
  record =
    {
      package,
      image,
      versionCode,
      part,
      from,
    }:
    {
      id = "android:${package}";
      inherit
        package
        image
        part
        from
        ;
      apk = fileOf package versionCode;
      version-code = versionCode;
    };

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

  # THE PART'S TARGET, which its realizer evaluates alone.
  build =
    { entry, ... }:
    done {
      packages = [
        (inPart {
          inherit (entry) package;
          versionCode = entry.pin."version-code";
          src = pkgs.fetchurl {
            inherit (entry.pin) url hash;
            name = fileOf entry.package entry.pin."version-code";
          };
        })
      ];
    };

  # THE PERSON'S LINE, for the part `part`.
  listed =
    { entry, part }:
    record {
      inherit (entry) package image;
      inherit part;
      versionCode = entry.pin."version-code";
      from = "person";
    };
}
