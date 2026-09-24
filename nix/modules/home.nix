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

  # THE ELEVEN DIRECTORIES, from the ONE file that holds them.
  #
  # Names relative to home; the `${home}/` goes on here. Written out again in
  # this file, the sidebar of the file manager and the compositor's
  # `screenshot-path` would each have their own idea of where projects live —
  # see `nix/user-dirs.nix` for who else reads it.
  dirs = import ../user-dirs.nix;

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

      # THE ELEVEN DIRECTORIES, section 2. The names come from `nix/user-dirs.nix`.
      #
      # `xdg.userDirs` writes `~/.config/user-dirs.dirs`, which is what a file
      # dialog, a browser download and `xdg-open` actually read. Setting the
      # environment variable alone would move nothing.
      xdg.userDirs = {
        enable = true;
        createDirectories = true;
        desktop = "${home}/${dirs.desktop}";
        documents = "${home}/${dirs.documents}";
        download = "${home}/${dirs.download}";
        music = "${home}/${dirs.music}";
        pictures = "${home}/${dirs.pictures}";
        publicShare = "${home}/${dirs.publicShare}";
        templates = "${home}/${dirs.templates}";
        videos = "${home}/${dirs.videos}";

        # `XDG_PROJECTS_DIR` is not in the freedesktop spec — `xdg-user-dirs-update`
        # does not know it and no third-party tool will read it. home-manager
        # writes it anyway, and it defaults to `~/Projects`: leaving it alone
        # would have put a directory nobody asked for in the generated file.
        projects = "${home}/${dirs.projects}";

        # AND THE TENTH, WHICH HOME-MANAGER HAS NO OPTION FOR.
        #
        # The key here is NAKED: the module builds `XDG_<key>_DIR` around what
        # it is given, so `SCREENSHOT` comes out as `XDG_SCREENSHOT_DIR`. Written
        # as `XDG_SCREENSHOT_DIR` it would come out `XDG_XDG_SCREENSHOT_DIR_DIR`.
        #
        # And `createDirectories` above covers `extraConfig` too, which is the
        # whole reason this line exists rather than a `tmpfiles` rule: writing it
        # is what MAKES the directory. Until now the distro declared nowhere for
        # a capture to land, so one went wherever the program that took it
        # chose. `nix/modules/scroll.nix` reads the same entry for
        # `RONOCTUA_SCREENSHOTS`, which is what the footer builds a capture's
        # name from, and a check compares the two.
        extraConfig = {
          SCREENSHOT = "${home}/${dirs.screenshot}";
          # THE ELEVENTH, the player's music videos — read by the shell as
          # `XDG_MUSIC_VIDEO_DIR` (`player_browse::places`).
          MUSIC_VIDEO = "${home}/${dirs.musicVideo}";
        };
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

        # THE SETTINGS MOVED TO `nix/modules/atuin.nix`, AND SO DID THE
        # DECISION THEY CARRIED.
        #
        # What used to be here was `auto_sync = false` with this reason:
        #
        #     NO SYNC. Syncing shell history to a third-party server would put
        #     every command — including the ones with a token in argv —
        #     outside this machine. Section 9 does not allow that quietly.
        #
        # It was right about the risk and it decided for the person, and the
        # owner asked for the other answer. Two things that sentence did not
        # weigh: Atuin's sync is END TO END ENCRYPTED, so the server holds
        # bytes nobody there can read; and with `ronoctua.atuin.server.enable`
        # the server can be this household's, so "third party" need not
        # describe the arrangement at all. The command with a token in its
        # argv is still a command — which is why `secrets_filter` and
        # `history_filter` are DRAWN ROWS of `LEADER s h` rather than defaults
        # nobody reads.
        #
        # And `settings` is gone rather than moved, because home-manager writes
        # it as a symlink INTO THE STORE: read-only, so choosing between two
        # search modes would need a `nixos-rebuild`. That is the sentence
        # `LEADER s l` exists to remove. `nix/modules/atuin.nix` seeds the file
        # once, the way `~/.shellrc.local` below is seeded, and the view edits
        # it a line at a time.
      };
    };
  };

  # `~/.shellrc.local` — created once, never overwritten.
  #
  # It is the file that holds what belongs to ONE machine. The system must give
  # it a shape, and then get out of the way: rewriting it on every rebuild would
  # eat exactly the token the person put there.
  # `~/.config` IS THE PERSON'S, and this is the one place that makes it.
  #
  # Activation runs as root. Two seed scripts (`atuinConfig`, `doomLocal`)
  # each did a `mkdir -p` under `~/.config`, which on a fresh home made
  # `~/.config` itself root's — and home-manager, which runs as the person,
  # then could not link a single file there. That was the
  # `[FAILED] Home Manager environment for noctua` of the first `nix run .#vm`.
  # A seed that writes under `~/.config` lists this in its `deps` and creates
  # its own folder with `install -d -o`, never `mkdir -p`.
  system.activationScripts.configHome = {
    deps = [ "users" ];
    text = ''
      if [ ! -d ${home}/.config ]; then
        install -d -m 0755 -o ${cfg.user} -g users ${home}/.config
      fi
      # AND A HOME THAT WENT THROUGH THE DEFECT IS REPAIRED: the folder
      # itself, never recursively — what is inside may be anybody's.
      for d in ${home}/.config ${home}/.config/doom ${home}/.config/atuin; do
        if [ -d "$d" ] && [ "$(stat -c %u "$d")" = 0 ]; then
          chown ${cfg.user}:users "$d"
        fi
      done
    '';
  };

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
