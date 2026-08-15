# Development environment — section 8.
#
# "No config file the dev has to fill in by hand. No tool outside
# Nix." You enter with `direnv allow` once and never again.
{
  pkgs,
  inputs,
  gitHooks,
  wit,
  tokensRs,
  i18nRs,
  niriActions,
  regularFont,
  emojiFont,
  lucideFont,
  bufferComponents,
  iconSuperpack,
  ghosttyVt,
}:

let
  rust = pkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-src"
      "rust-analyzer"
      "clippy"
      "rustfmt"
    ];
    # Section 7.6: native buffers and the design system compile to wasip2.
    targets = [
      "wasm32-wasip2"
      "wasm32-unknown-unknown"
    ];
  };

  py = pkgs.python3.withPackages (ps: [
    ps.fonttools
    ps.brotli
    ps.zopfli
    ps.freetype-py
  ]);
in
pkgs.mkShell {
  name = "ronoctua-distro";

  packages = [
    rust
    py

    # --- Nix ---------------------------------------------------------
    pkgs.nixfmt
    pkgs.nix-tree
    pkgs.nvd

    # --- Wayland / graphics ------------------------------------------
    # Needed for the nested loop (section 7.12, speed 1).
    pkgs.wayland
    pkgs.wayland-protocols
    pkgs.libxkbcommon
    pkgs.vulkan-loader
    pkgs.vulkan-tools
    # Software Vulkan rasteriser. Section 9.3 of the plan: it is what runs
    # in the correctness tests. Performance is NEVER measured under it.
    pkgs.mesa

    # --- VM and images -----------------------------------------------
    pkgs.qemu
    pkgs.OVMF
    pkgs.swtpm # emulated TPM2, for the PCR 15 test (milestone 8)

    # --- tools -------------------------------------------------------
    # No `git-lfs` here: it was for the fonts, and they never enter git at all
    # now — they are Release assets. Nothing in this tree is an LFS object.
    pkgs.git
    pkgs.jq
    pkgs.ripgrep
    pkgs.fd
    pkgs.pkg-config
  ];

  # The renderer links wgpu; the loader has to find the libs at run
  # time inside the devshell.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
    pkgs.wayland
    pkgs.libxkbcommon
    pkgs.vulkan-loader
  ];

  # Section 7.6: pin wasmtime and wit-bindgen to the same WIT version. The
  # mismatch shows up as a confusing type error at instantiation, which is
  # expensive to diagnose. Fixed here, in one place.
  RONOCTUA_WIT_VERSION = "0.2.0";

  # THE SAME GENERATED FILES `nix/rust.nix` PASSES TO CRANE.
  #
  # Without them, `cargo build` inside the devshell fails in
  # `ronoctua-tokens`'s `build.rs` — and section 8 promises there is nothing to
  # fill in by hand. Failing loudly there is right (masking it with a fallback
  # would be worse); what was missing was the devshell offering what crane already offers.
  RONOCTUA_TOKENS_RS = "${tokensRs}/tokens.rs";
  RONOCTUA_I18N_DIR = "${i18nRs}";
  # The pinned niri's action table — see nix/niri-actions.nix.
  RONOCTUA_NIRI_ACTIONS = "${niriActions}/niri-actions.rs";
  RONOCTUA_FONT_REGULAR = regularFont;
  RONOCTUA_FONT_EMOJI = emojiFont;
  RONOCTUA_FONT_LUCIDE = lucideFont;
  # THE SAME DATA FILES `nix/rust.nix` GIVES CRANE.
  #
  # They were missing here, and the divergence is expensive in a specific way:
  # `option_env!` is resolved at COMPILE time, so `cargo test` inside the
  # devshell built binaries that believed the files did not exist. The installer
  # then found an empty layout list, refused every layout, and the screen never
  # advanced — a test that spun at full CPU instead of failing, while the same
  # code passed under `nix flake check`.
  RONOCTUA_XKB_BASE_LST = "${pkgs.xkeyboard_config}/share/X11/xkb/rules/base.lst";
  RONOCTUA_LOCALE_SUPPORTED = "${pkgs.glibcLocales}/share/i18n/SUPPORTED";
  RONOCTUA_TZ_TAB = "${pkgs.tzdata}/share/zoneinfo/zone1970.tab";
  RONOCTUA_GHOSTTY_VT = "${ghosttyVt}";
  RONOCTUA_SETSID = "${pkgs.util-linux}/bin/setsid";
  # The two process monitors and where ncurses finds its entry. Same reason as
  # everything else in this block: `option_env!` is resolved at COMPILE time, so
  # a devshell without them builds a footer that believes they do not exist —
  # `Ctrl+Alt+Esc` would answer "could not open" in the nested loop and work in
  # production, which is the divergence this block exists to close.
  RONOCTUA_BTOP = "${pkgs.btop}/bin/btop";
  RONOCTUA_BOTTOM = "${pkgs.bottom}/bin/btm";
  RONOCTUA_TERMINFO_DIRS = "${pkgs.ncurses}/share/terminfo";
  RONOCTUA_XWAYLAND_SATELLITE = "${pkgs.xwayland-satellite}/bin/xwayland-satellite";
  # THE FIVE COMPONENTS AS A TABLE, and not one variable at a time.
  #
  # What the devshell runs is a binary `cargo` built, and a `cargo` binary has
  # no wrapper — so what `nix/rust.nix` hands the installed shell has to be
  # handed over here. By hand it had already drifted: four of the five were
  # written out and `RONOCTUA_BUFFER_EDITOR` was not, so `LEADER f f` answered
  # "could not open" in the devshell and worked in production. Through the
  # table, the sixth buffer arrives on its own.
  env = bufferComponents;

  RONOCTUA_JOURNALCTL = "${pkgs.systemd}/bin/journalctl";
  RONOCTUA_NIX = "${pkgs.nix}/bin/nix";
  # The icon superpackage, for the PIXMAP step of the app
  # launcher. Without it `nix run .#dev` would show only the glyphs, and half
  # of the path that depends most on the screen would never be seen here.
  RONOCTUA_ICONS = "${iconSuperpack}/share/icons/noctua";
  # The WIT directory already merged with the generated `tokens.wit`.
  RONOCTUA_WIT_DIR = "${wit}";

  shellHook = ''
    echo "ronoctua-distro — devshell"
    echo

    # `target/` must NOT live inside the tree.
    #
    # Git is what Nix uses to filter a flake's source, and only tracked files
    # get in. `target/` is in `.gitignore` and so is already invisible — but the
    # cost of getting this wrong was measured before the repository existed,
    # when `nix flake check` copied the ENTIRE directory into the store on every
    # edit. With `target/` inside, that is 19 GB per edit; without it, 1.3 MB.
    #
    # Measured: 44 copies, 793 GB, 42% of the disk. See docs/ARCHITECTURE.md.
    export CARGO_TARGET_DIR="''${XDG_CACHE_HOME:-$HOME/.cache}/ronoctua-distro/target"
    mkdir -p "$CARGO_TARGET_DIR"

    # Installed on entering the devshell, and not by an installer step: a hook
    # lives in `.git/hooks`, which git never clones. Whoever clones this
    # repository gets the hook the first time they run `nix develop`, and not
    # from a README instruction nobody reads.
    if [ -d .git ] && [ ! -L .git/hooks/pre-commit ]; then
      mkdir -p .git/hooks
      ln -sf ${gitHooks}/hooks/pre-commit .git/hooks/pre-commit
      echo "  hook de pre-commit instalado (fmt + clippy)"
      echo
    fi

    # `tokens.wit` is GENERATED, and `wit_bindgen::generate!` takes the path
    # as a literal — there is no pointing it at the store. So it is
    # materialised in the tree, the way `nix/buffers.nix` does in the sandbox: that way
    # `../../crates/ronoctua-protocol/wit` stays true in both.
    if [ -d crates/ronoctua-protocol/wit ]; then
      mkdir -p crates/ronoctua-protocol/wit/deps/tokens
      install -m644 ${wit}/deps/tokens/tokens.wit \
        crates/ronoctua-protocol/wit/deps/tokens/tokens.wit
    fi

    echo "  nix run .#quick    os checks sem VM (~1 min) — nao fecha nada"
    echo "  nix flake check    verifica tudo"
    echo "  nix run .#dev      niri aninhado + rodape (leader = Ctrl+Alt+Space)"
    echo "  nix run .#vm       sistema completo em VM"
    echo "  nix run .#bench    mede o trabalho de CPU do frame"
    echo "  nix run .#install  instala (ensaia sem --execute)"
    echo "  nix build .#iso    gera o ISO"
    echo
  '';
}
