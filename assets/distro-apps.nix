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
# - `offered`: the heavy programs the installer lists for a `full`
#   installation, each one marked or not, in the order shown (see below).
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
#     fallback = false;                  # optional: never opens a file by name order
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
    "cadcraft"
    "chrome"
    "claude-code"
    "codex"
    "common-lisp"
    "craft-launcher"
    "crosspipe"
    "deckcraft"
    "delta"
    "designcraft"
    "dig"
    "duf"
    "dust"
    "effectcraft"
    "emacs"
    "exiftool"
    "eza"
    "ffmpeg"
    "figcraft"
    "file"
    "filmcraft"
    "flutter"
    "fzf"
    "gh"
    "ghostscript"
    "ghostty"
    "git-lfs"
    "go"
    "gridcraft"
    "htop"
    "hyperfine"
    "imagemagick"
    "iotop"
    "javascript"
    "just"
    "killall"
    "libavif"
    "libheif"
    "libreoffice"
    "libva-utils"
    "libwebp"
    "lightcraft"
    "lm-sensors"
    "lsof"
    "ltrace"
    "mpv"
    "mtr"
    "nix-tools"
    "nmap"
    "onlyoffice"
    "orca-slicer"
    "oxipng"
    "pavucontrol"
    "pciutils"
    "pdfcraft"
    "photocraft"
    "poppler-utils"
    "procs"
    "python"
    "rust"
    "sd"
    "shellcheck"
    "shfmt"
    "smartmontools"
    "socat"
    "solidcraft"
    "soundcraft"
    "sox"
    "starship"
    "tcpdump"
    "tealdeer"
    "tmux"
    "tokei"
    "tree"
    "usbutils"
    "vectorcraft"
    "vlc"
    "vscode"
    "watchexec"
    "wget"
    "whois"
    "wordcraft"
    "yq"
    "yt-dlp"
    "zoxide"
    "zsh"
  ];

  retired = {
    cadcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    deckcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    designcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    effectcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    figcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    filmcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    gridcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    lightcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    pdfcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    photocraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    soundcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    vectorcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
    wordcraft = "subsumed by the Craft Launcher (craft-launcher), which downloads and updates the Crafting Apps itself";
  };

  # WHAT THE INSTALLER OFFERS TO LEAVE OUT, one row per program of the
  # registry (`nix/offered.nix` writes the installer's table from here, and
  # refuses an id the ledger above does not have).
  #
  # A `full` installation lists these on one screen, marked as below: a
  # program the person unmarks is not downloaded now and no update brings it
  # — a decline for that machine alone, in their own `declined.json`, which
  # the software centre undoes. Only `full` shows the screen, because none of
  # them is on a `minimal` machine (`offered-programs-are-the-registrys`).
  #
  # IN THIS ORDER ON SCREEN, so the order is the list's: the development
  # tools, the editors and makers, media, the office, then the crafts
  # group. `name` is the program's own name, the same in every language — data
  # and not a text key, like a launcher entry's `Name=`.
  offered = [
    {
      id = "android-studio";
      checked = true;
      name = "Android Studio";
    }
    {
      id = "flutter";
      checked = true;
      name = "Flutter";
    }
    {
      id = "android-sdk";
      checked = true;
      name = "Android SDK";
    }
    {
      id = "vscode";
      checked = true;
      name = "Visual Studio Code";
    }
    {
      id = "blender";
      checked = true;
      name = "Blender";
    }
    {
      id = "orca-slicer";
      checked = true;
      name = "OrcaSlicer";
    }
    {
      id = "vlc";
      checked = true;
      name = "VLC";
    }
    {
      id = "onlyoffice";
      checked = true;
      name = "ONLYOFFICE";
    }
    # Unmarked: ONLYOFFICE opens the same documents, and two office suites
    # are a choice the person makes, not one the installer makes for them.
    {
      id = "libreoffice";
      checked = false;
      name = "LibreOffice";
    }
    {
      id = "craft-launcher";
      checked = true;
      name = "ArtCraft Launcher";
    }
    {
      id = "solidcraft";
      checked = true;
      name = "SolidCraft";
    }
  ];

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

  # THE CRAFTS GROUP (`nix/modules/crafts.nix`, on in the full profile): since
  # 2026-10-30 the ArtCraft Launcher (`storytold/craft-launcher`), which
  # downloads and updates the Crafting Apps by itself, under $HOME and
  # outside the generation (the owner's amendment of 2026-10-30, replacing
  # 'nothing imperative'), and SolidCraft (`tangency-labs/solidcraft`). Until
  # then the group was thirteen Crafting Apps, one line each; their ids are
  # `retired` above. One AppImage each, wrapped at build
  # (`appimageTools.wrapType2`) as `ronoctua-appimage-<key>`, and a part. Each
  # follows its repository's latest release, which `nix run .#distro-pins`
  # pins and `nix run .#release` moves before each gate. A person who turns
  # on following them (`distro-apps-follow-releases` in `options.json`, OFF
  # by default: `ronoctua_person::options`) moves their own machines past
  # those pins (`software/distro-pins.json`).
  # The pattern names the x86_64 image alone: the `.zsync` beside it and the
  # aarch64 one never match. No MIME default rides with them. And
  # `fallback = false`: the thirteen claimed photographs, films, songs and
  # documents, and the file manager's opener, which goes by name order where
  # no default chose, handed them to whichever app's name sorted first
  # ("FigCraft" before "Google Chrome"). They stay in "open with", and a
  # default — the person's or the distro's — still reaches them. The crafts
  # the launcher fetches are not lines here: the distro neither wraps them
  # nor writes their launchers, and neither they nor the launcher's licence
  # have been measured (`docs/PENDING.md`).
  #
  # LEFT OUT, and why:
  # - ArtCraft itself (`storytold/artcraft`, the AI engine): its releases
  #   carry a macOS and a Windows build, and no Linux one.
  # - AniCraft (`storytold/AniCraft`): no release to pin.
  apps = {
    # THE CRAFT LAUNCHER manages the Crafting Apps (PhotoCraft, VectorCraft,
    # FilmCraft, …) — it downloads and updates them by itself, under `$HOME`
    # and outside the generation (owner amendment 2026-10-30). The distro
    # declares the launcher, not each craft; the retired craft ids below are
    # the ones it subsumed.
    craft-launcher = {
      source = "appimage";
      key = "craft-launcher";
      name = "ArtCraft Launcher";
      follows = {
        github = {
          repo = "storytold/craft-launcher";
          assetPattern = "artcraft-launcher-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    # SOLIDCRAFT — parametric 3D CAD in Rust (`tangency-labs/solidcraft`,
    # MIT/Apache-2.0). Its x86_64 AppImage follows the repository's releases
    # through the distro's pin, which moves as the launcher's does; nothing
    # here updates it under $HOME (2026-10-30).
    solidcraft = {
      source = "appimage";
      key = "solidcraft";
      name = "SolidCraft";
      follows = {
        github = {
          repo = "tangency-labs/solidcraft";
          assetPattern = "solidcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
  };
}
