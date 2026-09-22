# Atuin — the shell's history, and whether it leaves this machine.
#
# WHAT CHANGED, AND WHO CHANGED IT
# ---------------------------------
# `nix/modules/home.nix` used to carry, in words:
#
#     # NO SYNC. Syncing shell history to a third-party server would put
#     # every command — including the ones with a token in argv — outside
#     # this machine. Section 9 does not allow that quietly.
#     auto_sync = false;
#
# That was right about the risk and it decided for the person. The owner asked
# for the opposite, and two things the sentence did not weigh are why it can be
# offered without the risk changing:
#
#   * Atuin's sync is END TO END ENCRYPTED. The server holds ciphertext; the
#     key is `atuin key` and never crosses. "A third-party server" is then a
#     store of bytes nobody there can read.
#   * The server can be YOURS — `services.atuin` is in the pinned nixpkgs, and
#     `ronoctua.atuin.server.enable` below is the whole of offering it.
#
# What has NOT changed is the command with a token in its `argv`, and that is
# why `secrets_filter` is a DRAWN ROW of `LEADER s h` rather than a default
# nobody reads: the setting that decides what is never recorded belongs beside
# the switch that decides where it goes.
#
# IT IS STILL OFF BY DEFAULT. The flake is the floor.
#
# WHY THE CONFIG IS SEEDED AND NOT LINKED
# ----------------------------------------
# home-manager's `programs.atuin.settings` writes `~/.config/atuin/config.toml`
# as a symlink into the store, which is READ-ONLY — so choosing between two
# search modes would need a `nixos-rebuild`. That is exactly the sentence
# `LEADER s l` was built to remove: "escolher entre duas traduções não é
# decisão do tamanho de um rebuild".
#
# So this module SEEDS the file once, the way `~/.shellrc.local` is seeded in
# `nix/modules/home.nix`, and the view edits it afterwards a line at a time
# (`ronoctua_atuin::config`, which is not a TOML reserialiser precisely so that
# the person's own comments survive). The flake's values are the TEMPLATE.
#
# Invariant 6 is not bent by this: what the flake declares is what a FRESH
# machine gets, and a preference a person changed is not a system fact — the
# theme, the locale and the display already work this way.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.atuin;
  user = config.ronoctua.user;
  home = config.users.users.${user}.home;

  # THE TEMPLATE. Every value here is what a machine that has never been told
  # otherwise gets; the view rewrites single lines of the copy.
  template = pkgs.writeText "atuin-config.toml" ''
    # Atuin, as this distro leaves it. THIS FILE IS YOURS: the distro writes it
    # once and never again, and `LEADER s h` edits it a line at a time —
    # comments included, which is why the editor is not a TOML reserialiser.
    #
    # What a rebuild DOES still decide is whether `atuin` is installed at all,
    # and whether this machine runs a server (`ronoctua.atuin.server.enable`).

    # SYNC IS OFF UNTIL YOU TURN IT ON. The history is end-to-end encrypted and
    # the key never leaves this machine — but a command with a token in its
    # argv is still a command, which is what `secrets_filter` below is for.
    auto_sync = ${lib.boolToString cfg.sync}
    sync_frequency = "${cfg.frequency}"
    ${lib.optionalString (cfg.address != "") ''sync_address = "${cfg.address}"''}

    # NO UPDATE CHECK. The version is the flake's, and a binary that phones
    # home to offer one it cannot install is a request nobody asked for.
    update_check = false

    style = "compact"
    inline_height = 20
    search_mode = "fuzzy"
    filter_mode_shell_up_key_binding = "session"

    # WHAT IS NEVER RECORDED. These are regular expressions, and they are the
    # answer to "a command with a token in its argv": a line that matches never
    # reaches the database, so it cannot reach a server either.
    secrets_filter = true
    history_filter = [
    ${lib.concatMapStringsSep "\n" (r: "  \"${r}\",") cfg.neverRecord}
    ]

    [sync]
    records = true
  '';
