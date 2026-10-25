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
    # UNFREE IS APPROVED (the owner, 2026-10-24): an unfree package never
    # refuses an evaluation — a hardware report's branch (the IPU6 camera)
    # or anything else. The distro's unfree PROGRAMS stay parts
    # (`nix/modules/parts.nix`, built from `nix/parts-pkgs.nix`) because they
    # are downloads, which hold back nothing there; and what nobody may
    # redistribute waits for the first network on a machine installed with
    # none (`ronoctua.hardware.deferUndistributable`, `nix/modules/host.nix`).
    # The person's own declarations still ask their yes
    # (`nix/modules/person.nix`): that is the software centre's question, not
    # this policy.
    allowUnfree = true;
  };

}
