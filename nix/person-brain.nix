# THE PERSON'S BRAIN MODULES, compiled into their machine — the 2026-10-05
# wave (belongings.md WP8b).
#
# The distro builds ONE chain out of its modules' folders: the loader
# (`nix/brain-modules.nix`), the texts (`i18nRs`), the glyphs measured into
# the icon table (`glyphdb`, `tokensRs`), the WIT enum the buffers are
# compiled against (`wit`, `buffers`), the notifications, the brain's
# registry (`brainModulesRs`), and the workspace that includes them all
# (`rust`). A machine whose person wrote modules of their own (`my-…`) needs
# every link again with the person's folder beside the distro's: a person's
# mark has a glyph the footer must know, its page has texts the footer
# translates, and the brain compiles its `module.rs`. A link left out would
# be a footer drawing a key in brackets, or a WIT enum the host and the
# buffers disagree on.
#
# So this OVERRIDES each link (`callPackage`'s `.override`) and writes none
# of them again: the same generators with the same arguments, one folder
# more. What it hands back has the shape the flake's `let` has — `rust`
# (every program, the brain and the footer included), `buffers`, and the
# loader — plus what `lib.machine` needs whole: `machineModules`, the
# distro's modules run with this `rust` and the approved NixOS halves, and
# `refused`, the folders left out with why.
#
# A FOLDER THAT DOES NOT LOAD IS LEFT OUT, never fatal (the loader's
# `personRefused`): it travels to every machine of the person, and a `throw`
# would stop them all. When none loads, every link is the distro's own —
# the same derivations, so the binary cache has them.
#
# THE ICON BROWSER'S TABLE (`iconset`) and the MIME glyphs are left as the
# distro made them: neither draws a module's mark, and both are data read at
# run time, not code.
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
  glyphdb,
  tokensRs,
  wit,
  buffers,
  i18nRs,
  notificationsRs,
  brainModulesRs,
  rust,
}:

let
  brainModules = import ./brain-modules.nix {
    inherit lib languages;
    personModulesDir = dir;
    personModulesOnly = only;
  };
  none = brainModules.personIds == [ ];
  glyphs = glyphdb.override { brainGlyphs = brainModules.glyphs; };
  tokens = tokensRs.override {
    glyphdb = glyphs;
    brainGlyphs = brainModules.glyphs;
  };
  witDir = wit.override { tokensRs = tokens; };
  components = buffers.override { wit = witDir; };
  texts = i18nRs.override { brainFragments = brainModules.i18nFragments; };
  notifications = notificationsRs.override {
    glyphdb = glyphs;
    brainNotifications = brainModules.notifications;
  };
  registry = brainModulesRs.override { inherit brainModules; };
  machineRust = rust.override {
    tokensRs = tokens;
    i18nRs = texts;
    notificationsRs = notifications;
    brainModulesRs = registry;
    bufferMemory = components.memory;
    bufferKeyboard = components.keyboard;
    bufferTransfers = components.transfers;
    bufferFormat = components.format;
    bufferBrain = components.brain;
    bufferExample = components.example;
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
  i18nRs = if none then i18nRs else texts;
  tokensRs = if none then tokensRs else tokens;
  buffers = if none then buffers else components;
  rust = chosen;
  # THE MACHINE'S NIXOS MODULES: the distro's, every `rust.*` of them this
  # one (the brain that compiled the person's modules, the footer that
  # translates them), and the NixOS half of each APPROVED id — through the
  # loader's gate, never the raw `module.nix`: the gate keeps it shut while
  # the brain is off and refuses an import outside its folder and a
  # `disabledModules`. An approved id with no half, or one left out, adds
  # nothing.
  machineModules =
    approved:
    distroModulesFor chosen ++ map (id: halves.${id}) (lib.filter (id: halves ? ${id}) approved);
}
