# Building the Rust workspace inside Nix — section 7.12.
#
# CARGO PLANS, NIX BUILDS (`nix/rust/`, nix-design §1): cargo computes the
# build plan once (`nix/rust/plan.json`, `nix run .#rust-plan`), and every
# compilation unit of it — each library, build script, program, test
# harness, test run, clippy and rustfmt of ours, and each third-party crate —
# is a derivation of its own, in three flavours (`release`, `loop`, `test`).
# An edited line rebuilds the units built from it and nothing else; that is
# the granularity `tests/build-granularity.nix` holds. crane is only what
# vendors the sources and builds the planner and the two third-party layers
# the profile bench measures on (`nix/rust/layers.nix`): it compiles no crate
# of ours.
{
  lib,
  stdenv,
  pkgs,
  crane,
  rustToolchain,
  pkg-config,
  makeBinaryWrapper,
  wayland,
  libxkbcommon,
  vulkan-loader,
  libGL,
  fonts,
  noto-fonts-color-emoji,
  tokensRs,
  # THE PALETTES (`nix/themes.nix`): `palettes.bin`, read at RUN time by every
  # binary that paints, through `RONOCTUA_THEMES` — never a compilation input.
  themed,
  # The notification catalogue and filters (section 4.2), generated from
  # `assets/notifications.nix` in the same shape as the tokens.
  notificationsRs,
  bluetoothProfilesRs,
  audioNamesRs,
  syncthingOptionsRs,
  torOptionsRs,
  atuinOptionsRs,
  atuin,
  keyboardsRs,
  keyd,
  # The cache policy, generated from `assets/cache.nix`.
  cacheRs,
  # The programs the installer offers to leave out, from the ledger's `offered`.
  offeredRs,
  # The camera's effects, generated from `assets/camera-effects.nix`.
  cameraEffectsRs,
  # The person-segmentation model (`nix/segment-model.nix`), read at run time.
  segmentModel,
  # The two hand models (`nix/hand-model.nix`), a directory, the same way.
  handModels,
  syntaxRs,
  mimeDb,
  # THE TEXTS (`nix/i18n.nix`): the merged `<lang>.ftl`, read at RUN time by
  # every binary that speaks (`RONOCTUA_TEXTS`) — and each crate's `key`
  # module, written at evaluation from the names of its own fragment alone
  # (`texts.keys`, `texts.keysOf`), which is all a compilation reads of them.
  texts,
  compositorDeeds,
  coreutils,
  util-linux,
  xdg-utils,
  # `nmcli`, for importing a WireGuard or OpenVPN file — the one network
  # thing the bus does not do. It comes with NetworkManager, which this
  # system already installs.
  networkmanager,
  # The settings portal's own store, for the foreign toolkits' text size.
  dconf,
  # WirePlumber's client, for the ONE thing BlueZ cannot answer: which audio
  # profile a device can be put in. See `RONOCTUA_WPCTL` below.
  wireplumber,
  # PipeWire's own dump, for reading what those profiles ARE.
  pipewire,
  systemd,
  btop,
  bottom,
  ncurses,
  fontconfig,
  zsh,
  polkit,
  nix,
  wl-clipboard,
  xkeyboard_config,
  glibcLocales,
  ghosttyVt,
  torrentShim,
  tzdata,
  bufferMemory,
  bufferKeyboard,
  bufferTransfers,
  bufferFormat,
  bufferBrain,
  bufferExample,
  iconSuperpack,
  lucideFontFile,
  marksFontFile,
  # The faces the declared fallback chain names, and the chain itself as Rust.
  # The FONTS travel on the wrapper (read at run time, 54 MB, and a store path
  # in the compiler's environment rebuilds all thirty-three crates); the chain
  # is baked, because it is code.
  atlasData,
  coverageFonts,
  chainRs,
  widthRs,
  worldRs,
  broadcastRs,
  tonguesRs,
  primaryFontFile,

  # The brain's modules as Rust, generated from their folders — see
  # `nix/brain-modules-rs.nix`: the daemon's part (the registry, the person's
  # manifests) and the library's (the distro's manifests).
  brainModulesRs,
  brainManifestsRs,
  # AND WHAT THE FOOTER DRAWS OF THEM (`nix/brain-catalogue.nix`): the
  # modules' marks and glyphs, actions, events and notifications — read at RUN
  # time by every binary that draws them (`RONOCTUA_BRAIN_CATALOGUE`), never a
  # compilation input.
  brainCatalogue,

  # The offline English dictionary (`nix/wordsgen.nix`), for the tests only:
  # see `RONOCTUA_WORDS` in the tests' environment below.
  wordsData,
  # The three icon tables in one directory (`nix/iconset.nix`), read at RUN time.
  iconset,
  # The calculator's unit table (`nix/unitsgen.nix`), compiled in.
  unitsRs,

  # What the belongings keep (`nix/belongings.nix`), compiled in.
  belongingsRs,

  src,

  # AN EDIT, SIMULATED, for the granularity checks and `nix run
  # .#rebuild-cost` (`rust.override { crateSourceOverrides = …; }`): see
  # `nix/rust/units.nix`. Empty in every build of the flake.
  crateSourceOverrides ? { },
}:

let
  # WHERE TOR LISTENS AND WHERE ITS FRAGMENT GOES, from the one file
  # `nix/modules/tor.nix` reads too. See `nix/tor.nix` for why the two
  # sides may not each carry their own copy of the number.
  torFacts = import ./tor.nix;
  # `crane.mkLib` takes the whole `pkgs`, not an attrset with `system`.
  craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

  # The face embedded in the binary (see ronoctua-render): the installer runs on
  # a TTY before any GPU exists, and a minimal environment may not have
  # share/fonts mounted.
  #
  # It is the Iosevka WITHOUT the 75 codepoints whose default presentation is
  # emoji: the primary face wins the fallback chain, and with them it drew `UP!`
  # in monochrome next to a coloured `ATM`. See `nix/primary-font.nix`.
  regularFont = primaryFontFile;
  # THE EMOJI FACE, embedded alongside the Iosevka.
  #
  # The Iosevka draws 75 of the 1,184 single-codepoint emoji (6%), and none of the
  # 2,760 sequences — the rest would come out as tofu. This face is the only thing
  # on screen with colour outside the tokens, and the reason is that emoji are
  # CONTENT, like the clipboard's image preview, and not chrome.
  #
  # CBDT/CBLC bitmaps, which is the format `swash` decodes; `glyphon` already
  # has a separate colour atlas for it. See `crates/ronoctua-render/src/text.rs`.
  emojiFont = "${noto-fonts-color-emoji}/share/fonts/noto/NotoColorEmoji.ttf";

  # THE LUCIDE FACE, generated by `lucidegen.nix` rather than taken from a package.
  #
  # It is the Lucide font with its `cmap` REWRITTEN: the source codepoints
  # (`U+E038..U+E6BB`) collide with 634 of the Nerd Fonts ones inside the Iosevka,
  # and the primary face wins the fallback chain — the icons would come out drawn
  # as some other symbol, with no tofu to give it away. See `nix/lucidegen.nix`.
  lucideFont = lucideFontFile;

  # THE MARKS FACE, and it is the only one this repository DRAWS.
  #
  # A mark the interface has to draw and that exists in none of the other four
  # faces has nowhere else to go: invariant 10 forbids a pixmap, invariant 9
  # forbids a hard-coded codepoint. It is built out of `assets/icons/marks.nix`
  # at `U+F3000`, past Nerd Fonts and past Lucide, and the generator checks the
  # range is free in all three before writing. See `nix/marksgen.nix`.
  marksFont = marksFontFile;

  # WHAT A UNIT'S SOURCES ARE is not a filter of the whole tree any more: each
  # target's fileset (`nix/rust/units.nix`) is its crate's `Cargo.toml`,
  # `build.rs`, its sources and what its `data` names — the `.wgsl`, the
  # `.wit`, a test's fixtures —, so a file of one crate is no input of another
  # and `buffers/` (a workspace of its own each, `nix/buffers.nix`) is no
  # input of any.
  #
  # THE PLAYER'S ENGINE: libmpv, over the SAME ffmpeg-full the rest of the
  # system decodes with — libopenmpt, gme, modplug, bluray, dvdnav, dav1d and
  # libass are what make it open what DeaDBeeF and VLC open. The stock mpv is
  # built against the smaller ffmpeg.
  mpvFull = pkgs.mpv-unwrapped.override { ffmpeg = pkgs.ffmpeg-full; };

  # THE FEATURES EVERY BUILD TURNS ON, from the one place that says them:
  # `[workspace.metadata.ronoctua] build-features` in the root `Cargo.toml`
  # (the why is written there). The plan reads it, and through the plan the
  # layers (whose requests carry the features it unified) and every unit.
  buildFeatures =
    (fromTOML (builtins.readFile (src + "/Cargo.toml"))).workspace.metadata.ronoctua.build-features;

  # WHAT EVERY COMPILATION IS GIVEN, ours and the third-party crates' alike
  # (the units, `unitArgs`; a third-party build script, `depBase`; and the
  # profile bench's layers, `layerArgs`): the C libraries the dependencies link
  # and how the TLS stack's C is built — and nothing of `generated`, which a
  # unit takes only as its crate declares. A `RONOCTUA_*` here would be in
  # every derivation of the build, and a value nothing in most of them reads
  # would rebuild them all when it moved (`deps-layer-is-lean`,
  # `units-see-only-what-they-read`).
  buildBase = {
    nativeBuildInputs = [ pkg-config ];
    # These DO belong here: the dependencies themselves link against them —
    # `wayland-sys` and `khronos-egl` do not build without the libraries.
    buildInputs = [
      wayland
      libxkbcommon
      vulkan-loader
      libGL
      # THE CATALOGUES' DATABASE (the player's channels, video-metrics): rusqlite
      # links the system's SQLite, which carries FTS5, instead of compiling a
      # bundled copy — no C build in the dependency layer.
      pkgs.sqlite
      # THE PASSWORD CHECK of `ronoctua-pam`, which links libpam and talks to it
      # through its conversation callback; the password never crosses a process.
      pkgs.linux-pam
      # ── anchor: L02 ──
    ];
    # ── anchor: L02 ──
    # THE TLS STACK'S C, PINNED TO ONE WAY OF BUILDING IT. `aws-lc-sys`
    # (under rustls and our own digests, `Cargo.toml`) picks its builder by
    # what it finds: its `cc` one with the pregenerated bindings when nothing
    # asks for bindgen, cmake otherwise — and, before either, an AWS-LC it
    # can DETECT on the system. Both are said here, so the layer builds the
    # same everywhere and a builder that cannot is a loud failure rather than
    # a different library: `cc` (no cmake, no Go, no clang in this closure),
    # and never a system copy. The release profile's `-ffile-prefix-map`,
    # which that builder adds itself, is what keeps the vendored source's
    # store path out of the binaries.
    AWS_LC_SYS_CMAKE_BUILDER = "0";
    AWS_LC_SYS_USE_SYSTEM = "0";
  };

  # WHAT ONLY OUR OWN CRATES READ.
  #
  # Every one of these reaches the Rust through `option_env!` or a `build.rs`
  # of ours — see the comment on each. A third-party crate has never heard of
  # any of them, which is why the dependency layer above does not get them.
  generated = {
    # The two generators. ronoctua-tokens's `build.rs` fails loudly if the
    # first is missing, instead of generating a fallback that would mask the error.
    RONOCTUA_TOKENS_RS = "${tokensRs}/tokens.rs";
    RONOCTUA_NOTIFICATIONS_RS = "${notificationsRs}/notifications.rs";
    # THE SERVICE CATALOGUE, read by `ronoctua-bluetooth`'s build script for
    # the same reason: a table in Nix, a `match` nowhere.
    RONOCTUA_BLUETOOTH_PROFILES_RS = "${bluetoothProfilesRs}/profiles.rs";
    # OUR OWN WORD FOR A SOUND PORT, where we have one. ALSA calls a socket
    # `[Out] Speaker` and nobody upstream translates it; what is not in the
    # table draws with PipeWire's description, which is why this is a small
    # file and not an attempt at every port that exists.
    RONOCTUA_AUDIO_NAMES_RS = "${audioNamesRs}/names.rs";
    # EVERY OPTION SYNCTHING EXPOSES, and which of them the flake owns. The
    # module PUTs twelve of them into the running service at each activation,
    # so offering those as editable would be offering a key whose effect the
    # next `nixos-rebuild` undoes in silence — invariant 6, on the screen.
    RONOCTUA_SYNCTHING_OPTIONS_RS = "${syncthingOptionsRs}/options.rs";
    RONOCTUA_TOR_OPTIONS_RS = "${torOptionsRs}/options.rs";
    RONOCTUA_ATUIN_OPTIONS_RS = "${atuinOptionsRs}/options.rs";
    # THE BINARY, by store path: `LEADER s h` runs it from a shell process
    # whose `PATH` is whatever the session has, and a bare name fails far
    # from its cause.
    RONOCTUA_ATUIN = "${atuin}/bin/atuin";
    # THE KEYBOARD VOCABULARY, and the leader's own rule with it: the module
    # that configures the daemon and the shell that renders per-keyboard files
    # both read `assets/keyboards.nix`, so the rule cannot drift between them.
    RONOCTUA_KEYBOARDS_RS = "${keyboardsRs}/keyboards.rs";
    # `keyd check`, which is the daemon's own parser. It is asked BEFORE a
    # rendered file is installed, because a keyd that refuses its config is a
    # keyboard with no leader on a machine somebody is holding.
    RONOCTUA_KEYD = "${keyd}/bin/keyd";
    # THE CACHE POLICY, and three crates read it rather than three crates
    # deciding. How long a cached file may live is the sweep's business, how
    # often it happens is the brain's, and when a thumbnail is remade is the
    # file manager's — but `freshDays < keepDays` is a promise none of the three
    # can keep alone, so the number comes from one file and the build refuses
    # the pair that breaks it.
    RONOCTUA_CACHE_RS = "${cacheRs}/cache.rs";
    # THE PROGRAMS A `full` INSTALLATION LISTS, and which come unmarked: the
    # installer's eighteenth question reads the ledger's `offered` as a table,
    # so its build never evaluates a machine (`nix/offered.nix`).
    RONOCTUA_OFFERED_RS = "${offeredRs}/offered.rs";
    # THE CAMERA'S EFFECTS, one declaration for the studio's arithmetic and
    # the text keys both languages owe. See `nix/camera-effects.nix`.
    RONOCTUA_CAMERA_EFFECTS_RS = "${cameraEffectsRs}/effects.rs";
    # WHERE libmpv IS, for `ronoctua-player-engine`'s build script — the ONE
    # crate that links it, which also puts it in that binary's RUNPATH.
    RONOCTUA_LIBMPV_DIR = "${mpvFull}/lib";
    # THE SYNTAX TABLE, and it is one file for the same reason the update
    # sequence is: what crosses to Rust is the finished table, not the
    # declaration. `crates/ronoctua-syntax`'s `build.rs` copies it into
    # `OUT_DIR` and refuses to generate anything itself (invariant 5).
    RONOCTUA_SYNTAX_RS = "${syntaxRs}/syntax.rs";
    # THE TEXT KEYS' NAMES, and nothing of their values: the directory each
    # speaking crate's `src/key.rs` includes its `<crate>.rs` from, written at
    # evaluation by `nix/text-keys.nix` out of the names of its own fragment.
    # Here, every crate's file (the devshell's and the nested loop's `cargo`
    # builds the whole workspace); a UNIT is handed a directory holding its
    # own crate's file alone (`perCrateEnv` below), so a key added to one
    # crate rebuilds that crate and what links it. The sentences are
    # `RONOCTUA_TEXTS`, read at run time (`keyed`, below), so a value edit
    # leaves every one of these as it was.
    RONOCTUA_KEYS_DIR = "${texts.keys}";
    # `ronoctua_i18n::LANGUAGES`, from `assets/i18n/languages.nix`.
    RONOCTUA_LANGUAGES_RS = "${texts.languagesRs}";
    # The MIME table, for `ronoctua-files`'s `build.rs`, which copies its
    # `mimedb.rs` out of this directory — read at COMPILE time only: the
    # `shared-mime-info` package is a build input of the derivation that made
    # this file, and nothing of it survives into the closure — which is what
    # `files-closure-has-no-mime-db` measures rather than assumes.
    RONOCTUA_MIME_DIR = "${mimeDb}";
    # The compositor's command table, DECLARED in
    # `assets/compositor-deeds.nix`. It is what makes "every OS action has a
    # command" true without a hand-written list in Rust — and the generator
    # refuses a verb the pinned compositor does not have.
    RONOCTUA_DEEDS = "${compositorDeeds}/deeds.rs";
    RONOCTUA_FONT_REGULAR = regularFont;
    RONOCTUA_FONT_EMOJI = emojiFont;
    RONOCTUA_FONT_LUCIDE = lucideFont;
    RONOCTUA_FONT_MARKS = marksFont;
    # The fallback chain, generated from nix/fontconfig.nix — see nix/chain.nix.
    RONOCTUA_CHAIN_RS = "${chainRs}/chain.rs";
    # How many cells a character takes — Unicode's own answer, not ours.
    # See nix/width.nix; `ronoctua-protocol` is what reads it.
    RONOCTUA_WIDTH_RS = "${widthRs}/width.rs";
    # The glance's places, generated from assets/world.nix — see nix/world.nix.
    RONOCTUA_WORLD_RS = "${worldRs}/world.rs";
    # The player's channels by theme, generated from assets/channels.nix — see
    # nix/broadcast.nix.
    RONOCTUA_BROADCAST_RS = "${broadcastRs}/broadcast.rs";
    # The languages of content, generated from assets/tongues.nix — see
    # nix/tongues.nix.
    RONOCTUA_TONGUES_RS = "${tonguesRs}/tongues.rs";

    # THE TWO PROGRAMS THE SHELL SPAWNS, BY PATH AND NOT BY NAME.
    #
    # Relying on `PATH` cost two VM tests: a systemd scope comes up with a
    # minimal `PATH`, and the failure shows up far from the cause — "cannot open
    # display" in the app, "failed to execute" on the terminal's SCREEN. With the
    # store path, the binary carries the reference and Nix guarantees it exists.
    RONOCTUA_SETSID = "${util-linux}/bin/setsid";
    # `setpriv`, ITS SIBLING, and for the sibling's reason.
    #
    # The level meter's `pw-cat` has to die when the footer does, and the
    # mechanism is `PR_SET_PDEATHSIG` — one prctl between the fork and the
    # exec. In Rust that is `Command::pre_exec`, which is `unsafe`, in the one
    # place in a program where nothing is safe. `setpriv --pdeathsig KILL`
    # makes exactly that call, in a binary from the same package as `setsid`,
    # audited by far more people. The cost is one extra `exec` per meter, and
    # a meter is started when a cursor lands on a microphone.
    #
    # MEASURED, and it is why this exists at all: without it a footer killed
    # with `SIGKILL` left `pw-cat` recording. Watched on this machine — the
    # parent gone, the child still in `epoll_wait` with the device open. A
    # broken stdout pipe does NOT end it.
    RONOCTUA_SETPRIV = "${util-linux}/bin/setpriv";
    # `choom` raises a foreign application's `oom_score_adj` so the kernel
    # reaches for it before it reaches for the compositor — see `foreign.rs`.
    RONOCTUA_CHOOM = "${util-linux}/bin/choom";
    # `keyboard-restore`'s `systemctl`. It runs from inside a user unit, where
    # PATH is minimal — see `crates/.../keyboard.rs`.
    RONOCTUA_SYSTEMCTL = "${systemd}/bin/systemctl";
    # THE SCOPE PROBE, and the reason a foreign app is never started twice.
    #
    # `foreign::launch` puts every app it opens in a scope of its own, and a
    # scope's exit status is the APP's: a program that exits with 1 on its own
    # and a `systemd-run` that could not make a scope look the same from here.
    # So the shell asks once, with a program that can only succeed, whether
    # scopes work at all — and from then on never relaunches anything. Both
    # by path: this runs inside the footer's unit.
    RONOCTUA_SYSTEMD_RUN = "${systemd}/bin/systemd-run";
    RONOCTUA_TRUE = "${coreutils}/bin/true";

    # TOR: WHERE IT LISTENS, WHERE ITS FRAGMENT GOES, AND THE TWO BRIDGES.
    #
    # The address comes from `nix/tor.nix`, which `nix/modules/tor.nix` reads
    # too. It used to be a literal in the crate against a module that declared
    # no `SOCKSPort` at all — the two agreed because the nixpkgs default was
    # the same number, and nothing would have said otherwise the day either
    # moved. `tor-socks-is-declared-once` compares them both ways now.
    RONOCTUA_TOR_SOCKS = "${torFacts.socks.address}:${toString torFacts.socks.port}";
    RONOCTUA_TOR_FRAGMENT = torFacts.fragmentDirectory;
    # THE TWO BRIDGES' PROGRAMS ARE NOT HERE. `RONOCTUA_TOR_OBFS4` (lyrebird)
    # and `RONOCTUA_TOR_SNOWFLAKE` were, and no crate read either: the view's
    # `ClientTransportPlugin` ring writes the transport's word and no `exec`
    # path. A value nothing reads is a reason for a unit to rebuild when it
    # moves, so they left when every crate began declaring what it reads
    # (`[package.metadata.ronoctua]`); `nix/modules/tor.nix` still installs
    # both programs.
    # The URL opener, for Syncthing's interface (`LEADER s s a`).
    # A store path, and not the name: inside a systemd unit PATH is
    # minimal and the failure would show up far from the cause.
    RONOCTUA_XDG_OPEN = "${xdg-utils}/bin/xdg-open";
    # `nmcli`, for the ONE thing the bus cannot do: importing a WireGuard or
    # OpenVPN file. The translation from those formats to a NetworkManager
    # profile is thirty settings that upstream maintains, and `nmcli` is where
    # it lives — a second one here would be a second thing to keep in step
    # with a format somebody else defines. It comes with NetworkManager, which
    # this system already has, so the closure gains nothing.
    RONOCTUA_NMCLI = "${networkmanager}/bin/nmcli";
    # `dconf`, for telling foreign applications how big the text is.
    #
    # MEASURED, and the first version of this comment measured the wrong
    # thing. It said "the closure gains nothing" because
    # `programs.dconf.enable` is already on for the settings portal — which is
    # true of the installed SYSTEM and says nothing about this DERIVATION,
    # and `rust.shell` is seeded into the ISO. `nix why-depends` shows the
    # binary really does reference it.
    #
    # The honest number is `0,4 MiB on a 1735,9 MiB closure`: glib is already
    # there, so dconf brings only its own two outputs (82 KiB and 323 KiB).
    #
    # What it buys is the one layer a GTK 4 application, Chromium and
    # Electron all read WHILE RUNNING — the two `settings.ini` files
    # `nix/modules/foreign.nix` writes are in the store and read-only, so they
    # are the machine's floor and not the person's choice.
    RONOCTUA_DCONF = "${dconf}/bin/dconf";
    # `wpctl` AND `pw-dump`, for the audio profile — and the closure gains
    # NOTHING, which is the measurement this waited on.
    #
    # WHO KNOWS THE PROFILE IS PIPEWIRE, not BlueZ. `MediaTransport1` says
    # what is PLAYING; it cannot say what a device could be put in, so the
    # switch between "music in stereo" (A2DP) and "a call with the
    # microphone" (HFP) is not on the bus this crate speaks. The choice was
    # `pactl` — which drags `pkgs.pulseaudio` in for its client alone — or
    # these two, and `nix path-info -r` on the system's own toplevel answers
    # it: `wpctl` and `pw-dump` are already in `sw/bin`, because
    # `services.pipewire` puts them there. `pactl` is not.
    RONOCTUA_WPCTL = "${wireplumber}/bin/wpctl";
    RONOCTUA_PW_DUMP = "${pipewire}/bin/pw-dump";
    # `pw-cat` IS THE LEVEL METER, and it is the third program out of the two
    # packages already named above — so the closure gains nothing again.
    #
    # It is the ONE thing in this repository that opens a microphone, and it
    # runs only while a row for that source is under the cursor. See
    # `crates/ronoctua-audio/src/meter.rs` for why that guard is a value with
    # a `Drop` and not a flag.
    RONOCTUA_PW_CAT = "${pipewire}/bin/pw-cat";
    # `RONOCTUA_EMACSCLIENT` IS NOT SET HERE, and its absence is the point —
    # the same point `RONOCTUA_ICONS` makes below, learned twice.
    #
    # THE EDITOR OF THIS DISTRO IS EMACS, and the manager opens text with
    # `emacsclient` — not `emacs`, because `nix/modules/emacs.nix` runs it as a
    # DAEMON: a second process would take seconds to start and would not see the
    # buffers the session already has. `-c` (a frame of its own) and not `-nw`:
    # the manager is a graphical thing and what it opens has to be one too.
    #
    # Baked in with `option_env!`, its store path became a reference the
    # workspace carries into EVERY closure — and `ronoctua.editor.emacs = false`
    # in the `minimal` profile removed the package from `systemPackages` and left
    # all of Emacs in the system anyway. The comment that used to sit here even
    # said "it may be ABSENT", which is exactly the shape of a switch that does
    # not switch. `nix why-depends` said so; `profile-minimal-closes` is what
    # asked the question.
    #
    # It is read from the ENVIRONMENT instead, set by `nix/modules/emacs.nix` —
    # inside its own `mkIf`, so it exists precisely when Emacs does — and by
    # `nix/dev.nix`. Like the superpack, this is data the shell can do without.
    # THE TWO PROCESS MONITORS — `Ctrl+Alt+Esc` / `LEADER+Esc` and `LEADER+S-Esc`.
    #
    # By store path for the usual reason, in its sharpest form: they are what one
    # opens when the machine is already struggling, inside a scope whose PATH is
    # minimal. They are in `nix/modules/cli.nix` too, and that is not
    # duplication — there they are commands one TYPES, here they are what the
    # shortcut runs, and neither implies the other.
    RONOCTUA_BTOP = "${btop}/bin/btop";
    RONOCTUA_BOTTOM = "${bottom}/bin/btm";
    # WHERE ncurses FINDS `xterm-256color`.
    #
    # The terminal now tells the program what terminal it is (`TERM`), and a
    # name whose terminfo is missing brings back the very failure that fixes:
    # `cannot initialize terminal type`, and the process dies. NixOS does define
    # `TERMINFO_DIRS` for every profile — this is the floor for when it does not
    # (the nested loop on a host that is not NixOS), and the variable is only
    # set when the environment left it empty.
    RONOCTUA_TERMINFO_DIRS = "${ncurses}/share/terminfo";
    # What seals the Wi-Fi password into the TPM. A store path for the same reason
    # as the others: the agent runs as a systemd unit, where PATH is minimal.
    RONOCTUA_SYSTEMD_CREDS = "${systemd}/bin/systemd-creds";
    # WHAT COPIES. `Effect::Copy` was a `tracing::info!` and nothing more: the
    # glyph browser's nine copy formats never put anything on the
    # clipboard.
    #
    # `wl-copy` and not a `wl_data_device` of our own because whoever OWNS the
    # selection has to stay alive to serve it, and the buffer's window dies when
    # the browser is closed — copying a glyph IN ORDER to close and paste is the
    # use case. `wl-copy` leaves a minimal process serving, which is what makes
    # the copy outlive the buffer.
    RONOCTUA_WL_COPY = "${wl-clipboard}/bin/wl-copy";
    # `wl-copy`'s PATH, and not ours.
    #
    # It does `exec cat` to serve the selection to whatever process pastes. Under a
    # systemd unit PATH is minimal, and `cat` is not there: the copy
    # failed with `exec cat: No such file or directory` — AGENTS.md's trap one
    # level deeper, because what spawns by NAME is the program we
    # spawn by path.
    RONOCTUA_WL_COPY_PATH = lib.makeBinPath [ coreutils ];
    # What READS the clipboard, for `Ctrl+V`. Same package as
    # `wl-copy`, and the same reason for coming in by path.
    RONOCTUA_WL_PASTE = "${wl-clipboard}/bin/wl-paste";
    # THE FIVE `RONOCTUA_BUFFER_*` ARE NOT HERE, and their absence is the point —
    # see `bufferComponents` below.
    #
    # THE LIST OF xkb LAYOUTS, by store path.
    #
    # The installer validates the sixth answer against it, and it is the SAME
    # file the `keyboard-layout-exists` check reads — a hand-written list would
    # go stale and refuse a layout the system supports.
    #
    # A store path and not `/run/current-system/...`: the installer runs from the
    # ISO, where there is no current-system yet. Falling back to that path would
    # give an empty list, and validation would refuse everything.
    RONOCTUA_XKB_BASE_LST = "${xkeyboard_config}/share/X11/xkb/rules/base.lst";
    # THE LOCALE LIST, from glibc's own `SUPPORTED`, for the same reason and by
    # the same route: the installer offers what the system can generate, and a
    # list written by hand here would refuse a locale glibc supports.
    RONOCTUA_LOCALE_SUPPORTED = "${glibcLocales}/share/i18n/SUPPORTED";
    # THE TIMEZONE LIST, from tzdb's own table. Same route, same reason: the
    # installer offers what the system has, and never a list kept by hand.
    RONOCTUA_TZ_TAB = "${tzdata}/share/zoneinfo/zone1970.tab";
    # The console prompt, for the boot with nobody logged in. By PATH: a unit's
    # `PATH` is minimal and a bare name fails far from the cause.
    RONOCTUA_ASK_PASSWORD = "${systemd}/bin/systemd-ask-password";
    # The terminal engine. `ronoctua-term`'s `build.rs` reads this to find the
    # library; without it the crate compiles with the alacritty engine alone,
    # and the differential degrades to a golden file — which it says out loud.
    RONOCTUA_GHOSTTY_VT = "${ghosttyVt}";

    # THE TORRENT ENGINE. `ronoctua-torrent`'s `build.rs` reads this to find the
    # shim; without it the crate compiles with no engine at all, and the queue
    # says a torrent job cannot be taken instead of taking one that never moves.
    RONOCTUA_TORRENT_SHIM = "${torrentShim}";
    # WHAT THE MEMORY PANEL SPAWNS, all by store path and for the usual reason:
    # a user unit's `PATH` is minimal. `journalctl` counts how many times the
    # protection acted, and `nix` answers what the store weighs — a question
    # only its own database can answer without walking the disk for minutes.
    #
    # AND THE LINE THAT OUTLIVES THE FOOTER READS THE SAME ONE: its `see the
    # logs` opens the journal in the rescue console. One declaration, two
    # readers — see `crates/.../lifeline.rs`.
    RONOCTUA_JOURNALCTL = "${systemd}/bin/journalctl";
    RONOCTUA_NIX = "${nix}/bin/nix";
    # WHAT THE RESCUE CONSOLE SPAWNS, and the reason is sharper here than
    # anywhere else: it runs from a unit on a VT, where `PATH` is minimal, and
    # it is opened BECAUSE something is already broken. A name that does not
    # resolve would fail on the one screen that exists to explain failures.
    #
    # `fc-cache` answers the screen full of boxes; the shell is what the
    # terminal row hands the screen to; and `pkttyagent` is how a console with
    # no desktop can ask polkit anything at all — without it, every privileged
    # row would answer "Interactive authentication required" and stop there.
    RONOCTUA_FC_CACHE = "${fontconfig}/bin/fc-cache";
    RONOCTUA_RESCUE_SHELL = "${zsh}/bin/zsh";
    RONOCTUA_PKTTYAGENT = "${polkit}/bin/pkttyagent";
    # WHAT THE ARCHIVES RUN. Same route, same reason.
    #
    # `bsdtar` writes and reads the tar family for `ronoctua-archive` — the
    # one crate that runs it, beside 7zz and unrar (the file manager's
    # packing went there, through the transfer queue). Its own program is
    # 440 KB; what its closure adds beyond what the system already carries is
    # measured by `iso-is-measured`, not assumed. Its store path comes from
    # `nix/archive-tools.nix`, with the other two (liblz4 linked in).
    # The discard pass of an overwriting delete — the one step that reaches the
    # SSD controller, and the reason `Verb::Trim` exists. It needs root, so it
    # goes through the same narrow unit as the other three.
    RONOCTUA_FSTRIM = "${util-linux}/bin/fstrim";
    # "Open as root": our own terminal running `pkexec`. It is polkit's own
    # front door, so the authorisation is the one the desktop already knows how
    # to ask for — and this crate still never sees a password.
    RONOCTUA_PKEXEC = "${polkit}/bin/pkexec";
    # THE ICON SUPERPACKAGE — the PIXMAP step of section 4.2's chain, for a
    # FOREIGN application that brings its own `Icon=`. It had existed since the
    # icons milestone and never had a path into the Rust: `Chain` has the field
    # `superpacote` and nobody filled it in.
    #
    # Invariant 10 still stands: what consults it is the HOST, and no
    # design system component reaches it — the WIT `world` has no
    # filesystem.
    # `RONOCTUA_ICONS` IS NOT SET HERE, and its absence is the point.
    #
    # Baked in, the icon superpack's store path is a reference this binary
    # carries into every closure — so `ronoctua.icons.superpack = false` removed
    # it from `environment.systemPackages` and left all 0,61 GiB in the system
    # anyway. Measured with `nix why-depends`, not assumed.
    #
    # It is read from the ENVIRONMENT instead (`crates/.../apps.rs`), set by
    # `nix/modules/icons.nix` and by `nix/dev.nix`. It can be: unlike `setsid`
    # or the wasm buffers, this is data the shell can do without.

    # THE BRAIN'S MODULES, one part per crate (`nix/brain-modules-rs.nix`).
    #
    # The LIBRARY's (`ronoctua-brain`, which the footer links): one
    # `<id>/manifest.rs` per folder under `crates/ronoctua-brain/modules/`,
    # `distro.rs`, `ids.txt` and `shared.txt` — the distro's alone, so it is
    # the same derivation on a machine with a person's module. The DAEMON's
    # (`ronoctua-brain-daemon`): the person's manifests, the registry
    # `all.rs` over every module, and `ids.txt` — which its `build.rs`
    # compares with the folders it sees, refusing a stale environment rather
    # than compiling a module list that is not the tree's. Directories: each
    # crate includes several files out of its own.
    RONOCTUA_BRAIN_MODULES_DIR = "${brainManifestsRs}";
    RONOCTUA_BRAIN_REGISTRY_DIR = "${brainModulesRs}";

    # THE CALCULATOR'S UNITS, generated from `assets/units.nix`. In `generated`
    # and not at run time: it is CODE — the table the engine matches words and
    # factors against — and a few dozen kilobytes that change when a unit does.
    RONOCTUA_UNITS_RS = "${unitsRs}/units.rs";

    # WHAT THE SECURITY CENTRE'S ROOT HELPER RUNS, by store path: sudo resets
    # the environment and a unit's `PATH` is minimal. `visudo` checks the
    # passwordless window's line BEFORE it can be read (a sudoers syntax error
    # locks every sudo out, the rescue included); `nft` checks and applies the
    # person's table. `systemctl` and `systemd-run` are already above.
    RONOCTUA_VISUDO = "${pkgs.sudo}/bin/visudo";
    RONOCTUA_NFT = "${pkgs.nftables}/bin/nft";

    # sys-boundaries: THE PROGRAMS OF THREE BOUNDARY CRATES, baked like
    # `RONOCTUA_XDG_OPEN` (a store path the binary carries, never a name on a
    # unit's minimal `PATH`). `ronoctua-git` runs git (and ssh, for a remote
    # over SSH); `ronoctua-archive` runs 7zz and unrar besides `bsdtar` above,
    # both under `RONOCTUA_SETSID`. Each crate reads the environment first, so
    # a test or a unit can point it elsewhere. Podman is NOT here: it is read
    # at run time (`nix/modules/containers.nix`), because a baked path would
    # put podman's closure into every profile, the minimal one included.
    RONOCTUA_GIT = "${pkgs.git}/bin/git";
    RONOCTUA_SSH = "${pkgs.openssh}/bin/ssh";
    # git's askpass helper, BUILT APART from the workspace so the workspace can
    # carry its path: every program linking `ronoctua-git`, extracted or not,
    # finds it (see `nix/git-askpass.nix`).
    RONOCTUA_GIT_ASKPASS = "${
      import ./git-askpass.nix { inherit pkgs rustToolchain; }
    }/bin/ronoctua-git-askpass";
    # `RONOCTUA_UNRAR` is NOT here: it is in `atRunTime` (see there).
    inherit (import ./archive-tools.nix { inherit pkgs; })
      RONOCTUA_7ZZ
      RONOCTUA_BSDTAR
      ;

    # WHAT THE BELONGINGS KEEP, generated from `assets/belongings.nix`. In
    # `generated`: it is CODE — the rules the engine walks the home with — and
    # a few kilobytes that change when a rule does.
    RONOCTUA_BELONGINGS_RS = "${belongingsRs}/belongings.rs";
    # THE DEPLOY KEY'S MAKER, by store path, for `RONOCTUA_SSH`'s reason:
    # openssh is already in the closure through it.
    RONOCTUA_SSH_KEYGEN = "${pkgs.openssh}/bin/ssh-keygen";

    # THE 2026-10-05 WAVE: one region per package, after its anchor.
    # ── anchor: L01 ──
    # ── anchor: L02 ──
    # THE BELONGINGS' HTTPS, POST-QUANTUM ONLY: the OpenSSL configuration
    # `ronoctua-git` runs git under for a post-quantum-only token — the hybrid
    # ML-KEM groups alone (`nix/pq-tls.nix`). A few hundred bytes, baked like
    # `RONOCTUA_GIT` so that every program linking the crate finds it.
    RONOCTUA_GIT_PQ_TLS = "${import ./pq-tls.nix { inherit pkgs; }}";
    # ── anchor: L03 ──
    # THE MACHINE DIRECTORY, by its one name (`nix/machine.nix`,
    # `machine-dir-has-one-name`): the rescue console's papers, the person's
    # files and the machine's life all read it from here. A value and not a
    # store path, so it costs no reference — and it is in `generated` because
    # `env!` needs it at compile time, where a missing value is a refusal to
    # build rather than a guess.
    RONOCTUA_MACHINE_DIR = (import ./machine.nix).machineDir;
    # ── anchor: L05 ──
    # ── anchor: L06 ──
    # ── anchor: L08 ──
    # ── anchor: L10 ──
    # THE SECURITY KEY'S REGISTRATION (`ronoctua_security::u2f::Enrolment`):
    # `pamu2fcfg`, run as the person from the screen, by store path. The
    # module that trusts what it makes (`pam_u2f`) is the same package.
    RONOCTUA_PAMU2FCFG = "${pkgs.pam_u2f}/bin/pamu2fcfg";
    # ── anchor: L11 ──
    # THE PERSON'S BRAIN MODULES (`my-…`), when this build has any: the
    # folder their `module.rs` is read from, which the daemon's `build.rs`
    # compares with the table's `person` lines — the daemon's alone; the
    # library never reads it. It rides on the daemon's table
    # (`brainModulesRs.personModulesDir`), so the two are one copy.
    # ABSENT, not empty, for a build with none — the distro's: the name is
    # `null` then, and Nix leaves the attribute out.
    ${
      if (brainModulesRs.personModulesDir or null) == null then
        null
      else
        "RONOCTUA_BRAIN_PERSON_MODULES_DIR"
    } =
      "${brainModulesRs.personModulesDir}";
    # ── anchor: L12 ──
    # ── anchor: L13 ──
    # ── anchor: L14 ──
    # THE LLM CLI'S TWO PROGRAMS, by store path (`ronoctua-llm`): the shell the
    # person's own status line is chained through — Claude Code runs it in a
    # shell, so the chain must too — and the `env` a session of an account is
    # opened with. Both already in every closure (bash, coreutils).
    RONOCTUA_LLM_SH = "${pkgs.bash}/bin/bash";
    RONOCTUA_LLM_ENV = "${coreutils}/bin/env";
    # ── anchor: L15 ──
    # THE DISTRO'S OWN REPOSITORY, for `LEADER / c` (`dev-clone`): what the
    # footer clones when somebody wants to work on the distro from it. A value
    # and not a store path — it costs no reference — and in `generated`, next
    # to the release this build came from, rather than a literal in a crate.
    RONOCTUA_DISTRO_REPOSITORY = "https://github.com/ronoctua/ronoctua-distro.git";
    # ── anchor: L17 ──
    # ── anchor: L22 ──
    # WHAT THIS GENERATION SHIPS — the versions the security centre's
    # post-quantum section reads (`ronoctua_security::quantum::built`) and
    # whether Syncthing's source narrows TLS's curves (`nix/pq-facts.nix`). A
    # few hundred bytes of JSON naming no store path; it changes when nixpkgs
    # moves, as every unit does anyway.
    RONOCTUA_PQ_FACTS = "${import ./pq-facts.nix { inherit pkgs; }}";
  };

  # THE FIVE COMPONENTS — and they are not in `generated` on purpose.
  #
  # # The measurement that took them out
  #
  # `option_env!` with a store path is a REFERENCE, and it is charged for where
  # the compilation happens: with the six here, editing one COMMENT in any
  # buffer changed the hash of the whole workspace (one crane derivation then;
  # today every unit that reads one, and every one above). Measured then, on
  # the four derivations a person waited for:
  #
  #   ronoctua-0.1.0 (the workspace)   rebuilt      2m15 – 5m46
  #   ronoctua-tests                   rebuilt      the workspace again, in test mode
  #   ronoctua-clippy                  rebuilt      idem
  #   memory-instance-fast             re-ran       and this one is correct: it
  #                                                 measures the component
  #
  # None of them reads a `.wasm` at compile time. It is the icon superpack's
  # lesson (further down) reached from the other side: there a baked path kept a
  # switch from switching, here it kept the loop from being a loop.
  #
  # # And why the reference does not simply disappear
  #
  # Because it must not. Whoever installs this has to get the components
  # WITHOUT compiling them — that is the whole point of the ISO's seed. So the
  # reference moves one derivation UP: the shell's bundle (`shell`, from
  # `nix/rust/outputs.nix`) is what carries it, and what the system, the VMs
  # and the nested loop all run. `iso-carries-the-seed`
  # and `profile-minimal-closes` name them, and would say so if they left.
  bufferComponents = {
    RONOCTUA_BUFFER_MEMORY = "${bufferMemory}/lib/ronoctua_buffer_memory.wasm";
    RONOCTUA_BUFFER_KEYBOARD = "${bufferKeyboard}/lib/ronoctua_buffer_keyboard.wasm";
    RONOCTUA_BUFFER_TRANSFERS = "${bufferTransfers}/lib/ronoctua_buffer_transfers.wasm";
    RONOCTUA_BUFFER_FORMAT = "${bufferFormat}/lib/ronoctua_buffer_format.wasm";
    RONOCTUA_BUFFER_BRAIN = "${bufferBrain}/lib/ronoctua_buffer_brain.wasm";
    # THE REFERENCE for several keyboards in one app: a list each keyboard
    # moves its own selection in, and a form each fills at once
    # (`ronoctua_design::Carets`). Reached by name, `example-open`.
    RONOCTUA_BUFFER_EXAMPLE = "${bufferExample}/lib/ronoctua_buffer_example.wasm";
  };

  # THE DATA A BINARY IS KEYED TO, and not only pointed at.
  #
  # The palettes are read at run time (`ronoctua_tokens::palettes`), and they
  # are written for ONE set of colour roles: the file carries a hash of the
  # role names, and a binary compiled with other roles refuses it — loudly,
  # naming the variable, because a palette read through the wrong index is
  # every colour on screen wrong with nothing failing.
  #
  # SO THE WRAPPERS `--set` THESE, and do not `--set-default` them like the
  # rest of `atRunTime`. A login session exports `RONOCTUA_THEMES` too
  # (`nix/modules/scroll.nix`), and it keeps the value it logged in with: after
  # a switch that added a colour role, the compositor's binds and a terminal
  # would start the NEW binary with the OLD palettes — and the refusal would
  # take the leader down until the next login. The wrapped binary always
  # reads the palettes of its own build. Nothing is lost: what one would swap
  # by hand is the palettes of the same roles, and a `cargo` binary (the
  # devshell, `.#dev`, `cargo test`) has no wrapper and reads the variable.
  #
  # THE TEXTS, for the same reason in words: a binary is compiled with the
  # key NAMES (`texts.keys`) and reads the sentences at run time, and texts
  # that lack a key it was compiled with are another build's — refused at its
  # start, one line naming `RONOCTUA_TEXTS` and exit 78
  # (`ronoctua_i18n::Texts::checked`). A session that kept an older directory
  # would take the leader down after a switch that added a key, exactly as an
  # older palette would.
  #
  # AND THE BRAIN'S CATALOGUE, for the same reason in marks: the footer draws
  # a module's mark from the glyph it measured, registers the actions it
  # lists and raises the notifications it declares — the modules this build's
  # daemon compiled. A session's older file would draw yesterday's modules
  # over today's brain; one the program cannot read, or whose keys its texts
  # lack, is refused at its start (`ronoctua_brain::catalogue::checked`).
  keyed = {
    RONOCTUA_THEMES = "${themed}";
    RONOCTUA_TEXTS = "${texts}";
    RONOCTUA_BRAIN_CATALOGUE = "${brainCatalogue}/${brainCatalogue.file}";
  };
  # `--set` for what is keyed, `--set-default` for everything else.
  wrapFlags =
    vars:
    lib.concatStringsSep " " (
      lib.mapAttrsToList (
        name: path: "${if keyed ? ${name} then "--set" else "--set-default"} ${name} ${path}"
      ) vars
    );
  # THE OTHER BINARIES THAT PAINT, beside the footer in the same bundle, and
  # what each is keyed to: the PNG camera, the rescue console and the
  # installer (a terminal, in the palette's colours) paint AND speak; the
  # player's daemon (the subtitle lanes' colours) paints and says no word —
  # it links no text, and its closure needs none. They get the keyed data
  # only — the rescue console and the installer run with no coverage faces
  # and no atlas on purpose (`RONOCTUA_COVERAGE_FONTS`). Wherever one is
  # installed — under its own wrapper or as a copy in the shell's bundle —
  # it is wrapped with these (`nix/rust/outputs.nix`); `wrapPainters` is the
  # same over a `bin/` that holds them all (`nix/rust/profiles.nix`).
  #
  # NONE OF THEM DRAWS A BRAIN MODULE'S MARK: the catalogue is the footer's
  # (`atRunTime`), and a painter that never reads it does not carry it.
  painters = {
    ronoctua-render-png = { inherit (keyed) RONOCTUA_THEMES RONOCTUA_TEXTS; };
    ronoctua-rescue = { inherit (keyed) RONOCTUA_THEMES RONOCTUA_TEXTS; };
    ronoctua-install = { inherit (keyed) RONOCTUA_THEMES RONOCTUA_TEXTS; };
    ronoctua-player = { inherit (keyed) RONOCTUA_THEMES; };
  };
  wrapPainters = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (
      bin: vars: "wrapProgram $out/bin/${bin} --inherit-argv0 ${wrapFlags vars}"
    ) painters
  );

  # EVERYTHING THE WRAPPER SETS, and not only the components.
  #
  # The eight above are `.wasm`; this adds what is read at run time for the same
  # reason and is not one — the coverage faces. Keeping the two apart is not
  # tidiness: `buffer-components-are-wired` asserts every name in
  # `bufferComponents` ends in `.wasm`, which is exactly the guard that catches
  # a component pointed at the wrong kind of thing, and a font directory in
  # there would have had to weaken it.
  atRunTime = bufferComponents // {
    # WHAT A BINARY IS KEYED TO (`keyed`, above): here too, for the devshell
    # and the nested loop, whose `cargo` binaries have no wrapper.
    inherit (keyed) RONOCTUA_THEMES RONOCTUA_TEXTS RONOCTUA_BRAIN_CATALOGUE;
    # THE SEGMENTATION MODEL, at run time for `RONOCTUA_ATLAS`'s reason: a
    # model update must not rebuild a unit, and absence is a refusal
    # drawn on the background effects, not an error.
    RONOCTUA_SEGMENT_MODEL = "${segmentModel}";
    # THE HAND MODELS, for the same reason: the gestures refuse without them.
    RONOCTUA_HAND_MODELS = "${handModels}";
    # THE FALLBACK FACES. Absent, the renderer keeps its four embedded ones and
    # says how many it has (`Text::coverage`) — which is what the installer and
    # the rescue console run with, and they draw two Latin languages.
    RONOCTUA_COVERAGE_FONTS = "${coverageFonts}";
    # THE ZONE FILES, for the glance's other places.
    #
    # `tzdata` was already here as a BUILD input (`RONOCTUA_TZ_TAB`, the
    # installer's list of zones); this is the first thing that needs the
    # DIRECTORY at run time. `/etc/localtime` answers for the machine's own
    # zone and the glance asks about several.
    RONOCTUA_ZONEINFO = "${tzdata}/share/zoneinfo";
    # THE WORLD. Borders, inland water, named places and the catalogue of
    # downloadable street regions — about six megabytes, baked by
    # `nix/atlas.nix`.
    #
    # AT RUN TIME AND NOT IN `generated`, and the reason is measured: a store
    # path in `generated` is a compilation INPUT of every unit that reads it,
    # so re-baking the table — a newer Natural Earth tag, a fresher GeoNames
    # dump, `cities1000` instead of `cities500` — would rebuild those units and
    # every one above them, for a value no compilation reads.
    # `units-see-only-what-they-read` holds what each unit is handed, and
    # `atlas-is-at-run-time` turns the rule on this variable.
    #
    # AND ABSENCE IS NOT AN ERROR, which is `RONOCTUA_COVERAGE_FONTS`' rule:
    # the installer and the rescue console have no atlas, and there the info
    # panel shows the coordinate and no globe, with a sentence saying why.
    RONOCTUA_ATLAS = "${atlasData}/world.atlas";
    # THE LAYOUTS THE MACHINE HAS, for a keyboard of its own in Independent
    # mode: xkeyboard-config's list — the same package the compositor compiles
    # a keymap from, so a layout offered is a layout that exists. Read as DATA
    # (`ronoctua_keyboards::layouts_in`); absence offers only the machine's own.
    RONOCTUA_XKB_LIST = "${pkgs.xkeyboard_config}/share/X11/xkb/rules/evdev.lst";

    # THE ICON TABLES, at run time for `RONOCTUA_ATLAS`'s reason: ~9 MB that a
    # font release, a lexicon edit or a brain module's glyph re-bakes (the
    # glyph index names a module's token, a person's included:
    # `nix/iconset.nix`), and no compilation reads.
    # Absence is a refusal the icons app draws (`icons-why-no-variable`), not an
    # error. `iconset-is-at-run-time` holds it here.
    RONOCTUA_ICONSET = "${iconset}";

    # UNRAR, AT RUN TIME AND NOT IN `generated`, because it is UNFREE: baked,
    # its store path was a reference of the workspace, and `nix run .#release`
    # never pushes to the public cache what refers to an unfree program — so
    # the workspace, the most expensive build there is, would never be in the
    # cache, and every machine and every installation would compile it. Set by
    # the wrapper here, by the queue's unit (`nix/modules/transfers.nix`) and
    # exported by the devshell and the nested loop; the system carries unrar
    # anyway (`nix/modules/archives.nix`), so no closure grows.
    # `release-cache-holds-only-free-builds` refuses an unfree program in
    # `generated`.
    inherit (import ./archive-tools.nix { inherit pkgs; }) RONOCTUA_UNRAR;

  };

  # CARGO PLANS, NIX BUILDS (`nix/rust/`, nix-design §1): the plan cargo made
  # once (`plan.json`, regenerated by `nix run .#rust-plan`) and one
  # derivation per compilation unit in three flavours — the third-party ones
  # included (phase 2 of §1.4). The two layers cargo builds from the plan's
  # third-party half (phase 1, `nix/rust/layers.nix`) are read by no unit:
  # `nix run .#profile-bench` measures a profile's variants on them, and
  # `rust.unitsWith { depsFrom = "layers"; }` builds the units on them for a
  # comparison.
  plannerArgs = {
    inherit
      lib
      pkgs
      craneLib
      src
      buildFeatures
      ;
    inherit (pkgs) python3;
  };
  planner = import ./rust/planner.nix plannerArgs;
  planFile = ./rust/plan.json;
  plan = builtins.fromJSON (builtins.readFile planFile);
  layerArgs = {
    inherit
      lib
      pkgs
      craneLib
      plan
      ;
    inherit (pkgs) python3;
    inherit (planner) vendor;
    cargoLock = fromTOML (builtins.readFile (src + "/Cargo.lock"));
    rootManifest = fromTOML (builtins.readFile (src + "/Cargo.toml"));
    inherit (buildBase) nativeBuildInputs buildInputs;
    env = {
      inherit (buildBase) AWS_LC_SYS_CMAKE_BUILDER AWS_LC_SYS_USE_SYSTEM;
    };
  };
  layers = import ./rust/layers.nix layerArgs;
  unitArgs = {
    inherit
      lib
      stdenv
      pkgs
      rustToolchain
      src
      plan
      layers
      generated
      testEnv
      ;
    inherit (pkgs) python3;
    inherit (planner) inputsHash vendor;
    inherit crateSourceOverrides;
    # A VALUE THAT IS EACH CRATE'S OWN: the keys' directory holding that
    # crate's file and nothing else, so a unit's derivation moves with its
    # own keys alone (`generated` has the directory of them all).
    perCrateEnv = {
      RONOCTUA_KEYS_DIR = crate: "${texts.keysOf crate}";
    };
    # Present in `generated` only on a machine with the person's brain
    # modules; the brain declares it either way.
    optionalNames = [ "RONOCTUA_BRAIN_PERSON_MODULES_DIR" ];
    # Names only: what `run-env-absent` may declare (the tests never get the
    # values).
    atRunTimeNames = lib.attrNames atRunTime;
    inherit (buildBase) nativeBuildInputs buildInputs;
    testLibraryPath = [ (lib.makeLibraryPath [ wayland ]) ];
    # Phase 2 of nix-design §1.4 (third-party crates unit by unit): each
    # registry package's source — crane's per-crate download, the very
    # derivation the vendoring links — and what a unit running a third-party
    # build script is given, the layers' own.
    depSources = lib.listToAttrs (
      map (p: lib.nameValuePair "${p.name}@${p.version}" (craneLib.downloadCargoPackage p)) (
        lib.filter (p: p ? source && p ? checksum) layerArgs.cargoLock.package
      )
    );
    depBase = {
      inherit (buildBase) nativeBuildInputs buildInputs;
      inherit (layerArgs) env;
    };
    # WHAT A CRATE LOADS WITH `dlopen`, and so what a program whose closure
    # holds it needs in its RUNPATH (no `DT_NEEDED` puts it there): the GPU
    # layer the Vulkan loader (`ash::Entry::load_from` in `wgpu-hal` itself,
    # and `ash`), EGL (`khronos-egl`) and, on Wayland, libwayland-egl
    # (`wayland-sys`'s `egl`); `wayland-sys` (with its `dlopen` feature, which
    # the workspace turns on) libwayland; `xkbcommon-dl` libxkbcommon. libxkbcommon itself is LINKED by the
    # `xkbcommon` crate, and the linker writes that one. A program with none
    # of these crates in its closure — the queue, the brain, the rescue
    # console, the installer — carries none of the graphics stack.
    dlopened = lib.mapAttrs (_: map (p: lib.makeLibraryPath [ p ])) {
      wgpu-hal = [
        vulkan-loader
        libGL
        wayland
      ];
      ash = [ vulkan-loader ];
      khronos-egl = [ libGL ];
      wayland-sys = [ wayland ];
      xkbcommon-dl = [ libxkbcommon ];
    };
  };
  unitBuilder = import ./rust/units.nix unitArgs;
  releaseUnits = unitBuilder.units "release";
  loopUnits = unitBuilder.units "loop";
  # A SET OF UNITS TO KEEP: one link farm (what `nix build` of it builds and
  # `nix path-info -S` measures), and its members' paths as text — what is
  # rooted is the ones the store holds, so naming them builds nothing.
  #
  # EVERY OUTPUT OF EACH UNIT, `metrics` beside `out` (a few bytes: its
  # compiler's peak and time). The batteries ask for every output of every
  # unit (`<drv>^*`, `nix/gate.nix`), and a derivation with one output gone
  # is built again whole: with only `out` rooted, a collection took the
  # `metrics` and the next `.#most` compiled every kept unit again — and
  # `.#rebuild-cost` and `.#rust-peaks` lost what they read.
  keptUnits =
    flavour: units:
    let
      outputsOf = d: [
        d.out
        d.metrics
      ];
      every = lib.concatMap outputsOf units;
    in
    pkgs.linkFarm "ronoctua-kept-${flavour}-units" (
      map (o: {
        name = baseNameOf (builtins.unsafeDiscardStringContext o.outPath);
        path = o;
      }) every
    )
    // {
      paths = map (o: builtins.unsafeDiscardStringContext o.outPath) every;
      inherit flavour;
    };

  # THE PROGRAMS IN THEIR PLACES (`nix/rust/outputs.nix`): every release
  # program by name, the named ones the modules run, the shell bundle, the
  # benchmarks and what the ISO carries.
  outputs = import ./rust/outputs.nix {
    inherit lib pkgs makeBinaryWrapper;
    release = releaseUnits;
    loop = loopUnits;
    crates = unitBuilder.crates;
    inherit
      atRunTime
      wrapFlags
      painters
      ;
  };

  # WHAT THE TESTS ARE GIVEN beyond `generated`: the programs and the data they
  # run against for real, each one closing the trap of a test that returns
  # early without its variable and passes having proved nothing. A test unit
  # takes only what its crate declares (`test-env` when compiled, `run-env`
  # when run). Nothing ships them.
  testEnv = {
    RONOCTUA_FFMPEG = "${pkgs.ffmpeg-full}/bin/ffmpeg";
    RONOCTUA_FFPROBE = "${pkgs.ffmpeg-full}/bin/ffprobe";
    RONOCTUA_EXIFTOOL = "${pkgs.exiftool}/bin/exiftool";
    RONOCTUA_PDFTOPPM = "${pkgs.poppler-utils}/bin/pdftoppm";
    RONOCTUA_PDFINFO = "${pkgs.poppler-utils}/bin/pdfinfo";
    # AND THE THREE OLE READERS, for the same reason the five above are here:
    # a test that reaches one of them without the variable set answers
    # `Refusal::Missing` and proves nothing, which is the trap AGENTS.md
    # records as "every ffmpeg test was EMPTY".
    RONOCTUA_XLS2CSV = "${pkgs.catdoc}/bin/xls2csv";
    # AND THE WORLD, for the same reason again: `globe::world()` returns
    # `None` without it, the test that asks London where it is returns early,
    # and it proves nothing while passing. The variable is the one the
    # WRAPPER sets at run time; this is the same path, for the tests.
    RONOCTUA_ATLAS = "${atlasData}/world.atlas";
    # AND THE MODEL, so `the_mask_is_mediapipes_own` compares against
    # MediaPipe instead of returning early and proving nothing.
    RONOCTUA_SEGMENT_MODEL = "${segmentModel}";
    # And the hands', so `the_outputs_are_tflites_own` compares.
    RONOCTUA_HAND_MODELS = "${handModels}";
    # AND THE NIX THE MACHINE RUNS, so `the_pinned_nix_prints_roots_this_reads`
    # asks the real `nix-store` how it prints a root — against a store of its
    # own in the build directory — instead of returning early: the cache
    # sweep's guard is only as good as its reading of that listing.
    RONOCTUA_NIX_STORE = "${pkgs.nix}/bin/nix-store";

    # THE OFFLINE ENGLISH DICTIONARY (`nix/wordsgen.nix`), so
    # `the_real_dictionary_answers_in_a_few_reads` reads the converted
    # WordNet instead of returning early and proving nothing. Not in
    # `atRunTime`: the installed session gets it from `nix/modules/words.nix`,
    # on the profile that carries it.
    RONOCTUA_WORDS = "${wordsData}";
    # AND THE ICON TABLES, so `ronoctua-iconset`'s tests search the real
    # tables — `house` at U+E0F5, the twenty-three double-width arrows —
    # instead of refusing for want of the variable.
    RONOCTUA_ICONSET = "${iconset}";
    # AND THE ZONE FILES, so the calculator's `now in Tokyo` is asked of
    # tzdata and not skipped.
    RONOCTUA_ZONEINFO = "${tzdata}/share/zoneinfo";
    # AND THE PALETTES, which every test that resolves a colour reads at
    # run time (`ronoctua_tokens::palettes`): the role tables and the
    # pictures compare against the real bytes, and without the variable the
    # first colour asked for panics, naming it. A RUN input: a test unit
    # gets it only when it runs (`run-env` of `ronoctua-tokens`, so every
    # crate above it), apart from the compilation, so a seed's edit compiles
    # no test harness (`colour-seeds-rebuild-no-rust`).
    RONOCTUA_THEMES = "${themed}";
    # AND THE TEXTS, on the same footing: every test that says a word reads
    # the merged files at run time (`ronoctua_i18n`), and without them the
    # first sentence asked for panics, naming `RONOCTUA_TEXTS`. A test unit
    # gets it when it runs (`run-env` of `ronoctua-i18n`, so every crate
    # above it).
    RONOCTUA_TEXTS = "${texts}";
    # AND THE BRAIN'S CATALOGUE, on the same footing: the tests that draw a
    # module's mark, list the leader's actions or make the notification book
    # read it at run time, and without it the first one panics, naming
    # `RONOCTUA_BRAIN_CATALOGUE` (`run-env` of `ronoctua-brain` and
    # `ronoctua-notify`, so every crate above them).
    RONOCTUA_BRAIN_CATALOGUE = keyed.RONOCTUA_BRAIN_CATALOGUE;

    # AND A KERNEL FOR THE FIREWALL'S SNAPSHOTS. `nft -c` talks to the
    # kernel, which a build may not; in a user and network namespace of its
    # own it may, and nothing outside can be touched. Without this the test
    # that puts every snapshot through `nft` returns early and proves
    # nothing — the trap above, again.
    RONOCTUA_UNSHARE = "${util-linux}/bin/unshare";

    # sys-boundaries: the programs the boundary crates' tests run for real
    # (`ronoctua-git` on fixture repositories, `ronoctua-archive` with the
    # passwords on stdin). Also baked through `generated`; set here at run
    # time too, as the five above are, so the tests read what they would
    # read in a unit.
    RONOCTUA_GIT = "${pkgs.git}/bin/git";
    RONOCTUA_SSH = "${pkgs.openssh}/bin/ssh";
    RONOCTUA_SETSID = "${util-linux}/bin/setsid";
    # `script`, a terminal for `a_hostile_repository_runs_nothing`: git
    # starts a pager only on one, and the test shows the pager door open.
    RONOCTUA_SCRIPT = "${util-linux}/bin/script";
    inherit (import ./archive-tools.nix { inherit pkgs; })
      RONOCTUA_7ZZ
      RONOCTUA_UNRAR
      RONOCTUA_BSDTAR
      ;

    # ── anchor: L01 ──
    # ── anchor: L02 ──
    # The post-quantum-only TLS of git, read at run time as a unit would.
    RONOCTUA_GIT_PQ_TLS = "${import ./pq-tls.nix { inherit pkgs; }}";
    # ── anchor: L06 ──
    # ── anchor: L07 ──
    # ── anchor: L08 ──
    # ── anchor: L09 ──
    # ── anchor: L10 ──
    # ── anchor: L12 ──
    # ── anchor: L13 ──
    # ── anchor: L14 ──
    # ── anchor: L17 ──
    # ── anchor: L22 ──
  };
