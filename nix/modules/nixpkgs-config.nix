# How `pkgs` itself is configured — overlays and the unfree policy.
#
# WHY IT IS NOT IN base.nix
# -------------------------
# A VM test builds `pkgs` OUTSIDE and hands it to the machine. In that shape
# `nixpkgs.config` accepts exactly one definition, and the framework has already
# made it: any definition of ours is one too many, and evaluation dies with
# "defined multiple times while it's expected to be unique".
#
# The tests import `base.nix` plus the module under test — never this file. So
# the policy lives here, in `distroModules` only, and the tests keep the `pkgs`
# the framework gave them. Splitting it this way is what lets the same option
# tree serve a real system and a test machine.
{
  config,
  lib,
  pkgs,
  inputs,
  ...
}:

{
  # THE SAME RUST THAT BUILDS THIS REPOSITORY.
  #
  # The flake already uses `rust-overlay` for its own derivations; without the
  # overlay reaching the SYSTEM's pkgs, `rust-bin` is undefined in dev.nix and
  # the alternative would be nixpkgs' `rustc` — a second, different compiler on
  # the same machine.
  nixpkgs.overlays = [
    inputs.rust-overlay.overlays.default
    # THE SAME yt-dlp AS THE FLAKE'S — see nix/yt-dlp.nix.
    (import ../yt-dlp.nix)
  ];
  # `nixpkgs.config` IS ONE DEFINITION, and has to be: a VM test hands `pkgs`
  # in from outside, and in that shape the option takes a single definition.
  # See the note in nix/modules/dev.nix.
  nixpkgs.config = {
    # ── anchor: L25 ──
    # THE SYSTEM ADMITS NOTHING UNFREE OF THE DISTRO'S: every such program —
    # Chrome, VS Code, Claude Code, Android Studio and the SDK, unrar, uade —
    # is a PART (`nix/modules/parts.nix`), built from the parts' one door
    # (`nix/parts-pkgs.nix`) and kept out of the system's closure: never on
    # the published ISO, never in a public cache, and a download that fails
    # holds back nothing. No `allowUnfree` either: the SDK, which forced it,
    # has an instance of its own there.
    allowUnfree = false;

    allowUnfreePredicate =
      pkg:
      # ── anchor: L03 ──
      # ONLY WHAT THE PERSON SAID YES TO when declaring it — by the same name
      # this predicate compares (`lib.getName`), from the consumer of their
      # declarations (`nix/modules/person.nix`). An unfree package declared
      # without that consent is skipped there and never reaches this line.
      builtins.elem (lib.getName pkg) config.ronoctua.person.unfreeNames;
  };

}
