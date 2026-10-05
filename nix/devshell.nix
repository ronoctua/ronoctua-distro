# Development environment — section 8.
#
# "No config file the dev has to fill in by hand. No tool outside
# Nix." You enter with `direnv allow` once and never again.
{
  pkgs,
  inputs,
  gitHooks,
  wit,
  atRunTime,
  generated,
  iconSuperpack,
  # Where cargo writes (nix/cargo-dirs.nix).
  cargoDirs,
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

  # What the workspace LINKS, as in `nix/rust.nix`'s `buildInputs`: SQLite for
  # the catalogues and libpam for `ronoctua-pam`. As build inputs, so pkg-config
  # finds their `.pc` files the way crane's build does.
  buildInputs = [
    pkgs.sqlite
    pkgs.linux-pam
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

  # EVERYTHING THE BUILD BAKES, AS A TABLE — the same one crane gets.
  #
  # `option_env!` is resolved at COMPILE time, so a binary `cargo` built inside
  # the devshell has exactly what this attrset holds and nothing else. Written
  # out one at a time, eighteen of them were, this list drifted from
  # `nix/rust.nix` twice: the buffer of the editor was missing (and the file manager's key
  # answered "could not open" here and worked in production), and
  # `RONOCTUA_SYSTEMCTL` was never here at all — so `cargo test` built code that
  # believed `systemctl` did not exist.
  #
  # Splatted, the nineteenth arrives on its own. `nix/dev.nix` reads the same
  # table for the same reason, and `dev-exports-what-the-build-bakes` is the
  # guard on that side.
  env = generated // atRunTime;
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
    #
    # WHERE instead is `nix/cargo-dirs.nix`, and only there: both variables,
    # under the cache, in a directory named after this shell's compiler. The
    # path used to be spelled out here, in `.envrc` and in the loop — three
    # copies kept in step by hand.
    ${cargoDirs.exports}

    # Installed on entering the devshell, and not by an installer step: a hook
    # lives in `.git/hooks`, which git never clones. Whoever clones this
    # repository gets the hook the first time they run `nix develop`, and not
    # from a README instruction nobody reads.
    #
    # THE TEST IS WHERE IT POINTS, and not whether it exists or resolves. It
    # used to be `! -L`, and a symlink whose store path the garbage collector
    # took is still a symlink: the hook stayed dead, and git reports it as a
    # hook it cannot run, which reads like a permission problem. Then it was
    # `! -e`, which relinks a dangling one but keeps an OLD one that still
    # resolves — the commit-msg hook was changed and the link went on running
    # the first form. So each hook is relinked whenever it is not this
    # derivation's.
    if [ -d .git ]; then
      for hook in pre-commit commit-msg; do
        if [ "$(readlink ".git/hooks/$hook" || true)" != "${gitHooks}/hooks/$hook" ]; then
          mkdir -p .git/hooks
          ln -sfn "${gitHooks}/hooks/$hook" ".git/hooks/$hook"
          echo "  $hook hook installed"
        fi
      done
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

    echo "  nix run .#quick    ten chosen ones (~1 min) — a subset, not a category"
    echo "  nix run .#most     the ones that do not build an installation — before committing"
    echo "  nix run .#gate     everything, by the machine's RAM — only it closes invariant 7"
    echo "  nix run .#dev      nested compositor + footer (leader = Ctrl+Alt+Space)"
    echo "  nix run .#vm       the whole system in a VM"
    echo "  nix run .#bench    measures the frame's CPU work"
    echo "  nix run .#install  installs (rehearses without --execute)"
    echo "  nix build .#iso    builds the ISO"
    echo
  '';
}