in
rec {
  # crane, for what it still does: vendoring, the planner, the profile
  # bench's layers, and the check that sets the third-party units beside
  # cargo (`third-party-units-are-cargo-s`).
  inherit craneLib;
  # THE PLAN AND ITS UNITS (`nix/rust/`): `planner.plan` is the derivation
  # whose `plan.json` `nix run .#rust-plan` copies into the tree; `layers` the
  # two third-party layers of phase 1 (no unit reads them); `release`, `loop`
  # and `test` the units of each flavour (`deps`, `libs`, `bins`, `testBins`,
  # `testRuns`, `clippy`, `fmt`).
  inherit
    planner
    layers
    testEnv
    buildFeatures
    ;
  # AND WHAT IS KEPT OF EACH (`keep`): the compiled units of the version in
  # use, which nothing installed references — a program is linked from them
  # and cites none — so a collection takes them, and the next build pays them
  # all again. Rooted while current: on a machine by `ronoctua-machine apply`
  # after a build that worked, on a checkout by `nix/kept-sources.nix`, each
  # only the ones its store holds (`nix/keep-units.nix`). The release set is
  # every third-party unit, library, build script and `ship` program of the
  # release graph (the shell's own program among them: the bundle COPIES it,
  # so the system does not hold it); the loop's is its third-party units,
  # which the test graph shares — what the loop's layer used to be.
  release = releaseUnits // {
    keep = keptUnits "release" (
      lib.attrValues releaseUnits.deps
      ++ lib.attrValues releaseUnits.libs
      ++ lib.attrValues releaseUnits.scripts
      ++ lib.filter (b: b.passthru.role == "ship") (lib.attrValues releaseUnits.bins)
    );
  };
  loop = loopUnits // {
    keep = keptUnits "loop" (lib.attrValues loopUnits.deps);
  };
  test = unitBuilder.units "test";
  # WHICH UNITS ARE HEAVY — built alone, with the cores the memory leaves
  # beside them — is no list here: it is a unit's measured class, like light
  # and medium (`nix/rust/unit-classes.json`, read by `nix/build-sizing.nix`;
  # `nix run .#rust-peaks -- --write` writes it from `metrics/peak-kib`).
  # One test of a crate, out of the harness `test.testBins` built — see
  # `nix/rust/units.nix`.
  testsOf = unitBuilder.testsOf test;
  # What a third-party unit that runs a build script is given (the layers'
  # own): for the check that sets those units beside cargo.
  inherit (unitArgs) depBase;
  # What each crate declares, as the unit builder read it (for checks).
  unitCrates = unitBuilder.crates;
  # The same builder with some of its arguments replaced — for the checks
  # that prove what it refuses.
  unitsWith = args: import ./rust/units.nix (unitArgs // args);
  # And the layers with theirs (a plan edited at evaluation): what a check
  # asks to see which edits move a layer.
  layersWith = args: import ./rust/layers.nix (layerArgs // args);
  # THE RELEASE PROFILE'S VARIANTS, for `nix run .#profile-bench` and nothing
  # else (`nix/rust/profiles.nix`: the plan of each is read from a build).
  profileVariants = import ./rust/profiles.nix {
    inherit lib pkgs;
    plannerWith = args: import ./rust/planner.nix (plannerArgs // args);
    inherit layersWith unitsWith;
    inherit (layerArgs) rootManifest;
    shellWrapFlags = wrapFlags atRunTime;
    inherit wrapPainters;
    graphicsLibraryPath = lib.makeLibraryPath [
      vulkan-loader
      wayland
      libxkbcommon
      libGL
    ];
  };
  # THE THREE AGGREGATES: every crate's test run, clippy and rustfmt, each
  # its own unit (`test.testRuns`, `test.clippy`, `test.fmt`) gathered in one
  # link farm — what the checks `rust-tests`, `rust-clippy` and `rust-fmt`
  # are, and what the pre-commit hook builds (`rust-clippy`). Building one
  # builds the units it gathers and nothing else: a line of one crate reruns
  # that crate's units and those above it, never the workspace.
  tests = pkgs.linkFarm "rust-tests" (
    lib.mapAttrsToList (name: path: { inherit name path; }) test.testRuns
  );
  clippy = pkgs.linkFarm "rust-clippy" (
    lib.mapAttrsToList (name: path: { inherit name path; }) test.clippy
  );
  fmt = pkgs.linkFarm "rust-fmt" (lib.mapAttrsToList (name: path: { inherit name path; }) test.fmt);
  # EVERY DERIVATION OF THE BUILD AS DATA (`nix/rust/units.nix`, `index`):
  # what `legacyPackages.<system>.rustUnits` is, for `nix run .#rebuild-cost`
  # and the granularity checks.
  unitIndex = unitBuilder.index {
    inherit release loop test;
    layers = {
      inherit (layers) release loop;
    };
    aggregates = {
      rust-tests = {
        drv = tests;
        members = lib.attrValues test.testRuns;
      };
      rust-clippy = {
        drv = clippy;
        members = lib.attrValues test.clippy;
      };
      rust-fmt = {
        drv = fmt;
        members = lib.attrValues test.fmt;
      };
    };
  };
  # ONE BYTE APPENDED TO EACH FILE, as `crateSourceOverrides`: the edit
  # `nix run .#rebuild-cost -- <file>…` simulates. A path is the
  # repository's, under a crate's directory; any other is refused, naming
  # it (a file outside every crate is no unit's source).
  oneByteEdits =
    files:
    let
      dirs = lib.mapAttrsToList (crate: p: {
        inherit crate;
        inherit (p) dir;
      }) plan.packages;
      crateOf =
        file:
        lib.findFirst (
          d: lib.hasPrefix "${d.dir}/" file
        ) (throw "${file} is in no crate's directory: no unit compiles it") dirs;
      one =
        file:
        let
          d = crateOf file;
          rel = lib.removePrefix "${d.dir}/" file;
          path = src + "/${file}";
        in
        {
          ${d.crate}.${rel} =
            if builtins.pathExists path then
              builtins.toFile (baseNameOf file) (builtins.readFile path + "\n")
            else
              throw "${file} does not exist";
        };
    in
    lib.foldl' lib.recursiveUpdate { } (map one files);
  # THE ONE TRUTH about where the components are. The wrapper below reads it;
  # so do the devshell and the nested loop, which run a binary `cargo` built and
  # that therefore has no wrapper. Three readers, one table.
  inherit bufferComponents atRunTime;
  # And which of them a binary is keyed to (`--set`, never `--set-default`):
  # an app that runs an unwrapped binary sets these the same way.
  inherit keyed;
  # AND THE WHOLE TABLE OF WHAT THE BUILD BAKES, for the same readers and one
  # sharper reason.
  #
  # Everything in here reaches the Rust through `option_env!`, which resolves at
  # COMPILE time — so a binary that `cargo` built outside Nix has whatever the
  # environment held at that moment, and nothing else. `nix/dev.nix` was writing
  # six of these out by hand and missing twenty-five; the hot reload's binary
  # therefore had no `btop`, no clipboard, no `ghostty-vt` and no terminfo. It
  # went unnoticed because the reload was wired to a command that did not exist,
  # so no edit ever replaced the store's binary. That masking outlived the fix:
  # the command went on being one cargo refuses — a `--bin` whose package the
  # line did not select — until `dev-rebuilds-what-sits-beside` was taught to
  # check the packages and not the spelling.
  #
  # Exported whole, the list stops being copied — and
  # `dev-exports-what-the-build-bakes` fails if a new one is added here and not
  # picked up over there.
  inherit generated;

  # THE PROGRAMS IN THEIR PLACES (`nix/rust/outputs.nix`), all out of the
  # units:
  #
  #   bins.<bin>   every release program but `apart`, as built: what a check
  #                runs by name
  #   shell        THE SHELL AS THE SYSTEM RUNS IT: the footer and what its
  #                code finds beside it, under the wrapper that carries the
  #                components — what the module's `ExecStart`, the VM tests
  #                and the compositor's binds point at
  #   loopShell    the same over the loop's programs, which `nix run .#dev`
  #                runs
  #   installer, rescue, brain, …   the 31 programs a module, an app or the
  #                ISO runs, each its own unit (a painter under the palettes'
  #                wrapper): a derivation references what its bytes cite, and
  #                a unit's bytes are one program's — the "extract the binary"
  #                rule, now by construction rather than by `cp`
  #   benches      the four benchmarks, for `.#bench`
  #   shipped      what the ISO carries: the shell and every `ship` program
  #   roles        each program's role, by name
  #
  # `securityRoot` is NOT in `atRunTime`, and that stays load-bearing: the
  # devshell reads that table, and a program's path there would make entering
  # the shell build it. The session gets it from `nix/modules/security.nix`
  # (`RONOCTUA_SECURITY_ROOT`).
  inherit (outputs)
    bins
    shell
    loopShell
    benches
    shipped
    roles
    painters
    installer
    rescue
    player
    formatDisk
    filesRoot
    consoleFont
    transfers
    browser
    software
    studio
    keys
    brain
    emptyTrash
    cacheSweep
    diskRelief
    index
    videoMetrics
    securityRoot
    scan
    belongings
    seal
    machine
    person
    privateRoot
    incusJob
    llm
    cachix
    stickWrite
    networkConsole
    networkAgent
    vscode
    ;
}
