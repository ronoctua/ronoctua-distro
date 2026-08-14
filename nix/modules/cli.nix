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
# by the installer ISO, where most of this does not exist.
{ pkgs, ... }:

{
  environment.systemPackages = with pkgs; [
    # ---- reading and searching ----
    less # the `$PAGER`; `$LESS` is set in shellrc
    bat # `cat` with highlighting, and the `$MANPAGER`
    eza # `ls`, with git status and tree
    ripgrep
    fd
    fzf
    tree

    # ---- data ----
    jq
    yq-go
    delta # git diff with syntax highlighting

    # ---- transfer ----
    curl
    wget
    aria2 # segmented download; the one that saves a 4 GB ISO on bad wifi
    rsync
    # NOT for this repository — its fonts are Release assets and no file here is
    # an LFS object. It is here because a machine that clones OTHER people's
    # repositories meets LFS, and without it the clone silently yields pointers.
    git-lfs
    gh

    # ---- the machine's state ----
    htop
    btop
    iotop
    procs # `ps` that is readable
    dust # `du` sorted by size
    duf # `df` with a bar
    lsof
    strace
    ltrace
    pciutils
    usbutils
    smartmontools

    # ---- network ----
    iproute2 # `ss`, `ip` — `ports` and `ips` in shellrc use these
    nmap # the `scan` function
    tcpdump
    mtr
    dig
    whois
    socat
    ethtool
    iw

    # ---- day to day ----
    tmux
    tealdeer # `tldr`: the example one actually wanted from `man`
    hyperfine
    tokei
    sd # `sed s///` without the escaping
    just
    watchexec
    file
    which
    killall
    lm_sensors

    # ---- shell hygiene ----
    shellcheck
    shfmt
  ];

  # `tldr` needs its cache built once; without this the first run of every new
  # machine is a download that fails behind a captive portal.
  systemd.services.tealdeer-update = {
    description = "Atualiza o cache do tldr";
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
}
