# THE ONE DOOR FOR A PACKAGE THAT IS NOT FREE (`docs/ARCHITECTURE.md`,
# "Partes"; AGENTS.md, the L25 amendment).
#
# The system's `pkgs` admits nothing unfree of the distro's
# (`nix/modules/nixpkgs-config.nix`): such a program is a PART, built from
# this instance and kept out of the system's closure — never on the
# published ISO, never in a public cache, and a download that fails holds
# back nothing. One instance per evaluation (`ronoctua.partsPkgs`, in
# `nix/modules/parts.nix`; the flake's own tables call this file once), so a
# predicate is written once and nixpkgs is evaluated once more, not once per
# program.
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
