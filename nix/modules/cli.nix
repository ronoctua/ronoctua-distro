# The terminal's tools — the ones typed every day.
#
# WHY A MODULE OF ITS OWN, AND NOT MORE LINES IN base.nix
# -------------------------------------------------------
# `base.nix` is what makes the machine BOOT and have a user. What is here is
# comfort: take it out and the system still comes up. Keeping the two apart is
# what lets someone building a smaller image drop this file and know exactly
# what they lost.
#
# Every tool here is referenced by `assets/shell/shellrc.sh`, which guards each
# one with `command -v`. The guard is not paranoia: the same shell file is used
# by the installer ISO, where most of this does not exist — and it is what
# lets a person decline any one of them.
#
# EACH TOOL IS A PROGRAM OF THE REGISTRY (`nix/modules/distro-apps.nix`), one
# entry per tool, because "no tmux here" is the decision a person makes. The
# few lines left in `systemPackages` at the end are tools this file does NOT
# bring alone — NixOS or another module installs them on every machine — and
# an entry for one would be a decline that can never be honoured.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  # One entry per tool, BY ITS NIXPKGS NAME: the id is the attribute, so
  # the two are written once.
  named = lib.genAttrs [
    # ---- reading and searching ----
    "bat" # `cat` with highlighting, and the `$MANPAGER`
    "eza" # `ls`, with git status and tree
    "tree"

    # ---- data ----
    "delta" # git diff with syntax highlighting

    # ---- transfer ----
    "wget"
    "aria2" # segmented download; the one that saves a 4 GB ISO on bad wifi
    # NOT for this repository — its fonts are Release assets and no file here is
    # an LFS object. It is here because a machine that clones OTHER people's
    # repositories meets LFS, and without it the clone silently yields pointers.
    "git-lfs"
    "gh"

    # ---- the machine's state ----
    "iotop"
    "procs" # `ps` that is readable
    "dust" # `du` sorted by size
    "duf" # `df` with a bar
    "lsof"
    "ltrace"
    "pciutils"
    "usbutils"
    "smartmontools"

    # ---- network ----
    "nmap" # the `scan` function
    "tcpdump"
    "mtr"
    "dig"
    "whois"
    "socat"

    # ---- day to day ----
    "tmux"
    "tealdeer" # `tldr`: the example one actually wanted from `man`
    "hyperfine"
    "tokei"
    "sd" # `sed s///` without the escaping
    "just"
    "watchexec"
    "file"
    "killall"

    # ---- shell hygiene ----
    "shellcheck"
    "shfmt"
  ] (name: pkgs.${name});
  # And the two whose nixpkgs name is not the one a person knows them by: an
  # id is a slug (`lm_sensors` is not one), and `yq-go` is the `yq` on the
  # prompt.
  tools = named // {
    yq = pkgs.yq-go;
    lm-sensors = pkgs.lm_sensors;
  };
in
{
  # The registry the tools are written into (see `browser.nix` for why it is
  # imported here too).
  imports = [ ./distro-apps.nix ];

  ronoctua.apps = lib.mapAttrs (_: package: { inherit package; }) tools // {
    # THE THREE WITH A LAUNCHER ENTRY. `btop` is THE DEFAULT PROCESS MANAGER,
    # the one `Ctrl+Alt+Esc` opens, and `bottom` the second, on
    # `LEADER+S-Esc` — and declining either takes it off the prompt and NOT
    # off the shortcut: what the shortcut runs is the store path baked into
    # the shell by `nix/rust.nix`, because a scope's PATH is minimal and this
    # is precisely the window one opens when the machine is already
    # struggling.
    htop = {
      package = pkgs.htop;
      desktopIds = [ "htop.desktop" ];
    };
    btop = {
      package = pkgs.btop;
      desktopIds = [ "btop.desktop" ];
    };
    bottom = {
      package = pkgs.bottom; # `btm`
      desktopIds = [ "bottom.desktop" ];
    };
  };

  # NOT ENTRIES: the tools a decline could never take away, because NixOS or
  # another module of the distro installs them on every machine anyway
  # (measured on the three systems' `systemPackages`). They stay listed here
  # so this file still says, in one place, what the terminal has.
  #   - `less` (the `$PAGER`; `$LESS` is set in shellrc), `curl`, `which`,
  #     `iproute2` (`ss`, `ip` — `ports` and `ips` in shellrc), `rsync` and
  #     `strace`: NixOS's own (`system-path.nix`, the network modules);
  #   - `ripgrep`, `fd`, `jq`: the base's (`base.nix`);
  #   - `iw`, `ethtool`: the network's tools for looking at the link
  #     (`network.nix`).
  # `fzf` is the shell's, in the registry (`nix/modules/home.nix`): a second
  # copy here would keep it on a machine whose person declined it.
  environment.systemPackages = with pkgs; [
    less
    curl
    which
    iproute2
    rsync
    strace
    ripgrep
    fd
    jq
    iw
    ethtool
    # ON THE PERSON'S PATH, not just the shell's: the shell copies through its
    # own `RONOCTUA_WL_COPY` (`nix/rust.nix`), but a terminal program the person
    # runs — Claude Code pasting a screenshot, which shells out to
    # `wl-paste --type image/png` — finds the clipboard only if `wl-paste` and
    # `wl-copy` are on `PATH`. Without this, a capture that IS on the clipboard
    # still read as "no image found".
    wl-clipboard
  ];

  # `tldr` needs its cache built once; without this the first run of every new
  # machine is a download that fails behind a captive portal. Only where the
  # program is: a cache refreshed for a `tldr` the person declined is a weekly
  # download for nobody, and its store path would keep the program in the
  # system's closure.
  systemd.services.tealdeer-update = lib.mkIf config.ronoctua.distroApps.installed.tealdeer {
    description = "Updates the tldr cache";
    after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    startAt = "weekly";
    serviceConfig = {
      Type = "oneshot";
      User = "nobody";
      # A missing cache is not a reason to fail a boot.
      ExecStart = "${pkgs.tealdeer}/bin/tldr --update";
      SuccessExitStatus = [
        0
        1
      ];
    };
  };
  # And the update's own refresh of the person's cache (`tldr-cache`), the
  # shape `software.nix` has for the steps of a support this machine lacks.
  ronoctua.update.steps.tldr-cache = lib.mkDefault config.ronoctua.distroApps.installed.tealdeer;
}
