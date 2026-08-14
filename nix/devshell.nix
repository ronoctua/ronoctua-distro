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
  bufferGlyphs,
  bufferUnicode,
  bufferLucide,
  iconSuperpack,
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
    pkgs.git
    pkgs.git-lfs # the fonts live here: 4.1 GB
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
  RONOCTUA_SETSID = "${pkgs.util-linux}/bin/setsid";
  RONOCTUA_XWAYLAND_SATELLITE = "${pkgs.xwayland-satellite}/bin/xwayland-satellite";
  RONOCTUA_BUFFER_GLYPHS = "${bufferGlyphs}/lib/ronoctua_buffer_glyphs.wasm";
  RONOCTUA_BUFFER_UNICODE = "${bufferUnicode}/lib/ronoctua_buffer_unicode.wasm";
  RONOCTUA_BUFFER_LUCIDE = "${bufferLucide}/lib/ronoctua_buffer_lucide.wasm";
  # The icon superpackage, for the PIXMAP step of the app
  # launcher. Without it `nix run .#dev` would show only the glyphs, and half
  # of the path that depends most on the screen would never be seen here.
  RONOCTUA_ICONS = "${iconSuperpack}/share/icons/noctua";
  # The WIT directory already merged with the generated `tokens.wit`.
  RONOCTUA_WIT_DIR = "${wit}";

  shellHook = ''
    echo "ronoctua-distro — devshell"
    echo

    # The fonts live in git-lfs. Without `git lfs pull` the build fails much
    # later with "corrupt font", which is expensive to diagnose. We warn
    # here, early and cheaply.
    if [ -d .git ] && [ -f assets/fonts/noctua-iosevka/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf ]; then
      if head -c 60 assets/fonts/noctua-iosevka/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf \
         | grep -q 'git-lfs.github.com/spec'; then
        echo "  AVISO: as fontes sao ponteiros git-lfs, nao arquivos."
        echo "         Rode:  git lfs install && git lfs pull"
        echo
      fi
    fi

    # `target/` must NOT live inside the tree.
    #
    # This project is not a git repository (invariant 2), and it is git that Nix
    # uses to filter a flake's source. Without it, `nix flake check` copies
    # the ENTIRE directory into the store — and since the path is addressed by
    # content, every edit mints a fresh copy. With `target/` inside,
    # that is 19 GB per edit; without it, 1.3 MB.
    #
    # Measured: 44 copies, 793 GB, 42% of the disk. See docs/ARCHITECTURE.md.
    export CARGO_TARGET_DIR="''${XDG_CACHE_HOME:-$HOME/.cache}/ronoctua-distro/target"
    mkdir -p "$CARGO_TARGET_DIR"

    # The hooks only get installed if the repository already exists. Invariant 2
    # forbids creating it, and `git init` is the owner's call — until then this does nothing.
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

    echo "  nix flake check    verifica tudo"
    echo "  nix run .#dev      niri aninhado + rodape (leader = Alt+Space)"
    echo "  nix run .#vm       sistema completo em VM"
    echo "  nix run .#bench    mede o trabalho de CPU do frame"
    echo "  nix run .#install  instala (ensaia sem --executar)"
    echo "  nix build .#iso    gera o ISO"
    echo
  '';
}
