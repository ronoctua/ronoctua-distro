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
        overlays = [ inputs.rust-overlay.overlays.default ];
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
          downloads = atlasData.sources ++ [ pkgs.bibata-cursors.src ];
        }
      }/bin/ronoctua-keep-sources";

      # The glance's places (`LEADER d`). One declaration, and the build
      # refuses a zone `zone1970.tab` does not know — see nix/world.nix.
      worldRs = pkgs.callPackage ./nix/world.nix {
        worldFile = ./assets/world.nix;
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

      i18nRs = pkgs.callPackage ./nix/i18n.nix {
        i18nDir = ./assets/i18n;
        inherit languages;
      };

      # THE THEMES, imported ONCE, like the languages. And the palettes, SOLVED
      # once for all of them: `assets/tokens/colors.nix` is recipes, and this
      # derivation is where they become values (section 7.6, one wall earlier).
      themeSpec = import ./assets/themes/themes.nix;
      # THE ORDER AND THE DEFAULT ARE TWO FACTS — see that file. Nothing
      # here may take `builtins.head themes` to mean "the default" again.
      themes = themeSpec.order;
      defaultTheme = themeSpec.default;
      themed = pkgs.callPackage ./nix/themes.nix {
        inherit themes;
        themesDir = ./assets/themes;
        recipesFile = ./assets/tokens/colors.nix;
      };

      # THE CURSORS, one Xcursor theme per cursor colour. See nix/cursors.nix.
      cursors = pkgs.callPackage ./nix/cursors.nix { inherit themed defaultTheme; };

      tokensRs = pkgs.callPackage ./nix/tokens.nix {
        inherit glyphdb themed defaultTheme;
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

      # The Rust workspace, built by crane (section 7.12).
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        targets = [ "wasm32-wasip2" ];
      };
      # DISPOSABLE Secure Boot keys, for the VM tests only.
      sbKeys = pkgs.callPackage ./nix/testing/sb-keys.nix { };

      # The WIT directory with the generated token enum inside (section 7.6).
      wit = pkgs.callPackage ./nix/wit.nix {
        inherit tokensRs;
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
        inherit languages;
        inherit unicodedb lucidedb;
        inherit wit;
        inherit rustToolchain glyphdb;
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

      # THE BROWSER EXTENSION, with its texts and colours GENERATED from the
      # same `assets/i18n` and `assets/tokens` everything else reads. Invariants
      # 8 and 9 do not stop at the browser's edge.
      extension = pkgs.callPackage ./nix/extension.nix {
        # The DEFAULT theme's palette, as a flat `key -> 0xRRGGBBAA` map. The
        # extension cannot see the person's choice, so it wears the default.
        colorsJson = "${themed}/${defaultTheme}.json";
        spaceFile = ./assets/tokens/space.nix;
        i18nDir = ./assets/i18n;
        inherit languages;
        source = ./extension;
      };

      rust = pkgs.callPackage ./nix/rust.nix {
        inherit (inputs) crane;
        inherit
          pkgs
          rustToolchain
          fonts
          tokensRs
          i18nRs
          mimeDb
          notificationsRs
          bluetoothProfilesRs
          audioNamesRs
          syncthingOptionsRs
          torOptionsRs
          atuinOptionsRs
          keyboardsRs
          updatesRs
          cacheRs
          syntaxRs
          compositorDeeds
          ;
        inherit (pkgs) xkeyboard_config;
        bufferGlyphs = buffers.glyphs;
        bufferUnicode = buffers.unicode;
        bufferLucide = buffers.lucide;
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
          ;
        primaryFontFile = "${primaryFont}/NoctuaIosevkaNerdFontMono-Regular.ttf";
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

      # THE UPDATE TABLE, generated from `assets/updates.nix` — the same shape
      # `assets/notifications.nix` has: one declaration, and from it the Rust
      # table, the text keys both languages owe, and the refusal of a field
      # that does not exist.
      updatesTable = pkgs.callPackage ./nix/updates.nix {
        updatesFile = ./assets/updates.nix;
      };
      # The Rust half of it, named on its own because that is what goes into the
      # compilation's environment — the whole attrset also carries the negative
      # fixture and the runner factory, neither of which belongs in a build.
      updatesRs = updatesTable.table;

      # THE CACHE POLICY, generated from `assets/cache.nix` — four numbers the
      # brain, the sweep and the file manager all have to agree about, and one
      # relationship between two of them that the build refuses to let go
      # wrong. No generator script: four integers and a word do not need one.
      cacheRs = pkgs.callPackage ./nix/cache.nix {
        cacheFile = ./assets/cache.nix;
      };

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
          ;
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
        # THE WHOLE TABLE THE COMPILATION GETS, and not six values copied out of
        # it: the loop compiles the binary itself, so what it does not export the
        # binary does not have. See `nix/dev.nix`'s `bakedExports`.
        inherit (rust) atRunTime generated;
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
      slowNames = import ./tests/slow.nix;
      mostApp = pkgs.callPackage ./nix/gate.nix {
        inherit system keepSources;
        name = "ronoctua-most";
        what = "targets";
        targets = lib.subtractLists slowNames (builtins.attrNames self.checks.${system});
        left = slowNames;
      };

      distroModules = [
        ./nix/modules/base.nix
        # The table that turns `ronoctua.profile` into the eight switches. It
        # comes right after `base.nix` because that is where the option it reads
        # is declared, and before every module whose default it sets.
        ./nix/modules/profile.nix
        # The cgroup, oomd and zram policy. It comes early because what it
        # protects — the compositor and the footer — is declared later.
        ./nix/modules/memory.nix
        ./nix/modules/rescue.nix
        # AFTER the rescue console, and the order is load-bearing: the update
        # sequence's `system-rebuild` step BORROWS `ronoctua-rescue-rebuild`
        # rather than declaring a second `nixos-rebuild switch --flake
        # /etc/nixos#<person>`. One of those in the repository, not two, and an
        # `assertions` entry says so if the rescue console is ever turned off.
        ./nix/modules/update.nix
        # AFTER the update module, because the brain's one task asks systemd to
        # start the units that module declares. Nix does not care about the
        # order; a person reading the list does.
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
        ./nix/modules/home.nix
        ./nix/modules/cli.nix
        ./nix/modules/archives.nix
        ./nix/modules/media.nix
        ./nix/modules/dev.nix
        ./nix/modules/nixpkgs-config.nix
        ./nix/modules/browser.nix
        ./nix/modules/emacs.nix
        inputs.home-manager.nixosModules.home-manager
        {
          # Injects the packages built by this flake into the modules.
          _module.args = {
            ronoctua = {
              inherit fonts fontconfigConf iconSuperpack;
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
              # `rust.shell` and not `rust.workspace`: it is the wrapped one that
              # knows where the five components are, and the unit's `ExecStart`
              # comes from here. See `nix/rust.nix`.
              shell = rust.shell;
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
              # The cache sweep, extracted for the brain's reason and one of
              # its own: a program whose job is `unlink` must not have the
              # whole shell in its closure.
              cacheSweep = rust.cacheSweep;
              # And the one that empties the trash, extracted for the same
              # reason and one sharper: what it deletes is the only copy.
              emptyTrash = rust.emptyTrash;
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
        # THE SEVEN COMPONENTS, each buildable on its own.
        #
        # Three of them used to be missing here, and it showed the day the
        # nested loop learned to rebuild a buffer: `nix build .#buffer-memory`
        # did not exist, so the newest buffers were the ones the loop could not
        # reload. What names them now is `nix/buffers.nix`'s own table.
        buffer-example = buffers.example;
        buffer-glyphs = buffers.glyphs;
        buffer-unicode = buffers.unicode;
        buffer-lucide = buffers.lucide;
        buffer-memory = buffers.memory;
        buffer-keyboard = buffers.keyboard;
        buffer-transfers = buffers.transfers;
        buffer-format = buffers.format;
        buffer-brain = buffers.brain;
        i18n-rs = i18nRs;

        # THE UPDATE SEQUENCE, in three pieces, exposed so a check can reach
        # them without evaluating a whole NixOS system.
        #
        # `updates-negative` is the fixture that must FAIL to build: it is a
        # table with a step that installs, and a guard nobody proves is a guard
        # that stopped working quietly. The same shape `nix/glyphgen.nix` and
        # the icon table already use.
        updates-rs = updatesTable.table;
        updates-negative = updatesTable.negativeFixture;
        # And the same pair for the highlighter: `syntax-negative` is a table
        # in which one language steals another's type and another claims prose,
        # and it must FAIL to build.
        syntax-rs = syntaxTable.table;
        syntax-negative = syntaxTable.negativeFixture;
        update-runners = self.nixosConfigurations.noctua.config.ronoctua.update.package;

        # The shell: the two-line footer.
        ronoctua-shell = rust.shell;

        # THE SCREENSHOTS, on a real compositor. `nix build .#shots` leaves the
        # PNGs in the store — see `nix/shots.nix` for why it is a VM and not a
        # script on somebody's machine.
        shots = pkgs.callPackage ./nix/shots.nix {
          inherit
            scroll
            fonts
            themed
            themes
            defaultTheme
            ;
          ronoctua-shell = rust.shell;
          # The daemon, so the brain's line can be photographed. It is the only
          # region of the footer that no buffer draws, so `nix run .#png`
          # cannot reach it and a real compositor is the only vehicle.
          ronoctua-brain = rust.brain;
          # AND THE QUEUE, because a drop ends in it. `files::queued` hands the
          # work to `ronoctua-transfers` over its socket, so a scene that drags
          # a file onto a folder and then asks whether the file MOVED is asking
          # a question only the daemon can answer.
          ronoctua-transfers = rust.transfers;
        };

        # AND ONE PACKAGE PER SCENE, so a delivery can photograph the screen it
        # changed instead of the whole system.
        #
        # The owner read the cost off the deliveries — *"os mesmos screenshots
        # dos mesmos locais estão sendo gerados toda vez que você testa algo
        # novo"* — and he is right about the arithmetic: a full run is four
        # minutes and thirty-five pictures, of which a delivery changes one or
        # two. `nix build .#shots-manager` is the file manager and nothing
        # else. `.#shots` above is still every scene, and is still what proves
        # the whole screen.
      }
      // lib.listToAttrs (
        map (scene: {
          name = "shots-${scene}";
          value = pkgs.callPackage ./nix/shots.nix {
            inherit
              scroll
              fonts
              themed
              themes
              defaultTheme
              ;
            only = [ scene ];
            ronoctua-shell = rust.shell;
            ronoctua-brain = rust.brain;
            ronoctua-transfers = rust.transfers;
          };
        }) (map (s: s.id) (import ./nix/shots-scenes.nix))
      )
      // {

        iso = self.nixosConfigurations.installer.config.system.build.isoImage;
      };

      # =================================================================
      # nixosConfigurations
      # =================================================================
      nixosConfigurations = {
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
                # `rust.installer`, and NOT `rust.workspace`: the workspace
                # derivation holds every binary, and a derivation references
                # what its bytes mention — `ronoctua-shell` was putting Zig,
                # clang, LLVM and the icon superpack on an ISO with no desktop.
                installerCrate = rust.installer;

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
                  # The workspace, and with it — by baked store path —
                  # `libghostty-vt`. 1,97 GiB of closure that no cache has.
                  #
                  # `rust.shell` and NOT `rust.workspace`: the five wasm buffers
                  # used to arrive by carriage, exactly as the superpack below
                  # did, and cutting that reference is what stopped a buffer
                  # edit from recompiling everything (`nix/rust.nix`). What
                  # carries them now is the wrapper — and if it ever stops,
                  # `iso-carries-the-seed` names the five one by one.
                  rust.shell
                  # NOT in the workspace's closure: the binary embeds only the
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
                ]
                # AND THE FLAKE'S OWN INPUTS, as sources.
                #
                # To build the machine's own top — `etc`, the initrd, the
                # hostname, the disk — Nix has to EVALUATE the flake, and
                # evaluation needs nixpkgs, disko and the compositor's source, which `flake.lock`
                # names but does not contain.
                ++ builtins.filter (p: p != null) (map (i: i.outPath or null) (builtins.attrValues inputs));
                # The flake's own tree, for the ISO to carry in
                # `/etc/nixos` — see the comment over there.
                flake = self;
              };
            }
          ];
        };

        # THE PRODUCTION SYSTEM. This is the name the installer builds:
        # `disko --flake …#noctua` and `nixos-install --flake …#noctua`.
        #
        # It did not exist. The installer's plan had been naming it since milestone 8, and
        # nothing flagged it — DRY RUN mode prints the commands without running them, and
        # `nix flake check` does not evaluate an attribute nobody references.
        #
        # `host.nix` is OPTIONAL: on the first `disko` run it has not been
        # written yet, and without `optional` evaluation would fail before the disk
        # existed. When it does exist, it brings `ronoctua.disk`, the hostname and the
        # `resume_offset`.
        noctua = nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = { inherit inputs; };
          modules =
            distroModules
            ++ [
              ./nix/modules/host.nix
              inputs.nixos-facter-modules.nixosModules.facter
            ]
            ++ nixpkgs.lib.optional (builtins.pathExists ./host.nix) ./host.nix;
        };

        # THE SEED — the reduced system, and it is not what gets installed.
        #
        # `nixos-install` still targets `#noctua` in both profiles; the profile
        # is a line in `host.nix`, so a machine can change its mind with a
        # rebuild instead of a reinstall. What this attribute is for is being
        # BUILT, so its closure can travel on the ISO: everything the reduced
        # profile needs, already realised, so the target machine compiles
        # nothing. See `nix/installer/iso.nix`.
        #
        # The same modules as `noctua` — including `host.nix`, so the store
        # paths line up with the ones the installed system will ask for.
        minimal = nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = { inherit inputs; };
          modules = distroModules ++ [
            ./nix/modules/host.nix
            inputs.nixos-facter-modules.nixosModules.facter
            { ronoctua.profile = "minimal"; }
          ];
        };

        # Development VM — a complete system, without real hardware.
        vm = nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = { inherit inputs; };
          modules = distroModules ++ [ ./nix/modules/vm.nix ];
        };
      };

      nixosModules = {
        default = {
          imports = distroModules;
        };
        fonts = ./nix/modules/fonts.nix;
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
          updatesTable
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
          syncthingOptionsRs
          torOptionsRs
          atuinOptionsRs
          keyboardsRs
          compositorDeeds
          torrentShim
          extension
          potProvider
          rustToolchain
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
        # product of jobs and cores, builds the three heavy derivations one at a
        # time, and — if `rustc` dies anyway — says who killed it.
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
          echo "== the short loop: a chosen subset =="
          ${keepSources}
          # THE LIST COMES FROM `tests/quick.nix`, not from here.
          #
          # Written here it would be the only copy, and nothing could check it.
          # Coming from a file that `quick-names-exist` also reads, a renamed
          # check is reported instead of dropping out of the loop in silence.
          nix build --no-link --keep-going -L \
            ${
              lib.concatMapStringsSep " \\\n            " (n: ".#checks.${system}.${n}") (
                import ./tests/quick.nix
              )
            } \
            "$@"
          echo
          echo "OK — and this is a SUBSET, not a category."
          echo "     They are ten chosen for the editing loop. It is not complete and"
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
          # host's pointers move nothing on the host. They come back when the
          # VM closes. The keyboard is left alone.
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
            else
              pass+=("$a")
            fi
          done
          if [ "''${pointers:-}" = yes ]; then
            found=0
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
              # AND QEMU'S OWN step aside: grabbed, the host's pointers no
              # longer reach its emulated tablet, which would be a cursor no
              # hand moves. `nix/modules/vm.nix` is the reader of this flag.
              export QEMU_KERNEL_PARAMS="''${QEMU_KERNEL_PARAMS:-} ronoctua_pointers"
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
                echo "Licence: SIL Open Font License 1.1 (\`LICENSE-OFL.txt\` at the"
                echo "repository root). The ronoctua-distro code is MIT; these files are not."
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

        # Builds the ISO and says where it is.
        iso = mkScriptApp "ronoctua-iso" "builds the installer's ISO" ''
          out=$(nix build --no-link --print-out-paths .#iso)
          echo "the ISO came out at:"
          find "$out/iso" -name '*.iso' -printf '  %p  (%s bytes)\n'
        '';

        # The real installer. Without `--execute` it does a DRY RUN: it shows the
        # `host.nix` it would generate and the steps it would run, and exits. That is the default
        # on purpose — the first step erases an entire disk.
        install = mkScriptApp "ronoctua-install" "installs the production version" ''
          exec ${rust.workspace}/bin/ronoctua-install "$@"
        '';

        # Measures performance on real hardware (section 9.3 of the plan: section 5.2's
        # targets are NEVER measured under lavapipe).
        bench = mkScriptApp "ronoctua-bench" "measures section 5.2's targets on real hardware" ''
          echo "ronoctua-bench — section 5.2"
          echo
          echo "== the frame's CPU work =="
          ${rust.workspace}/bin/ronoctua-bench-layout "''${1:-20000}"
          echo
          echo "== text shaping =="
          ${rust.workspace}/bin/ronoctua-bench-shaping
          echo
          echo "== component instantiation =="
          ${rust.workspace}/bin/ronoctua-bench-instance \
            ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm 200
          echo
          echo "== frame targets, on real hardware =="
          # Section 9.3 demands they be measured here and never under lavapipe. It needs a
          # graphics session; without one, the command says what is missing instead of
          # inventing a number.
          if [ -n "''${WAYLAND_DISPLAY:-}''${DISPLAY:-}" ]; then
            ${benchFrames}/bin/ronoctua-bench-frames "''${2:-6}" "''${3:-15}"
          else
            echo "No graphical session: the frame targets were not measured."
            echo "Run this command from inside a Wayland or X session."
          fi
        '';

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
        # The graphics environment is the checks' own: lavapipe, Mesa's software
        # Vulkan rasteriser. That is not a performance choice — it is what makes
        # the command answer the same on this machine, on one with no GPU and
        # inside a check. Section 9.3 still holds: frame time is NEVER measured
        # here.
        png = mkScriptApp "ronoctua-png" "draws a buffer into a PNG, with no VM" ''
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
          # It is set here and not inherited: `rust.workspace` is deliberately
          # unwrapped (the wrapper is `rust.shell`, and it wraps the footer
          # only), so this launcher is one of the nine that has to say it.
          export RONOCTUA_COVERAGE_FONTS=${rust.atRunTime.RONOCTUA_COVERAGE_FONTS}
          exec ${rust.workspace}/bin/ronoctua-render-png "$@"
        '';
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
        inherit iconSuperpack;
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
          fi

          exit "$failed"
        '';
      };
    };
}