in
{
  options.ronoctua.atuin = {
    sync = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether a FRESH machine starts with sync on. Off, and the reason is in
        this module's header: the risk it names is real and the decision is the
        person's. `LEADER s h` turns it on without a rebuild.
      '';
    };

    address = lib.mkOption {
      type = lib.types.str;
      default = "";
      example = "https://history.example.org";
      description = ''
        The sync server a fresh machine points at. EMPTY by default, and
        deliberately: an address names somebody's machine, and
        `nix/modules/secrets.nix` is explicit that what is personal does not
        enter this repository. Type it in `LEADER s h`.
      '';
    };

    frequency = lib.mkOption {
      type = lib.types.str;
      default = "10m";
      description = "How often Atuin syncs by itself, when it is syncing.";
    };

    neverRecord = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "^ "
        "^curl .*-H .*[Aa]uthorization"
        "^export [A-Z_]*(TOKEN|SECRET|PASSWORD|KEY)"
        "(^|[[:space:]])--password[= ]"
        "(^|[[:space:]])--token[= ]"
      ];
      description = ''
        Commands that never reach the history — regular expressions, matched
        against the whole line.

        The first one is a LEADING SPACE, which is the gesture every shell
        already has for "do not record this" and which `HIST_IGNORE_SPACE` in
        `assets/shell/zshrc.zsh` already honours locally. The rest are the
        shapes that carry a secret in `argv`, which is the risk this module's
        header names.

        They are a DEFAULT and not a promise: `LEADER s h` draws them, and a
        list nobody reads is not a safeguard.
      '';
    };

    server = {
      enable = lib.mkEnableOption ''
        an Atuin sync server on this machine.

        Off by default: most machines running this distro are clients. The
        owner's words — *"alguns poucos que também funcionarão como servidor"*.
      '';

      port = lib.mkOption {
        type = lib.types.port;
        default = 8888;
        description = "What the server listens on.";
      };

      address = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
        description = ''
          What it binds to. The loopback by default — reach it over the tailnet
          or put a reverse proxy with TLS in front.
        '';
      };

      openFirewall = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Open the port. OFF, and a decision rather than a side effect: a
          machine that serves only its own tailnet needs no hole.
        '';
      };

      openRegistration = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Let new accounts be made. Off: a history server that accepts anybody
          who can reach it is one that stores somebody else's shell history.
          Turn it on, register, turn it off.
        '';
      };
    };
  };

  config = lib.mkMerge [
    {
      # THE TEMPLATE IS WRITTEN ONCE AND NEVER AGAIN — the shape
      # `~/.shellrc.local` already has in `nix/modules/home.nix`, and for the
      # same reason: rewriting it on every rebuild would eat exactly the
      # setting the person changed.
      system.activationScripts.atuinConfig = {
        deps = [
          "users"
          "configHome"
        ];
        text = ''
          if [ ! -e ${home}/.config/atuin/config.toml ]; then
            install -d -m 0755 -o ${user} -g users ${home}/.config/atuin
            cp ${template} ${home}/.config/atuin/config.toml
            chown -R ${user}:users ${home}/.config/atuin
            chmod 600 ${home}/.config/atuin/config.toml
          fi
        '';
      };
    }

    (lib.mkIf cfg.server.enable {
      services.atuin = {
        enable = true;
        host = cfg.server.address;
        port = cfg.server.port;
        openRegistration = cfg.server.openRegistration;
        openFirewall = cfg.server.openFirewall;
        database.createLocally = true;
      };

      assertions = [
        {
          assertion = cfg.server.address == "127.0.0.1" || cfg.server.openFirewall;
          message = ''
            ronoctua.atuin.server binds to ${cfg.server.address} but the
            firewall is closed, so nothing can reach it. Either bind to
            127.0.0.1 and put a reverse proxy in front, or set openFirewall.
          '';
        }
      ];
    })
  ];
}
