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

  retired = { };

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
  # tools, the editors and makers, media, the office, then the Crafting
  # Apps. `name` is the program's own name, the same in every language — data
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
      id = "photocraft";
      checked = true;
      name = "PhotoCraft";
    }
    {
      id = "vectorcraft";
      checked = true;
      name = "VectorCraft";
    }
    {
      id = "filmcraft";
      checked = true;
      name = "FilmCraft";
    }
    {
      id = "lightcraft";
      checked = true;
      name = "LightCraft";
    }
    {
      id = "pdfcraft";
      checked = true;
      name = "PdfCraft";
    }
    {
      id = "effectcraft";
      checked = true;
      name = "EffectCraft";
    }
    {
      id = "designcraft";
      checked = true;
      name = "DesignCraft";
    }
    {
      id = "wordcraft";
      checked = true;
      name = "WordCraft";
    }
    {
      id = "soundcraft";
      checked = true;
      name = "SoundCraft";
    }
    {
      id = "deckcraft";
      checked = true;
      name = "DeckCraft";
    }
    {
      id = "gridcraft";
      checked = true;
      name = "GridCraft";
    }
    {
      id = "figcraft";
      checked = true;
      name = "FigCraft";
    }
    {
      id = "cadcraft";
      checked = true;
      name = "CADCraft";
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

  # THE CRAFTING APPS (`storytold/<app>` on GitHub, Apache-2.0, pure Rust):
  # one AppImage each, wrapped at build (`appimageTools.wrapType2`) as
  # `ronoctua-appimage-<key>`, in the `crafts` group (`nix/modules/crafts.nix`,
  # on in the full profile). They follow their repository's latest release,
  # which `nix run .#distro-pins` pins; the owner moves the pins, by hand.
  # Following releases is the default model now, not a gesture a person turns
  # on: the whole system rolls on `nixos-unstable` and these fast-movers track
  # their upstream rather than sit on a stable pin
  # (`software/distro-pins.json`, the owner's amendment of 2026-10-30,
  # replacing 'nothing imperative').
  # The pattern names the x86_64 image alone: the `.zsync` beside it and the
  # aarch64 one never match. No MIME default rides with them: they are 0.x.
  # And `fallback = false`: their launchers claim photographs, films, songs
  # and documents, and the file manager's opener, which goes by name order
  # where no default chose, would hand them to whichever app's name sorts
  # first ("FigCraft" before "Google Chrome"). They stay in "open with", and
  # a default — the person's or the distro's — still reaches them.
  #
  # LEFT OUT, and why:
  # - ArtCraft itself (`storytold/artcraft`, the AI engine): its releases
  #   carry a macOS and a Windows build, and no Linux one.
  # - AniCraft (`storytold/AniCraft`): no release to pin.
  #
  # The Craft Launcher (`storytold/craft-launcher`) downloads and updates the
  # Craft Apps by itself, under $HOME and outside the generation — and that is
  # now the chosen mechanism for them: the app layer self-updates while the OS
  # substrate stays declarative with rollback (the owner's amendment of
  # 2026-10-30, replacing 'nothing imperative').
  apps = {
    cadcraft = {
      source = "appimage";
      key = "cadcraft";
      name = "CADCraft";
      follows = {
        github = {
          repo = "storytold/cadcraft";
          assetPattern = "cadcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    deckcraft = {
      source = "appimage";
      key = "deckcraft";
      name = "DeckCraft";
      follows = {
        github = {
          repo = "storytold/deckcraft";
          assetPattern = "deckcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    designcraft = {
      source = "appimage";
      key = "designcraft";
      name = "DesignCraft";
      follows = {
        github = {
          repo = "storytold/designcraft";
          assetPattern = "designcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    effectcraft = {
      source = "appimage";
      key = "effectcraft";
      name = "EffectCraft";
      follows = {
        github = {
          repo = "storytold/effectcraft";
          assetPattern = "effectcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    figcraft = {
      source = "appimage";
      key = "figcraft";
      name = "FigCraft";
      follows = {
        github = {
          repo = "storytold/figcraft";
          assetPattern = "figcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    filmcraft = {
      source = "appimage";
      key = "filmcraft";
      name = "FilmCraft";
      follows = {
        github = {
          repo = "storytold/filmcraft";
          assetPattern = "filmcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    gridcraft = {
      source = "appimage";
      key = "gridcraft";
      name = "GridCraft";
      follows = {
        github = {
          repo = "storytold/gridcraft";
          assetPattern = "gridcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    lightcraft = {
      source = "appimage";
      key = "lightcraft";
      name = "LightCraft";
      follows = {
        github = {
          repo = "storytold/lightcraft";
          assetPattern = "lightcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    pdfcraft = {
      source = "appimage";
      key = "pdfcraft";
      name = "PdfCraft";
      follows = {
        github = {
          repo = "storytold/pdfcraft";
          assetPattern = "pdfcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    photocraft = {
      source = "appimage";
      key = "photocraft";
      name = "PhotoCraft";
      follows = {
        github = {
          repo = "storytold/photocraft";
          assetPattern = "photocraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    soundcraft = {
      source = "appimage";
      key = "soundcraft";
      name = "SoundCraft";
      follows = {
        github = {
          repo = "storytold/soundcraft";
          assetPattern = "soundcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    vectorcraft = {
      source = "appimage";
      key = "vectorcraft";
      name = "VectorCraft";
      follows = {
        github = {
          repo = "storytold/vectorcraft";
          assetPattern = "vectorcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
    wordcraft = {
      source = "appimage";
      key = "wordcraft";
      name = "WordCraft";
      follows = {
        github = {
          repo = "storytold/wordcraft";
          assetPattern = "wordcraft-*-linux-x86_64.AppImage";
        };
      };
      fallback = false;
      group = "crafts";
    };
  };
}
