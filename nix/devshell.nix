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
  # `ronoctua-keys`, the checkout's text key files for cargo and its texts
  # (`nix/keys-checkout.nix`).
  keysCheckout,
}:

let
  # THE TOOLCHAIN IS A FILE OF ITS OWN (`nix/toolchain.nix`): every installed
  # system keeps the same list in its store, so `nix develop` on the distro
  # downloads no toolchain (`devshell-is-in-the-store`). What the checkout
  # generates (hooks, WIT, buffers, the `-rs` tables) follows the commit and
  # is built the first time.
  toolchain = import ./toolchain.nix { inherit pkgs; };

  # THE ICON SUPERPACK BY ITS PATH, AND NOT AS AN INPUT — here, in the
  # devshell, and only here. As an input, entering the devshell built or
  # downloaded its 0,61 GiB on every machine — the reduced profile included,
  # which leaves it out on purpose. Without the string's context the path is
  # only a name: the full profile's store has it (the system installs the same
  # derivation, `nix/modules/icons.nix`), and the hook below exports it only
  # when it is there. The `.#dev` loop, which is where the icons are SEEN,
  # keeps it as a real input (`nix/dev.nix`): running the loop brings it, on
  # any host, so no collection of garbage makes the launcher lose its
  # pixmaps without a word.
  icons = builtins.unsafeDiscardStringContext "${iconSuperpack}/share/icons/noctua";
in
pkgs.mkShell {
  name = "ronoctua-distro";

  packages = toolchain.tools;

  # What the workspace LINKS, as in `nix/rust.nix`'s `buildInputs`: SQLite for
  # the catalogues and libpam for `ronoctua-pam`. As build inputs, so pkg-config
  # finds their `.pc` files the way the units do.
  buildInputs = toolchain.links;

  # The renderer links wgpu; the loader has to find the libs at run
  # time inside the devshell.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath toolchain.runtimeLibraries;

  # Section 7.6: pin wasmtime and wit-bindgen to the same WIT version. The
  # mismatch shows up as a confusing type error at instantiation, which is
  # expensive to diagnose. Fixed here, in one place.
  RONOCTUA_WIT_VERSION = "0.2.0";

  # EVERYTHING THE BUILD BAKES, AS A TABLE — the one the units take theirs from.
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
  # The WIT directory already merged with the generated `tokens.wit`.
  RONOCTUA_WIT_DIR = "${wit}";

  shellHook = ''
    echo "ronoctua-distro — devshell"
    echo

    # The icon superpackage, for the PIXMAP step of the app launcher. Without
    # it `nix run .#dev` would show only the glyphs, and half of the path that
    # depends most on the screen would never be seen here. Only when the store
    # has it (see `icons` above); otherwise the line says how to bring it.
    if [ -d "${icons}" ]; then
      export RONOCTUA_ICONS="${icons}"
    else
      unset RONOCTUA_ICONS
      echo "  no icon superpack in the store: apps show their category glyph."
      echo "  nix build .#noctua-icon-theme --out-link ~/.cache/ronoctua-distro/icons  brings it (0.61 GiB)"
      echo
    fi

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

    # THE TEXT KEYS AND THE TEXTS, for this checkout's `cargo` and the
    # programs it builds (`nix/keys-checkout.nix`).
    #
    # The table above hands `RONOCTUA_KEYS_DIR` the evaluation's directory of
    # every crate's file — a store path that changes whenever any key does,
    # and with it every crate that speaks would recompile. Here the checkout
    # gets a directory of its own, outside the tree, kept in step with its
    # fragments by rewriting only the files that changed: a key added
    # recompiles the crate that says it. And `RONOCTUA_TEXTS` the texts of
    # the same fragments, beside it: the evaluation's are the tree's as it was
    # on entry, and a program built with a key added since would be refused
    # at its start by them (exit 78). `ronoctua-keys` (on the PATH) makes both
    # again — after a key is added or removed, or a sentence edited — and the
    # two variables keep naming them, so nothing needs the shell entered
    # again. When it cannot (a refused fragment, said above), the
    # evaluation's stay.
    #
    # THE CHECKOUT IS GIT'S TOP LEVEL, wherever the shell was entered: a
    # subdirectory with the evaluation's directory would compile against
    # another value of the variable, and cargo would recompile every crate
    # that speaks each time the two alternate.
    export PATH="${keysCheckout}/bin:$PATH"
    keys_tree=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    if [ -f "$keys_tree/Cargo.toml" ] && [ -d "$keys_tree/crates" ]; then
      if ronoctua-keys --tree "$keys_tree" > /dev/null; then
        RONOCTUA_KEYS_DIR=$(ronoctua-keys --tree "$keys_tree" --dir)
        RONOCTUA_TEXTS=$(ronoctua-keys --tree "$keys_tree" --texts-dir)
        export RONOCTUA_KEYS_DIR RONOCTUA_TEXTS
      else
        echo "  the texts and text keys of this checkout were not made: cargo compiles against the"
        echo "  evaluation's keys, and its programs read the evaluation's texts"
        echo
      fi
    fi

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

    echo "  nix run .#quick    the chosen ones (~1 min) — a subset, not a category"
    echo "  nix run .#most     the ones that do not build an installation — before committing"
    echo "  nix run .#gate     everything, by the machine's RAM — only it closes invariant 7"
    echo "  nix run .#dev      nested compositor + footer (leader = Ctrl+Alt+Space)"
    echo "  nix run .#vm       the whole system in a VM"
    echo "  nix run .#bench    measures the frame's CPU work"
    echo "  nix run .#install  installs (rehearses without --execute)"
    echo "  nix run .#iso      builds the ISO (and sweeps the old ones)"
    echo
  '';
}
