# THE PERSON'S BRAIN MODULES, compiled into their machine — the 2026-10-05
# wave (belongings.md WP8b), as data since the 2026-10-12 wave (B2).
#
# A machine whose person wrote modules of their own (`my-…`) builds four
# links again with the person's folder beside the distro's, and no other:
#
# - THE DAEMON'S REGISTRY (`brainModulesRs`): the daemon compiles a person's
#   `module.rs` and manifest — the one piece of the person's module that is
#   code;
# - THE BRAIN'S CATALOGUE (`brainCatalogue`): its marks and their glyphs, its
#   notices, actions, events and notifications, which the footer reads at RUN
#   time (`RONOCTUA_BRAIN_CATALOGUE`);
# - THE TEXTS (`texts`): its sentences, read at run time too. The key
#   registry the workspace is compiled against stays the distro's: a
#   person's keys are names the catalogue carries, never constants
#   (`nix/i18n.nix`);
# - THE GLYPH BROWSER'S INDEX (`iconset`): the icon browser and the fast
#   access name a module's glyph by its token — the person's too — out of
#   the catalogue's measured glyphs (`nix/iconset.nix`), read at run time
#   (`RONOCTUA_ICONSET`).
#
# EVERYTHING ELSE IS THE DISTRO'S, the same derivations: the icon table and
# the tokens (a module's glyph is measured into the catalogue,
# `nix/glyphgen.nix`'s `measure`), the notification table, the WIT and the
# six components, the brain's library (`brainManifestsRs`, the distro's
# manifests the footer's crate compiles) and every crate that draws or speaks
# — so the person's module rebuilds the daemon and the programs' wrappers,
# and nothing that draws (`person-module-rebuilds-only-the-daemon`).
#
# So this OVERRIDES the four links (`callPackage`'s `.override`) and writes
# none of them again: the same generators with the same arguments, one folder
# more. What it hands back has the shape the flake's `let` has — `rust`
# (every program, the brain and the footer included) and the loader — plus
# what `lib.machine` needs whole: `machineModules`, the distro's modules run
# with this `rust` and the approved NixOS halves, and `refused`, the folders
# left out with why.
#
# A FOLDER THAT DOES NOT LOAD IS LEFT OUT, never fatal (the loader's
# `personRefused`): it travels to every machine of the person, and a `throw`
# would stop them all — a key it needs that no text has included, asked of
# its texts at evaluation (`baseKeyNames`). When none loads, every link is
# the distro's own — the same derivations, so the binary cache has them.
{
  lib,
  languages,
  # The person's modules: one folder per module, `my-<id>/` — the machine
  # directory's `person/brain/`, a store path.
  dir,
  # The folders to read, by id, or `null` for every one: the machine leaves
  # out the ones it does not include, and the update's isolated verification
  # the ones that do not even evaluate (`personModulesOnly`).
  only ? null,
  # The flake's `distroModulesFor`: the distro's NixOS modules for a `rust`.
  distroModulesFor,
  texts,
  brainModulesRs,
  # The library's part, the distro's — never overridden here.
  brainManifestsRs,
  brainCatalogue,
  iconset,
  rust,
}:

let
  brainModules = import ./brain-modules.nix {
    inherit lib languages;
    personModulesDir = dir;
    personModulesOnly = only;
    # What a person's notification may name outside its own fragment.
    baseKeyNames = texts.baseNames;
  };
  none = brainModules.personIds == [ ];
  # THE TEXTS WITH THE PERSON'S FRAGMENTS: the sentences the footer reads at
  # run time. The key names each crate compiles (`texts.keys`) stay the
  # distro's, byte for byte: a person's fragment adds sentences, never a name a
  # crate is compiled with.
  machineTexts = texts.override { brainFragments = brainModules.i18nFragments; };
  # THE CATALOGUE WITH THE PERSON'S MODULES, asked of the person's texts.
  catalogue = brainCatalogue.override {
    inherit brainModules;
    keyNames = machineTexts.allNames;
  };
  # THE GLYPH BROWSER'S INDEX, naming the person's glyph tokens beside the
  # distro's — out of the glyphs the person's catalogue measured.
  machineIconset = iconset.override { brainGlyphs = catalogue.measured; };
  registry = brainModulesRs.override { inherit brainModules; };
  machineRust = rust.override {
    texts = machineTexts;
    brainModulesRs = registry;
    brainCatalogue = catalogue;
    iconset = machineIconset;
  };
  chosen = if none then rust else machineRust;
  halves = brainModules.personNixosModules;
in
{
  inherit brainModules;
  # THE FOLDERS LEFT OUT, by id, each with its sentences — for the machine's
  # report (`person-applied.json`), so a person learns why a module of theirs
  # is not running.
  refused = brainModules.personRefused;
  # Each link, for whoever asks what it was given (a check).
  brainModulesRs = if none then brainModulesRs else registry;
  brainCatalogue = if none then brainCatalogue else catalogue;
  texts = if none then texts else machineTexts;
  iconset = if none then iconset else machineIconset;
  # AND WHAT IS NOT A LINK: the library's part, the distro's whatever the
  # person wrote.
  inherit brainManifestsRs;
  rust = chosen;
  # THE MACHINE'S NIXOS MODULES: the distro's, every `rust.*` of them this
  # one (the brain that compiled the person's modules, the footer that
  # reads their catalogue), and the NixOS half of each APPROVED id — through
  # the loader's gate, never the raw `module.nix`: the gate keeps it shut
  # while the brain is off and refuses an import outside its folder and a
  # `disabledModules`. An approved id with no half, or one left out, adds
  # nothing.
  machineModules =
    approved:
    distroModulesFor chosen ++ map (id: halves.${id}) (lib.filter (id: halves ? ${id}) approved);
}
