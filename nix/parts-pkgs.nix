# THE ONE DOOR FOR A PACKAGE THAT IS NOT FREE (`docs/ARCHITECTURE.md`,
# "Partes"; AGENTS.md, the L25 amendment).
#
# The distro's unfree programs are PARTS, built from this instance and kept
# out of the system's closure: they are downloads — never on the published
# ISO, never in a public cache — and one that fails holds back nothing.
# (The system's own `pkgs` admits unfree since the owner's decision of
# 2026-10-24, `nix/modules/nixpkgs-config.nix`; the programs stay parts for
# what they are, not for a licence.) One instance per evaluation (`ronoctua.partsPkgs`, in
# `nix/modules/parts.nix`; the flake's own tables call this file once), so a
# predicate is written once and nixpkgs is evaluated once more, not once per
# program.
#
# The 2026-10-30 model (owner amendment, replacing "nothing imperative")
# leaves this door as it was. Claude Code stays a part from here, at the
# version `nix/claude-code.nix` pins: its native self-updater was reverted the
# same day (it fetches a generic-linux binary NixOS cannot run without
# nix-ld), so the build from here is the one that runs.
#
# BY NAME, each with its reason: the predicate is only ever asked about a
# package that is already unfree, so a name here lets in exactly that
# program.
{ pkgs }:

let
  inherit (pkgs) lib;
  inherit (pkgs.stdenv.hostPlatform) system;

  names = [
    "unrar" # the only thing that opens a .rar properly
    "uade" # Amiga modules, the player's (`nix/player-tools.nix`)
    "google-chrome" # asked for by name (`nix/modules/browser.nix`)
    "vscode" # Microsoft's build (`nix/modules/vscode.nix`)
    # The same VS Code in its FHS environment (`nix/vscode.nix`), by the name
    # its store path has.
    "code"
    "claude-code" # asked for by name; prebuilt (`nix/modules/dev.nix`)
    "android-studio"
  ];
in
{
  inherit names;

  unfree = import pkgs.path {
    inherit system;
    # THE ONE Claude Code, pinned newer than this nixpkgs' and apart from its
    # packaging — so the agent is new enough to speak the current models
    # (`nix/claude-code.nix`).
    overlays = [ (import ./claude-code.nix) ];
    config.allowUnfreePredicate = p: builtins.elem (lib.getName p) names;
  };

  # THE ANDROID SDK IGNORES A PREDICATE: androidenv evaluates its components
  # through a pkgs the predicate never reaches (measured; the exact name was
  # refused with it in the list). So it has an instance of its own, whose
  # `allowUnfree` reaches the SDK and Android Studio built from it — and
  # nothing of the system's.
  android = import pkgs.path {
    inherit system;
    config = {
      allowUnfree = true;
      android_sdk.accept_license = true;
    };
  };
}
