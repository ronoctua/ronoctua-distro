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

let
  cfg = config.ronoctua.dev;
in
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
    # The Android SDK's licence, accepted declaratively rather than through
    # `NIXPKGS_ACCEPT_ANDROID_SDK_LICENSE` in someone's environment.
    android_sdk.accept_license = cfg.android;

    # THE SDK FORCES `allowUnfree`, AND THAT IS A REAL LOSS.
    #
    # The predicate below names each unfree package and its reason, which is
    # what keeps an unexpected one from slipping in. androidenv does not
    # respect it: its components are evaluated through a pkgs the predicate
    # never reaches, and even the exact name (`sdk-tools-linux-4333796.zip`)
    # is refused with it in the list.
    #
    # So enabling Android widens the door for the whole system. What narrows it
    # again is the option itself: `ronoctua.dev.android = false` puts the
    # predicate back in charge.
    allowUnfree = cfg.android;

    allowUnfreePredicate =
      pkg:
      let
        name = lib.getName pkg;
      in
      builtins.elem name [
        "unrar" # the only thing that opens .rar properly
        "google-chrome" # asked for by name
        "android-studio"
        "claude-code" # asked for by name; prebuilt, unfree
      ]
      # The SDK is not one package: it is dozens of them, and the inner ones are
      # named after the ZIP they came from (`platform-tools_r37.0.0-linux.zip`),
      # not after the SDK. Listing them by hand is a list that goes stale on the
      # next bump.
      #
      # A prefix rule is safe here for a reason worth stating: this predicate is
      # only ever consulted for a package that is ALREADY unfree. Matching a name
      # cannot let a free package through, and the only unfree things shaped like
      # these are androidenv's.
      || lib.any (prefix: lib.hasPrefix prefix name) [
        "android-sdk-"
        "platform-tools"
        "build-tools"
        "platforms-"
        "cmdline-tools"
        "sources-"
        "ndk-"
        "emulator"
        "extras-"
        "system-images"
        "google-apis"
        "patcher"
        "skiaparser"
        "sdk-tools"
        "tools_r"
      ]
      # ── anchor: L03 ──
      # AND WHAT THE PERSON SAID YES TO when declaring it — by the same name
      # this predicate compares (`lib.getName`), from the consumer of their
      # declarations (`nix/modules/person.nix`). An unfree package declared
      # without that consent is skipped there and never reaches this line.
      || builtins.elem name config.ronoctua.person.unfreeNames;
  };

}
