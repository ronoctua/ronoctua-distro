# Common Lisp, in both profiles — the terminal's, and only the terminal's.
#
# # Who reads it
#
# `nix/modules/dev.nix` puts it on the PATH through the program registry
# (`ronoctua.apps.common-lisp`, carried in both profiles like rust), and
# `flake.nix` keeps what it downloads (`sources`) under the garbage
# collector's root. Each calls this file with the `pkgs` it has, the shape
# `nix/toolchain.nix` has for the same reason.
#
# Emacs is not touched by any of it (the owner's decision of 2026-10-19:
# plain Emacs, the person's configuration): SLIME, SLY or an LSP client is
# the person's to add, and swank, slynk and alive-lsp are here for whoever
# connects.
#
# # Three Lisps, one set of libraries
#
# SBCL is the one the libraries are BUILT for: Hydra builds `sbclPackages`,
# so the set below costs a download and no compilation, and loading it writes
# no fasl anywhere (each store source is translated onto itself). CCL and ECL
# read the SAME sources — the SBCL set's packages are source trees with the
# fasls beside them — and compile each library the first time it is loaded,
# into `~/.cache/common-lisp`. A `ccl.withPackages` would have been compiled
# on the machine at every rebuild, or seeded on the ISO: Hydra builds none of
# it.
#
# # Why ASDF is told by a CONFIGURATION DIRECTORY, and not by its variables
#
# nixpkgs' `withPackages` wrapper prefixes `CL_SOURCE_REGISTRY` and
# `ASDF_OUTPUT_TRANSLATIONS` with values that end in `:` — "and then what
# was configured before". A value the person set that ends in `:` too, or the
# SBCL's own value inherited by a CCL started from it, makes TWO inherits, and
# ASDF refuses to start: "Only one inherited configuration allowed". So the
# three Lisps leave both variables alone, and each has `asdfConfig` first on
# its `XDG_CONFIG_DIRS`, where ASDF reads the system's configuration: after
# the person's variable and the person's own files, never instead of them.
# What is SBCL's only — the translation of the store onto itself, swank and
# alive-lsp — is under `#+sbcl`, so a CCL that inherits the directory reads
# none of it.
{
  pkgs,
  lib ? pkgs.lib,
}:

let
  inherit (pkgs) sbcl;

  # THE LIBRARIES, the ones a Common Lisp program of today reaches for first:
  # the utilities (alexandria, serapeum, str, iterate), regular expressions,
  # threads and parallelism, pattern matching, time, HTTP (dexador) and JSON
  # (jzon), logging, two test frameworks, the foreign-function interface, and
  # the two editor servers (swank for SLIME, slynk for SLY).
  libraries =
    ps: with ps; [
      alexandria
      serapeum
      cl-ppcre
      str
      bordeaux-threads
      lparallel
      iterate
      trivia
      local-time
      dexador
      jzon # nixpkgs' alias of `com_dot_inuoe_dot_jzon`
      log4cl
      fiveam
      parachute
      cffi
      swank
      slynk
    ];

  # OURS, because nixpkgs has no package of it: the language server VS Code's
  # Alive extension speaks to, usable by any LSP client.
  aliveLsp = import ./alive-lsp.nix { inherit pkgs; };

  # What `sbcl` sees: the set and alive-lsp.
  offered = libraries sbcl.pkgs ++ [ aliveLsp ];

  # EVERY LIBRARY THERE IS, and what each stands on: the closure over
  # `lispLibs`. Keyed by the store path, which is what makes two mentions of
  # one library one.
  closure = map (item: item.package) (
    builtins.genericClosure {
      startSet = map (p: {
        key = p.outPath;
        package = p;
      }) offered;
      operator =
        item:
        map (p: {
          key = p.outPath;
          package = p;
        }) (item.package.lispLibs or [ ]);
    }
  );
  # SBCL'S ONLY: swank's loader writes its fasls into the store path it was
  # built in (nixpkgs' `swank-pure-paths.patch`), which another Lisp cannot
  # do — a SLIME that connects to CCL or ECL brings its own swank — and Alive
  # reads `sb-introspect`.
  sbclOnly =
    p:
    builtins.elem p.pname [
      "swank"
      "alive-lsp"
    ];

  # The configuration ASDF reads from `XDG_CONFIG_DIRS`: each file of a
  # `.conf.d` directory is a list of directives, and the directory inherits
  # what comes after it by itself.
  sourceRegistry = lib.concatMapStrings (
    p: (if sbclOnly p then "#+sbcl " else "") + ''(:tree "${p}/")'' + "\n"
  ) closure;
  outputTranslations = lib.concatMapStrings (p: ''#+sbcl ("${p}/" "${p}/")'' + "\n") closure;
  asdfConfig = pkgs.linkFarm "ronoctua-common-lisp-asdf" [
    {
      name = "common-lisp/source-registry.conf.d/50-ronoctua.conf";
      path = pkgs.writeText "50-ronoctua-source-registry.conf" sourceRegistry;
    }
    {
      name = "common-lisp/asdf-output-translations.conf.d/50-ronoctua.conf";
      path = pkgs.writeText "50-ronoctua-output-translations.conf" outputTranslations;
    }
  ];

  # The foreign libraries the set declares (the OpenSSL cl+ssl opens).
  nativeLibraryPath = lib.makeLibraryPath (lib.concatMap (p: p.nativeLibs or [ ]) closure);

  # A LISP'S ENTRY POINT, rewrapped: the configuration first on
  # `XDG_CONFIG_DIRS`, and `/etc/xdg` after everything — the specification's
  # default, which a variable that was unset would otherwise lose.
  wrapArgs = ''
    --prefix XDG_CONFIG_DIRS : ${asdfConfig} \
    --suffix XDG_CONFIG_DIRS : /etc/xdg \
    --prefix LD_LIBRARY_PATH : ${lib.escapeShellArg nativeLibraryPath}'';

  # SBCL's, with the ASDF the set was built with (`$ASDF`, which the seeded
  # `~/.sbclrc` loads) and nixpkgs' own flags (the heap).
  probe = sbcl.pkgs.alexandria;
  sbclWrapped =
    pkgs.runCommand "sbcl-with-libraries-${sbcl.version}"
      {
        nativeBuildInputs = [ pkgs.makeBinaryWrapper ];
      }
      ''
        makeWrapper ${sbcl}/bin/sbcl "$out/bin/sbcl" \
          --add-flags ${lib.escapeShellArg (toString probe.flags)} \
          --set ASDF "${probe.asdfFasl}/asdf.${probe.faslExt}" \
          ${wrapArgs}
      '';

  # A LISP WHOSE ENTRY POINTS ARE REWRAPPED, and everything else of its
  # package kept as it is (its manual, ECL's `ecl-config`). `ASDF` is the
  # SBCL's compiled ASDF: inherited by a CCL started from an SBCL, a seed that
  # read it would load SBCL's fasl into CCL.
  rewrapped =
    package: programs:
    pkgs.symlinkJoin {
      name = "${package.pname}-${package.version}";
      paths = [ package ];
      nativeBuildInputs = [ pkgs.makeBinaryWrapper ];
      postBuild = lib.concatMapStrings (program: ''
        rm "$out/bin/${program}"
        makeWrapper ${package}/bin/${program} "$out/bin/${program}" \
          --unset ASDF \
          ${wrapArgs}
      '') programs;
      inherit (package) meta;
    };

  # WHAT `quicklisp.lisp` WOULD FETCH, pinned: the client, its `setup.lisp`,
  # the ASDF it carries and the description that names all three — over
  # HTTPS, with the hashes the description itself publishes. NOT
  # `quicklisp.lisp`: it fetches only over `http://` and checks no hash.
  clientVersion = "2021-02-13";
  setupVersion = "2021-02-11";
  quicklispFiles = {
    tar = pkgs.fetchurl {
      url = "https://beta.quicklisp.org/client/${clientVersion}/quicklisp.tar";
      hash = "sha256-qKPIyRtR3RhRdautTXw5meu04lIL5abO4hJwNaxsh74=";
    };
    setup = pkgs.fetchurl {
      url = "https://beta.quicklisp.org/client/${setupVersion}/setup.lisp";
      hash = "sha256-VJ/j5+DyZp2u3phDfJnNYOAsC4U209E1yaqdNG7ZUbY=";
    };
    asdf = pkgs.fetchurl {
      url = "https://beta.quicklisp.org/asdf/3.2.1/asdf.lisp";
      hash = "sha256-UZEvP3wsYsIE9RXZc0bVYBFSg5m78KdqEjMYND69i/A=";
    };
    clientInfo = pkgs.fetchurl {
      url = "https://beta.quicklisp.org/client/${clientVersion}/client-info.sexp";
      hash = "sha256-tPUVxe0gTZ+k6oY35gwLEldWCfRQ8mkZ2/uDXuKZM+A=";
    };
  };

  # The fetcher `quicklisp-install` puts in `local-init/`: a file of the
  # person's home, so it names `/run/current-system/sw/bin/curl` and no
  # store path a collection could take.
  httpsFetcher = ../../assets/common-lisp/quicklisp-https.lisp;
in
{
  inherit
    libraries
    aliveLsp
    asdfConfig
    clientVersion
    ;

  # SBCL: the package as nixpkgs builds it — its manual and info pages, and
  # `lib/sbcl` with the source `M-.` reaches — whose `bin/sbcl` is the one that
  # sees the libraries. A JOIN and not `hiPrio` beside the package: two
  # `bin/sbcl` in `system.path` are decided by the order of the list, in
  # silence.
  sbcl = pkgs.symlinkJoin {
    name = "sbcl-${sbcl.version}";
    paths = [ sbcl ];
    postBuild = ''
      rm "$out/bin/sbcl"
      ln -s ${sbclWrapped}/bin/sbcl "$out/bin/sbcl"
    '';
    # What its `sbcl` offers, for the checks (`common-lisp-in-both-profiles`).
    passthru.libraries = offered;
    inherit (sbcl) meta;
  };

  # CCL (`ccl` and the kernel's own name, `lx86cl64`) and ECL, reading the
  # same libraries' sources.
  ccl = rewrapped pkgs.ccl [
    "ccl"
    "lx86cl64"
  ];
  ecl = rewrapped pkgs.ecl [ "ecl" ];

  # QLOT, a project's pinned libraries (`qlfile.lock`) under its `.qlot`.
  # What it starts sees the PROJECT's libraries and nothing of the system's:
  # the bare Lisps ahead on its PATH, the system's ASDF configuration taken
  # off `XDG_CONFIG_DIRS`, and `ASDF` and `ASDF_OUTPUT_TRANSLATIONS` unset — a
  # qlot started from inside our `sbcl` inherits all three. Our curl goes
  # first too: qlot's own HTTPS fetcher runs `curl -sSL`, which follows a
  # redirect to plain HTTP, and that is a limit of qlot's, said in
  # `docs/ARCHITECTURE.md`.
  qlot = pkgs.writeShellApplication {
    name = "qlot";
    runtimeInputs = [
      pkgs.curl
      sbcl
      pkgs.ccl
      pkgs.ecl
    ];
    text = ''
      kept=""
      IFS=: read -r -a dirs <<< "''${XDG_CONFIG_DIRS:-}"
      for dir in "''${dirs[@]}"; do
        if [ -n "$dir" ] && [ "$dir" != ${asdfConfig} ]; then
          kept="''${kept:+$kept:}$dir"
        fi
      done
      if [ -n "$kept" ]; then
        export XDG_CONFIG_DIRS="$kept"
      else
        unset XDG_CONFIG_DIRS
      fi
      unset ASDF ASDF_OUTPUT_TRANSLATIONS
      exec ${lib.getExe pkgs.sbclPackages.qlot-cli} "$@"
    '';
  };

  # `alive-lsp [--port N] [--stdio]`: the server on a free loopback port, or
  # bridged to standard input and output (`setpriv` ties the server's life
  # to the bridge's).
  aliveLspCommand = pkgs.writeShellApplication {
    name = "alive-lsp";
    runtimeInputs = [
      pkgs.socat
      pkgs.coreutils
      pkgs.util-linux
    ];
    text = "sbcl=${sbclWrapped}/bin/sbcl\n" + builtins.readFile ./alive-lsp.sh;
  };

  quicklisp = quicklispFiles // {
    # `quicklisp-install [--path DIR]`: Quicklisp in the home, built from the
    # pinned files, fetching what comes after over HTTPS.
    install = pkgs.writeShellApplication {
      name = "quicklisp-install";
      runtimeInputs = [
        pkgs.coreutils
        pkgs.findutils
        pkgs.gnutar
      ];
      text = ''
        tar_file=${quicklispFiles.tar}
        setup_file=${quicklispFiles.setup}
        asdf_file=${quicklispFiles.asdf}
        client_info_file=${quicklispFiles.clientInfo}
        fetcher_file=${httpsFetcher}
        sbcl=${sbclWrapped}/bin/sbcl
        client_version=${clientVersion}
      ''
      + builtins.readFile ./quicklisp-install.sh;
    };
  };

  # THE THREE INIT FILES, seeded into a home where each is missing:
  # `ronoctua-common-lisp-seed HOME [USER GROUP]`. With a user, every write is
  # that user's own (`setpriv`), never root's in a folder the person owns —
  # root following a link the person planted would write where root may. A
  # file or a link that is there, a dangling one included, is left alone.
  seed = pkgs.writeShellApplication {
    name = "ronoctua-common-lisp-seed";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.util-linux
    ];
    text = ''
      home=$1
      user=''${2:-}
      group=''${3:-}
      plant() {
        if [ -e "$home/$2" ] || [ -L "$home/$2" ]; then
          return 0
        fi
        if [ -n "$user" ]; then
          setpriv --reuid="$user" --regid="$group" --init-groups install -m 0644 "$1" "$home/$2"
        else
          install -m 0644 "$1" "$home/$2"
        fi
      }
      plant ${../../assets/common-lisp/sbclrc.lisp} .sbclrc
      plant ${../../assets/common-lisp/ccl-init.lisp} .ccl-init.lisp
      plant ${../../assets/common-lisp/eclrc.lisp} .eclrc
    '';
  };

  # WHAT THIS FILE DOWNLOADS, for the root `nix/kept-sources.nix` holds:
  # alive-lsp's tag would come back from GitHub, but Quicklisp's client is a
  # file on one server.
  sources = [
    aliveLsp.fetched
    quicklispFiles.tar
    quicklispFiles.setup
    quicklispFiles.asdf
    quicklispFiles.clientInfo
  ];
}
