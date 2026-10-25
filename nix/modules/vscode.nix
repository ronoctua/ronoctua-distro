# VS Code — Microsoft's build in its FHS environment, its telemetry off by
# policy, and the person's extensions recorded so they travel with the
# belongings.
#
# WHAT IS DECLARED
# ----------------
# - The program (`nix/vscode.nix`: `pkgs.vscode` inside the FHS environment
#   nixpkgs makes for it, the machine's policy folder bound in; unfree, and
#   `nix/modules/nixpkgs-config.nix` names both the program and the
#   environment), an entry of the registry in the `vscode` group, which
#   `profile.nix` turns on in the full profile. Wayland's own: the session
#   exports `NIXOS_OZONE_WL` (`scroll.nix`), which nixpkgs' wrapper turns into
#   `--ozone-platform-hint=auto` — and `auto` goes by the session's
#   `XDG_SESSION_TYPE=wayland` (`scroll.nix` gives it to the manager), as
#   Chrome's does: without it, measured in `tests/vscode-vm.nix`, Electron
#   takes X11 and, with no X display, ends.
# - Its telemetry OFF BY POLICY (`/etc/vscode/policy.json`), and Chromium's
#   spell-check dictionary never fetched (`nix/vscode.nix`, a rule of
#   Chromium's own on every start's command line).
# - `ronoctua-vscode` on the person's `PATH`, with `code` set
#   (`RONOCTUA_VSCODE`) — `ronoctua-vscode install` reinstalls what is
#   missing from a terminal —, the same variable for the footer and the
#   queue, and the user unit that records the extensions installed here into
#   the person's list (`ronoctua-vscode record`).
#
# Nothing of the person's: their settings, keybindings and snippets are
# theirs, and travel with their belongings (the target `vscode`,
# `assets/belongings.nix`), like Emacs's.
#
# WHY THE MICROSOFT BUILD, AND NOT VSCODIUM
# -----------------------------------------
# The owner's decision (2026-10-20): the person asked for VS Code by name.
# The Marketplace's terms admit only Microsoft's products, and several of
# the extensions a person works with (remote development, Pylance, Copilot)
# are Microsoft's own and refuse to run in another build.
#
# WHY THE FHS ENVIRONMENT: `nix/vscode.nix` says it — an extension's own
# programs (rust-analyzer's server) do not start on NixOS without one,
# measured in the vm test.
#
# WHY A POLICY, AND WHERE
# -----------------------
# The distro's stance is that nothing reports on the person by default. A
# setting in `settings.json` would be the person's file — which the distro
# never writes, and which their belongings bring back as THEY left it. A
# POLICY is the machine's: VS Code reads it before any setting, shows the
# setting as managed, and a person's `settings.json` cannot turn it back on.
# Read in the pinned 1.119's own code (`out/main.js`): on Linux the policy
# service is a `FilePolicyService` over the fixed path `/etc/vscode/policy.json`
# — not `/etc/<applicationName>/` (that would be `/etc/code/`): the path is
# written into the program, whatever `product.json` says. `TelemetryLevel` is
# the policy of `telemetry.telemetryLevel` (since 1.99); `off` sends nothing,
# crash reports included. `tests/vscode-vm.nix` proves the file read, from VS
# Code's own log, inside the FHS environment.
#
# WHY A PATH UNIT
# ---------------
# The list of the person's extensions (`ronoctua_vscode`) follows VS Code's
# manifest (`~/.vscode/extensions/extensions.json`), which VS Code rewrites in
# place whenever an extension is installed or uninstalled — an event, so the
# record waits for it (`PathChanged=`), and never a clock; and the list
# itself, so one that a sync brought is recorded against what is here.
# systemd watches a path that does not exist yet from its nearest folder that
# does (a machine where VS Code never ran has no `~/.vscode`). While the
# service runs the unit does not watch, so the record reads the manifest
# again until it holds still. It also runs once when the person's manager
# starts, for what changed while it was not running.
{ vscodeTool }:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.editor;
  installed = config.ronoctua.distroApps.installed.vscode;
  # The parts' one nixpkgs (`nix/parts-pkgs.nix`): VS Code is not free.
  partsPkgs = config.ronoctua.partsPkgs or (import ../parts-pkgs.nix { inherit pkgs; });
  parts = config.ronoctua ? parts;
  # `code` BY THE PART'S STABLE PATH: the queue's child and the footer run
  # with a minimal `PATH`, and a bare name fails far from its cause — and a
  # store path would pull VS Code, unfree, into the system's closure. Read
  # on every use (`ronoctua_vscode::code`): it may not have arrived.
  code = if parts then "${config.ronoctua.parts.vscode.path}/bin/code" else null;
  # The person's command, inside the part, names the program by store path:
  # it comes and goes with it.
  codeInThePart = "${cfg.vscodePackage}/bin/code";
  # THE PERSON'S COMMAND, with `code` set: `ronoctua-vscode install` from a
  # terminal. A binary wrapper, as the shipped programs have.
  vscodeCli =
    pkgs.runCommand "ronoctua-vscode-command"
      {
        nativeBuildInputs = [ pkgs.makeBinaryWrapper ];
        meta.mainProgram = "ronoctua-vscode";
      }
      ''
        mkdir -p $out/bin
        makeBinaryWrapper ${vscodeTool}/bin/ronoctua-vscode $out/bin/ronoctua-vscode \
          --inherit-argv0 --set RONOCTUA_VSCODE ${codeInThePart}
      '';
