# The user's home — section 2, and invariant 6.
#
# WHY home-manager, AND WHY THE DOTFILES ARE FILES
# ------------------------------------------------
# The three shell files live in `assets/shell/` as real `.sh`/`.zsh`/`.bash`,
# and home-manager only links them. They are NOT built from a Nix string, and
# that is deliberate: they are full of `${VAR}`, which inside a Nix `''…''`
# fence is antiquotation, not text. Writing them inline would mean escaping
# every one of them — and the day someone forgets, Nix evaluates a shell
# variable at build time and the error lands nowhere near the cause.
#
# As files they are also readable with a shell's own tools: `shellcheck` runs on
# them, an editor highlights them, and a diff of one line is one line.
#
# WHAT IS NOT HERE
# ----------------
# `~/.shellrc.local`. It holds tokens and credentials (that is what it is for),
# so it is created EMPTY on the machine and never versioned — invariant 13. The
# activation script below writes it once and never touches it again.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua;
  home = "/home/${cfg.user}";

  # The template as a FILE, not a heredoc. A heredoc's terminator has to sit in
  # column 0, and `nix fmt` reindents the whole activation script — which turns
  # the marker into text and the script into a shell that never returns.
  shellrcLocalTemplate = pkgs.writeText "shellrc.local.template" ''
    # This machine's own config. NEVER versioned — it holds tokens (invariant 13).
    #
    # Sourced LAST by ~/.shellrc, so anything here wins over the shared config.
    # Put here: credentials, paths that exist only on this machine, per-machine
    # overrides. Keep it out of any backup that leaves the disk.
  '';
in
{
  # zsh is the login shell. `programs.zsh` at the SYSTEM level is what makes it
  # a valid shell in /etc/shells and puts /etc/zshenv in place; without it,
  # `users.defaultUserShell` gives a user who cannot log in.
  programs.zsh = {
    enable = true;
    # OUR `.zshrc` runs `compinit` itself, with a cache in XDG and `-C`. Leaving
    # NixOS's on as well means compinit twice per prompt — measurable, and the
    # kind of slowness nobody ever tracks down.
    enableCompletion = false;
    enableBashCompletion = false;
  };
  users.defaultUserShell = pkgs.zsh;

  # The plugins are sourced by `.zshrc` from `/run/current-system/sw/share/…`.
  # They are in systemPackages and not in home.packages on purpose: the path is
  # then the same for every user and for a root shell.
  environment.systemPackages = with pkgs; [
    zsh
    zsh-autosuggestions
    zsh-syntax-highlighting
    bash-preexec # bash has no preexec hook; atuin needs this one
    atuin
    starship
    fzf
    zoxide
    direnv
    nix-direnv
    bash-completion
  ];

  # `nix-direnv` caches the devshell, so `cd` into the repository does not
  # re-evaluate the flake every time.
  environment.pathsToLink = [ "/share/nix-direnv" ];

  home-manager = {
    useGlobalPkgs = true;
    useUserPackages = true;
    backupFileExtension = "hm-bak";

    users.${cfg.user} = {
      home.stateVersion = "26.05";

      home.file = {
        ".shellrc".source = ../../assets/shell/shellrc.sh;
        ".zshrc".source = ../../assets/shell/zshrc.zsh;
        ".bashrc".source = ../../assets/shell/bashrc.bash;
      };

      # THE NINE DIRECTORIES, section 2.
      #
      # `xdg.userDirs` writes `~/.config/user-dirs.dirs`, which is what a file
      # dialog, a browser download and `xdg-open` actually read. Setting the
      # environment variable alone would move nothing.
      xdg.userDirs = {
        enable = true;
        createDirectories = true;
        desktop = "${home}/main/temp";
        documents = "${home}/main/archive";
        download = "${home}/main/temp";
        music = "${home}/main/media/audio-music";
        pictures = "${home}/main/media/image";
        publicShare = "${home}/main/share/public";
        templates = "${home}/templates";
        videos = "${home}/main/media/video";

        # `XDG_PROJECTS_DIR` is not in the freedesktop spec — `xdg-user-dirs-update`
        # does not know it and no third-party tool will read it. home-manager
        # writes it anyway, and it defaults to `~/Projects`: leaving it alone
        # would have put a directory nobody asked for in the generated file.
        #
        # Through `extraConfig` it comes out as `XDG_XDG_PROJECTS_DIR_DIR`,
        # because that attribute already builds the key around what it is given.
        projects = "${home}/main/prj";
      };

      programs.starship = {
        enable = true;
        # `.zshrc` and `.bashrc` run `starship init` themselves, last, so that
        # nothing overwrites the prompt. home-manager's own hook would run
        # earlier, and the symptom would be a prompt that sometimes reverts.
        enableZshIntegration = false;
        enableBashIntegration = false;
        settings = {
          add_newline = false;
          command_timeout = 1000;
          directory.truncation_length = 4;
          git_status.disabled = false;
          nix_shell.format = "via [$symbol$state]($style) ";
        };
      };

      programs.atuin = {
        enable = true;
        # Same reason as starship: our files call `atuin init` with
        # `--disable-up-arrow`, and two inits fight over Ctrl+R.
        enableZshIntegration = false;
        enableBashIntegration = false;
        settings = {
          # NO SYNC. Syncing shell history to a third-party server would put
          # every command — including the ones with a token in argv — outside
          # this machine. Section 9 does not allow that quietly.
          auto_sync = false;
          update_check = false;
          style = "compact";
          inline_height = 20;
          search_mode = "fuzzy";
          filter_mode_shell_up_key_binding = "session";
        };
      };
    };
  };

  # `~/.shellrc.local` — created once, never overwritten.
  #
  # It is the file that holds what belongs to ONE machine. The system must give
  # it a shape, and then get out of the way: rewriting it on every rebuild would
  # eat exactly the token the person put there.
  system.activationScripts.shellrcLocal = {
    deps = [ "users" ];
    text = ''
      if [ ! -e ${home}/.shellrc.local ]; then
        cp ${shellrcLocalTemplate} ${home}/.shellrc.local
        chown ${cfg.user}:users ${home}/.shellrc.local
        chmod 600 ${home}/.shellrc.local
      fi
    '';
  };
}
