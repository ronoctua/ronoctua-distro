# The Crafting Apps — the switch of their group, and nothing else.
#
# The apps themselves are lines of the table in `assets/distro-apps.nix`
# (`source = "appimage"`, `group = "crafts"`): each is the release AppImage of
# its GitHub repository, pinned in `assets/distro-pins.json` and wrapped at
# build by the registry (`nix/modules/distro-apps.nix`). What no table line
# can hold is the group's switch, which `profile.nix` turns on in the full
# profile and a `host.nix` turns on anywhere — so this module declares it,
# and the registry's `crafts` group reads it as the profile and the host set
# it (`carried`). A person still declines any one of them by its id.
#
# NO MIME DEFAULT: the apps are 0.x, and a default would send a person's
# documents to a program still finding its feet. What opens a type is
# `office.nix`'s and the other modules' business. And what the apps' own
# entries CLAIM — photographs, films, sound, documents — never opens a file by
# the catalogue's name order: each line of the table says `fallback = false`
# (`files-open-by-choice` asks the built system). They stay in "open with",
# and a default, the person's or the distro's, still reaches them.
{ lib, ... }:

{
  options.ronoctua.crafts.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      The Crafting Apps (PhotoCraft, VectorCraft, FilmCraft, LightCraft,
      PdfCraft, EffectCraft, DesignCraft, WordCraft, SoundCraft, DeckCraft,
      GridCraft, FigCraft and CADCraft), each its release AppImage, wrapped.
      On in the full profile.
    '';
  };
}
