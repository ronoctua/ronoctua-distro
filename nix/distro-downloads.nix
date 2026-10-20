# The distro's pinned downloads — the AppImages and apks of the table in
# `assets/distro-apps.nix`, at the pins of `assets/distro-pins.json` — named
# ONCE.
#
# # Why the name lives here
#
# A fixed-output path is its name and its hash. `nix run .#distro-pins`
# downloads each new release to hash it (`nix store prefetch-file`), and
# when it names the file as the build does, that download IS the build's
# source: the store has it, and the build never fetches it again. The
# tool's `file_name_of` (`nix/scripts/distro-pins.py`) writes the same two
# names; `distro-pins-move-only-what-follows` holds the tool to them.
#
# # Who reads it
#
# The registry (`nix/modules/distro-apps.nix`), for what a machine installs,
# and the flake's kept sources (`nix/kept-sources.nix`), so a garbage
# collection never takes a pinned file the internet may no longer give back
# — a release that upstream deletes is gone for good.
{ fetchurl }:

rec {
  # An AppImage: `<key>-<version>.AppImage`.
  appimage =
    {
      key,
      version,
      url,
      hash,
    }:
    fetchurl {
      inherit url hash;
      name = "${key}-${version}.AppImage";
    };

  # An apk: `<package>_<version code>.apk`, F-Droid's own file name.
  apk =
    {
      package,
      versionCode,
      url,
      hash,
    }:
    fetchurl {
      inherit url hash;
      name = "${package}_${versionCode}.apk";
    };

  # EVERY PINNED FILE of a table, for the kept sources: each pin with its
  # table line, the ones whose line is gone left out (the registry refuses
  # such a pin by name; holding it here would hold nothing anyone builds).
  pinned =
    { table, pins }:
    let
      of =
        kind: f:
        map (id: f table.${id} pins.${kind}.${id}) (
          builtins.filter (id: (table.${id}.source or null) == kind) (
            builtins.attrNames (pins.${kind} or { })
          )
        );
    in
    of "appimage" (
      t: p:
      appimage {
        inherit (t) key;
        inherit (p) version url hash;
      }
    )
    ++ of "android" (
      t: p:
      apk {
        inherit (t) package;
        inherit (p) url hash;
        versionCode = toString p.version-code;
      }
    );
}
