# Emacs as a daemon, and Doom's config as versioned files.
#
# WHERE THE LINE IS, AND WHY IT IS THERE
# --------------------------------------
# Invariant 6 says: if it is not in the flake, it does not exist. Doom does not
# fit that cleanly, and pretending otherwise would be the dishonest option. So
# the boundary is drawn where it actually falls:
#
#   DECLARATIVE — `init.el`, `packages.el`, `config.el` live in `assets/doom/`
#   and home-manager writes them to `~/.config/doom`. They are versioned, they
#   diff, and a rebuild puts them back exactly as the repository has them.
#
#   IMPERATIVE — what Doom DOES with them: cloning the framework, resolving
#   packages, byte-compiling. That is a network fetch into `~/.config/emacs`, and
#   there is no honest way to call it declarative. `ronoctua-doom-sync` runs it,
#   is idempotent, and is the only command on this system that reaches outside
#   the flake.
#
# WHY A DAEMON AT ALL: one process loads the whole configuration ONCE. Without
# it, every `$EDITOR` invocation pays Doom's full startup — seconds — which is
# enough that one silently stops using it and the config rots.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua;
  emacsPkg = pkgs.emacs30;

  # The `doom doctor` list, plus what the `:lang` modules in `init.el` need.
  # Missing one of these does not fail loudly: the module just quietly does less,
  # and the symptom is a feature that "never worked".
  doomDeps = with pkgs; [
    git
    ripgrep
    fd
    coreutils
    gnutar
    gzip
    cmake # vterm and tree-sitter grammars
    gnumake
    libtool
    sqlite # forge, org-roam
    editorconfig-core-c
    pandoc # :lang markdown export
    shellcheck # :checkers syntax for sh
    shfmt # :editor format
    nixfmt # :lang nix
    nil
    graphviz # org export of diagrams
    (aspellWithDicts (d: [
      d.en
      d.pt_BR # :checkers spell, both languages — section 2
    ]))
  ];

  # Same reason as in home.nix: a heredoc terminator must be in column 0, and
  # `nix fmt` reindents activation scripts.
  doomLocalTemplate = pkgs.writeText "doom-local.el" ''
    ;;; local.el — this machine's own. NEVER versioned (invariant 13).
    ;;
    ;; Loaded LAST by config.el, so anything here wins.
    ;; (setq user-full-name "..."
    ;;       user-mail-address "...")
  '';

  doomSync = pkgs.writeShellApplication {
    name = "ronoctua-doom-sync";
    runtimeInputs = [ emacsPkg ] ++ doomDeps;
    text = ''
      # IDEMPOTENT BY DESIGN: clone if absent, sync always.
      #
      # It has to be safe to run on every login and by hand at any moment,
      # because the alternative is a machine where nobody is sure whether the
      # config on disk is the config that is loaded.
      framework="''${XDG_CONFIG_HOME:-$HOME/.config}/emacs"

      if [ ! -d "$framework/.git" ]; then
        echo "ronoctua: cloning Doom into $framework"
        git clone --depth 1 https://github.com/doomemacs/doomemacs "$framework"
        "$framework/bin/doom" install --no-config --no-env --no-fonts --force
      fi

      # `--force`: the config is REPLACED by the repository on every rebuild, so
      # there is never anything local to preserve here. Prompting would only
      # block a login.
      echo "ronoctua: doom sync"
      "$framework/bin/doom" sync --force
      echo "ronoctua: done. Restart the daemon with: systemctl --user restart emacs"
    '';
  };
in
{
  options.ronoctua.editor.emacs =
    lib.mkEnableOption "Emacs 30 and Doom's dependencies (~0.6 GiB)"
    // {
      default = true;
    };

  # EVERYTHING here hangs off the switch, `home-manager` files included: a
  # machine with no Emacs has no use for a Doom configuration in the home
  # directory, and the activation script would create `local.el` for a framework
  # that is not installed.
  config = lib.mkIf cfg.editor.emacs {
    environment.systemPackages = [ doomSync ] ++ doomDeps;

    # WHERE THE FILE MANAGER SENDS TEXT — and it is set HERE, inside this
    # `mkIf`, rather than baked into the binary.
    #
    # `nix/rust.nix` used to hold it as an `option_env!`, and that made the
    # whole of Emacs a reference the workspace carried into every closure:
    # `ronoctua.editor.emacs = false` took the package off `systemPackages` and
    # changed nothing at all. It is the same trap the icon superpack sprang, and
    # the same answer — read it from the environment, set it where the switch
    # is. `profile-minimal-closes` is the check that refuses the other way.
    #
    # A store PATH and never a name: the footer runs inside a scope whose `PATH`
    # is minimal, and a bare name fails far from its cause.
    systemd.user.services.ronoctua-footer.environment.RONOCTUA_EMACSCLIENT =
      "${emacsPkg}/bin/emacsclient";

    services.emacs = {
      enable = true;
      package = emacsPkg;
      # NOT `install = true`. That adds `emacs.desktop` and a `client` entry to the
      # applications menu, and invariant 14 says the system has no menu — the
      # locator (`LEADER a`) reads the same `.desktop` files and would list two
      # Emacsen, one of which does not use the daemon.
      install = false;
      defaultEditor = false; # `$EDITOR` is set by assets/shell/shellrc.sh
      startWithGraphical = false; # a terminal Emacs must work without a session
    };

    home-manager.users.${cfg.user} = {
      xdg.configFile = {
        "doom/init.el".source = ../../assets/doom/init.el;
        "doom/packages.el".source = ../../assets/doom/packages.el;
        "doom/config.el".source = ../../assets/doom/config.el;
      };
    };

    # `~/.config/doom/local.el` — identity and anything else personal, created
    # once and never overwritten. Same arrangement as `~/.shellrc.local`, and the
    # same reason: invariant 13 keeps a name and an e-mail out of the repository.
    system.activationScripts.doomLocal = {
      deps = [ "users" ];
      text = ''
        target=/home/${cfg.user}/.config/doom/local.el
        if [ ! -e "$target" ]; then
          mkdir -p "$(dirname "$target")"
          cp ${doomLocalTemplate} "$target"
          chown ${cfg.user}:users "$target"
        fi
      '';
    };
  };
}
