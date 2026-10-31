# The crafts group — its switch, and nothing else.
#
# The group's programs are lines of the table in `assets/distro-apps.nix`
# (`source = "appimage"`, `group = "crafts"`): since 2026-10-30 two of them,
# the ArtCraft Launcher, which downloads and updates the Crafting Apps by
# itself under the person's home and outside the generation, and SolidCraft.
# Each is the release AppImage of its GitHub repository, pinned in
# `assets/distro-pins.json`, wrapped at build by the registry
# (`nix/modules/distro-apps.nix`) and a part. What no table line can hold is
# the group's switch, which `profile.nix` turns on in the full profile and a
# `host.nix` turns on anywhere — so this module declares it, and the
# registry's `crafts` group reads it as the profile and the host set it
# (`carried`). A person still declines either by its id.
#
# NO MIME DEFAULT: nothing of the group is chosen to open a type. And what
# their entries CLAIM never opens a file by the catalogue's name order: each
# line of the table says `fallback = false` (`files-open-by-choice` asks the
# built system). They stay in "open with", and a default, the person's or the
# distro's, still reaches them. The crafts the launcher fetches are not the
# distro's: it neither wraps them nor writes their launchers, and what they
# claim has not been measured (`docs/PENDING.md`).
{ lib, ... }:

{
  options.ronoctua.crafts.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      The craft tools, each its release AppImage, wrapped. The group now
      carries the ArtCraft Launcher — which downloads and updates the Crafting
      Apps (PhotoCraft, VectorCraft, FilmCraft and the rest) by itself, under
      the person's home and outside the generation — and SolidCraft, parametric
      3D CAD. On in the full profile.
    '';
  };
}
