{
  description = "ronoctua-distro — a NixOS distribution with no bar, dock or start menu: only the content and a two-line footer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    # --- compositor -----------------------------------------------------
    # Invariant 3: NO FORK, NO PATCH — neither of the compositor nor of the
    # wlroots it carries inside `subprojects/`. Consumed as upstream, pinned
    # to a tagged release.
    #
    # `flake = false`: upstream ships no `flake.nix`. What builds it is
    # `nix/scroll.nix`, against THIS project's nixpkgs — the header there
    # carries why not the third-party flake (it brings a second nixpkgs, and
    # the whole delta turned out to be one patch release of libdrm) and where
    # wlroots is.
    #
    # THERE IS NO SECOND INPUT FOR DRIFT, and that is a decision. Half of the
    # IPC surface is i3's, frozen by compatibility for a decade; the drift
    # risk that a calendar-versioned compositor carried is not the same size,
    # and the generator already refuses at build time a command the pinned
    # source does not have.
    scroll-src = {
      url = "github:dawsers/scroll?ref=refs/tags/1.12.21";
      flake = false;
    };

    # --- system and installation ----------------------------------------
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    lanzaboote = {
      url = "github:nix-community/lanzaboote/v1.1.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # THE DOTFILES ARE DECLARATIVE TOO (invariant 6).
    #
    # Without it, `~/.zshrc` would be an activation script copying files into
    # $HOME — imperative, with no rollback, and drifting the moment someone
    # edits the copy. home-manager makes the shell a generation like any other.
    home-manager = {
      url = "github:nix-community/home-manager/release-26.05";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixos-facter-modules.url = "github:nix-community/nixos-facter-modules";
    nixos-hardware.url = "github:NixOS/nixos-hardware";
    sops-nix = {
      url = "github:Mic92/sops-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Declarative Flatpaks, READY and holding nothing but the Flathub remote:
    # the person installs imperatively through `ronoctua-software` (invariant
    # 6's dated exception), and this is the door for the day something should
    # be declared instead. A TAG, never `latest` (which moves) nor `main`. The
    # flake has no inputs of its own, so there is nothing to `follows`.
    nix-flatpak.url = "github:gmodena/nix-flatpak/?ref=v0.7.0";

    # --- rust -----------------------------------------------------------
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # --- fonts ----------------------------------------------------------
    # The noctua-iosevka family (4.1 GB) is NOT in this tree, and the reason is
    # measured, not aesthetic: while the files sat inside the flake's tree, EVERY
    # edit to any file in the project wrote a new 4.1 GB path into the store,
    # because Nix copies the whole tree. As a pinned input they are fetched once
    # and cached forever — and `nix flake check` goes back to being instant.
    #
    # Nor are they in git, in any repository: git keeps every version forever, so
    # one Iosevka bump would add another 4.1 GB that no `gc` takes back. They are
    # build output, and they go out as assets of a RELEASE of this repository,
    # under a tag of their own (`fonts-v1`) so as not to be confused with the ISO.
    #
    # FOUR inputs and not one, because a Release asset is capped at 2 GB and
    # `nix run .#fonts-release` therefore packs one tarball per folder. The four
    # are reassembled into a single `noctua-iosevka/` tree in `nix/fonts.nix`.
    #
    # `file+https://` and not plain `https://`: the bare form is inferred as
    # `type = "tarball"`, and Nix would unpack each asset into the store — 4.1 GB
    # of loose faces that the very next derivation copies again. `file+` keeps
    # them as the 1.4 GB of compressed files they are, and the build unpacks them
    # once, straight into the directory where the naming gets fixed.
    #
    # WHAT PINS THEM is `flake.lock`, through the `narHash` of each asset — not a
    # tag and not a rev. `fonts-v1` is a mutable pointer like any other git tag;
    # the hash is what makes a re-download either identical or a hard error.
    fonts-ttf = {
      url = "file+https://github.com/ronoctua/ronoctua-distro/releases/download/fonts-v1/noctua-iosevka-TTF.tar.zst";
      flake = false;
    };
    fonts-ttf-nerdfonts = {
      url = "file+https://github.com/ronoctua/ronoctua-distro/releases/download/fonts-v1/noctua-iosevka-TTF-nerdfonts.tar.zst";
      flake = false;
    };
    fonts-woff2 = {
      url = "file+https://github.com/ronoctua/ronoctua-distro/releases/download/fonts-v1/noctua-iosevka-WOFF2.tar.zst";
      flake = false;
    };
    fonts-woff2-nerdfonts = {
      url = "file+https://github.com/ronoctua/ronoctua-distro/releases/download/fonts-v1/noctua-iosevka-WOFF2-nerdfonts.tar.zst";
      flake = false;
    };

    # NOTE about icon data (glyphnames.json):
    #
    # There is NO `nerd-fonts` input here, on purpose. The Nerd Fonts
    # repository is several GB (it carries every patched font), and we only
    # need ONE file: glyphnames.json, the authority on glyph NAMES.
    # Pinning the whole repository as an input would cost minutes and
    # gigabytes on every lock resolution.
    #
    # Instead, `nix/glyphgen.nix` fetches only that file with fetchurl and a
    # fixed hash. The expected version is cross-checked against the font's own nameID 5,
    # and the build fails if they diverge (section 6.3).
  };

  outputs =
    inputs@{ self, nixpkgs, ... }:
    let
      # Section 2: target machines are x86_64 with UEFI and TPM2. A single system.
      system = "x86_64-linux";
      lib = nixpkgs.lib;

      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          inputs.rust-overlay.overlays.default
          # THE ONE yt-dlp, newer than the pinned nixpkgs' — see nix/yt-dlp.nix.
          (import ./nix/yt-dlp.nix)
        ];
      };

      # ---------------------------------------------------------------
      # Font layer (section 6 of the document, section 1 of the plan)
      # ---------------------------------------------------------------
      fonts = pkgs.callPackage ./nix/fonts.nix {
        # Folder name → the Release asset that carries it. The names are the
        # ones `fix-font-names.py` and `mkFontPkg` already use; keeping them as
        # the KEY means the mapping cannot silently go out of step.
        fontAssets = {
          "TTF" = inputs.fonts-ttf;
          "TTF-nerdfonts" = inputs.fonts-ttf-nerdfonts;
          "WOFF2" = inputs.fonts-woff2;
          "WOFF2-nerdfonts" = inputs.fonts-woff2-nerdfonts;
        };
      };

      fontconfigConf = pkgs.callPackage ./nix/fontconfig.nix {
        inherit fonts;
        # THE FALLBACK FONTS, so the nested loop's `FONTCONFIG_FILE` finds
        # something. On NixOS what installs them is `fonts.packages`; on a
        # non-NixOS host they only exist if they are here.
        fallbackFonts = with pkgs; [
          noto-fonts
          noto-fonts-color-emoji
          dejavu_fonts
        ];
      };

      # Glyph layer: the icon token table (6.3) and the browser's database
      # (6.5), generated from the real font file by a single generator.
      glyphdb = pkgs.callPackage ./nix/glyphgen.nix {
        inherit languages;
        inherit fonts;
        iconTokensFile = ./assets/tokens/icons.nix;
        typographyFile = ./assets/tokens/typography.nix;
        trayTableFile = ./assets/icons/tray-glyphs.nix;
        lexiconFile = ./assets/lexicon/tags.en-pt.toml;
        overlayFile = ./assets/lexicon/overlay.toml;
        # A token naming `ours-<mark>` resolves HERE and is cross-checked
        # against the face that holds it, exactly as an `md-` one is checked
        # against the Nerd Font. See `nix/marksgen.nix`.
        inherit marksdb;
      };

      # The Unicode symbol browser's database (`LEADER i u`). It is the
      # INTERSECTION of the UCD with the font: the renderer has no fallback, so
      # a character the font does not draw would turn into tofu.
      unicodedb = pkgs.callPackage ./nix/unicodegen.nix {
        inherit languages;
        inherit fonts;
        # The SAME lexicon as the glyph one: the vocabulary belongs to Portuguese, not to the
        # buffer. Translating `arrow` twice would mean watching them diverge.
        lexiconFile = ./assets/lexicon/tags.en-pt.toml;
      };

      # The renderer's PRIMARY face: the Iosevka minus the 75 codepoints whose
      # default presentation is emoji. Without this it wins the fallback and draws `UP!`
      # in monochrome next to a coloured `ATM`. See `nix/primary-font.nix`.
      primaryFont = pkgs.callPackage ./nix/primary-font.nix {
        inherit fonts;
      };

      # THE FACES THE FALLBACK CHAIN NAMES — every script the renderer promises
      # to draw, one file per family, refusing to build if a family has none.
      # The chain itself is declared in `nix/fontconfig.nix`, which is also
      # where fontconfig's half comes from: one list, two consumers.
      coverageFonts = pkgs.callPackage ./nix/coverage-fonts.nix {
        inherit (fontconfigConf) chainJson;
      };

      # The same chain, as Rust. Generated rather than written twice — the
      # renderer must not be able to disagree with fontconfig about which face
      # draws a script. See nix/chain.nix.
      # How many cells a character takes. Unicode's own property, baked; the
      # cell arithmetic of the whole footer rests on it. See nix/width.nix.
      widthRs = pkgs.callPackage ./nix/width.nix { };

      # THE WORLD, for the globe and for the address under it. Borders, inland
      # water, named places and the catalogue of downloadable street regions,
      # all in one table the pure crate reads at RUN time. See nix/atlas.nix,
      # and `assets/atlas.nix` for the sources and their licences.
      atlasData = pkgs.callPackage ./nix/atlas.nix { };

      # THE DOWNLOADS, held by a root so a garbage collection cannot take what
      # the internet may no longer give back. See nix/kept-sources.nix.
      keepSources = "${
        pkgs.callPackage ./nix/kept-sources.nix {
          inherit inputs;
          downloads =
            atlasData.sources
            ++ broadcastRs.sources
            # COMMON LISP'S: alive-lsp's tag and the Quicklisp client's four
            # files, which live on one server (`nix/common-lisp/`).
            ++ (import ./nix/common-lisp { inherit pkgs; }).sources
            ++ [
              pkgs.bibata-cursors.src
              # yt-dlp's tag, pinned by nix/yt-dlp.nix: an old tag is never
              # taken down, but this keeps a rebuild from needing GitHub.
              pkgs.yt-dlp.src
              segmentModel
              handModels.task
              # CHROME'S .deb, which Google's pool serves only while the release
              # is current: `kiosk-vm` (in `.#most`) and the `full` profile's
              # browser build from it, and no cache has an unfree source.
              unfreePkgs.google-chrome.src
            ]
            # THE DISTRO'S APPIMAGES AND APKS at their pins (the Crafting
            # Apps, ~0,95 GB): a release upstream deletes is gone for good,
            # and the pin is what the gate tested. See nix/distro-downloads.nix.
            ++ (import ./nix/distro-downloads.nix { inherit (pkgs) fetchurl; }).pinned {
              table = (import ./assets/distro-apps.nix).apps;
              pins = builtins.fromJSON (builtins.readFile ./assets/distro-pins.json);
            };
          # AND THE COMPILED UNITS of this checkout's tree: the release
          # graph's, and the loop's third-party ones the tests build on.
          units = {
            release = rust.release.keep;
            loop = rust.loop.keep;
          };
          keepUnits = pkgs.callPackage ./nix/keep-units.nix { };
          releaseDays = (import ./assets/cache.nix).oldCompilerDays;
        }
      }/bin/ronoctua-keep-sources";

      # HOW MANY COMPILATIONS FIT AT ONCE, and the plan of the units, each by
      # its measured class: one arithmetic for the batteries, `.#sized-build`,
      # `.#quick` and a machine's own apply. See nix/build-sizing.nix.
      buildSizing = pkgs.callPackage ./nix/build-sizing.nix { };
      # `nix build`, sized: every unit it reaches its own build, by its
      # class, the heavy ones alone, the test runs after every compilation.
      sizedBuild = buildSizing.sizedBuild (buildSizing.unitPlan { units = rust.unitIndex; });

      # The glance's places (`LEADER g`, `leader::GLANCE_KEYS`). One declaration,
      # and the build refuses a zone `zone1970.tab` does not know — see
      # nix/world.nix.
      worldRs = pkgs.callPackage ./nix/world.nix {
        worldFile = ./assets/world.nix;
      };

      # The player's channels by theme (`ronoctua_broadcast::table`). One
      # declaration, and the build refuses an iptv-org category that does not
      # exist — see nix/broadcast.nix.
      broadcastRs = pkgs.callPackage ./nix/broadcast.nix {
        channelsFile = ./assets/channels.nix;
      };

      # The languages of content (`ronoctua_player::tongues`): what a subtitle
      # is searched in and a stream is tagged with. One declaration, and the
      # build refuses a row that could not work — see nix/tongues.nix.
      tonguesRs = pkgs.callPackage ./nix/tongues.nix {
        tonguesFile = ./assets/tongues.nix;
      };

      chainRs = pkgs.callPackage ./nix/chain.nix {
        inherit (fontconfigConf) chainJson;
        primaryFontFile = "${primaryFont}/NoctuaIosevkaNerdFontMono-Regular.ttf";
      };

      # The Lucide icon browser's database (`LEADER i l`). Here the font does not
      # DELIMIT the database, it is REWRITTEN: Lucide's source codepoints
      # collide with 634 Nerd Fonts ones in the Iosevka, and the primary face wins the
      # fallback. See `nix/lucidegen.nix`.
      lucidedb = pkgs.callPackage ./nix/lucidegen.nix {
        inherit languages;
        inherit fonts;
        # The SAME lexicon as the other two: the vocabulary belongs to Portuguese, not to the
        # buffer.
        lexiconFile = ./assets/lexicon/tags.en-pt.toml;
      };

      # The face holding the marks this repository DRAWS — the fourth, and the
      # only one with no upstream font behind it. A mark the interface has to
      # draw and that exists in none of the five faces cannot become a pixmap
      # (invariant 10) nor a hard-coded codepoint (invariant 9): it is drawn
      # into a face of ours. See `nix/marksgen.nix`.
      marksdb = pkgs.callPackage ./nix/marksgen.nix {
        inherit fonts lucidedb;
        marksFile = ./assets/icons/marks.nix;
      };

      # Icon superpackage (6.6) — a pixmap fallback for foreign
      # apps. Invariant 10: the design system does NOT consult it.
      iconSuperpack = pkgs.callPackage ./nix/icon-superpack { };

      # Tokens: from assets/tokens/*.nix into a Rust table (section 7.6).
      # THE LANGUAGES, imported ONCE. Everything downstream — the generator,
      # the module's enum, the checks, the drivers, the extension — takes the
      # list from here rather than naming a language of its own.
      languages = import ./assets/i18n/languages.nix;

      # THE TEXTS: one merged, checked `<lang>.ftl` per language, made of
      # every crate's own fragment (`crates/<crate>/i18n/`), the extension's
      # and the brain modules', read by every program at RUN TIME
      # (`RONOCTUA_TEXTS`) — and, as `texts.keys`/`texts.keysOf`, each crate's
      # `key` module the Rust compiles against, written at evaluation from the
      # key NAMES of its own fragment alone. A sentence edited rebuilds no
      # crate. See nix/i18n.nix and nix/text-keys.nix.
      texts = pkgs.callPackage ./nix/i18n.nix {
        src = ./.;
        extensionDir = ./extension/i18n;
        inherit languages;
        brainFragments = brainModules.i18nFragments;
      };

      # THE THEMES, imported ONCE, like the languages. And the palettes, SOLVED
      # once for all of them: `assets/tokens/colors.nix` is recipes, and this
      # derivation is where they become values (section 7.6, one wall earlier).
      themeSpec = import ./assets/themes/themes.nix;
      # THE ORDER AND THE DEFAULT ARE TWO FACTS — see that file. Nothing
      # here may take `builtins.head themes` to mean "the default" again.
      themes = themeSpec.order;
      defaultTheme = themeSpec.default;
      # THE TOKENS' NAMES, read at EVALUATION: the colour roles and the icon
      # tokens, never a value. The Rust and the WIT are compiled against these
      # alone, so a seed, a recipe's value or the colour generator can move and
      # no crate and no buffer is rebuilt. See nix/token-names.nix.
      tokenNames = import ./nix/token-names.nix {
        inherit lib;
        recipesFile = ./assets/tokens/colors.nix;
        shapedFile = ./assets/tokens/shaped.nix;
        iconTokensFile = ./assets/tokens/icons.nix;
      };
      themed = pkgs.callPackage ./nix/themes.nix {
        inherit themes defaultTheme;
        roleNames = tokenNames.roles;
        themesDir = ./assets/themes;
        recipesFile = ./assets/tokens/colors.nix;
      };

      # THE CURSORS, one Xcursor theme per cursor colour. See nix/cursors.nix.
      cursors = pkgs.callPackage ./nix/cursors.nix { inherit themed defaultTheme; };

      tokensRs = pkgs.callPackage ./nix/tokens.nix {
        inherit glyphdb;
        roleNames = tokenNames.roles;
        trayTableFile = ./assets/icons/tray-glyphs.nix;
        categoryTableFile = ./assets/icons/category-glyphs.nix;
        typographyFile = ./assets/tokens/typography.nix;
        spaceFile = ./assets/tokens/space.nix;
      };

      # The notification catalogue and the filters the distro ships with
      # (section 4.2). Same shape as the tokens: declared once in Nix, and what
      # reads the table cannot invent a field that is not in it.
      notificationsRs = pkgs.callPackage ./nix/notifications.nix {
        inherit glyphdb;
        notificationsFile = ./assets/notifications.nix;
        # AND WHAT SOMEBODY SHARED. The shell writes this file — one filter at
        # a time, by name, after saying on screen that it becomes public — and
        # the build is what turns it into part of the distro.
        sharedFile = ./assets/shared/notifications.json;
      };

      # WHAT A BLUETOOTH SERVICE ID MEANS (section 4.2's shape, one file later).
      # A person reading `00001812-0000-1000-8000-00805f9b34fb` learns nothing;
      # this turns the catalogue into the table, the owed keys, and a refusal.
      bluetoothProfilesRs = pkgs.callPackage ./nix/bluetooth-profiles.nix {
        profilesFile = ./assets/bluetooth-profiles.nix;
      };

      # WHAT A SOUND PORT IS CALLED. ALSA names the sockets and nobody upstream
      # translates them; this turns our own words into the table, the owed keys,
      # and the refusal of an entry matched against nothing.
      audioNamesRs = pkgs.callPackage ./nix/audio-names.nix {
        namesFile = ./assets/audio-names.nix;
      };

      # EVERY OPTION THESE FOUR SURFACES EXPOSE, declared once each.
      #
      # Written by hand on the screen side, each of these would be a few dozen
      # to a few hundred decisions nobody could audit, and a rename upstream
      # would be invisible. One derivation makes the Rust table, the keys both
      # languages owe, and the refusal of a duplicate id, a rangeless number or
      # a ring of one.
      #
      # THE SCOPES ARE ORDERED HERE, because the order is the order the screen
      # draws in and an attribute set sorts alphabetically. See `nix/options.nix`.
      syncthingOptionsRs = pkgs.callPackage ./nix/options.nix {
        domain = "syncthing";
        scopes = [
          "folder"
          "device"
          "global"
          "interface"
        ];
        optionsFile = ./assets/syncthing-options.nix;
      };

      # ATUIN: what `LEADER s h` may write into `config.toml`. The `leaving`
      # scope is the one this whole feature turns on — it holds the switch AND
      # the filters that decide what is never recorded, which is the answer to
      # the objection `nix/modules/home.nix` used to state.
      atuinOptionsRs = pkgs.callPackage ./nix/options.nix {
        domain = "atuin";
        scopes = [
          "leaving"
          "search"
          "keys"
          "advanced"
        ];
        optionsFile = ./assets/atuin-options.nix;
      };

      # TOR: what the view may write into the torrc fragment. The flake-owned
      # rows are in here too, drawn and refusing — see `assets/tor-options.nix`.
      torOptionsRs = pkgs.callPackage ./nix/options.nix {
        domain = "tor";
        scopes = [
          "service"
          "path"
          "bridges"
          "onion"
          "advanced"
        ];
        optionsFile = ./assets/tor-options.nix;
      };

      # WHAT A KEY CAN BE MADE INTO, and the leader's own rule. Two places read
      # it — the NixOS module that configures keyd, and the shell that renders
      # a file per keyboard — so it is declared once.
      keyboardsRs = pkgs.callPackage ./nix/keyboards.nix {
        keyboardsFile = ./assets/keyboards.nix;
      };

      # The MIME database, from freedesktop's own files into a Rust table.
      #
      # `shared-mime-info` is a BUILD input and never a runtime one: its 75 MiB
      # closure is glib and libxml2, which belong to the program that compiled
      # the database upstream. What crosses is 125 KB of data — see
      # `nix/mimegen.nix` and the check `files-closure-has-no-mime-db`.
      mimeDb = pkgs.callPackage ./nix/mimegen.nix {
        inherit glyphdb;
        mimeTableFile = ./assets/icons/mime-glyphs.nix;
      };

      # The compositor's commands, DECLARED by us (section 13).
      #
      # Not extracted from a binary: the commands are free text and the only
      # list of them is in the compositor's source, which this generator reads
      # to refuse a verb that does not exist. See `nix/compositor-deeds.nix`.
      compositorDeeds = pkgs.callPackage ./nix/compositor-deeds.nix {
        deedsFile = ./assets/compositor-deeds.nix;
        scrollSrc = inputs.scroll-src;
      };

      # The toolchain the Rust workspace is built with (section 7.12): every
      # unit of `nix/rust.nix`, the devshell and the buffers.
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        targets = [ "wasm32-wasip2" ];
      };
      # WHERE A LOCAL `cargo` WRITES, named after this toolchain: the devshell,
      # the loop and every installed session read it from here, and nowhere
      # else spells the directories. See nix/cargo-dirs.nix.
      cargoDirs = import ./nix/cargo-dirs.nix { inherit (rustToolchain) version; };
      # DISPOSABLE Secure Boot keys, for the VM tests only.
      sbKeys = pkgs.callPackage ./nix/testing/sb-keys.nix { };

      # The WIT directory with the generated token enum inside (section 7.6).
      wit = pkgs.callPackage ./nix/wit.nix {
        tokensWit = tokenNames.wit;
        staticWit = ./crates/ronoctua-protocol/wit;
      };

      # THE SOFTWARE VULKAN THE CHECKS ALREADY USE, from the one file that names
      # it. `nix run .#png` draws with wgpu and needs an ICD; taking it from
      # here instead of writing the path again means a Mesa that renames the
      # file breaks both at once, and is fixed once.
      xvfbIcd = (import ./nix/software-gpu.nix { inherit pkgs lib; }).icd;

      # The native buffers, for `wasm32-wasip2`. After `rustToolchain`
      # because they use the SAME toolchain — the wasm target is already in it.
      buffers = pkgs.callPackage ./nix/buffers.nix {
        inherit wit;
        inherit rustToolchain;
        inherit widthRs;
        # `ronoctua-design` names the highlighter as an OPTIONAL path
        # dependency, so cargo resolves it in every buffer's tree even where
        # the feature is off — and its build script wants the finished table.
        inherit syntaxRs;
      };

      # THE TERMINAL ENGINE, from a pinned Ghostty commit. In the `let` and not
      # only under `packages` because `nix/rust.nix` needs the PATH: the crate
      # that links it has to be told where it is, like every other data file this
      # project hands the Rust.
      ghosttyVt = pkgs.callPackage ./nix/ghostty-vt.nix { };

      # THE C BORDER IN FRONT OF LIBTORRENT, built on its own so that no `.cpp`
      # has to survive `nix/rust.nix`'s file set — and so `build.rs` stays a
      # reader of one variable. See `nix/torrent-shim.nix` for the measurement
      # that chose libtorrent over librqbit.
      torrentShim = pkgs.callPackage ./nix/torrent-shim.nix { };

      # THE PO TOKEN MINTER, in its script shape: it runs when a token is
      # needed and exits. See `nix/pot-provider.nix` for why the server half is
      # deliberately not installed.
      potProvider = pkgs.callPackage ./nix/pot-provider.nix { };

      # THE BROWSER EXTENSION, with its texts (its own fragment,
      # `extension/i18n/`, which the texts merge too) and colours GENERATED
      # from what everything else reads. Invariants 8 and 9 do not stop at the
      # browser's edge.
      extension = pkgs.callPackage ./nix/extension.nix {
        # The DEFAULT theme's palette, as a flat `key -> 0xRRGGBBAA` map. The
        # extension cannot see the person's choice, so it wears the default.
        colorsJson = "${themed}/${defaultTheme}.json";
        spaceFile = ./assets/tokens/space.nix;
        i18nDir = ./extension/i18n;
        inherit languages;
        source = ./extension;
      };

      # THE BRAIN'S MODULES, read out of their folders ONCE: the Rust registry
      # below, the checks, the batteries' split, and — handed in by name —
      # the texts (`texts`) and the brain's catalogue (`brainCatalogue`). Only
      # `nix/modules/brain.nix` imports the loader again, for `imports`. See
      # `nix/brain-modules.nix`.
      brainModules = import ./nix/brain-modules.nix { inherit lib languages; };
      # ONE PART PER CRATE OF THE BRAIN: the distro's manifests for the library
      # the footer links (`ronoctua-brain`), and the registry — every module,
      # the person's included on a machine — for the daemon
      # (`ronoctua-brain-daemon`). Two derivations, so a person's module moves
      # only the second. See `nix/brain-modules-rs.nix`.
      brainManifestsRs = pkgs.callPackage ./nix/brain-modules-rs.nix {
        inherit brainModules rustToolchain languages;
        part = "library";
      };
      brainModulesRs = pkgs.callPackage ./nix/brain-modules-rs.nix {
        inherit brainModules rustToolchain languages;
        part = "daemon";
      };
      # AND WHAT THE FOOTER DRAWS OF THEM, as data it reads at run time
      # (`RONOCTUA_BRAIN_CATALOGUE`): every module's marks and their measured
      # glyphs, notices, actions, events and notifications, reasons and keys.
      # No crate is compiled against it, so a person's module moves this, the
      # texts and the daemon, and nothing that draws. See
      # `nix/brain-catalogue.nix`.
      brainCatalogue = pkgs.callPackage ./nix/brain-catalogue.nix {
        inherit brainModules glyphdb;
        notificationsFile = ./assets/notifications.nix;
        sharedFile = ./assets/shared/notifications.json;
        keyNames = texts.allNames;
      };

      # THE OFFLINE ENGLISH DICTIONARY: the Open English WordNet as two sorted
      # TSV files `ronoctua_words` reads by binary search. Read at run time from
      # `RONOCTUA_WORDS`, which `nix/modules/words.nix` sets on the full profile.
      wordsData = pkgs.callPackage ./nix/wordsgen.nix { };
      # THE ICON TABLES IN ONE DIRECTORY, for `ronoctua-iconset` — read at RUN
      # time (`RONOCTUA_ICONSET`, `atRunTime` in `nix/rust.nix`), the glyph
      # index naming the brain modules' glyph tokens too (`brainGlyphs`, the
      # catalogue's measured glyphs). See `nix/iconset.nix`.
      iconset = pkgs.callPackage ./nix/iconset.nix {
        glyphdb = glyphdb.db;
        lucidedb = lucidedb.db;
        unicodedb = unicodedb.db;
        brainGlyphs = brainCatalogue.measured;
      };
      # THE CALCULATOR'S UNITS, generated from `assets/units.nix`: the Rust
      # table `ronoctua-calc` includes, and the refusal of a word that names two
      # units or a unit a language has no words for. See `nix/unitsgen.nix`.
      unitsTable = pkgs.callPackage ./nix/unitsgen.nix {
        unitsFile = ./assets/units.nix;
        inherit languages;
      };
      unitsRs = unitsTable.table;

      # WHAT THE BELONGINGS KEEP, generated from `assets/belongings.nix`: the
      # Rust table `ronoctua-belongings` includes, and the fixture that proves
      # its generator refuses a broken table. See `nix/belongings.nix`.
      belongingsTable = pkgs.callPackage ./nix/belongings.nix {
        belongingsFile = ./assets/belongings.nix;
      };
      belongingsRs = belongingsTable.table;

      # ── anchor: L01 ──
      # ── anchor: L03 ──
      # THE MACHINE'S OWN FLAKE: `lib.machine` and `lib.machineFlake`, given
      # this distro — its revision, its inputs and its modules. A machine's
      # `flake.nix` reaches them as `ronoctua.lib`; see `nix/machine.nix`.
      machineLib = (import ./nix/machine.nix).lib {
        inherit
          self
          inputs
          distroModules
          system
          ;
      };

      # THE REFERENCE MACHINE: what `noctua` and `minimal` are built from.
      #
      # Through `lib.machine` and nothing else, so the reference is built the
      # way every installed machine builds itself; over `tests/fixtures/machine/`,
      # whose `host.nix` is exactly what the installer writes
      # (`host-nix-golden-evaluates`) and whose hardware report is a synthetic
      # one — nothing of anybody's machine.
      #
      # A `host.nix` at this tree's ROOT is the old installer's layout — a copy
      # of the distro with the machine's file beside it — and it is REFUSED,
      # not ignored: built as before, that machine would be this reference one,
      # its disk included.
      referenceMachine =
        {
          extraModules ? [ ],
        }:
        if builtins.pathExists ./host.nix then
          throw (
            "flake.nix: there is a host.nix at the distro's root. `nixosConfigurations.noctua` is "
            + "the distro's REFERENCE machine now, built from tests/fixtures/machine; an installed "
            + "machine has a flake of its own (`nix flake init -t <distro>#machine`) beside its host.nix."
          )
        else
          machineLib.machine {
            dir = ./tests/fixtures/machine;
            inherit extraModules;
          };
      # ── anchor: L04 ──
      # ── anchor: L11 ──
      # THE PERSON'S BRAIN MODULES (`my-…`), compiled into their machine: the
      # daemon's registry and the data the footer reads — the brain's
      # catalogue, the texts and the glyph browser's index — built again with
      # the person's folder beside the distro's, and the workspace given them;
      # every crate that draws or speaks stays the distro's. A function of
      # that folder (`dir`, the machine directory's `person/brain/`) and of
      # the folders to read (`only`, the included ones), for `lib.machine`,
      # whose modules are `(withPersonModules { dir; only; }).machineModules
      # <approved ids>` and whose report lists `.refused`. See
      # `nix/person-brain.nix`.
      withPersonModules =
        {
          dir,
          only ? null,
        }:
        import ./nix/person-brain.nix {
          inherit
            lib
            languages
            dir
            only
            distroModulesFor
            texts
            brainModulesRs
            brainManifestsRs
            brainCatalogue
            iconset
            rust
            ;
        };
      # ── anchor: L15 ──
      # ── anchor: L16 ──
      # THE DISTRO'S OWN OUTPUTS A PERSON'S PUBLIC BINARY CACHE MAY RECEIVE:
      # the store paths of the ISO's seed (the compositor, the shell, the
      # fonts, the icons, the index), AS THE DISTRO EVALUATES THEM — never a
      # machine's, which may carry the person's brain modules — and without
      # their string context, so a machine's system does not come to
      # reference (and build) a seed it does not use
      # (`nix/modules/binary-cache.nix`).
      seedRoots = map (d: builtins.unsafeDiscardStringContext d.outPath) (
        builtins.filter lib.isDerivation self.nixosConfigurations.installer._module.args.ronoctua.seed
      );
      # THE RELEASE and the three programs around it (`nix/release.nix`):
      # what `apps` offers as `release`, `fetch-iso`, `adopt-machine` and
      # `apply-here`, and what `release-refuses-dirty-tree` builds again with
      # stand-ins for the programs it drives. The reference machine is read
      # only by the two that change a machine — its wrapped `ronoctua-machine`
      # and its record's folder.
      releaseApps = pkgs.callPackage ./nix/release.nix {
        gate = "${gateApp}/bin/ronoctua-gate";
        machineDir = (import ./nix/machine.nix).machineDir;
        machine = self.nixosConfigurations.noctua.config.ronoctua.machine;
      };

      rust = pkgs.callPackage ./nix/rust.nix {
        inherit (inputs) crane;
        inherit
          pkgs
          rustToolchain
          fonts
          tokensRs
          # THE PALETTES, read at run time (`palettes.bin`, `RONOCTUA_THEMES`)
          # by every binary that paints — the wrappers carry them.
          themed
          # THE TEXTS, read at run time (`RONOCTUA_TEXTS`) by every binary
          # that speaks; only their key NAMES (`texts.keys`) are compiled.
          texts
          mimeDb
          notificationsRs
          bluetoothProfilesRs
          audioNamesRs
          syncthingOptionsRs
          torOptionsRs
          atuinOptionsRs
          keyboardsRs
          cacheRs
          offeredRs
          cameraEffectsRs
          segmentModel
          handModels
          syntaxRs
          compositorDeeds
          ;
        inherit (pkgs) xkeyboard_config;
        bufferMemory = buffers.memory;
        bufferKeyboard = buffers.keyboard;
        bufferTransfers = buffers.transfers;
        bufferFormat = buffers.format;
        bufferBrain = buffers.brain;
        # The reference for several keyboards in one app (`buffers/example`).
        bufferExample = buffers.example;
        inherit iconSuperpack ghosttyVt torrentShim;
        # The THIRD face embedded in the renderer, generated along with the database: without
        # it the 1,668 icons would come out as tofu.
        lucideFontFile = "${lucidedb.db}/NoctuaLucide.ttf";
        # And the FOURTH, which is the only one with no upstream font behind
        # it: the marks this repository draws itself, because a UI icon is a
        # glyph (invariant 10) and there was no glyph to be had.
        marksFontFile = "${marksdb.db}/NoctuaMarks.ttf";
        inherit
          atlasData
          coverageFonts
          chainRs
          widthRs
          worldRs
          broadcastRs
          tonguesRs
          ;
        primaryFontFile = "${primaryFont}/NoctuaIosevkaNerdFontMono-Regular.ttf";

        inherit brainModulesRs brainManifestsRs;
        # WHAT THE FOOTER DRAWS OF THE BRAIN'S MODULES, read at run time
        # (`RONOCTUA_BRAIN_CATALOGUE`, keyed like the palettes and the texts).
        inherit brainCatalogue;

        inherit wordsData iconset unitsRs;

        inherit belongingsRs;

        src = ./.;
      };

      # The graphics setup for a non-NixOS host, shared by `dev` and
      # `bench`: two copies would diverge, and `bench` would start measuring a
      # path `dev` does not use.
      graphicsHost = pkgs.callPackage ./nix/graphics-host.nix { };

      # `nix run .#dev` — the nested compositor (section 7.12, speed 1).
      # Hot reload's watcher. A package of its own so it can be TESTED —
      # see the header of nix/reload.nix.
      reloadApp = pkgs.callPackage ./nix/reload.nix { };
      # A checkout's text key files for `cargo`, and its texts — the devshell's
      # hook and the loop's watcher. See nix/keys-checkout.nix.
      keysCheckout = pkgs.callPackage ./nix/keys-checkout.nix {
        inherit languages;
        # THE MODULES' TEXT NAMESPACES, for the texts made from the tree (each
        # module's own folder there), as the `texts` derivation hands them.
        textModules = map (f: { inherit (f) id prefixes; }) brainModules.i18nFragments;
      };

      # One footer per session, guaranteed. A package of its own for the same reason as
      # `reloadApp`: it was a function inside `dev.nix` and it did not work — see the
      # header of nix/footer-single.nix.
      singleFooter = pkgs.callPackage ./nix/footer-single.nix { };

      # Git hooks ready for when the repository exists (section 8). Invariant
      # 2 forbids creating it; the devshell links them if `.git` appears.
      gitHooks = pkgs.callPackage ./nix/git-hooks.nix { };

      # The snapshot for when the shortcuts freeze — see the header of
      # nix/diagnostics.nix.
      diagnostics = pkgs.callPackage ./nix/diagnostics.nix {
        inherit scroll;
      };

      # THE SOFTWARE CENTRE'S SEARCH INDEX, evaluated from the system's own
      # nixpkgs — so what the search shows is what `nix profile add nixpkgs#…`
      # installs. RUN-TIME data, handed to the modules and never to the
      # compilation: a nixpkgs bump must not rebuild a unit for it (the
      # `atRunTime` side of `nix/rust.nix`). See `nix/software-index.nix`.
      softwareIndex = import ./nix/software-index.nix {
        inherit pkgs;
        nixpkgs = inputs.nixpkgs;
      };
      # EVERY NAME THE SUPERPACK ANSWERS TO, for the web apps' icon search in
      # the distro's collection. See `nix/icon-names.nix`.
      iconNames = pkgs.callPackage ./nix/icon-names.nix { inherit iconSuperpack; };
      # THE SOFTWARE CENTRE'S PROGRAMS for the two places no NixOS module
      # reaches — the nested loop and the photograph stage — from the ONE table
      # the installed system's units read too (`nix/software-tools.nix`).
      # Every support is on here: the loop REHEARSES (`nix/dev.nix`), so a
      # tool's presence decides what the screen offers and nothing is run.
      softwareTools = import ./nix/software-tools.nix {
        inherit lib pkgs;
        nix = pkgs.nix;
        index = softwareIndex;
        nixpkgsRev = inputs.nixpkgs.rev;
        flatpak = pkgs.flatpak;
        appimageRun = pkgs.appimage-run;
        waydroid = pkgs.waydroid;
        chrome = true;
        icons = "${iconSuperpack}/share/icons/noctua";
        inherit iconNames;
      };

      # THE CACHE POLICY, generated from `assets/cache.nix` — four numbers the
      # brain, the sweep and the file manager all have to agree about, and one
      # relationship between two of them that the build refuses to let go
      # wrong. No generator script: four integers and a word do not need one.
      cacheRs = pkgs.callPackage ./nix/cache.nix {
        cacheFile = ./assets/cache.nix;
      };

      # THE PROGRAMS THE INSTALLER OFFERS TO LEAVE OUT, generated from the
      # ledger's `offered` (`assets/distro-apps.nix`): the installer's table,
      # and the refusal of a row the screen could not draw or a decline could
      # not name (see `nix/offered.nix`).
      offeredRs = pkgs.callPackage ./nix/offered.nix {
        ledgerFile = ./assets/distro-apps.nix;
      };

      # THE CAMERA'S EFFECTS, generated from `assets/camera-effects.nix`: the
      # Rust table and the text keys, and the refusal of a declaration no key
      # could turn (see `nix/camera-effects.nix`).
      cameraEffectsRs = pkgs.callPackage ./nix/camera-effects.nix {
        effectsFile = ./assets/camera-effects.nix;
      };

      # THE SEGMENTATION MODEL, pinned by version and hash.
      segmentModel = pkgs.callPackage ./nix/segment-model.nix { };

      # THE HAND MODELS the gestures run, pinned by version and hash.
      handModels = pkgs.callPackage ./nix/hand-model.nix { };

      # THE SYNTAX TABLE, generated from `assets/syntax.nix` — the same shape
      # again, and it takes `mimeDb` because a language claims MIME TYPES and
      # the generator refuses one the database does not have. That list comes
      # out of the derivation that already read freedesktop's files, so the
      # pinned version stays in one place.
      syntaxTable = pkgs.callPackage ./nix/syntax.nix {
        inherit mimeDb;
        syntaxFile = ./assets/syntax.nix;
      };
      syntaxRs = syntaxTable.table;

      # The compositor. See `nix/scroll.nix` for why it is an override of
      # sway's derivation and where wlroots is.
      scroll = pkgs.callPackage ./nix/scroll.nix { scrollSrc = inputs.scroll-src; };

      # THE STAGE'S DRIVER, and the app that hands it a scene. Two stages:
      # the default one without the player's decoders' table, and the one a
      # scene asks for with `--player-tools` (`nix/shot.nix`, `playerTools`).
      shotStage =
        withPlayerTools:
        (pkgs.callPackage ./nix/shot.nix {
          inherit
            scroll
            fonts
            themed
            defaultTheme
            withPlayerTools
            ;
          ronoctua-shell = rust.shell;
          ronoctua-brain = rust.brain;
          ronoctua-transfers = rust.transfers;
          ronoctua-player = rust.player;
          # The software CLI the stage's footer is handed: a program of its
          # own, not a neighbour in the shell's bundle.
          ronoctua-software = rust.software;
          inherit softwareTools;
        }).driver;
      shotDriver = shotStage false;
      shotToolsDriver = shotStage true;
      # The stage for OTHER PEOPLE'S apps (`--apps`): Flatpak, the portal and
      # a route out, and no footer — see the header of `nix/shot-apps.nix`.
      shotAppsDriver = (pkgs.callPackage ./nix/shot-apps.nix { inherit scroll; }).driver;
      shotApp = pkgs.writeShellApplication {
        name = "ronoctua-shot";
        runtimeInputs = [ pkgs.coreutils ];
        text = ''
          driver=${shotDriver}
          if [ "''${1:-}" = "--apps" ]; then
            driver=${shotAppsDriver}
            shift
          elif [ "''${1:-}" = "--player-tools" ]; then
            # BUILT ONLY WHEN ASKED: the app carries the stage's derivation,
            # not its output, so the decoders' table (+198 MiB) is paid by the
            # scene that needs it and by no other run.
            driver=$(nix-store --realise ${shotToolsDriver.drvPath})
            shift
          fi
          if [ "$#" -ne 1 ] || [ ! -f "$1" ]; then
            echo "usage: nix run .#shot -- [--apps | --player-tools] <scene.py>" >&2
            exit 2
          fi
          scene=$(realpath "$1")
          cache="''${XDG_CACHE_HOME:-$HOME/.cache}/ronoctua-distro"
          out="$cache/shot"
          # EMPTIED EVERY RUN: a picture of what was built before is not kept.
          rm -rf "$out"
          mkdir -p "$out"
          # THE MACHINE'S DISK GOES ON DISK. The driver keeps it under
          # `XDG_RUNTIME_DIR`, which is a tmpfs: sparse or not, every byte a
          # Flatpak runtime writes into it would be RAM. Removed when done.
          state="$cache/shot-state"
          rm -rf "$state"
          mkdir -p "$state"
          chmod 700 "$state"
          trap 'rm -rf "$state"' EXIT
          echo "shot: $scene -> $out"
          XDG_RUNTIME_DIR="$state" TMPDIR="$state" RONOCTUA_SHOT_SCENE="$scene" \
            "$driver/bin/nixos-test-driver" -o "$out"
          ls -1 "$out"
        '';
      };

      devApp = pkgs.callPackage ./nix/dev.nix {
        inherit
          atlasData
          keepSources
          cursors
          graphicsHost
          reloadApp
          scroll
          singleFooter
          rustToolchain
          cargoDirs
          keysCheckout
          ;
        # What the player runs; the loop's player is started from beside the
        # footer and has no unit to take them from.
        # The Matroska tools too: the loop writes a film's tags like a machine
        # with the media profile does (it pays mkvtoolnix's closure, which a
        # development shell can afford).
        playerTools = import ./nix/player-tools.nix {
          inherit pkgs;
          matroska = true;
        };
        # And the software centre's, for the same reason.
        inherit softwareTools;
        # `rust.loopShell` AND NOT `rust.shell`: the nested loop is where one
        # LOOKS at the interface, never where one measures it, and it is the
        # most-run command in the project. Measured on this machine, the same
        # edited line is 148 s at the installed profile and 19 s at this one.
        # See `[profile.loop]` in the root `Cargo.toml`.
        #
        # `benchFrames` right below keeps `rust.shell`, deliberately: that one
        # exists to measure, and measuring a build nobody installs would answer
        # about the wrong binary.
        ronoctua-shell = rust.loopShell;
        # The loop's own software CLI (`RONOCTUA_SOFTWARE`): a program of its
        # own at the loop's profile, never a neighbour in the shell's bundle.
        ronoctua-software = rust.loop.bins.ronoctua-software;
        # THE WHOLE TABLE THE COMPILATION GETS, and not six values copied out of
        # it: the loop compiles the binary itself, so what it does not export the
        # binary does not have. See `nix/dev.nix`'s `bakedExports`.
        inherit (rust) generated;
        # And the offline dictionary, which the installed session gets from
        # `nix/modules/words.nix` and never from `atRunTime` (a wrapper's
        # `--set-default` would keep its 17 MB in every profile's closure):
        # the loop pays it, as it pays mkvtoolnix's.
        atRunTime = rust.atRunTime // {
          RONOCTUA_WORDS = "${wordsData}";
        };
        inherit fonts fontconfigConf iconSuperpack;
        inherit themed themes defaultTheme;
      };

      # Section 5.2's FRAME targets. Outside `nix flake check` on purpose:
      # section 9.3 forbids measuring them under lavapipe, and a check has no GPU.
      benchFrames = pkgs.callPackage ./nix/bench-frames.nix {
        inherit scroll;
        ronoctua-shell = rust.shell;
        inherit graphicsHost;
      };

      # `.#profile-bench`: the release profile's variants and their benches
      # (`nix/profile-bench.nix`). Named only by the app at run time.
      profileBench = import ./nix/profile-bench.nix {
        inherit lib pkgs system;
        inherit (rust) profileVariants;
        inherit scroll graphicsHost;
        themes = rust.keyed.RONOCTUA_THEMES;
        component = "${buffers.brain}/lib/ronoctua_buffer_brain.wasm";
      };

      # ---------------------------------------------------------------
      # Helpers
      # ---------------------------------------------------------------
      # App script: a derivation with a single binary.
      # `meta` is not decorative — without it `nix flake check` warns on every app.
      mkScriptApp =
        name: description: text:
        let
          drv = pkgs.writeShellApplication {
            inherit name text;
            runtimeInputs = with pkgs; [
              gnutar
              zstd
              coreutils
              gawk
            ];
          };
        in
        {
          type = "app";
          program = "${drv}/bin/${name}";
          meta = {
            inherit description;
            platforms = [ system ];
          };
        };

      # ---------------------------------------------------------------
      # The distro's NixOS modules
      # ---------------------------------------------------------------
      # THE SOURCES NO CACHE HAS, and the downloader that resumes.
      #
      # One entry, and it is derived: name, URL and hash come from the package's
      # own `src`, so they cannot disagree with what the build wants. What a
      # person maintains is the attribute name. See `nix/prefetch.nix` for the
      # measurements that made this necessary.
      #
      # The assertion is the guard: this workaround exists BECAUSE the source
      # has a single mirror. If upstream ever adds one, `fetchurl` can fall over
      # to it on its own and the entry should go — and the assertion says so
      # instead of leaving a loop nobody questions.
      unfreePkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };
      singleMirrorSources =
        let
          from =
            src:
            assert lib.assertMsg (builtins.length src.urls == 1)
              "${src.name}: now has ${toString (builtins.length src.urls)} mirrors — nix/prefetch.nix is no longer needed for it";
            {
              inherit (src) name outputHash;
              url = builtins.head src.urls;
              # THE PATH GOES IN WITHOUT ITS CONTEXT, and that is the whole
              # point. `src.outPath` carries a dependency on the very
              # fixed-output derivation this exists to fetch: with the context
              # kept, building the downloader FETCHES the file first, and the
              # tool that works around the failure cannot be built until the
              # failure stops happening. Discarding the context leaves the
              # string, which is all the script wants — something to hand to
              # `nix-store --query --valid`.
              path = builtins.unsafeDiscardStringContext src.outPath;
            };
        in
        map from [
          # 1.37 GB, unfree — so Hydra never builds it and no cache has it.
          unfreePkgs.android-studio.unwrapped.src
        ];
      prefetchApp = pkgs.callPackage ./nix/prefetch.nix {
        sources = map (s: {
          inherit (s) name url path;
          hash = s.outputHash;
        }) singleMirrorSources;
      };

      # The gate that sizes itself to the machine. See `nix/gate.nix` for the
      # arithmetic and where its numbers were measured.
      gateApp = pkgs.callPackage ./nix/gate.nix {
        inherit system keepSources;
        prefetch = "${prefetchApp}/bin/ronoctua-prefetch";
        sizing = buildSizing;
        units = rust.unitIndex;
        targets = builtins.attrNames self.checks.${system};
        alone = slowNames;
      };

      # THE MIDDLE BATTERY, derived and not hand-written.
      #
      # `checks` minus `tests/slow.nix`. Derived, because a hand-written list is
      # what `.#quick` already is, and it aged exactly as one would expect: nine
      # checks were born in a single day and none of them made it in.
      #
      # `test-tiers-are-total` demands that every name in `slow.nix` exist in
      # `checks`; everything else lands here by construction, so a new check
      # joins on its own.
      # AND EVERY BRAIN MODULE'S OWN, from its `checks.nix` (`slow = [ … ]`).
      slowNames = import ./tests/slow.nix ++ brainModules.slowChecks;
      mostApp = pkgs.callPackage ./nix/gate.nix {
        inherit system keepSources;
        name = "ronoctua-most";
        what = "targets";
        sizing = buildSizing;
        units = rust.unitIndex;
        targets = lib.subtractLists slowNames (builtins.attrNames self.checks.${system});
        left = slowNames;
      };

      # THE DISTRO'S MODULES AS A FUNCTION OF THE WORKSPACE THEY RUN — the
      # distro's own `rust`, or a machine's built with the person's brain
      # modules (`withPersonModules`). Every `rust.*` below is the argument.
      distroModulesFor = rust: [
        ./nix/modules/base.nix
        # The table that turns `ronoctua.profile` into the eight switches. It
        # comes right after `base.nix` because that is where the option it reads
        # is declared, and before every module whose default it sets.
        ./nix/modules/profile.nix
        # The cgroup, oomd and zram policy. It comes early because what it
        # protects — the compositor and the footer — is declared later.
        ./nix/modules/memory.nix
        # The store looks after itself: Nix's free-space floor, the system's
        # generations by count, and the build logs' age. Beside the memory
        # policy because it is the disk's half of the same promise — the
        # machine keeps itself running with nobody doing anything.
        ./nix/modules/store.nix
        ./nix/modules/rescue.nix
        # THE BRAIN, AND WITH IT EVERY MODULE'S NIXOS HALF — the update's
        # sequence included, whose rebuild step BORROWS the rescue console's
        # `ronoctua-rescue-rebuild` rather than declaring a second `nixos-rebuild
        # switch --flake /etc/nixos#<person>`; an `assertions` entry says so if
        # the rescue console is ever turned off.
        ./nix/modules/brain.nix
        ./nix/modules/cache.nix
        # The file manager's privileged half: one unit, one polkit rule, and
        # where it opens. Everything else about it needs no configuration.
        ./nix/modules/files.nix
        # The other half of the manager's sidebar: udisks2, and no polkit rule
        # of our own. After `files.nix` because it is that view's second half
        # and nothing else in the system reads it.
        ./nix/modules/disks.nix
        # Power — UPower, power-profiles-daemon, logind; the modeline's
        # battery and profile marks, and `LEADER s p`.
        ./nix/modules/power.nix
        # The screens. After `power.nix` because it is the same shape — one
        # udev rule opening one sysfs attribute to one group, and no polkit
        # rule at all — and because a person reading the list finds the two
        # pieces of hardware with a brightness next to each other.
        ./nix/modules/display.nix
        ./nix/modules/bluetooth.nix
        # The queue of everything that takes a while, and the unit that owns
        # it. After `files.nix` because what it runs is that crate's work.
        ./nix/modules/transfers.nix
        ./nix/modules/aria2.nix
        # The camera studio: the unit that owns every camera, and the kernel's
        # virtual camera it writes to.
        ./nix/modules/studio.nix
        # The player: the unit that plays, and the decoders it runs.
        ./nix/modules/player.nix
        ./nix/modules/browser-extension.nix
        ./nix/modules/fonts.nix
        ./nix/modules/scroll.nix
        ./nix/modules/keyboard.nix
        # Independent keyboards: keyd's replacement when every keyboard is its
        # own, beside the module that sets keyd up.
        ./nix/modules/keys.nix
        ./nix/modules/icons.nix
        ./nix/modules/foreign.nix
        ./nix/modules/terminal.nix
        ./nix/modules/network.nix
        # The two tunnels beside the network they ride on: Tailscale's socket
        # and Tor's control socket, each with its own view section and its own
        # modeline mark. Tor carries the one new polkit rule in the system.
        ./nix/modules/tailscale.nix
        ./nix/modules/tor.nix
        ./nix/modules/syncthing.nix
        # THE TWO SYNCS THE OWNER ASKED FOR. Both install a CLIENT that costs
        # nothing until it is told where a server is, and both offer a SERVER
        # that is off by default — "a maioria dos computadores rodarão como
        # cliente, mas haverão alguns poucos que também funcionarão como
        # servidor".
        ./nix/modules/atuin.nix
        ./nix/modules/clipcascade.nix
        ./nix/modules/secrets.nix
        ./nix/modules/persist.nix
        # Beside the persistence it shares a disk with: `~/.cache` as a
        # subvolume of its own, the kernel's reclaim, the scrub, and no
        # snapshot of the store or of a cache. Everything btrfs-only is gated
        # on btrfs being there, so the VM and every other machine evaluate.
        ./nix/modules/filesystem.nix
        ./nix/modules/home.nix
        ./nix/modules/cli.nix
        ./nix/modules/archives.nix
        ./nix/modules/media.nix
        ./nix/modules/dev.nix
        ./nix/modules/nixpkgs-config.nix
        ./nix/modules/browser.nix
        ./nix/modules/emacs.nix
        # VS Code, its telemetry off by policy, and the unit that records the
        # person's extensions into the list their belongings carry — a
        # function of the EXTRACTED `ronoctua-vscode`. On in the full profile.
        (import ./nix/modules/vscode.nix { vscodeTool = rust.vscode; })
        # THE PERSON'S OWN PROGRAMS (invariant 6's dated exception): the CLI's
        # wrapper and the units' tools, then the three supports it drives —
        # Flatpak (with nix-flatpak, ready and declaring nothing but Flathub),
        # AppImage and Android. After the browser, whose switch decides whether
        # a web app has a Chrome to open in; the persistence of the two system
        # directories is in `persist.nix`, which holds the whole list.
        ./nix/modules/software.nix
        ./nix/modules/flatpak.nix
        ./nix/modules/appimage.nix
        ./nix/modules/android.nix

        # The offline English dictionary, on the profile that carries it.
        ./nix/modules/words.nix

        # The door lock the player's adult catalogue asks: the PAM service
        # `ronoctua-unlock`, password only, with a delay after a failure.
        ./nix/modules/unlock.nix

        # The super index of fast-access: its unit, its limits, the watches
        # it needs. A function of the EXTRACTED binary, so the unit's closure
        # is the indexer and never the desktop.
        (import ./nix/modules/index.nix { index = rust.index; })

        # The video catalogue's scanner: the unit the brain starts every few hours
        # and its command line. A function of the EXTRACTED binary, so the unit's
        # closure is the scanner and never the desktop.
        (import ./nix/modules/video-metrics.nix { videoMetrics = rust.videoMetrics; })

        # The security centre's foundation: nftables and the person's table,
        # the sudo window's include, the shields' one polkit rule, fprintd
        # behind two PAM gates, fwupd read only. The root helper comes from
        # the workspace EXTRACTED, next to the import: a unit that runs as
        # root must not carry the desktop, and the `ronoctua` argument set
        # below is not where a lane adds a line.
        ./nix/modules/security.nix
        { ronoctua.security.helper = rust.securityRoot; }
        # AND THE QUEUE'S CHILD THAT SENDS A FILE TO VIRUSTOTAL (`Work::Scan`),
        # by store path: the queue's unit has a session `PATH`, and a bare
        # name would be found or not by accident.
        (
          { config, lib, ... }:
          {
            systemd.user.services.ronoctua-transfers.environment = lib.mkIf config.ronoctua.transfers.enable {
              RONOCTUA_SCAN = "${rust.scan}/bin/ronoctua-scan";
            };
          }
        )

        # sys-boundaries: the person's containers — Podman, rootless and
        # compatible with Docker (`docker`, `DOCKER_HOST`, `podman-compose`).
        # After `filesystem.nix`, whose `ownSubvolumes` it adds
        # `~/.local/share/containers` to. On in the full profile.
        ./nix/modules/containers.nix

        # The belongings (`LEADER s v`): the queue runs their CLI, extracted
        # so `age` stays out of the queue and the footer; the module hands
        # its path to the queue's unit.
        ./nix/modules/belongings.nix
        { ronoctua.belongings.cli = rust.belongings; }

        # Blender and OrcaSlicer, and OrcaSlicer as the default for the three
        # model types in `/etc/xdg/mimeapps.list`. On in the full profile.
        ./nix/modules/workshop.nix
        # ONLYOFFICE and LibreOffice, each the default for its own family of
        # documents (Microsoft's, the open ones). On in the full profile.
        ./nix/modules/office.nix
        # The Crafting Apps' switch (the apps are AppImages of the table in
        # `assets/distro-apps.nix`). On in the full profile.
        ./nix/modules/crafts.nix

        # THE 2026-10-05 WAVE: the machine's own flake, the person's
        # declarations and the distro's program registry — options only until
        # their packages land (they change nothing yet). One region per
        # package, after its anchor.
        ./nix/modules/machine.nix
        ./nix/modules/person.nix
        ./nix/modules/distro-apps.nix
        # ── anchor: L01 ──
        # ── anchor: L02 ──
        # SSH: the key exchange post-quantum first, an sshd with ed25519
        # alone — the table is `assets/ssh.nix`, shared with the ISO.
        ./nix/modules/ssh.nix
        # ── anchor: L03 ──
        # ── anchor: L04 ──
        # ── anchor: L05 ──
        # The machine's life — the boot trial, the apply the rescue and the
        # software centre start, the migration of the old layout — runs the
        # EXTRACTED `ronoctua-machine`, set next to the module that wraps it.
        {
          ronoctua.machine = {
            tool = rust.machine;
            # HOW ITS SYSTEM IS BUILT: the release graph's units, each by its
            # class, and the release units kept after a build that worked.
            units = lib.filterAttrs (_: e: e.flavour == "release") rust.unitIndex;
            keptUnits = rust.release.keep.paths;
          };
        }
        # ── anchor: L06 ──
        # The person's declarations, copied into the machine directory by a
        # narrow root unit, and a brain module's root part approved through
        # the sudo door. Its program comes EXTRACTED, next to the import: a
        # unit that runs as root must not carry the desktop.
        ./nix/modules/person-apply.nix
        { ronoctua.person.tool = rust.person; }
        # ── anchor: L10 ──
        # Security keys: smart cards, GnuPG's agent, and pam_u2f for sudo —
        # always `sufficient` — trusting root's list of the person's keys.
        ./nix/modules/yubikey.nix
        # ── anchor: L11 ──
        # ── anchor: L12 ──
        # /private, the person's encrypted folder, and the root side its
        # four units run (`nix/rust.nix`, extracted).
        ./nix/modules/private.nix
        { ronoctua.private.helper = rust.privateRoot; }
        # ── anchor: L13 ──
        # INCUS, containers and virtual machines of the person's (full
        # profile), and the queue's child for its long work: extracted, so
        # the queue carries only the job's wire.
        ./nix/modules/incus.nix
        { ronoctua.incus.job = rust.incusJob; }
        # ── anchor: L14 ──
        # The LLM accounts' CLI on the system's PATH: Claude Code runs an
        # account's status line from `/run/current-system/sw/bin`, and the
        # usage unit runs its `refresh`. Extracted (`rust.llm`), like the
        # belongings' CLI.
        ./nix/modules/llm.nix
        { ronoctua.llm.cli = rust.llm; }
        # ── anchor: L15 ──
        # ── anchor: L16 ──
        # THE PERSON'S BINARY CACHE (their `options.json`, root's validated
        # copy): a substituter before `cache.nixos.org` and its key, root's
        # netrc for a private one, and what the update's push reads — the
        # push's program (EXTRACTED, `rust.cachix`) and the distro's own
        # outputs a public cache may receive.
        ./nix/modules/binary-cache.nix
        {
          ronoctua.binaryCache = {
            tool = rust.cachix;
            publicRoots = seedRoots;
          };
        }
        # ── anchor: L17 ──
        # The installer stick's root writer: its unit (a function of the
        # EXTRACTED binary, so the unit that writes a whole drive as root
        # carries the writer and never the desktop) and its staging folder.
        (import ./nix/modules/stick.nix { stickWrite = rust.stickWrite; })
        # ── anchor: L19 ──
        # ── anchor: L22 ──
        # Encrypt anything: the queue runs `ronoctua-seal`, EXTRACTED so that
        # `age` stays out of the queue and the footer; the module hands its
        # path to the queue's unit.
        ./nix/modules/seal.nix
        { ronoctua.seal.cli = rust.seal; }
        # ── anchor: L24 ──
        # THE LLM ACCOUNTS' VIEW: the agents' programs handed to the footer by
        # store path, each following its program in the registry (the usage
        # unit is the LLM accounts' brain module's own).
        ./nix/modules/agents.nix

        inputs.home-manager.nixosModules.home-manager
        {
          # Injects the packages built by this flake into the modules.
          _module.args = {
            ronoctua = {
              inherit fonts fontconfigConf iconSuperpack;
              # THE WORLD, for the player's directories: the daemon places a
              # radio or TV page on the globe from the atlas's anchors, and a
              # door only draws it (`nix/modules/player.nix`).
              atlas = atlasData;
              # THE COMPOSITOR ITSELF, for the one line that needs the binary:
              # the unit's `ExecStart`. The config FILE it is given never names
              # it, which is exactly why this was missing and nothing said so —
              # building `/etc/scroll/config` forces the text and not the
              # program, so the gap only showed when a check evaluated a whole
              # system. `the-session-reaches-its-target` now forces the unit.
              inherit scroll;
              # THE THEMES: the list (the option's enum) and the solved
              # palettes (files the modules splice or point at; never read at
              # evaluation).
              inherit themes themed defaultTheme;
              # THE CURSORS, one Xcursor theme per cursor colour: the
              # compositor module puts them where the compositor looks.
              inherit cursors;
              # The shell's binary: the compositor module generates the chord
              # binds from its command table.
              #
              # `rust.shell`, the bundle under the wrapper: it is the one that
              # knows where the five components are, and the unit's `ExecStart`
              # comes from here. See `nix/rust.nix`.
              shell = rust.shell;
              # The rescue console on its VT, and NetworkManager's secret agent
              # and the boot's Wi-Fi console: programs of their own, never
              # `${shell}/bin/…` — that put the footer's closure into three
              # units that draw no window (`shell-bundle-is-only-the-shell`).
              rescue = rust.rescue;
              networkConsole = rust.networkConsole;
              networkAgent = rust.networkAgent;
              # The file manager's privileged helper, EXTRACTED from the
              # workspace. The unit that runs it runs as root, and handing it
              # the whole workspace would put wgpu and the wasm runtime there.
              filesRoot = rust.filesRoot;
              # The queue's daemon, EXTRACTED for the same reason: it is a unit
              # that outlives the session, and its closure should be the copy
              # and not the desktop.
              transfers = rust.transfers;
              # The keys daemon, EXTRACTED for the same reason: it runs as root
              # in keyd's place when the keyboards are independent.
              keys = rust.keys;
              # The brain's daemon, EXTRACTED for the same reason as the queue's
              # — and one more: it decides WHEN the machine rebuilds itself, and
              # a unit with no screen must not carry the renderer that would
              # come with the whole workspace. `brain-unit-is-narrow` measures
              # that rather than asserting it.
              brain = rust.brain;
              # The studio's daemon, EXTRACTED for the same reason: it owns the
              # camera and draws nothing.
              studio = rust.studio;
              # And the player's, for the same reason: libmpv, never the renderer.
              player = rust.player;
              # The software CLI, EXTRACTED for the same reason and one sharper:
              # the update runs it as root for the Android image, and its job is
              # a package manager's, never a desktop's. `nix/modules/software.nix`
              # wraps it with this machine's tools.
              software = rust.software;
              # Its search index and the superpack's names — run-time DATA,
              # passed like the models above and never compiled in.
              inherit softwareIndex iconNames;
              # The segmentation model its background effects run.
              inherit segmentModel;
              # And the hands', which the gestures read.
              inherit handModels;
              # The cache sweep, extracted for the brain's reason and one of
              # its own: a program whose job is `unlink` must not have the
              # whole shell in its closure.
              cacheSweep = rust.cacheSweep;
              # And the disk's relief, extracted for the same reason: it
              # deletes cargo builds and asks the daemon for a collection.
              diskRelief = rust.diskRelief;
              # Where a local `cargo` writes: the session names the build
              # directory from the same file the devshell does.
              inherit cargoDirs;
              # And the one that empties the trash, extracted for the same
              # reason and one sharper: what it deletes is the only copy.
              emptyTrash = rust.emptyTrash;
              # The kiosk's native-messaging host, EXTRACTED for the queue's
              # reason: Chrome starts it beside the person's browser, and its
              # closure is a JSON relay, never the desktop.
              browser = rust.browser;
              # The browser extension, already generated: `nix/modules/browser-extension.nix`
              # packs and signs it on the machine, because the id comes from a
              # key that may not live in this repository.
              extension = extension;
              # The PO token minter. Referenced only when
              # `ronoctua.transfers.poToken` asks for it, so a machine that
              # never downloads a video does not carry Deno.
              potProvider = potProvider;
            };
          };
        }
      ];
      distroModules = distroModulesFor rust;
    in
    {
      # =================================================================
      # packages
      # =================================================================
      packages.${system} = {
        default = self.packages.${system}.iso;

        # Fonts — five derivations. See docs/ARCHITECTURE.md, section "Fontes".
        inherit (fonts)
          noctua-iosevka
          noctua-iosevka-core
          noctua-iosevka-ttf
          noctua-iosevka-ttf-nerdfonts
          noctua-iosevka-woff2
          noctua-iosevka-woff2-nerdfonts
          noctua-console-psf
          ;

        fontconfig-conf = fontconfigConf.confDir;
        # The COMPLETE file, for `FONTCONFIG_FILE` — see nix/fontconfig.nix.
        fontconfig-full = fontconfigConf.fullConf;
        fallback-chain = fontconfigConf.chainJson;
        # The faces that chain names, one file per family — see
        # nix/coverage-fonts.nix. Buildable on its own so the manifest can be
        # read without a shell in front of it.
        coverage-fonts = coverageFonts;
        # The same chain as Rust, so it can be read without a compiler.
        chain-rs = chainRs;
        width-rs = widthRs;
        world-rs = worldRs;
        broadcast-rs = broadcastRs;
        tongues-rs = tonguesRs;

        # Generated glyph layer: icons.json, glyphs.json, coverage.md.
        glyphdb = glyphdb.db;

        # THE TERMINAL ENGINE, built from a pinned Ghostty commit.
        #
        # Exposed by name so it is reachable and buildable, and NOT yet wired
        # into any crate: nothing links it, so nothing can pretend to use it. The
        # step that makes `nix flake check` build it is the FFI, which is where a
        # half-written engine would start returning an empty grid — see
        # `docs/PENDING.md`, item 1.
        ghostty-vt = ghosttyVt;

        # THE COMPOSITOR UNDER EVALUATION, exposed so it can be built and run on
        # its own — `nix run .#scroll -- --config <file>` — without bringing up
        # the footer. It is exposed so it can be built and run on its own —
        # `nix run .#scroll -- -c <file>` — without bringing up the footer.
        inherit scroll;

        # The shim, buildable and inspectable on its own: it carries the
        # `sizeof` table the layout check compares the Rust against.
        torrent-shim = torrentShim;
        pot-provider = potProvider;

        # The extension, unpacked and inspectable: `share/ronoctua-extension`
        # is what Chrome is pointed at, and what the packer signs.
        browser-extension = extension;

        # Merged icon theme, with manifest and coverage report.
        noctua-icon-theme = iconSuperpack;

        # Rust token table, generated from the .nix files.
        tokens-rs = tokensRs;
        # The solved palettes, their fragments and the gates' report.
        themes = themed;
        inherit cursors;
        mime-db = mimeDb;
        notifications-rs = notificationsRs;
        bluetooth-profiles-rs = bluetoothProfilesRs;
        audio-names-rs = audioNamesRs;
        syncthing-options-rs = syncthingOptionsRs;
        tor-options-rs = torOptionsRs;
        atuin-options-rs = atuinOptionsRs;
        keyboards-rs = keyboardsRs;
        compositor-deeds = compositorDeeds;
        # THE COMPONENTS, each buildable on its own.
        #
        # Three of them used to be missing here, and it showed the day the
        # nested loop learned to rebuild a buffer: `nix build .#buffer-memory`
        # did not exist, so the newest buffers were the ones the loop could not
        # reload. What names them now is `nix/buffers.nix`'s own table.
        buffer-example = buffers.example;
        buffer-memory = buffers.memory;
        buffer-keyboard = buffers.keyboard;
        buffer-transfers = buffers.transfers;
        buffer-format = buffers.format;
        buffer-brain = buffers.brain;
        # The texts every program reads at run time, buildable on their own:
        # a sentence edited rebuilds this and the wrappers, and no crate.
        inherit texts;

        # THE HIGHLIGHTER'S TABLE, and its negative fixture: `syntax-negative`
        # is a table in which one language steals another's type, another
        # claims prose and a third makes its comment's marker a word
        # character, and it must FAIL to build. (The update sequence's pair
        # travels with its brain module, whose checks build it.)
        syntax-rs = syntaxTable.table;
        syntax-negative = syntaxTable.negativeFixture;
        update-runners = self.nixosConfigurations.noctua.config.ronoctua.update.package;
        # THE SOFTWARE CENTRE'S DATA, each buildable on its own and neither
        # touching the Rust: the search index of the system's nixpkgs, and every
        # name the icon superpack answers to.
        software-nix-index = softwareIndex;
        icon-names = iconNames;

        # The shell: the two-line footer.
        ronoctua-shell = rust.shell;

        # The brain's modules as Rust: the registry the daemon compiles, and the
        # distro's manifests the library (and so the footer) compiles.
        brain-modules-rs = brainModulesRs;
        brain-manifests-rs = brainManifestsRs;
        # And what the footer reads of them at run time.
        brain-catalogue = brainCatalogue;

        # The offline English dictionary, buildable on its own: two sorted TSV
        # files out of the Open English WordNet (`nix/wordsgen.nix`).
        words-data = wordsData;
        # The calculator's unit table, and the fixture that proves its
        # generator refuses (`units-are-total` reads both).
        units-table = unitsRs;
        units-negative-fixture = unitsTable.negativeFixture;
        # The three icon tables as `RONOCTUA_ICONSET` names them.
        inherit iconset;

        # The super index's unit, extracted (`nix/rust.nix`).
        ronoctua-index = rust.index;

        # The video catalogue's scanner, extracted (`nix/rust.nix`).
        ronoctua-video-metrics = rust.videoMetrics;

        # The security centre's root helper, buildable on its own — what
        # `relax-line-parses` runs, and what the units run as root.
        ronoctua-security-root = rust.securityRoot;

        # The belongings' CLI, extracted (`nix/rust.nix`), and the table of
        # what they keep with its refusing fixture — what
        # `belongings-manifest-covers-the-stores` reads.
        ronoctua-belongings = rust.belongings;
        belongings-table = belongingsRs;
        belongings-table-refuses = belongingsTable.refuses;

        # ── anchor: L01 ──
        # The sealer the queue runs for `Work::Seal`, extracted
        # (`nix/rust.nix`): what `pq-age-interop` hands the Go `age`.
        ronoctua-seal = rust.seal;
        # ── anchor: L03 ──
        # ── anchor: L05 ──
        # The machine's life, buildable on its own — what its checks run, and
        # what the units run as root.
        ronoctua-machine = rust.machine;
        # ── anchor: L06 ──
        # Root's half of the person's data, buildable on its own — what the
        # person-apply checks run, and what its units run as root.
        ronoctua-person = rust.person;
        # ── anchor: L12 ──
        # The /private vault's root side, buildable on its own — what its
        # units run as root.
        ronoctua-private-root = rust.privateRoot;
        # ── anchor: L13 ──
        # The Incus job, extracted (`nix/rust.nix`).
        ronoctua-incus-job = rust.incusJob;
        # ── anchor: L14 ──
        # The LLM accounts' CLI, extracted — what `statusline-chains` runs.
        ronoctua-llm = rust.llm;
        # ── anchor: L16 ──
        # The push to the person's binary cache, extracted (`nix/rust.nix`).
        ronoctua-cachix = rust.cachix;
        # ── anchor: L17 ──
        # The installer stick's root writer, extracted (`nix/rust.nix`).
        ronoctua-stick-write = rust.stickWrite;
      }
      // {

        iso = self.nixosConfigurations.installer.config.system.build.isoImage;
      };

      # =================================================================
      # nixosConfigurations
      # =================================================================
      nixosConfigurations = {
        # ── anchor: L03 ──
        # THE REFERENCE PRODUCTION SYSTEM — the full profile — and what the
        # gate builds as "the machine somebody installs" (`m2`). It is no
        # longer the name the installer installs: a machine installs its own
        # flake, whose attribute is named after the person.
        noctua = referenceMachine { };

        # THE SEED — the reduced system, and it is not what gets installed.
        #
        # What this attribute is for is being BUILT, so its closure can travel
        # on the ISO: everything the reduced profile needs, already realised,
        # so the target machine compiles nothing. See `nix/installer/iso.nix`.
        # The same reference machine as `noctua`, so the store paths line up
        # with the ones an installed system asks for; the fixture's `host.nix`
        # says `full`, as the installer's default does, and this says the
        # other.
        minimal = referenceMachine {
          extraModules = [ { ronoctua.profile = lib.mkForce "minimal"; } ];
        };
        # ── anchor: L20 ──
        # Installer ISO — built by the flake itself (section 7.1).
        installer = nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = { inherit inputs; };
          modules = [
            ./nix/installer/iso.nix
            {
              _module.args.ronoctua = {
                inherit
                  fonts
                  fontconfigConf
                  themes
                  themed
                  defaultTheme
                  ;
                # `rust.installer`, the installer's own unit: a derivation
                # references what its bytes mention, and crane's workspace,
                # which held every binary, put Zig, clang, LLVM and the icon
                # superpack on an ISO with no desktop through `ronoctua-shell`.
                installerCrate = rust.installer;
                # THE PERSON'S TWO PROGRAMS the installer runs (`nix/installer/iso.nix`):
                # the belongings' CLI and root's applier of the person's
                # declarations, extracted.
                belongingsCli = rust.belongings;
                personCli = rust.person;

                # THE SEED — WHAT NO PUBLIC CACHE HAS, and nothing else.
                #
                # `nixos-install` does not download a system, it BUILDS one.
                # Most of a NixOS closure arrives prebuilt from
                # `cache.nixos.org`; what this project makes does not exist
                # there, and that is what the target machine was compiling every
                # time — the shell, the compositor, `libghostty-vt`
                # and its Zig toolchain, the five wasm buffers. They are the
                # compilations this host's own `earlyoom` kills for weight.
                #
                # The reduced SYSTEM's whole closure would have fitted here too,
                # and it was measured: the ISO came to 4380 MiB against a
                # ceiling of 4096. Stock nixpkgs is 3,5 GiB of that, and it is
                # the part a cache serves in minutes. So the ISO carries what
                # only this repository can produce, and the target downloads the
                # rest instead of compiling it.
                seed = [
                  # THE COMPOSITOR, said out loud.
                  #
                  # The line below used to claim it arrived "by baked store
                  # path" with the workspace. It does not, and it did not:
                  # nothing in the shell references the compositor's binary —
                  # what names it is the UNIT, which is `/etc` and not a
                  # closure the ISO carries. So a full install would have
                  # BUILT a compositor and its vendored wlroots on the target
                  # machine, which is the afternoon this whole list exists to
                  # prevent. `iso-carries-the-seed` is what said so, by name.
                  scroll
                ]
                # OUR PROGRAMS, `rust.shipped`: the shell's bundle and every
                # `ship` program in the form a machine runs it
                # (`nix/rust/outputs.nix`). No test driver and no benchmark:
                # crane's workspace brought all fifty programs along with the
                # shell — and, through the terminal differential (a driver),
                # `libghostty-vt` and its toolchain, which nothing a machine
                # runs links.
                #
                # The wrapper and NOT the bare program: the five wasm buffers
                # used to arrive by carriage, exactly as the superpack below
                # did, and cutting that reference is what stopped a buffer
                # edit from recompiling everything (`nix/rust.nix`). What
                # carries them now is the wrapper — and if it ever stops,
                # `iso-carries-the-seed` names the five one by one.
                ++ rust.shipped
                ++ [
                  # NOT in the shell's closure: the binary embeds only the
                  # primary face. This is the family the system installs, it
                  # comes from a GitHub Release rather than from a cache, and
                  # nothing else in the closure fails the way a release asset
                  # can.
                  fonts.noctua-iosevka-ttf-nerdfonts
                  # THE ICON SUPERPACK, and it has to be said out loud now.
                  #
                  # It used to arrive by carriage: `ronoctua-shell` carried its
                  # store path baked in, so the workspace's closure brought it.
                  # Cutting that reference is what made
                  # `ronoctua.icons.superpack` a switch that actually switches —
                  # and it also took the package off this ISO, which would have
                  # left a full installation BUILDING a 0,61 GiB merge of icon
                  # themes on the target machine. `iso-carries-the-seed` is what
                  # said so.
                  #
                  # It is here regardless of profile: the ISO installs either
                  # one, and the seed is "what no cache has", not "what the
                  # reduced profile uses".
                  iconSuperpack
                  # THE SOFTWARE CENTRE'S SEARCH INDEX, in both profiles. It is
                  # 18 MB of text, and making it is an EVALUATION of the whole
                  # of nixpkgs: 4,0 GiB of resident memory and 36 s, measured
                  # on the build host — on a target installing from a
                  # live medium, that is the difference between a finished
                  # installation and the OOM killer.
                  softwareIndex
                  # And the superpack's names, for the same reason as the
                  # superpack: nothing but this repository makes them.
                  iconNames
                ]
                # AND THE FLAKE'S OWN INPUTS, as sources.
                #
                # To build the machine's own top — `etc`, the initrd, the
                # hostname, the disk — Nix has to EVALUATE the flake, and
                # evaluation needs nixpkgs, disko and the compositor's source, which `flake.lock`
                # names but does not contain.
                ++ builtins.filter (p: p != null) (map (i: i.outPath or null) (builtins.attrValues inputs));
                # The distro's tree, which the ISO carries for the installer to
                # make a machine's flake from — see the comment over there.
                flake = self;
              };
            }
          ];
        };

        # Development VM — a complete system, without real hardware.
        vm = nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = { inherit inputs; };
          modules = distroModules ++ [ ./nix/modules/vm.nix ];
        };
      };

      # `default` IS GONE, and that is the point: the distro's modules read
      # `inputs` (`specialArgs`) and the module arguments the flake hands
      # them, so a bare `imports = distroModules` broke in any flake but this
      # one. A machine reaches the distro through `lib.machine`, which gives
      # them all three.
      nixosModules = {
        fonts = ./nix/modules/fonts.nix;
      };

      # ── anchor: L03 ──
      # THE MACHINE'S FLAKE. `lib.machineFlake { dir = ./.; }` is the whole of
      # a machine's `outputs` (`templates/machine/flake.nix`); `lib.machine`
      # is the system alone, for whoever wants to add modules to it.
      lib = {
        inherit (machineLib) machine machineFlake;
      };

      # `nix flake init -t github:ronoctua/ronoctua-distro#machine` — what the
      # installer writes a new machine's directory from.
      templates.machine = {
        path = ./templates/machine;
        description = "A machine's own flake: the ronoctua distro as an input, beside this machine's host.nix";
      };

      # =================================================================
      # checks — section 7.12: `nix flake check` is the single command.
      # Each milestone is a named check (m1..m8). Milestones not yet attacked
      # are stubs that pass, so the plumbing exists from day one.
      # =================================================================
      checks.${system} = import ./tests/checks.nix {
        inherit scroll;
        inherit
          languages
          themes
          themed
          defaultTheme
          cursors
          ;
        # THE TOKENS' NAMES and the two generators no package exposes, for
        # `tests/theme-data.nix`.
        inherit tokenNames primaryFont atlasData;
        inherit
          pkgs
          lib
          self
          inputs
          system
          fonts
          fontconfigConf
          glyphdb
          unicodedb
          lucidedb
          marksdb
          syntaxTable
          iconSuperpack
          rust
          buffers
          sbKeys
          devApp
          reloadApp
          gateApp
          prefetchApp
          singleFooter
          wit
          mimeDb
          tokensRs
          notificationsRs
          bluetoothProfilesRs
          audioNamesRs
          broadcastRs
          syncthingOptionsRs
          torOptionsRs
          atuinOptionsRs
          keyboardsRs
          compositorDeeds
          torrentShim
          extension
          potProvider
          rustToolchain

          texts
          brainModules
          brainModulesRs
          brainManifestsRs
          brainCatalogue

          # ── anchor: L01 ──
          # ── anchor: L02 ──
          # ── anchor: L03 ──
          machineLib
          # ── anchor: L04 ──
          # ── anchor: L05 ──
          # ── anchor: L06 ──
          # ── anchor: L08 ──
          # ── anchor: L09 ──
          # ── anchor: L11 ──
          withPersonModules
          # ── anchor: L12 ──
          # ── anchor: L13 ──
          # ── anchor: L15 ──
          # ── anchor: L16 ──
          releaseApps
          # ── anchor: L17 ──
          # ── anchor: L19 ──
          # ── anchor: L20 ──
          # ── anchor: L22 ──
          ;
      };

      # =================================================================
      # apps — section 8. Flake apps, never a justfile or a Makefile.
      # =================================================================
      apps.${system} = {
        # The nested compositor + footer. The loop where 90% of the UI work
        # happens (section 7.12, speed 1).
        dev = {
          type = "app";
          program = "${devApp}/bin/ronoctua-dev";
          meta = {
            description = "the nested compositor + footer, for the UI working loop";
            platforms = [ system ];
          };
        };

        # ONE FEATURE'S PHOTOGRAPHS: `nix run .#shot -- <scene.py>`, the scene
        # written for the feature being built and kept OUTSIDE this repository.
        # The stage is `nix/shot.nix`; the pictures land in
        # `$XDG_CACHE_HOME/ronoctua-distro/shot`, emptied every run — nothing
        # of a feature built before is photographed again.
        shot = {
          type = "app";
          program = "${shotApp}/bin/ronoctua-shot";
          meta = {
            description = "photographs one feature on a real compositor, from a scene written for it";
            platforms = [ system ];
          };
        };

        # The snapshot for when the shortcuts freeze. READ-ONLY: it sends no
        # command, kills no process and changes no focus — a diagnostic that disturbs
        # what it is measuring destroys the evidence it came for.
        diagnostics = {
          type = "app";
          program = "${diagnostics}/bin/ronoctua-diagnostics";
          meta = {
            description = "a snapshot of the state when the shortcuts lock up";
            platforms = [ system ];
          };
        };

        # THE GATE, sized to the RAM of whoever runs it.
        #
        # `nix flake check` on its own is not enough on a machine with an
        # `earlyoom`: it kills `rustc` with SIGTERM and no diagnostic, and
        # `--max-jobs` does not save it. This one measures first, caps the
        # product of jobs and cores, evaluates before it builds and never while,
        # builds each Rust unit by its class (the heavy ones alone), runs the VM tests in a
        # phase of their own, packed by the memory they declare, and — if
        # something dies anyway — says who killed it.
        gate = {
          type = "app";
          program = "${gateApp}/bin/ronoctua-gate";
          meta = {
            description = "nix flake check, sized by the machine's RAM";
            platforms = [ system ];
          };
        };

        # THE EVERYDAY BATTERY — everything that does not build an installation.
        #
        # `most` and not `check`: `nix run .#check` and `nix flake check` would
        # be two near-homonyms hours apart, and whoever typed the wrong one
        # would only find out afterwards.
        #
        # It runs the VMs — measured, each script takes 17 to 48 s — and leaves
        # out whatever builds a NixOS system or the ISO. When it ends it SAYS
        # what it did not verify, by name: a battery that stays quiet about what
        # it skipped is a battery that gets mistaken for the gate.
        most = {
          type = "app";
          program = "${mostApp}/bin/ronoctua-most";
          meta = {
            description = "the checks that do not build an installation, by the machine's RAM";
            platforms = [ system ];
          };
        };

        # THE SOURCES THAT HAVE TO COME BY HAND, and why that is not imperative.
        #
        # It changes no configuration: it puts a file the flake ALREADY names
        # into the store, with the hash the flake ALREADY declares. What it adds
        # is a downloader that resumes, which `fetchurl` does not have.
        #
        # `.#gate` runs it on its own before checking. It is exposed separately
        # so it can be run first, on a good connection, without paying for the
        # whole gate.
        prefetch = {
          type = "app";
          program = "${prefetchApp}/bin/ronoctua-prefetch";
          meta = {
            description = "downloads, with resume, the fonts no cache has";
            platforms = [ system ];
          };
        };

        # THE SHORT LANE — what one runs in the middle of the work.
        #
        # # Why it exists, and why it does not replace the gate
        #
        # The gate is 30–45 minutes: four NixOS systems, the ISO and fourteen
        # virtual machines. That is the right price to CLOSE something
        # (invariant 7) and the wrong one to ask twenty times an afternoon —
        # so what happened instead is what happened all session: `cargo` typed
        # by hand inside `nix develop`, which no document mentions and which
        # checks none of the Nix side.
        #
        # This runs everything that needs no virtual machine, in one go so Nix
        # parallelises it: formatting (both languages, the buffers included),
        # clippy with `-D warnings`, the tests, the keymap, the two languages,
        # the buffers' labels and keys, and the guard on the dependency layer.
        # With a warm cache it is about a minute.
        #
        # It says out loud that it is not the gate. A short lane that lets one
        # believe the work is finished is worse than no short lane.
        quick = mkScriptApp "ronoctua-quick" "the checks that bring up no VM — the short loop" ''
          ${pkgs.callPackage ./nix/tree-guard.nix { }}/bin/ronoctua-tree-holds-no-build
          echo "== the short loop: a chosen subset =="
          ${keepSources}
          # THE LIST COMES FROM `tests/quick.nix`, not from here.
          #
          # Written here it would be the only copy, and nothing could check it.
          # Coming from a file that `quick-names-exist` also reads, a renamed
          # check is reported instead of dropping out of the loop in silence.
          # SIZED (`nix/build-sizing.nix`): every unit `rust-tests` and
          # `rust-clippy` reach its own build, by its class.
          ${sizedBuild}/bin/ronoctua-sized-build --no-link --keep-going -L \
            ${
              lib.concatMapStringsSep " \\\n            " (n: ".#checks.${system}.${n}") (
                import ./tests/quick.nix
              )
            } \
            "$@"
          # The units the tests and clippy were just built from exist now, so
          # they are held while they are this checkout's current ones.
          ${keepSources}
          echo
          echo "OK — and this is a SUBSET, not a category."
          echo "     They are the ones chosen for the editing loop. It is not complete and"
          echo "     does not aim to be: there are more than a hundred checks, and most of them"
          echo "     also bring up no VM."
          echo
          echo "     nix run .#most   everything but the ones that build an installation"
          echo "     nix run .#gate   everything (invariant 7)"
        '';

        # A complete system in a VM.
        #
        # THE RUNNER IS NAMED AFTER THE HOST, and this line said `run-nixos-vm`
        # from the first commit onward — a path that has never existed here.
        # NixOS builds `bin/run-''${networking.hostName}-vm`, and this system is
        # called `noctua-vm`, so `nix run .#vm` answered `No such file or
        # directory` and nothing else. Taking the name from the same option
        # that decides it is what stops the two from disagreeing again.
        vm = mkScriptApp "ronoctua-vm" "brings the whole system up in a VM" ''
          # THE GUEST GETS THIS MACHINE'S PANEL, and not QEMU's default.
          #
          # `-device virtio-gpu-pci` with no size gives the guest 1280x800 —
          # measured, by dumping the guest's own framebuffer over QMP — while
          # the screen it is shown on is 1920x1200. So the VM never showed
          # what the installed system looks like: a smaller desktop, scaled
          # into a window. "It does not open at the production size" is
          # exactly that, and the number was written down nowhere.
          #
          # `-global` AND NOT A SECOND `-device`: `nix/modules/vm.nix` already
          # adds the GPU, and a second one would give the guest two.
          # `-global driver.property=value` sets the property on the one that
          # is there.
          #
          # BY THE PROTOCOL, as `nix run .#dev` does it: `wl_output`'s mode is
          # what the panel really is. BY STORE PATH, because a flake app's
          # script does not get to assume a `PATH`.
          if [ -n "''${WAYLAND_DISPLAY:-}" ]; then
            # BY THE LABEL, never by a field number: the line reads
            # `width: 1920 px, height: 1200 px, refresh: …`, so the height is
            # the fifth field and not the fourth — and it moves the day the
            # line gains a word.
            panel=$(${pkgs.wayland-utils}/bin/wayland-info -i wl_output 2>/dev/null \
              | awk '/width: [0-9]+ px/ {
                       for (i = 1; i <= NF; i++) {
                         if ($i == "width:") w = $(i + 1)
                         if ($i == "height:") h = $(i + 1)
                       }
                       gsub(/[^0-9]/, "", w); gsub(/[^0-9]/, "", h)
                       if (w != "" && h != "") { print w "x" h; exit }
                     }') || true
            case "''${panel:-}" in
              [0-9]*x[0-9]*)
                echo "vm: the guest gets this screen's own $panel"
                export QEMU_OPTS="''${QEMU_OPTS:-} -global virtio-gpu-pci.xres=''${panel%x*} -global virtio-gpu-pci.yres=''${panel#*x}"
                # AND QEMU'S WINDOW DRAWS IN REAL PIXELS, when the host can.
                #
                # Under a Wayland host at a FRACTIONAL scale, QEMU's GTK
                # window is sized in logical pixels (1920 / 1.6 = 1200 wide),
                # hands that size to the guest as its preferred mode, and the
                # host then stretches the result back up: everything 1.6×
                # larger and resampled. Measured: the guest drew 1920x1200 at
                # scale 1 when asked headless, and 17.5 px per character on
                # the host's screen against the 10.8 it draws.
                #
                # The escape is the one `nix run .#dev` takes (see `the host's
                # own pixels` in nix/dev.nix): when the host's XWayland serves
                # REAL pixels — its root is as wide as the panel —, GTK goes
                # through X11 and one guest pixel is one screen pixel. When it
                # does not, nothing on this side can undo the host's scaling.
                x_root=""
                if [ -n "''${DISPLAY:-}" ]; then
                  x_root=$(${pkgs.xdpyinfo}/bin/xdpyinfo 2>/dev/null \
                    | awk '/^[[:space:]]*dimensions:/ { split($2, d, "x"); print d[1]; exit }') || true
                fi
                if [ -n "$x_root" ] && [ "$x_root" = "''${panel%x*}" ]; then
                  echo "vm: the host's X serves real pixels; the window goes through it"
                  export GDK_BACKEND=x11
                  # AND AT SCALE ONE, whatever the session exports. A host
                  # session may set `GDK_SCALE=2` for its own apps, and GTK
                  # honours it over X11 too: the window then measures itself
                  # in doubled pixels, hands the guest half the size, and
                  # stretches it back — the same enlargement by another road,
                  # measured after the X11 path alone changed nothing.
                  export GDK_SCALE=1
                  unset GDK_DPI_SCALE
                fi
                ;;
            esac
          fi
          # QEMU'S OWN WINDOW GETS A FONT CONFIG OF ITS OWN. Its GTK comes from
          # the store and reads the HOST's `/etc/fonts` unless told otherwise,
          # and a host with a newer fontconfig writes syntax this
          # one rejects: forty `Fontconfig warning: … invalid constant used`
          # lines on every start, about a window menu.
          #
          # NOT `makeFontsConf`: the file it writes still includes
          # `/etc/fonts/conf.d`, which is exactly the host's directory — the
          # warnings came back unchanged with it. One font directory and a
          # cache, and nothing of the host's.
          export FONTCONFIG_FILE=${pkgs.writeText "qemu-window-fonts.conf" ''
            <?xml version="1.0"?>
            <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
            <fontconfig>
              <dir>${pkgs.dejavu_fonts}/share/fonts</dir>
              <cachedir prefix="xdg">fontconfig</cachedir>
            </fontconfig>
          ''}
          # `--pointers`: EVERY MOUSE AND TOUCHPAD OF THE HOST, EACH ITS OWN.
          #
          # QEMU's window gathers every host pointer into ONE emulated device,
          # so without this the guest sees one mouse however many are on the
          # desk, and every one of them moves the same cursor — which is
          # exactly what a multi-cursor desktop cannot be tried with. Each
          # device passed as `virtio-input-host-pci` arrives in the guest as
          # itself, and the guest gives it a cursor of its own.
          #
          # OPT-IN, because QEMU GRABS what it is given: while the VM runs, the
          # host's pointers move nothing on the host, except its built-in
          # touchpad, which is kept. They come back when the VM closes. The
          # keyboard is left alone.
          #
          # BY UDEV, never by a name: `ID_INPUT_MOUSE` and `ID_INPUT_TOUCHPAD`
          # are what the host's own stack decided the device is. keyd's
          # virtual devices (vendor 0fac) are the keyboards' output and not a
          # hand, and are left on the host.
          pass=()
          for a in "$@"; do
            if [ "$a" = "--pointers" ]; then
              pointers=yes
            elif [ "$a" = "--keyboards" ]; then
              keyboards=yes
            elif [ "$a" = "--cameras" ]; then
              cameras=yes
            else
              pass+=("$a")
            fi
          done
          # AN EARLIER VM ON THIS SAME DISK IS STOPPED FIRST, and only that one.
          #
          # Two VMs cannot share a disk — QEMU refuses the image's lock — and
          # one left running from another terminal still holds the host's mice
          # and keyboards under `--pointers`/`--keyboards`, so this one could
          # not take them. The match is the disk's own path in QEMU's command
          # line, never a program's name: a VM of something else, or this
          # system installed and running, is not touched.
          disk=$(readlink -f "''${NIX_DISK_IMAGE:-${self.nixosConfigurations.vm.config.virtualisation.vmVariant.virtualisation.diskImage}}")
          for proc in /proc/[0-9]*; do
            if [ ! -O "$proc" ]; then
              continue
            fi
            line=$({ tr '\0' ' ' < "$proc/cmdline"; } 2>/dev/null) || continue
            case "$line" in
              *qemu-system*"file=$disk,"*)
                pid=''${proc#/proc/}
                echo "vm: stopping the VM already running on $disk (pid $pid)"
                kill "$pid" 2>/dev/null || true
                for _ in $(seq 1 50); do
                  if ! kill -0 "$pid" 2>/dev/null; then
                    break
                  fi
                  sleep 0.1
                done
                ;;
            esac
          done
          if [ "''${pointers:-}" = yes ]; then
            found=0
            # THE BUILT-IN TOUCHPAD'S PARENTS, found before anything is passed:
            # a touchpad reaches the system as TWO devices under one parent —
            # `…Touchpad`, which moves, and `…Mouse`, a compatibility interface
            # that never does — and handing the second over while the first
            # stays gave the guest a cursor nothing could move.
            kept=""
            for dev in /dev/input/event*; do
              props=$(${pkgs.systemd}/bin/udevadm info -q property -n "$dev" 2>/dev/null) || continue
              if grep -qE '^ID_INPUT_TOUCHPAD_INTEGRATION=internal$' <<< "$props"; then
                kept="$kept $(readlink -f "/sys/class/input/''${dev##*/}/device/device" || true)"
              fi
            done
            for dev in /dev/input/event*; do
              props=$(${pkgs.systemd}/bin/udevadm info -q property -n "$dev" 2>/dev/null) || continue
              if ! grep -qE '^ID_INPUT_(MOUSE|TOUCHPAD)=1$' <<< "$props"; then
                continue
              fi
              if grep -qiE '^ID_VENDOR_ID=0fac$' <<< "$props"; then
                continue
              fi
              name=$(${pkgs.gawk}/bin/awk -F= '$1 == "NAME" { gsub(/"/, "", $2); print $2 }' <<< "$props")
              if [ -z "$name" ]; then
                name=$(cat "/sys/class/input/''${dev##*/}/device/name" 2>/dev/null || echo "$dev")
              fi
              case "$name" in
                *keyd*) continue ;;
              esac
              # THE BUILT-IN TOUCHPAD STAYS WITH THE HOST, every interface of it
              # (`kept` above), the way `--keyboards`
              # keeps the built-in keyboard: a pointer the host still holds is
              # the one that leaves QEMU's window, and inside the guest it moves
              # QEMU's tablet — off until `LEADER s i` turns the virtual
              # machine's own pointers on.
              parent=$(readlink -f "/sys/class/input/''${dev##*/}/device/device" || true)
              if [ -n "$parent" ]; then
                case " $kept " in
                  *" $parent "*)
                    echo "vm: $name stays with the host"
                    continue
                    ;;
                esac
              fi
              # READ ACCESS, asked for once and only where it is missing: the
              # node is `root:input 0660`, and an ACL for this user is the
              # narrowest thing that opens it. It lasts until the device is
              # replugged or the host reboots.
              if [ ! -r "$dev" ] || [ ! -w "$dev" ]; then
                echo "vm: $name ($dev) needs read access; asking sudo for it"
                sudo ${pkgs.acl}/bin/setfacl -m "u:$(id -un):rw" "$dev"
              fi
              echo "vm: passing $name ($dev) to the guest as a pointer of its own"
              export QEMU_OPTS="''${QEMU_OPTS:-} -device virtio-input-host-pci,evdev=$dev"
              found=$((found + 1))
            done
            if [ "$found" = 0 ]; then
              echo "vm: --pointers found no mouse or touchpad on this host" >&2
            else
              echo "vm: $found pointer(s) are the guest's until it closes"
            fi
          fi
          # `--keyboards`: EVERY EXTERNAL KEYBOARD OF THE HOST, EACH ITS OWN —
          # what Independent mode is tried with. The BUILT-IN one stays with
          # the host (buses i8042 and host, `0011`/`0019`), so the host is never
          # left without a keyboard while the VM holds the others.
          if [ "''${keyboards:-}" = yes ]; then
            taken=0
            for dev in /dev/input/event*; do
              props=$(${pkgs.systemd}/bin/udevadm info -q property -n "$dev" 2>/dev/null) || continue
              if ! grep -qE '^ID_INPUT_KEYBOARD=1$' <<< "$props"; then
                continue
              fi
              if grep -qiE '^ID_VENDOR_ID=0fac$' <<< "$props"; then
                continue
              fi
              # A SECURITY TOKEN THAT ALSO TYPES — its one-time codes arrive
              # as keystrokes — carries a smart-card interface (class `0b`).
              # It is the key somebody signs with, and it stays with the host.
              if grep -qE '^ID_USB_INTERFACES=.*:0b[0-9a-f]{4}:' <<< "$props"; then
                continue
              fi
              bus=$(cat "/sys/class/input/''${dev##*/}/device/id/bustype" 2>/dev/null || echo "")
              case "$bus" in
                0011 | 0019) continue ;;
              esac
              name=$(cat "/sys/class/input/''${dev##*/}/device/name" 2>/dev/null || echo "$dev")
              case "$name" in
                *keyd*) continue ;;
              esac
              if [ ! -r "$dev" ] || [ ! -w "$dev" ]; then
                echo "vm: $name ($dev) needs read access; asking sudo for it"
                sudo ${pkgs.acl}/bin/setfacl -m "u:$(id -un):rw" "$dev"
              fi
              echo "vm: passing $name ($dev) to the guest as a keyboard of its own"
              export QEMU_OPTS="''${QEMU_OPTS:-} -device virtio-input-host-pci,evdev=$dev"
              taken=$((taken + 1))
            done
            if [ "$taken" = 0 ]; then
              echo "vm: --keyboards found no external keyboard on this host" >&2
            else
              echo "vm: $taken keyboard(s) are the guest's until it closes; the built-in one stays here"
            fi
          fi
          # `--cameras`: EVERY USB CAMERA OF THE HOST, as the USB DEVICE it is.
          #
          # A camera is not an evdev node that QEMU can forward: the guest has
          # to see the device itself, enumerate its interfaces and load its
          # own driver, which is what `usb-host` does. So what is passed is
          # the USB device every capture node hangs from — ONE per device, the
          # colour and the infrared nodes of one webcam being two interfaces
          # of the same thing.
          #
          # BY UDEV, never by a model: `ID_V4L_CAPABILITIES` says `:capture:`
          # on a node that captures, whatever its maker, and `ID_BUS=usb` is
          # what can travel. A camera on another bus (a MIPI sensor behind
          # libcamera) cannot, and is named so rather than skipped in silence.
          # The host loses the camera until the VM closes.
          if [ "''${cameras:-}" = yes ]; then
            seen=""
            for dev in /dev/video*; do
              props=$(${pkgs.systemd}/bin/udevadm info -q property -n "$dev" 2>/dev/null) || continue
              if ! grep -qE '^ID_V4L_CAPABILITIES=.*:capture:' <<< "$props"; then
                continue
              fi
              # Our own loopback, if the host has one, is not a camera.
              if grep -qE '^ID_V4L_PRODUCT=ronoctua$' <<< "$props"; then
                continue
              fi
              name=$(cat "/sys/class/video4linux/''${dev##*/}/name" 2>/dev/null || echo "$dev")
              if ! grep -qE '^ID_BUS=usb$' <<< "$props"; then
                echo "vm: $name ($dev) is not on USB, and cannot be passed" >&2
                continue
              fi
              # The USB DEVICE, not the interface: the first ancestor with a
              # `busnum`.
              at=$(readlink -f "/sys/class/video4linux/''${dev##*/}/device")
              while [ -n "$at" ] && [ "$at" != / ] && [ ! -e "$at/busnum" ]; do
                at=''${at%/*}
              done
              if [ ! -e "$at/busnum" ]; then
                continue
              fi
              case " $seen " in
                *" $at "*) continue ;;
              esac
              seen="$seen $at"
              bus=$(cat "$at/busnum")
              addr=$(cat "$at/devnum")
              node=$(printf '/dev/bus/usb/%03d/%03d' "$bus" "$addr")
              if [ ! -r "$node" ] || [ ! -w "$node" ]; then
                echo "vm: $name ($node) needs access; asking sudo for it"
                sudo ${pkgs.acl}/bin/setfacl -m "u:$(id -un):rw" "$node"
              fi
              echo "vm: passing $name (usb $bus:$addr) to the guest"
              export QEMU_OPTS="''${QEMU_OPTS:-} -device usb-host,bus=cameras.0,hostbus=$bus,hostaddr=$addr"
            done
            if [ -z "$seen" ]; then
              echo "vm: --cameras found no USB camera on this host" >&2
            else
              # One controller for all of them, named so the devices can say
              # which bus they go on.
              export QEMU_OPTS="-device qemu-xhci,id=cameras ''${QEMU_OPTS:-}"
              echo "vm: the camera(s) are the guest's until it closes"
            fi
          fi
          exec ${self.nixosConfigurations.vm.config.system.build.vm}/bin/run-${self.nixosConfigurations.vm.config.networking.hostName}-vm "''${pass[@]}"
        '';

        # Packs the fonts for a GitHub Release — the free way to publish 4,1 GB.
        #
        # WHY NOT GIT, AND WHY NOT LFS
        # ----------------------------
        # The fonts are 4,1 GB of generated binaries. In plain git that is a
        # repository GitHub refuses; in LFS it is four times the free 1 GB quota,
        # and the fix on offer is a paid data pack.
        #
        # A Release has no quota: 2 GB per asset, unlimited for a public
        # repository, and free. And `fetchurl` with a hash is every bit as
        # reproducible as a git rev — more honest, even, since these files are
        # build OUTPUT and not source.
        #
        # zstd -19 measured on this set: 24% of the original. 2,1 GB of patched
        # TTF becomes ~510 MB, comfortably inside one asset.
        fonts-release =
          mkScriptApp "ronoctua-fonts-release" "packages the fonts for a GitHub Release and prints the hashes"
            ''
              root="''${1:-}"
              if [ -z "$root" ] || [ ! -d "$root/noctua-iosevka" ]; then
                echo "usage: nix run .#fonts-release -- /path/with/noctua-iosevka" >&2
                exit 1
              fi
              out=$(mktemp -d -t ronoctua-fonts-release-XXXXXX)
              echo "packing into $out"
              echo

              # THE RELEASE NOTES ARE WRITTEN HERE, IN ENGLISH, AND NOT TYPED AT
              # PUBLISH TIME.
              #
              # A Release is the one thing this project publishes to an audience
              # that is not the owner, so it is in English — while the four
              # documents stay in Portuguese. Text typed by hand into the GitHub
              # form is text nobody reviews and nobody can diff; the first
              # release body was written that way and came out in Portuguese.
              # Generating it puts the hashes in it for free.
              notes="$out/RELEASE-NOTES.md"
              {
                echo "# noctua-iosevka — font files"
                echo
                echo "The built font files: Iosevka SemiExtended with the 27 naming fixes,"
                echo "plus the Nerd Fonts patch. 4.1 GB uncompressed, across four archives."
                echo
                echo "These are build OUTPUT and do not enter git: git keeps every version"
                echo "forever, and one Iosevka bump would double that with no way back. The"
                echo "flake fetches them from here, pinned by hash."
                echo
                echo "| asset | size | sha256 |"
                echo "|---|---|---|"
              } > "$notes"

              for dir in TTF TTF-nerdfonts WOFF2 WOFF2-nerdfonts; do
                [ -d "$root/noctua-iosevka/$dir" ] || continue
                asset="$out/noctua-iosevka-$dir.tar.zst"
                # `--sort=name` and a fixed mtime: without them the tarball's
                # hash changes on every run and the lock would have to be updated
                # for nothing.
                tar --sort=name \
                    --mtime='@0' --owner=0 --group=0 --numeric-owner \
                    -C "$root/noctua-iosevka" \
                    -cf - "$dir" \
                  | zstd -19 -T0 -q -o "$asset"
                size=$(stat -c %s "$asset")
                hash=$(nix hash file --sri "$asset")
                mb=$(awk "BEGIN{printf \"%.1f\", $size/1048576}")
                printf '%-34s %6s MB  %s\n' "$(basename "$asset")" "$mb" "$hash"
                # Double quotes with escaped backticks, and not single quotes: a
                # backtick inside single quotes reads as a command substitution
                # that will not expand (SC2016), and `writeShellApplication`
                # treats even the info level as an error.
                #
                # A comment whose FIRST word is the linter's name is parsed as a
                # directive, not as prose — which is how this very line first
                # failed the build with a syntax error.
                printf "| \`%s\` | %s MB | \`%s\` |\n" \
                  "$(basename "$asset")" "$mb" "$hash" >> "$notes"
              done

              {
                echo
                echo "Licence: SIL Open Font License 1.1 (\`LICENSES/LICENSE-OFL.txt\` in the"
                echo "repository). The ronoctua-distro code is MIT; these files are not."
              } >> "$notes"

              echo
              echo "release notes (in English, ready): $notes"
              echo
              echo "Upload the four as assets of a Release at"
              echo "  https://github.com/ronoctua/ronoctua-distro/releases/new"
              echo "with a tag OF THE FONTS' OWN (fonts-v1), not to be confused"
              echo "with the ISO's, and the body coming from the file above. Then run"
              echo "  nix flake update fonts-ttf fonts-ttf-nerdfonts fonts-woff2 fonts-woff2-nerdfonts"
            '';

        # Packs the ambient sounds' originals for a GitHub Release — the fonts'
        # way, for the fonts' reason: 901 MiB of recordings are not code, and git
        # keeps every version for ever.
        #
        # ONLY WHAT WAS APPROVED GOES IN: every original `assets/ambience.nix`
        # names, checked against the sha256 recorded when the owner approved it
        # by ear, and nothing else from the folder — a file that is not the one
        # the owner heard stops the packing here, not a build later. zstd -19
        # measured on the three WAVs: 901 MiB into 695 MiB (recorded noise
        # compresses poorly; a system carries the 42 MiB of loops, never this).
        #
        # THE SAME BYTES ON EVERY RUN: sorted names, a fixed mtime, owner and
        # mode, and zstd's output does not depend on its thread count (measured,
        # -T0 and -T3: one hash) — so the hash the input is locked by is the one
        # this prints, whoever packs. The files sit at the archive's root, which
        # is where `nix/ambience.nix` looks for them after unpacking.
        #
        # The input it becomes, `ambience-v1`, is NOT in this flake yet: an input
        # nobody can fetch stops every evaluation at the lock. The owner's steps,
        # from the Release to the lock, are in `docs/PENDING.md`.
        ambience-release =
          let
            table = import ./assets/ambience.nix;
            sums = pkgs.writeText "ambience-originals.sha256" (
              lib.concatMapStrings (s: "${s.sha256}  ${s.original}\n") table.sounds
            );
            credits = lib.concatMapStrings (
              s: "printf '| %s | %s | %s | %s |\\n' '${s.original}' '${s.author}' '${s.page}' '${s.licence}'\n"
            ) table.sounds;
          in
          mkScriptApp "ronoctua-ambience-release"
            "packages the ambient sounds' originals for a GitHub Release and prints the hash"
            ''
              root="''${1:-}"
              if [ -z "$root" ] || [ ! -d "$root" ]; then
                echo "usage: nix run .#ambience-release -- /path/with/the/originals" >&2
                exit 1
              fi
              (cd "$root" && sha256sum --check --strict --quiet ${sums})
              out=$(mktemp -d -t ronoctua-ambience-release-XXXXXX)
              asset="$out/ambience-originals.tar.zst"
              awk '{ print $2 }' ${sums} | sort > "$out/files"
              tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner --mode='u=rw,go=r' \
                  -C "$root" -cf - --files-from="$out/files" \
                | zstd -19 -T0 -q -o "$asset"
              rm "$out/files"
              size=$(stat -c %s "$asset")
              hash=$(nix hash file --sri "$asset")
              mb=$(awk "BEGIN{printf \"%.1f\", $size/1048576}")
              # The notes are written here, in English, as the fonts' are.
              notes="$out/RELEASE-NOTES.md"
              {
                echo "# ambience-v1 — the ambient sounds' originals"
                echo
                echo "The recordings the player's ambient sounds are cut from, as published"
                echo "on Freesound and approved by ear. They are not in git: the flake"
                echo "fetches this archive pinned by hash, and the build refuses any"
                echo "recording whose sha256 is not the approved one."
                echo
                echo "| recording | author | source | licence |"
                echo "|---|---|---|---|"
              } > "$notes"
              {
                ${credits}
                echo
                echo "| asset | size | sha256 |"
                echo "|---|---|---|"
                printf "| \`%s\` | %s MB | \`%s\` |\n" "$(basename "$asset")" "$mb" "$hash"
              } >> "$notes"
              echo "packed: $asset ($mb MB, $hash)"
              echo "release notes (in English, ready): $notes"
              echo
              echo "Publish it as the one asset of a Release with a tag OF ITS OWN:"
              echo "  gh release create ambience-v1 '$asset' --repo ronoctua/ronoctua-distro \\"
              echo "    --title 'ambience-v1' --notes-file '$notes'"
              echo "Then add the input (ambience-v1) and lock it alone:"
              echo "  nix flake lock --update-input ambience-v1"
            '';

        # Builds the ISO and says where it is.
        iso = mkScriptApp "ronoctua-iso" "builds the installer's ISO" ''
          out=$(nix build --no-link --print-out-paths .#iso)
          echo "the ISO came out at:"
          find "$out/iso" -name '*.iso' -printf '  %p  (%s bytes)\n'
        '';

        # The real installer. Without `--execute` it does a DRY RUN: it shows the
        # `host.nix` it would generate and the steps it would run, and exits. That is the default
        # on purpose — the first step erases an entire disk.
        #
        # THE PERSON'S TWO PROGRAMS ON ITS PATH, the ones built with it: the
        # installer runs them by name, and a release's installer started by
        # `nix run <release>#install` must run the release's, not the ISO's.
        install = mkScriptApp "ronoctua-install" "installs the production version" ''
          export PATH=${
            lib.makeBinPath [
              rust.belongings
              rust.person
            ]
          }:$PATH
          exec ${rust.installer}/bin/ronoctua-install "$@"
        '';

        # Measures performance on real hardware (section 9.3 of the plan: section 5.2's
        # targets are NEVER measured under lavapipe). The text is
        # `nix/bench.nix`, which `.#profile-bench` runs over each profile.
        bench = mkScriptApp "ronoctua-bench" "measures section 5.2's targets on real hardware" (
          import ./nix/bench.nix {
            benches = rust.benches;
            frames = benchFrames;
            themes = rust.keyed.RONOCTUA_THEMES;
            component = "${buffers.brain}/lib/ronoctua_buffer_brain.wasm";
          }
        );

        # THE RELEASE PROFILE, CHOSEN BY MEASUREMENT (nix-design §3): each
        # variant of `nix/rust/profiles.nix` built, `.#bench` run over each,
        # rounds interleaved, and the rule applied. Builds by name at run
        # time, so evaluating this app builds nothing.
        profile-bench = {
          type = "app";
          program = lib.getExe profileBench.app;
          meta = {
            description = "measures the release profile's variants with .#bench and names the one the rule picks";
            platforms = [ system ];
          };
        };

        # A BUFFER BECOMES A PNG, WITH NO COMPOSITOR AND NO VM.
        #
        # The owner's rule is that green is not proof: what appears on the
        # screen is opened on the screen before anybody says it is done. Until
        # this existed, obeying that rule cost a whole NixOS VM — boot the
        # compositor under Xvfb, screenshot the X root, crop by the geometry it
        # reported.
        # Three minutes to see whether a rule was touching a title's descenders,
        # and a rule that expensive is a rule that gets skipped: the title that
        # prompted this went months at twice the body's size with every check
        # green.
        #
        # Here it is 0.2 s. What differs from the screen is ONLY where the pixel
        # lands — a texture instead of a `wl_surface`; the conversion, the
        # margins, the cell, the atlas and the font are the production ones, by
        # way of the `paint` module.
        #
        # THE LOOP'S PROGRAMS, not the release ones: the profile changes how
        # fast a picture arrives, never its pixels, and an edit of a view is a
        # loop link of seconds where the release one is a thin-LTO link of
        # minutes — the same reason `.#dev` runs them.
        #
        # The graphics environment is the checks' own: lavapipe, Mesa's software
        # Vulkan rasteriser. That is not a performance choice — it is what makes
        # the command answer the same on this machine, on one with no GPU and
        # inside a check. Section 9.3 still holds: frame time is NEVER measured
        # here.
        png = mkScriptApp "ronoctua-png" "draws a buffer or a native door into a PNG, with no VM" ''
          # NO `LD_LIBRARY_PATH` HERE, and that is deliberate: it beats
          # `DT_RUNPATH`, and the binary already carries the loader it was built
          # against. Putting a second `libvulkan` in front of it is the same
          # mistake as reaching for the host's `/usr/lib64` — measured, and it
          # works without it.
          export VK_DRIVER_FILES=${xvfbIcd}
          export VK_ICD_FILENAMES=${xvfbIcd}
          export LIBGL_ALWAYS_SOFTWARE=1
          # THE COVERAGE FACES, because this is the tool the working rules
          # point at before the word "ready" — and a screenshot taken without
          # them would show tofu where the installed system shows Japanese.
          # It is set here and not inherited: the loop's programs below are
          # deliberately unwrapped (the wrapper is `rust.shell`'s, and it wraps
          # the footer only), so this launcher is one of the nine that has to
          # say it.
          export RONOCTUA_COVERAGE_FONTS=${rust.atRunTime.RONOCTUA_COVERAGE_FONTS}
          # THE PALETTES, which every picture is painted in: data read at run
          # time, and SET rather than defaulted, for the wrappers' reason — the
          # session may hold another build's (`rust.keyed`), and a palette is
          # refused by a binary it was not written for.
          export RONOCTUA_THEMES=${rust.keyed.RONOCTUA_THEMES}
          # AND THE TEXTS, for the same reason: a picture is drawn in words read
          # at run time, and a binary refuses (exit 78) texts that lack a key it
          # was compiled with.
          export RONOCTUA_TEXTS=${rust.keyed.RONOCTUA_TEXTS}
          # AND THE BRAIN'S CATALOGUE, for the same reason: a door draws the
          # modules' marks and teaches their actions from data read at run time.
          export RONOCTUA_BRAIN_CATALOGUE=${rust.keyed.RONOCTUA_BRAIN_CATALOGUE}
          # A NATIVE DOOR — the player's window, its column at every size —
          # is not a buffer: `render-png` cannot reach it, so `--door`
          # anywhere in the words is the shell's own camera
          # (`ronoctua-shell photograph`), in the same software Vulkan. The
          # world goes with it, for a door that draws the globe.
          for word in "$@"; do
            if [ "$word" = "--door" ]; then
              export RONOCTUA_ATLAS=${rust.atRunTime.RONOCTUA_ATLAS}
              export RONOCTUA_ICONSET=${rust.atRunTime.RONOCTUA_ICONSET}
              exec ${rust.loop.bins.ronoctua-shell}/bin/ronoctua-shell photograph "$@"
            fi
          done
          exec ${rust.loop.bins.ronoctua-render-png}/bin/ronoctua-render-png "$@"
        '';

        # ── anchor: L04 ──
        # THE DISTRO'S OWN PINS MOVE HERE, and only here: the AppImages and
        # Android apps of `assets/distro-apps.nix` that follow a source (a
        # GitHub repository, F-Droid) get the newest release's address and
        # hash in `assets/distro-pins.json`. The owner runs it, reads the
        # diff and commits by hand — a pin is what the gate tested, so it
        # never moves on a machine by itself. `--check` writes nothing.
        distro-pins =
          mkScriptApp "ronoctua-distro-pins" "moves the pins of the distro's own AppImages and Android apps"
            ''
              exec ${pkgs.python3}/bin/python3 ${./nix/scripts/distro-pins.py} "$@"
            '';
        # ── anchor: L05 ──
        # ── anchor: L15 ──
        # ── anchor: L16 ──
        # THE RELEASE — the owner's, and the only thing that moves the
        # `release` branch every machine follows. It refuses unless this is
        # `main`, clean and equal to `origin/main`; then the gate, the two
        # reference systems and the ISO (which carries its seed), the branch
        # (forward only) and the GitHub release. It never commits. The distro
        # has no binary cache: a person's own is theirs (`nix/modules/binary-cache.nix`).
        # `--dry-run` checks and says, `--rollback-to <rev>` is the one way back.
        release = {
          type = "app";
          program = lib.getExe releaseApps.release;
          meta = {
            description = "publishes the distro: gate, the release branch and the ISO";
            platforms = [ system ];
          };
        };

        # ANYBODY'S: the latest release's ISO, every part and the whole
        # checked against its SHA256SUMS, joined.
        fetch-iso = {
          type = "app";
          program = lib.getExe releaseApps.fetchIso;
          meta = {
            description = "downloads a release's ISO, checks it and joins its parts";
            platforms = [ system ];
          };
        };

        # A machine of the OLD layout (a copy of the distro in its machine
        # directory), whose own update cannot migrate it: migrate, then apply.
        adopt-machine = {
          type = "app";
          program = lib.getExe releaseApps.adoptMachine;
          meta = {
            description = "moves a machine of the old layout to a flake of its own that follows the release";
            platforms = [ system ];
          };
        };

        # The owner's checkout on the owner's machine, without a release:
        # `--override-input`, and the machine directory untouched.
        apply-here = {
          type = "app";
          program = lib.getExe releaseApps.applyHere;
          meta = {
            description = "builds this machine with the checkout in place of the distro input, and switches";
            platforms = [ system ];
          };
        };

        # THE BUILD PLAN, REGENERATED: cargo's unit graph of this tree
        # (`nix/rust/planner.nix`), normalised, copied into
        # `nix/rust/plan.json`. Cargo never runs in the tree — the planner is a
        # derivation over the manifests and the lock — and the units refuse a
        # plan made from other manifests, so after a change to a dependency,
        # a feature, a profile or the set of targets this is the next command.
        rust-plan = {
          type = "app";
          program = lib.getExe (
            pkgs.writeShellApplication {
              name = "ronoctua-rust-plan";
              runtimeInputs = [
                pkgs.git
                pkgs.coreutils
                pkgs.diffutils
              ];
              text = ''
                root=$(git rev-parse --show-toplevel)
                plan="$root/nix/rust/plan.json"
                if [ -f "$plan" ] && cmp -s ${rust.planner.plan}/plan.json "$plan"; then
                  echo "nix/rust/plan.json: already the plan of this tree"
                  exit 0
                fi
                install -D -m 0644 ${rust.planner.plan}/plan.json "$plan"
                echo "nix/rust/plan.json: written ($(wc -c < "$plan") bytes)"
              '';
            }
          );
          meta = {
            description = "regenerates nix/rust/plan.json, the build plan cargo makes of this tree";
            platforms = [ system ];
          };
        };

        # THE TRANSLATOR'S HELP, over the checkout it runs in: `missing
        # <language>` lists, fragment by fragment, every key the base language
        # has and that one lacks, with the sentence to translate (and every
        # orphan), and `where <key>` names the fragment that holds a key. The
        # sentences live with the crates that say them (`crates/<crate>/i18n/`),
        # some sixty files; the build refuses an incomplete language, and this
        # says what to write. See `nix/scripts/texts.py`.
        texts = {
          type = "app";
          program = lib.getExe (
            pkgs.writeShellApplication {
              name = "ronoctua-texts";
              runtimeInputs = [ pkgs.python3 ];
              text = ''
                exec python3 ${./nix/scripts/texts.py} --languages ${lib.concatStringsSep "," languages} "$@"
              '';
            }
          );
          meta = {
            description = "what a language lacks in the texts' fragments, and where a key lives";
            platforms = [ system ];
          };
        };

        # WHAT AN EDIT COSTS THE RUST BUILD, before anything is built: the
        # units, layers and aggregates that move (`legacyPackages.<system>.
        # rustUnits`, two evaluations compared), the programs relinked, the
        # test runs run again, and the time each moved entry took the last
        # time it was built — summed, and along the critical path. The working
        # tree against HEAD, `--against <rev>`, or `-- <file>…` for a one-byte
        # edit of each file. See `nix/scripts/rebuild-cost.py`.
        rebuild-cost = {
          type = "app";
          program = lib.getExe (
            pkgs.writeShellApplication {
              name = "ronoctua-rebuild-cost";
              runtimeInputs = [
                pkgs.git
                pkgs.python3
                pkgs.nix
              ];
              text = ''
                exec python3 ${./nix/scripts/rebuild-cost.py} --system ${system} "$@"
              '';
            }
          );
          meta = {
            description = "says what an edit rebuilds in the Rust build and what it costs, before building";
            platforms = [ system ];
          };
        };

        # `nix build`, SIZED TO THE MACHINE (`nix/build-sizing.nix`): what
        # it is given evaluated once, every unit it reaches that Nix would
        # build its own build, as many at once as their classes' slots fit
        # the memory, the heavy ones alone; then what it was given. The
        # command for a build of the unit graph by hand.
        sized-build = {
          type = "app";
          program = "${sizedBuild}/bin/ronoctua-sized-build";
          meta = {
            description = "builds what it is given sized to the machine: each unit by its class, as many as fit, the heavy ones alone";
            platforms = [ system ];
          };
        };

        # EACH UNIT AGAINST ITS CLASS: every unit's `metrics/peak-kib` read
        # from the store, the heaviest listed, and a unit past the slot of its
        # class (`nix/rust/unit-classes.json`) is an error — the class the
        # batteries and a machine's apply start it by has fallen behind the
        # build. `--write` writes the classes as measured into the checkout
        # it runs in. See `nix/scripts/rust-peaks.py`.
        rust-peaks = {
          type = "app";
          program =
            let
              n = buildSizing.numbers;
              raw = lib.importJSON buildSizing.classesFile;
              # The class each planned unit is built by, and how it was
              # found (`classesOf`): the one resolution, the plans'.
              found = buildSizing.classesOf {
                units = lib.attrNames (
                  lib.filterAttrs (_: u: lib.elem u.kind buildSizing.plannedKinds) rust.unitIndex
                );
                classes = raw.units;
              };
              facts = pkgs.writeText "ronoctua-rust-peaks.json" (
                builtins.toJSON {
                  units = lib.mapAttrs (_: u: { inherit (u) kind flavour metrics; }) rust.unitIndex;
                  # READ RAW, never through the sizing's check of it: this is
                  # what writes it again when it is stale.
                  classes = raw;
                  resolved = lib.mapAttrs (_: f: f.class) found;
                  how = lib.mapAttrs (_: f: f.how) found;
                  order = map (c: c.name) n.classes;
                  slots = lib.listToAttrs (map (c: lib.nameValuePair c.name c.slotMiB) n.classes);
                  inherit (n) headroomPercent unclassed;
                  kinds = buildSizing.plannedKinds;
                  shellKiB = n.shellMiB * 1024;
                  besideKiB = n.besideMiB * 1024;
                }
              );
            in
            lib.getExe (
              pkgs.writeShellApplication {
                name = "ronoctua-rust-peaks";
                runtimeInputs = [
                  pkgs.python3
                  pkgs.git
                ];
                text = ''
                  args=()
                  for a in "$@"; do
                    if [ "$a" = --write ]; then
                      root=$(git rev-parse --show-toplevel 2>/dev/null) || {
                        echo "ronoctua-rust-peaks: --write writes into the checkout it runs in, and this is none" >&2
                        exit 2
                      }
                      args+=(--write "$root/nix/rust/unit-classes.json")
                    else
                      args+=("$a")
                    fi
                  done
                  exec python3 ${./nix/scripts/rust-peaks.py} "''${args[@]}" ${facts}
                '';
              }
            );
          meta = {
            description = "holds each unit to its measured class, and with --write writes the classes as measured";
            platforms = [ system ];
          };
        };
      };

      # THE RUST BUILD, unit by unit, for whoever builds or measures one:
      # `nix build .#legacyPackages.<system>.rust.release.bins.ronoctua-zip`.
      # Not under `packages`: these are hundreds of derivations, most of them
      # a library nobody installs.
      legacyPackages.${system} = {
        inherit rust;
        # AND AS DATA (`nix/rust/units.nix`, `index`): every unit, layer and
        # aggregate with its derivation, its measurement and what it is
        # built from — and the same with files edited by one byte. What
        # `nix run .#rebuild-cost` compares.
        rustUnits = rust.unitIndex;
        rustUnitsWith =
          files: (rust.override { crateSourceOverrides = rust.oneByteEdits files; }).unitIndex;
        # Each release profile variant, with `.#bench` over its programs:
        # what `nix run .#profile-bench` builds by name.
        profileBench = profileBench.variants;
      };

      # =================================================================
      # devShell — section 8: `direnv allow` and never again.
      # =================================================================
      devShells.${system}.default = import ./nix/devshell.nix {
        inherit gitHooks wit;
        inherit pkgs inputs;
        # THE WHOLE TABLE, and not values copied out of it — see the comment on
        # `env` over there.
        inherit (rust) atRunTime generated;
        inherit iconSuperpack cargoDirs keysCheckout;
      };

      # THE FORMATTER FORMATS THE TREE — Nix and Rust, in one command.
      #
      # # Why it is not `pkgs.nixfmt` any more
      #
      # It was, and that cost seven hours once: `nix fmt` with no argument
      # becomes a bare `nixfmt` reading STDIN, which BLOCKS. The trap was
      # written down in `AGENTS.md` and worked around by always typing
      # `nix fmt .` — while the pre-commit hook and the `check-fmt` both told
      # the person to run the version that hangs.
      #
      # A trap one documents is a trap one still falls into. This wrapper
      # removes it: with no argument it formats the whole tree, which is what
      # the person meant.
      #
      # # And Rust too, including the buffers
      #
      # `cargo fmt --all` reaches the workspace's MEMBERS, and `buffers/` is in
      # `exclude` — so the five components' formatting was checked by nothing.
      # Each is its own workspace, so each gets its own call.
      formatter.${system} = pkgs.writeShellApplication {
        name = "ronoctua-fmt";
        runtimeInputs = [
          pkgs.nixfmt
          rustToolchain
          pkgs.fd
        ];
        text = ''
          # `nix fmt` passes on whatever paths it was given. `--check` is what
          # the pre-commit hook uses: confer, do not write.
          check=false
          paths=()
          for a in "$@"; do
            case "$a" in
              --check) check=true ;;
              *) paths+=("$a") ;;
            esac
          done
          # NO PATH MEANS THE TREE. This one line is the whole difference
          # between a formatter and a seven-hour hang.
          [ ''${#paths[@]} -gt 0 ] || paths=(.)

          failed=0

          # FILES AND DIRECTORIES ARE NOT THE SAME ARGUMENT TO `fd`.
          #
          # `fd` search paths must be DIRECTORIES: given a file it prints
          # "is not a directory" and exits non-zero. That error was being
          # swallowed (`2>/dev/null || true`), the file list came back empty,
          # the Rust half was skipped because `paths[0]` was not `.`, and the
          # whole thing exited 0 — so `nix fmt tests/checks.nix` reported
          # success having formatted nothing, and `nix fmt --check` on an
          # unformatted file PASSED. Found by formatting a file twice and
          # watching the `fmt` check fail anyway.
          #
          # Mixed arguments were worse than useless: the directory was walked,
          # the file was dropped, and nothing said which half had been done.
          dirs=()
          files=()
          for a in "''${paths[@]}"; do
            if [ -d "$a" ]; then
              dirs+=("$a")
            elif [ -e "$a" ]; then
              files+=("$a")
            else
              echo "nix fmt: does not exist: $a" >&2
              failed=1
            fi
          done

          # --- Nix ---------------------------------------------------------
          nix_files=()
          if [ ''${#dirs[@]} -gt 0 ]; then
            mapfile -t nix_files < <(fd --type f --extension nix --hidden \
              --exclude .git . "''${dirs[@]}")
          fi
          for f in ''${files[@]+"''${files[@]}"}; do
            case "$f" in
              *.nix) nix_files+=("$f") ;;
            esac
          done
          if [ ''${#nix_files[@]} -gt 0 ]; then
            if [ "$check" = true ]; then
              nixfmt --check "''${nix_files[@]}" || failed=1
            else
              nixfmt "''${nix_files[@]}"
            fi
          fi

          # --- Rust --------------------------------------------------------
          # Only when a DIRECTORY was asked for: formatting a single `.nix` file
          # must not reformat every crate as a side effect.
          #
          # It used to compare `paths[0]` with the literal `.`, so `nix fmt "$PWD"`
          # and `nix fmt ./` formatted no Rust at all and looked like a
          # whole-tree run — the same silence, one argument away.
          if [ ''${#dirs[@]} -gt 0 ] && [ -f Cargo.toml ]; then
            if [ "$check" = true ]; then
              cargo fmt --all -- --check || failed=1
            else
              cargo fmt --all
            fi
            # EACH BUFFER IS ITS OWN WORKSPACE — see the `exclude` in the root
            # manifest. Without this loop their formatting is checked by nothing.
            for manifest in buffers/*/Cargo.toml; do
              [ -f "$manifest" ] || continue
              if [ "$check" = true ]; then
                cargo fmt --manifest-path "$manifest" -- --check || failed=1
              else
                cargo fmt --manifest-path "$manifest"
              fi
            done
            # AND THE BRAIN'S MODULES, which `cargo fmt` never reaches: the
            # crate loads each `module.rs` by a `#[path]` inside an `include!`
            # of a generated file, and `rustfmt` does not expand macros. The
            # sample under `tests/modules/` is loaded the same way, and so are
            # the person's fixtures under `tests/person*/`. `rustfmt`
            # on the file itself, with the edition the manifest would have told
            # it — `brain-modules-fmt` checks the same files the same way.
            #
            # A FOLDER THAT IS NOT THERE IS LEFT OUT, never silenced: with no
            # module yet `modules/` does not exist, and `fd` told about it
            # fails — the trap the file/directory split above records.
            brain_dirs=()
            for d in crates/ronoctua-brain/modules crates/ronoctua-brain/tests/modules crates/ronoctua-brain/tests/person crates/ronoctua-brain/tests/person-refused; do
              if [ -d "$d" ]; then
                brain_dirs+=("$d")
              fi
            done
            brain_sources=()
            if [ ''${#brain_dirs[@]} -gt 0 ]; then
              mapfile -t brain_sources < <(fd --type f --extension rs . "''${brain_dirs[@]}")
            fi
            for source in ''${brain_sources[@]+"''${brain_sources[@]}"}; do
              if [ "$check" = true ]; then
                rustfmt --edition 2024 --check "$source" || failed=1
              else
                rustfmt --edition 2024 "$source"
              fi
            done
          fi

          exit "$failed"
        '';
      };
    };
}
