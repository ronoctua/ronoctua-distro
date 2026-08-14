# The development toolchains.
#
# WHY FROM THE FLAKE AND NOT FROM `mise`/`rustup`/`nvm`
# -----------------------------------------------------
# Those managers each keep their own copy of a compiler under $HOME, and then
# the answer to "which rustc is this" depends on which shell was opened. On this
# distro the answer is the generation — invariant 6. `~/.shellrc.local` can still
# override it on the day a nightly is genuinely needed, and that override is
# then visible in one file instead of implied by a directory's existence.
#
# ANDROID IS THE EXPENSIVE ONE. The SDK is several GB and unfree. It is behind
# `ronoctua.dev.android` so a machine that does not do mobile work can say so and
# get the closure back.
{
  config,
  lib,
  pkgs,
  inputs,
  ...
}:

let
  cfg = config.ronoctua.dev;

  # The SDK, composed. `androidenv` builds a directory that Studio, gradle and
  # flutter all accept — which is what `ANDROID_HOME` has to point at.
  androidSdk = pkgs.androidenv.composeAndroidPackages {
    platformVersions = [
      "34"
      "35"
    ];
    buildToolsVersions = [ "35.0.0" ];
    includeEmulator = false; # KVM inside a laptop that is already a VM host
    includeSystemImages = false;
    includeNDK = cfg.androidNdk;
    cmdLineToolsVersion = "13.0";
  };
in
{
  config = {
    environment.systemPackages =
      (with pkgs; [
        # ---- rust ----
        #
        # From `rust-overlay`, the same input the flake already uses to BUILD
        # this repository. Two Rusts on one machine is how a "works in the
        # devshell, fails outside" afternoon starts.
        (rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src" # rust-analyzer needs it to look inside std
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
        })
        cargo-watch
        cargo-edit
        cargo-nextest

        # ---- go ----
        go
        gopls
        gotools

        # ---- python ----
        (python3.withPackages (
          ps: with ps; [
            pip
            virtualenv
            requests
            rich
          ]
        ))
        ruff
        pyright
        uv # the fast resolver; still writes into a venv, not the system

        # ---- javascript ----
        nodejs_22
        pnpm
        bun
        typescript
        typescript-language-server

        # ---- nix itself ----
        nixfmt
        nil # the language server
        nix-tree # what is actually in this closure
        nix-output-monitor

        # ---- general ----
        gnumake
        cmake
        pkg-config
        gcc
        gdb
        sqlite
      ])
      ++ lib.optionals cfg.android (
        with pkgs;
        [
          android-studio
          android-tools # adb, fastboot
          flutter
          dart
          jdk17 # what gradle and Studio expect
          androidSdk.androidsdk
        ]
      );

    # ANDROID WANTS BOTH VARIABLES.
    #
    # `ANDROID_HOME` is the old name and still the one gradle, react-native and
    # flutter reach for; `ANDROID_SDK_ROOT` is the new one. Setting only the new
    # one gives a build that fails deep inside gradle with a path it invented.
    environment.sessionVariables = lib.mkIf cfg.android {
      ANDROID_HOME = "${androidSdk.androidsdk}/libexec/android-sdk";
      ANDROID_SDK_ROOT = "${androidSdk.androidsdk}/libexec/android-sdk";
      # Keeps `~/.android` out of $HOME's root, like everything else here.
      ANDROID_USER_HOME = "$XDG_DATA_HOME/android";
      # Flutter writes its own state; without this it lands in `~/.flutter`.
      FLUTTER_ROOT = "${pkgs.flutter}";
    };

    # NO `programs.adb`. Since systemd 258 it does nothing but assert, because
    # uaccess handles the device permissions on its own — `android-tools` in the
    # package list above is the whole of what `adb` needs now. The option is
    # still in every tutorial, which is why this comment exists instead of
    # silence.
  };
}
