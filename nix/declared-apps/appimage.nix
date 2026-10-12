# A DECLARED APPIMAGE — `{ source = "appimage"; key; name; pin; follows; icon; }`.
#
# The builder's contract is `nix/declared-apps/nix.nix`'s. The AppImage is
# fetched by its pin (`fetchurl` with the SRI hash the person's declaration
# carries — never a moving address) and wrapped by nixpkgs' own
# `appimageTools.wrapType2`, which runs it in an FHS environment without
# `appimage-run` and without anything written under `$HOME`.
#
# # The launcher entry comes from INSIDE the AppImage, inside the build
#
# `appimageTools.extract` unpacks it as a derivation, and the entry and the
# icon are read from that output by the wrapper's own install step — never at
# evaluation, which would be import-from-derivation and would download the
# AppImage to EVALUATE the system. The entry is rewritten, not copied: only
# the keys a launcher needs survive (`appimage-desktop.py`), and `Exec=` is
# ours, so a hostile `.desktop` inside an AppImage cannot choose what runs.
#
# The entry's id is the one the software centre gives an AppImage it installs
# by hand (`ronoctua-appimage-<key>.desktop`), so the declared copy and an
# imperative one are the same program to the launcher, and the reconciler
# removes the second once this one is applied.
{ lib, pkgs }:

let
  skip = reason: detail: {
    ok = false;
    inherit reason detail;
  };
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

  isKey = key: builtins.isString key && builtins.match "[a-z0-9][a-z0-9-]{0,63}" key != null;
  isSri =
    hash:
    builtins.isString hash
    && builtins.match "sha256-[A-Za-z0-9+/=]{44}|sha512-[A-Za-z0-9+/=]{88}" hash != null;
  isHttps =
    url:
    builtins.isString url
    && builtins.stringLength url <= 2048
    && builtins.match "https://[^/?#@[:space:][:cntrl:]]+([/?#][^[:space:][:cntrl:]]*)?" url != null;
  isText =
    text:
    builtins.isString text
    && text != ""
    && builtins.stringLength text <= 256
    && builtins.match ".*[[:cntrl:]\\].*" text == null;

  # A version a store path's name can carry; anything else is said as `pinned`.
  versionOf =
    v: if builtins.isString v && builtins.match "[A-Za-z0-9._+-]{1,64}" v != null then v else "pinned";

  # The person's own picture, when the declaration names one and the file is
  # the one it names (content-addressed: `<sha256>.png`).
  iconOf =
    { entry, icons }:
    let
      sha = entry.icon or null;
      file = icons + "/${sha}.png";
    in
    if icons == null || !builtins.isString sha || builtins.match "[0-9a-f]{64}" sha == null then
      null
    else if !builtins.pathExists file then
      null
    else if builtins.hashFile "sha256" file != sha then
      null
    else
      file;

  wrapped =
    { entry, icons }:
    let
      pname = "ronoctua-appimage-${entry.key}";
      version = versionOf entry.pin.version;
      src = pkgs.fetchurl {
        inherit (entry.pin) url hash;
        name = "${entry.key}-${version}.AppImage";
      };
      # The same arguments `wrapType2` hands its own `extract`, so the two are
      # one derivation and the AppImage is unpacked once.
      contents = pkgs.appimageTools.extract { inherit pname version src; };
      icon = iconOf { inherit entry icons; };
    in
    pkgs.appimageTools.wrapType2 {
      inherit pname version src;
      extraInstallCommands = ''
        ${pkgs.python3.interpreter} ${./appimage-desktop.py} \
          --contents ${contents} \
          --out "$out" \
          --key ${lib.escapeShellArg entry.key} \
          --name ${lib.escapeShellArg entry.name} \
          --program ${pname}${lib.optionalString (icon != null) " --icon ${icon}"}
      '';
      meta.mainProgram = pname;
    };
in
{
  shape =
    entry:
    if !isKey (entry.key or null) then
      "`key` is not a folded key"
    else if !isText (entry.name or null) then
      "`name` is not a name"
    else if !builtins.isAttrs (entry.pin or null) then
      "`pin` is missing"
    else if !isHttps (entry.pin.url or null) then
      "`pin.url` is not an https address"
    else if !isSri (entry.pin.hash or null) then
      "`pin.hash` is not an SRI hash"
    else
      null;

  build =
    {
      entry,
      icons ? null,
      ...
    }:
    let
      package = wrapped { inherit entry icons; };
      evaluated = builtins.tryEval package.drvPath;
    in
    if !evaluated.success then
      skip "refused-by-meta" "the wrapper of `${entry.key}` does not evaluate on this machine"
    else
      done { packages = [ package ]; };

  raw =
    {
      entry,
      icons ? null,
      ...
    }:
    wrapped { inherit entry icons; };
}
