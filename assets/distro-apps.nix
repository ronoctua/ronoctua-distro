# The distro's program registry — what is declared by hand about it.
#
# The registry itself is `ronoctua.apps.<id>` (`nix/modules/distro-apps.nix`):
# every module that installs a program for the person to choose to use
# writes its entry beside the code that owns it (`browser.nix` writes
# Chrome's, `media.nix` VLC's, `cli.nix` tmux's). What lives HERE is what no
# single module owns:
#
# - `essential`: the closed set of reasons a program may not be declined.
#   Each is the id of a sentence, `software-essential-why-<reason>`, in every
#   language (`essential-apps-say-why`). An entry names one or none.
# - `ids`: every id the registry has ever published, so a rename cannot drop
#   a person's decline in silence (`distro-app-ids-are-stable`): an id leaves
#   this list only into an entry's `formerly` or into `retired`.
# - `retired`: ids of programs the distro no longer carries, with the reason.
#   A decline of one is reported as unknown and harms nothing.
# - `infrastructure`: launcher entries in the system that belong to no
#   program of the registry, with the reason (`distro-apps-are-registered`).
# - `apps`: the distro's own programs from the sources that are not nixpkgs —
#   Flatpak, AppImage, web and Android — in the shapes the person's
#   declarations have. Their pins live in `assets/distro-pins.json`, written
#   by `nix run .#distro-pins`, never by hand when a source can be followed.
#
# An `apps` entry, by source (every field closed; `nix/modules/distro-apps.nix`
# refuses an unknown one):
#
#   <id> = {
#     source = "flatpak";  appId = "org.example.App";  origin = "flathub";
#     group = "base";  essential = null;  formerly = [ ];
#   };
#   <id> = {
#     source = "appimage";  key = "example";  name = "Example";
#     follows = { github = { repo = "owner/repo"; assetPattern = "*-x86_64.AppImage"; }; };
#     group = "base";
#   };                                   # `follows = null`: the pin is moved by hand
#   <id> = {
#     source = "web";  slug = "example";  name = "Example";
#     url = "https://example.org/";  profile = "Default";  category = "Network";
#     group = "base";
#   };
#   <id> = {
#     source = "android";  package = "org.example.app";
#     from = "f-droid";  image = "vanilla";   # `from = "direct"`: pinned by hand
#     group = "base";
#   };
{
  essential = [
    "the-login-shell"
  ];

  ids = [
    "alsa-utils"
    "android-sdk"
    "android-studio"
    "aria2"
    "atuin"
    "bat"
    "blender"
    "bottom"
    "btop"
    "build-tools"
    "chrome"
    "claude-code"
    "codex"
    "crosspipe"
    "delta"
    "dig"
    "duf"
    "dust"
    "emacs"
    "exiftool"
    "eza"
    "ffmpeg"
    "file"
    "flutter"
    "fzf"
    "gh"
    "ghostscript"
    "ghostty"
    "git-lfs"
    "go"
    "htop"
    "hyperfine"
    "imagemagick"
    "iotop"
    "javascript"
    "just"
    "killall"
    "libavif"
    "libheif"
    "libva-utils"
    "libwebp"
    "lm-sensors"
    "lsof"
    "ltrace"
    "mpv"
    "mtr"
    "nix-tools"
    "nmap"
    "orca-slicer"
    "oxipng"
    "pavucontrol"
    "pciutils"
    "poppler-utils"
    "procs"
    "python"
    "rust"
    "sd"
    "shellcheck"
    "shfmt"
    "smartmontools"
    "socat"
    "sox"
    "starship"
    "tcpdump"
    "tealdeer"
    "tmux"
    "tokei"
    "tree"
    "usbutils"
    "vlc"
    "watchexec"
    "wget"
    "whois"
    "yq"
    "yt-dlp"
    "zoxide"
    "zsh"
  ];

  retired = { };

  infrastructure = {
    # The Syncthing service's own page in a browser. The service is the
    # distro's synchronisation (`ronoctua-syncthing`), turned on and off in
    # its own view; the entry comes with the package the service runs.
    "syncthing-ui.desktop" =
      "the synchronisation service's page, which comes with the service and not as a program";
    # Waydroid is the SUPPORT for the person's Android apps
    # (`ronoctua.software.android`, a switch of the profile): the apps are
    # what the person installs, and this entry is the container's own.
    "Waydroid.desktop" = "the support for the person's Android apps, switched with the profile";
    # Incus's VGA console (`nix/modules/incus.nix`): `incus console
    # --type=vga` runs `remote-viewer`, so the viewer is the feature's
    # equipment, switched with the profile, and its entry comes along.
    "remote-viewer.desktop" =
      "the console of the person's Incus virtual machines, which comes with Incus and not as a program";
  };

  apps = { };
}