in
{
  # The registry this module writes the program's entry into (see
  # `browser.nix` for why it is imported here too).
  imports = [ ./distro-apps.nix ];

  options.ronoctua.editor = {
    vscode = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Visual Studio Code, Microsoft's build, in its FHS environment, with
        its telemetry off by policy. On in the full profile.
      '';
    };
    vscodePackage = lib.mkOption {
      type = lib.types.package;
      default = import ../vscode.nix { pkgs = partsPkgs.unfree; };
      defaultText = lib.literalExpression "import nix/vscode.nix { pkgs = config.ronoctua.partsPkgs.unfree; }";
      internal = true;
      description = ''
        The VS Code the registry makes a part: `vscode` in its FHS
        environment (`nix/vscode.nix`), from the parts' one door — the
        system's `pkgs` admits nothing unfree.
      '';
    };
  };

  config = lib.mkMerge [
    {
      # A PROGRAM OF THE REGISTRY (`nix/modules/distro-apps.nix`), outside
      # the `mkIf` below: the registry's names may not depend on a switch.
      # Its two launcher entries are the package's own (the environment
      # links them); the URL handler's is hidden (`NoDisplay`). The person's
      # command comes and goes with it.
      #
      # A PART (`nix/modules/parts.nix`): not free, it arrives after the
      # installation and stays out of the system's closure; its launcher
      # entries reach the session through the parts' view.
      ronoctua.apps.vscode = {
        package = cfg.vscodePackage;
        part = true;
        extraPackages = [ vscodeCli ];
        group = "vscode";
        desktopIds = [
          "code.desktop"
          "code-url-handler.desktop"
        ];
      };
    }

    # EVERYTHING ELSE FOLLOWS THE PROGRAM, and not only the switch: a person
    # who declined it keeps no policy, no variable and no unit pointing at a
    # program that is not there.
    (lib.mkIf installed {
      environment.etc."vscode/policy.json".text = builtins.toJSON { TelemetryLevel = "off"; };

      # WHERE `code` IS: the footer and its windows draw the extensions'
      # offer only where VS Code is (`offers::vscode_here`), and the queue's
      # child installs with it.
      systemd.user.services.ronoctua-footer.environment = lib.mkIf parts {
        RONOCTUA_VSCODE = code;
      };
      systemd.user.services.ronoctua-transfers =
        lib.mkIf ((config.ronoctua.transfers.enable or false) && parts)
          {
            environment.RONOCTUA_VSCODE = code;
          };

      systemd.user.paths.ronoctua-vscode-record = {
        description = "Records the VS Code extensions installed here into the person's list when VS Code's manifest or the list changes";
        wantedBy = [ "default.target" ];
        pathConfig.PathChanged = [
          "%h/.vscode/extensions/extensions.json"
          "%h/.config/Code/User/ronoctua-extensions.json"
        ];
      };
      systemd.user.services.ronoctua-vscode-record = {
        description = "Records the VS Code extensions installed here into the person's list";
        # ONCE AT THE START of the person's manager too: what changed while
        # it was not running.
        wantedBy = [ "default.target" ];
        # NO START LIMIT: installing ten extensions rewrites the manifest ten
        # times in a few seconds, and the default (five starts in ten
        # seconds) would fail the path unit itself — which then stops
        # watching. Nothing loops: a record that wrote the list starts it
        # once more, and that one finds nothing new and writes nothing.
        startLimitIntervalSec = 0;
        serviceConfig = {
          Type = "oneshot";
          # `-`: a list it refused to write over is said in its journal, and
          # never a failure of the session or of a switch.
          ExecStart = "-${vscodeTool}/bin/ronoctua-vscode record";
        };
      };
    })
  ];
}
