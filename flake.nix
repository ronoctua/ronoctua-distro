{
  description = "ronoctua-distro — distribuicao NixOS sem barra, dock ou menu iniciar: so o conteudo e um rodape de duas linhas";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

    # --- compositor -----------------------------------------------------
    # Invariant 3: NO FORK, NO PATCH. Consumed as upstream, pinned to a
    # tagged release. Upstream moved from YaLTeR/niri to niri-wm/niri and
    # today uses calendar versioning.
    niri = {
      url = "github:niri-wm/niri/v26.04";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Section 7.3: a second input running the SAME contract test, to
    # discover IPC drift before the release becomes mandatory.
    niri-next = {
      url = "github:niri-wm/niri";
      inputs.nixpkgs.follows = "nixpkgs";
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
        inherit fonts;
        iconTokensFile = ./assets/tokens/icons.nix;
        trayTableFile = ./assets/icons/tray-glyphs.nix;
        lexiconFile = ./assets/lexicon/tags.en-pt.toml;
        overlayFile = ./assets/lexicon/overlay.toml;
      };

      # The Unicode symbol browser's database (`LEADER i u`). It is the
      # INTERSECTION of the UCD with the font: the renderer has no fallback, so
      # a character the font does not draw would turn into tofu.
      unicodedb = pkgs.callPackage ./nix/unicodegen.nix {
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

      # The Lucide icon browser's database (`LEADER i l`). Here the font does not
      # DELIMIT the database, it is REWRITTEN: Lucide's source codepoints
      # collide with 634 Nerd Fonts ones in the Iosevka, and the primary face wins the
      # fallback. See `nix/lucidegen.nix`.
      lucidedb = pkgs.callPackage ./nix/lucidegen.nix {
        inherit fonts;
        # The SAME lexicon as the other two: the vocabulary belongs to Portuguese, not to the
        # buffer.
        lexiconFile = ./assets/lexicon/tags.en-pt.toml;
      };

      # Icon superpackage (6.6) — a pixmap fallback for foreign
      # apps. Invariant 10: the design system does NOT consult it.
      iconSuperpack = pkgs.callPackage ./nix/icon-superpack { };

      # Tokens: from assets/tokens/*.nix into a Rust table (section 7.6).
      i18nRs = pkgs.callPackage ./nix/i18n.nix {
        i18nDir = ./assets/i18n;
      };

      tokensRs = pkgs.callPackage ./nix/tokens.nix {
        inherit glyphdb;
        colorsFile = ./assets/tokens/colors.nix;
        trayTableFile = ./assets/icons/tray-glyphs.nix;
        categoryTableFile = ./assets/icons/category-glyphs.nix;
        typographyFile = ./assets/tokens/typography.nix;
        spaceFile = ./assets/tokens/space.nix;
      };

      # The compositor's 141 actions, extracted from the PINNED niri (section 13).
      niriActions = pkgs.callPackage ./nix/niri-actions.nix {
        niri = inputs.niri.packages.${system}.niri;
        # The pinned niri's TREE, for the `default-config.kdl`: it is the
        # authoritative list of the shortcuts the compositor ships.
        niriSrc = inputs.niri;
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

      # The native buffers, for `wasm32-wasip2`. After `rustToolchain`
      # because they use the SAME toolchain — the wasm target is already in it.
      buffers = pkgs.callPackage ./nix/buffers.nix {
        inherit unicodedb lucidedb;
        inherit wit;
        inherit rustToolchain glyphdb;
      };

      # THE TERMINAL ENGINE, from a pinned Ghostty commit. In the `let` and not
      # only under `packages` because `nix/rust.nix` needs the PATH: the crate
      # that links it has to be told where it is, like every other data file this
      # project hands the Rust.
      ghosttyVt = pkgs.callPackage ./nix/ghostty-vt.nix { };

      rust = pkgs.callPackage ./nix/rust.nix {
        inherit (inputs) crane;
        inherit
          pkgs
          rustToolchain
          fonts
          tokensRs
          i18nRs
          niriActions
          ;
        bufferEditor = buffers.editor;
        inherit (pkgs) xkeyboard_config;
        bufferGlyphs = buffers.glyphs;
        bufferUnicode = buffers.unicode;
        bufferLucide = buffers.lucide;
        bufferMemory = buffers.memory;
        inherit iconSuperpack ghosttyVt;
        # The THIRD face embedded in the renderer, generated along with the database: without
        # it the 1,668 icons would come out as tofu.
        lucideFontFile = "${lucidedb.db}/NoctuaLucide.ttf";
        primaryFontFile = "${primaryFont}/NoctuaIosevkaNerdFontMono-Regular.ttf";
        niri = inputs.niri.packages.${system}.niri;
        src = ./.;
      };

      # The graphics setup for a non-NixOS host, shared by `dev` and
      # `bench`: two copies would diverge, and `bench` would start measuring a
      # path `dev` does not use.
      graphicsHost = pkgs.callPackage ./nix/graphics-host.nix { };

      # `nix run .#dev` — nested niri (section 7.12, speed 1).
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
        niri = inputs.niri.packages.${system}.niri;
      };

      devApp = pkgs.callPackage ./nix/dev.nix {
        inherit
          graphicsHost
          reloadApp
          singleFooter
          rustToolchain
          tokensRs
          i18nRs
          niriActions
          ;
        niri = inputs.niri.packages.${system}.niri;
        ronoctua-shell = rust.workspace;
        inherit fonts fontconfigConf iconSuperpack;
        lucideFontFile = "${lucidedb.db}/NoctuaLucide.ttf";
        primaryFontFile = "${primaryFont}/NoctuaIosevkaNerdFontMono-Regular.ttf";
      };

      # Section 5.2's FRAME targets. Outside `nix flake check` on purpose:
      # section 9.3 forbids measuring them under lavapipe, and a check has no GPU.
      benchFrames = pkgs.callPackage ./nix/bench-frames.nix {
        niri = inputs.niri.packages.${system}.niri;
        ronoctua-shell = rust.workspace;
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
      # The gate that sizes itself to the machine. See `nix/gate.nix` for the
      # arithmetic and where its numbers were measured.
      gateApp = pkgs.callPackage ./nix/gate.nix { inherit system; };

      distroModules = [
        ./nix/modules/base.nix
        # The table that turns `ronoctua.profile` into the eight switches. It
        # comes right after `base.nix` because that is where the option it reads
        # is declared, and before every module whose default it sets.
        ./nix/modules/profile.nix
        # The cgroup, oomd and zram policy. It comes early because what it
        # protects — the compositor and the footer — is declared later.
        ./nix/modules/memory.nix
        ./nix/modules/fonts.nix
        ./nix/modules/niri.nix
        ./nix/modules/keyboard.nix
        ./nix/modules/icons.nix
        ./nix/modules/terminal.nix
        ./nix/modules/network.nix
        ./nix/modules/syncthing.nix
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
              inherit (inputs) niri;
              # The shell's binary: the niri module generates the chord
              # binds from its command table.
              shell = rust.workspace;
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
        fontconfig-completo = fontconfigConf.fullConf;
        fallback-chain = fontconfigConf.chainJson;

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

        # Merged icon theme, with manifest and coverage report.
        noctua-icon-theme = iconSuperpack;

        # Rust token table, generated from the .nix files.
        tokens-rs = tokensRs;
        niri-actions = niriActions;
        buffer-example = buffers.example;
        buffer-glyphs = buffers.glyphs;
        buffer-unicode = buffers.unicode;
        i18n-rs = i18nRs;

        # The shell: the two-line footer.
        ronoctua-shell = rust.workspace;

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
                inherit fonts fontconfigConf;
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
                # time — the shell, niri from the pinned input, `libghostty-vt`
                # and its Zig toolchain, the four wasm buffers. They are the
                # compilations this host's own `earlyoom` kills for weight.
                #
                # The reduced SYSTEM's whole closure would have fitted here too,
                # and it was measured: the ISO came to 4380 MiB against a
                # ceiling of 4096. Stock nixpkgs is 3,5 GiB of that, and it is
                # the part a cache serves in minutes. So the ISO carries what
                # only this repository can produce, and the target downloads the
                # rest instead of compiling it.
                seed = [
                  # The workspace, and with it — all by baked store path —
                  # the icon superpack, `libghostty-vt`, the four wasm buffers
                  # and niri itself. 1,97 GiB of closure that no cache has.
                  rust.workspace
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
                # evaluation needs nixpkgs, disko and niri, which `flake.lock`
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
          iconSuperpack
          rust
          buffers
          sbKeys
          devApp
          reloadApp
          singleFooter
          wit
          tokensRs
          niriActions
          ;
      };

      # =================================================================
      # apps — section 8. Flake apps, never a justfile or a Makefile.
      # =================================================================
      apps.${system} = {
        # Nested niri + footer. The loop where 90% of the UI work happens
        # (section 7.12, speed 1).
        dev = {
          type = "app";
          program = "${devApp}/bin/ronoctua-dev";
          meta = {
            description = "niri aninhado + rodape, para o loop de trabalho de UI";
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
            description = "instantaneo do estado quando os atalhos travam";
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
            description = "nix flake check dimensionado pela RAM da maquina";
            platforms = [ system ];
          };
        };

        # A complete system in a VM.
        vm = mkScriptApp "ronoctua-vm" "sobe o sistema completo em VM" ''
          exec ${self.nixosConfigurations.vm.config.system.build.vm}/bin/run-nixos-vm "$@"
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
          mkScriptApp "ronoctua-fonts-release" "empacota as fontes para um GitHub Release e imprime os hashes"
            ''
              root="''${1:-}"
              if [ -z "$root" ] || [ ! -d "$root/noctua-iosevka" ]; then
                echo "uso: nix run .#fonts-release -- /caminho/com/noctua-iosevka" >&2
                exit 1
              fi
              out=$(mktemp -d -t ronoctua-fonts-release-XXXXXX)
              echo "empacotando em $out"
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
              echo "notas do release (em ingles, prontas): $notes"
              echo
              echo "Suba os quatro como assets de um Release em"
              echo "  https://github.com/ronoctua/ronoctua-distro/releases/new"
              echo "com uma tag PROPRIA das fontes (fonts-v1), que nao se confunda"
              echo "com a do ISO, e o corpo vindo do arquivo acima. Depois rode"
              echo "  nix flake update fonts-ttf fonts-ttf-nerdfonts fonts-woff2 fonts-woff2-nerdfonts"
            '';

        # Builds the ISO and says where it is.
        iso = mkScriptApp "ronoctua-iso" "gera o ISO do instalador" ''
          out=$(nix build --no-link --print-out-paths .#iso)
          echo "ISO gerado em:"
          find "$out/iso" -name '*.iso' -printf '  %p  (%s bytes)\n'
        '';

        # The real installer. Without `--execute` it does a DRY RUN: it shows the
        # `host.nix` it would generate and the steps it would run, and exits. That is the default
        # on purpose — the first step erases an entire disk.
        install = mkScriptApp "ronoctua-install" "instala a versao de producao" ''
          exec ${rust.workspace}/bin/ronoctua-install "$@"
        '';

        # Measures performance on real hardware (section 9.3 of the plan: section 5.2's
        # targets are NEVER measured under lavapipe).
        bench = mkScriptApp "ronoctua-bench" "mede os alvos da secao 5.2 em hardware real" ''
          echo "ronoctua-bench — secao 5.2"
          echo
          echo "== trabalho de CPU do frame =="
          ${rust.workspace}/bin/ronoctua-bench-layout "''${1:-20000}"
          echo
          echo "== moldagem de texto =="
          ${rust.workspace}/bin/ronoctua-bench-shaping
          echo
          echo "== instanciacao de componente =="
          ${rust.workspace}/bin/ronoctua-bench-instance \
            ${buffers.glyphs}/lib/ronoctua_buffer_glyphs.wasm 200
          echo
          echo "== alvos de frame, em hardware real =="
          # Section 9.3 demands they be measured here and never under lavapipe. It needs a
          # graphics session; without one, the command says what is missing instead of
          # inventing a number.
          if [ -n "''${WAYLAND_DISPLAY:-}''${DISPLAY:-}" ]; then
            ${benchFrames}/bin/ronoctua-bench-frames "''${2:-6}" "''${3:-15}"
          else
            echo "Sem sessao grafica: os alvos de frame nao foram medidos."
            echo "Rode este comando de dentro de uma sessao Wayland ou X."
          fi
        '';
      };

      # =================================================================
      # devShell — section 8: `direnv allow` and never again.
      # =================================================================
      devShells.${system}.default = import ./nix/devshell.nix {
        inherit
          gitHooks
          wit
          tokensRs
          i18nRs
          niriActions
          ;
        inherit pkgs inputs;
        inherit (rust) regularFont emojiFont lucideFont;
        bufferGlyphs = buffers.glyphs;
        bufferUnicode = buffers.unicode;
        bufferLucide = buffers.lucide;
        bufferMemory = buffers.memory;
        inherit iconSuperpack ghosttyVt;
      };

      formatter.${system} = pkgs.nixfmt;
    };
}
