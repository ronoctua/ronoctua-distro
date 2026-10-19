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
  # The registry this module writes its programs into (see `browser.nix` for
  # why it is imported here too).
  imports = [ ./distro-apps.nix ];

  config = {
    # THE PROGRAMS ARE THE REGISTRY'S (`nix/modules/distro-apps.nix`): the
    # toolchains, Android and the coding agents are its entries at the end of
    # this file, each in the group of the switch that carries it.

    # THE LIFELINE'S "ASK AN AI" OPENS CLAUDE CODE by store path (the unit's
    # `PATH` is minimal). It is set here, beside the package, and not in
    # `scroll.nix` where the unit is declared: Claude Code is unfree, and a VM
    # test imports `scroll.nix` with a `pkgs` the predicate never reaches.
    # It follows the PROGRAM and not only the switch: a person who declined
    # Claude Code is not offered it by the lifeline either.
    systemd.user.services.ronoctua-lifeline.environment =
      lib.mkIf config.ronoctua.distroApps.installed.claude-code
        {
          RONOCTUA_CLAUDE = "${pkgs.claude-code}/bin/claude";
        };

    # ANDROID WANTS BOTH VARIABLES.
    #
    # `ANDROID_HOME` is the old name and still the one gradle, react-native and
    # flutter reach for; `ANDROID_SDK_ROOT` is the new one. Setting only the new
    # one gives a build that fails deep inside gradle with a path it invented.
    #
    # EACH FOLLOWS ITS PROGRAM, and not only the switch: a variable naming the
    # SDK keeps the SDK in the system's closure and points gradle at it, so a
    # person who declined it to get the gigabytes back would keep both.
    environment.sessionVariables = lib.mkMerge [
      (lib.mkIf config.ronoctua.distroApps.installed.android-sdk {
        ANDROID_HOME = "${androidSdk.androidsdk}/libexec/android-sdk";
        ANDROID_SDK_ROOT = "${androidSdk.androidsdk}/libexec/android-sdk";
      })
      (lib.mkIf cfg.android {
        # Keeps `~/.android` out of $HOME's root, like everything else here
        # (Studio's as much as the SDK's).
        ANDROID_USER_HOME = "$XDG_DATA_HOME/android";
      })
      (lib.mkIf config.ronoctua.distroApps.installed.flutter {
        # Flutter writes its own state; without this it lands in `~/.flutter`.
        FLUTTER_ROOT = "${pkgs.flutter}";
      })
    ];

    # NO `programs.adb`. Since systemd 258 it does nothing but assert, because
    # uaccess handles the device permissions on its own — `android-tools`, in
    # the registry's `android-sdk` below, is the whole of what `adb` needs now.
    # The option is still in every tutorial, which is why this comment exists
    # instead of silence.

    # THE 2026-10-05 WAVE: one region per package, after its anchor.
    # ── anchor: L04 ──
    # THE TOOLCHAINS ARE A GROUP OF THEIR OWN, and Android is another, and
    # each is a set of programs of the registry (`nix/modules/distro-apps.nix`)
    # that a person can keep off a machine: one entry per language, because
    # that is the decision a person makes ("no Go here"), with its tools
    # riding along.
    #
    # Building THIS repository does not depend on either: `nix develop` brings a
    # pinned toolchain of its own, which is the whole reason two Rusts on one
    # machine is a thing this file already warns about. What these are for is
    # working on OTHER code from the installed system.
    #
    # EACH ENTRY'S `package` IS THE ONE THAT SAYS THE PROGRAM IS THERE (the
    # registry's `still-present` looks for it): `gcc` and not `gnumake`, a
    # companion another entry could carry too.
    ronoctua.apps = {
      # ---- rust ----
      #
      # THE DEVSHELL'S OWN (`nix/toolchain.nix`), in both profiles (`when`):
      # two Rusts on one machine is how a "works in the devshell, fails
      # outside" afternoon starts. See the end of this file.
      rust = {
        package = (import ../toolchain.nix { inherit pkgs; }).rust;
        when = true;
        extraPackages = with pkgs; [
          cargo-watch
          cargo-edit
          cargo-nextest
          # THE LINKER `rustc` CALLS (`cc`). rust-overlay's `cargo` names no
          # linker and reaches its own C compiler only through
          # `propagated-build-inputs`, which an installed package never puts
          # on the PATH: without this, `cargo run` outside the devshell is
          # "linker `cc` not found" on any machine without `build-tools`.
          # It is the devshell's own `cc` (mkShell's `stdenv`) and already in
          # rust's closure, so it costs nothing (`devshell-is-in-the-store`).
          stdenv.cc
        ];
        group = "dev-toolchains";
      };

      # ---- go ----
      go = {
        package = pkgs.go;
        extraPackages = with pkgs; [
          gopls
          gotools
        ];
        group = "dev-toolchains";
      };

      # ---- python ----
      python = {
        package = pkgs.python3.withPackages (
          ps: with ps; [
            pip
            virtualenv
            requests
            rich
          ]
        );
        extraPackages = with pkgs; [
          ruff
          pyright
          uv # the fast resolver; still writes into a venv, not the system
        ];
        group = "dev-toolchains";
      };

      # ---- javascript ----
      javascript = {
        package = pkgs.nodejs_22;
        extraPackages = with pkgs; [
          pnpm
          bun
          typescript
          typescript-language-server
        ];
        group = "dev-toolchains";
      };

      # ---- nix itself ----
      nix-tools = {
        package = pkgs.nix-tree; # what is actually in this closure
        extraPackages = with pkgs; [
          nixfmt
          nil # the language server
          nix-output-monitor
        ];
        group = "dev-toolchains";
      };

      # ---- general ----
      build-tools = {
        package = pkgs.gcc;
        extraPackages = with pkgs; [
          gnumake
          cmake
          pkg-config
          gdb
          sqlite
        ];
        group = "dev-toolchains";
      };

      # ---- android ----
      android-studio = {
        package = pkgs.android-studio;
        group = "dev-android";
        desktopIds = [ "android-studio.desktop" ];
      };
      android-sdk = {
        package = androidSdk.androidsdk;
        extraPackages = with pkgs; [
          android-tools # adb, fastboot
          jdk17 # what gradle and Studio expect
        ];
        group = "dev-android";
      };
      flutter = {
        package = pkgs.flutter;
        extraPackages = [ pkgs.dart ];
        group = "dev-android";
      };

      # THE CODING AGENTS. nixpkgs' Claude Code wraps the vendor's binary with
      # `DISABLE_AUTOUPDATER=1`, so it never replaces itself behind the
      # generation's back (invariant 6); Codex is built from source. Their
      # state is the person's, under `$HOME`, like any program's.
      claude-code = {
        package = pkgs.claude-code;
        group = "dev-agents";
      };
      codex = {
        package = pkgs.codex;
        group = "dev-agents";
      };
    };

    # ── anchor: L15 ──
    # WORKING ON THE DISTRO FROM THE DISTRO, in both profiles.
    #
    # THE DEVSHELL'S TOOLCHAIN IS IN THE STORE (`nix/toolchain.nix`, the one
    # list `nix/devshell.nix` reads too): `nix develop` on an installed
    # machine finds every tool already there instead of downloading gigabytes
    # the first time somebody opens the repository. Kept, not installed —
    # `extraDependencies` holds paths without putting them on the PATH.
    # `devshell-is-in-the-store` compares the two lists derivation by
    # derivation, in both profiles.
    system.extraDependencies = (import ../toolchain.nix { inherit pkgs; }).kept;

    # AND RUST ON THE PATH, the devshell's very compiler: the registry's
    # `rust` entry (in the program table above) is this derivation, carried
    # in both profiles. A person can still decline it
    # (`software/declined.json`) and get the PATH back — the store keeps it
    # either way, for `nix develop`.

    # KVM, for the VM tests and `nix run .#vm`.
    #
    # THE MODULE FOLLOWS THE CPU, and it is nixos-facter that lists it: the
    # machine's `facter.json` carries the CPU's flags, and its module loads
    # `kvm-intel` for `vmx` and `kvm-amd` for `svm` — never both blind, since
    # the other vendor's module refuses to load and leaves
    # `systemd-modules-load.service` failed on every boot. Nothing is listed
    # here on top of it: a machine with no report (a test machine) gets its
    # module from the kernel, which autoloads it from the CPU's modalias.
    # `kvm-is-ready` asks the reference machine on a planted CPU of each
    # vendor, so a facter that stopped doing it would say so.
    # `/dev/kvm` belongs to the group `kvm` (systemd's udev rule); the person
    # in it runs QEMU with acceleration instead of emulating every
    # instruction.
    users.users.${config.ronoctua.user}.extraGroups = [ "kvm" ];
    # WHAT THIS MACHINE MAY BUILD. The VM tests ask for `kvm` and
    # `nixos-test`, the heavy builds for `big-parallel`, the frame budget
    # for `benchmark`. NixOS's default is the same four today; written out
    # because a default that changes is a gate that stops building its VM
    # tests on this machine without a word (`kvm-is-ready`).
    nix.settings.system-features = [
      "kvm"
      "nixos-test"
      "big-parallel"
      "benchmark"
    ];

    # THE REPOSITORY'S `.envrc` NEEDS NO DOWNLOAD. It asks for `nix-direnv`
    # by its function (`has nix_direnv_version`) and fetches it from GitHub
    # only when that is missing; the system's direnvrc sources it, so on
    # this distro the first `cd` into the checkout touches no network.
    # The HOOK stays in `assets/shell/shellrc.sh`, the one place each shell's
    # setup is written: the module's own integrations would hook direnv a
    # second time.
    programs.direnv = {
      enable = true;
      nix-direnv.enable = true;
      enableBashIntegration = false;
      enableZshIntegration = false;
      enableFishIntegration = false;
    };

    # THE AGENTS REACH THE FOOTER, by store path, as they reach the lifeline
    # (above): the LLM accounts open a session of either from the footer's
    # own process (`ronoctua_llm::launch_arguments`). Each follows its
    # PROGRAM — a person who declined Codex is not offered a session of it.
    systemd.user.services.ronoctua-footer.environment = lib.mkMerge [
      (lib.mkIf config.ronoctua.distroApps.installed.claude-code {
        RONOCTUA_CLAUDE = "${pkgs.claude-code}/bin/claude";
      })
      (lib.mkIf config.ronoctua.distroApps.installed.codex {
        RONOCTUA_CODEX = "${pkgs.codex}/bin/codex";
      })
    ];
  };
}
