# THE PERSON'S BINARY CACHE, as the machine sees it.
#
# The distro has no binary cache of its own: its ISO carries its seed, and a
# machine builds what only this repository makes. The PERSON may have one —
# their own Cachix cache, named in their `options.json`
# (`ronoctua_person::options::BinaryCache`: its name, its public key, public
# or private), which travels in the plain part of their belongings, SIGNED,
# and reaches this machine as the root applier's validated copy
# (`ronoctua.person.options`, `nix/machine.nix`). Nothing else here declares
# a substituter or a key (`binary-cache-declared-once`).
#
# WHAT IT DOES, only when that copy names a cache the judge takes
# (`nix/binary-cache.nix`, held to the Rust validator by one table):
#
#   - the cache comes FIRST among the substituters, BEFORE `cache.nixos.org`
#     and never instead of it (NixOS appends the public cache with
#     `mkAfter`), with its key among the trusted ones;
#   - a PRIVATE cache is read through root's netrc (`nix.settings.netrc-file`),
#     which the sudo door writes when the person sets the token, and the
#     installer at installation — never this module: the token is a secret,
#     and a secret in a Nix expression is a secret in the world-readable
#     store;
#   - `/etc/ronoctua/binary-cache.json` says what the update's push needs
#     (`ronoctua_cachix::system::System`): the cache, and — for a PUBLIC one —
#     the distro's own outputs it may receive and the unfree names it may
#     never receive.
#
# A COPY THAT NAMES NO CACHE, or one the judge refuses, adds nothing at all:
# `cache.nixos.org` stays the only substituter, and the push declines,
# said. A refused value is not mended into a plausible cache.
#
# # Why the public roots are the distro's evaluation, not this machine's
#
# A public cache is redistribution: it must never receive a path that
# carries the person's data (their host name, their user, `/etc`, their
# declared programs, their brain modules). The roots it may receive are the
# distro's own seed — what its ISO carries — AS THE DISTRO EVALUATES IT
# (`publicRoots`, set by `flake.nix` from the installer's own list): a store
# path is a function of everything that went into it, so a path the distro's
# evaluation gives cannot hold anything of the person's. A machine whose shell
# carries the person's brain modules builds ANOTHER shell, which is not among
# them. They are written without their string context: the list costs the
# system no reference, and a root this machine never built is not valid in
# its store and is skipped by the push.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.binaryCache;
  shared = import ../binary-cache.nix;
  user = config.ronoctua.user;

  file = config.ronoctua.person.options or null;
  said = if file == null then null else builtins.fromJSON (builtins.readFile file);
  cache = shared.judge said;

  # A STORE PATH'S NAME, from a value that names one (`/nix/store/<hash>-uade-…/bin/uade`),
  # without the string's context: the names are asked about, never built.
  storeNameOf =
    value:
    let
      plain = builtins.unsafeDiscardStringContext value;
      rest = lib.removePrefix "${builtins.storeDir}/" plain;
      first = builtins.head (lib.splitString "/" rest);
    in
    if builtins.isString value && lib.hasPrefix "${builtins.storeDir}/" plain then
      builtins.substring 33 (-1) first
    else
      null;
  storeNamesIn = values: builtins.filter (n: n != null && n != "") (map storeNameOf values);
  nameOf = p: if p ? name then builtins.unsafeDiscardStringContext p.name else "";

  # THE NAMES THIS MACHINE CARRIES, for the unfree question: its packages,
  # the person's own, and every program a session or a user unit is handed
  # by store path (the player's `uade`, the archives' `unrar` come in by a
  # nixpkgs of their own, `nix/release-unfree.nix` says why).
  candidates = lib.unique (
    builtins.filter (n: n != "") (
      map nameOf config.environment.systemPackages
      ++ map nameOf (config.home-manager.users.${user}.home.packages or [ ])
      ++ storeNamesIn (
        builtins.filter builtins.isString (lib.attrValues config.environment.sessionVariables)
      )
      ++ storeNamesIn (
        builtins.filter builtins.isString (
          lib.concatMap (s: lib.attrValues (s.environment or { })) (
            lib.attrValues config.systemd.user.services
          )
        )
      )
      ++ config.ronoctua.person.unfreeNames
    )
  );
  # WHICH OF THEM ARE UNFREE, asked by the release's own three questions
  # (`nix/release-unfree.nix`) of this machine's packages and predicate — by
  # package name, which is what the push compares.
  unfree = lib.unique (
    map (n: (builtins.parseDrvName n).name) (
      import ../release-unfree.nix {
        config = config.nixpkgs.config;
        inherit pkgs;
        names = candidates;
      }
    )
  );

  public = cache != null && !cache.private;

  # WHAT THE PUSH READS (`ronoctua_cachix::system::System`).
  systemFile = {
    format = shared.format;
    cache = {
      inherit (cache) name visibility;
      public-key = cache.publicKey;
    };
    public-roots = if public then map builtins.unsafeDiscardStringContext cfg.publicRoots else [ ];
    unfree = if public then unfree else [ ];
    android-unfree = public && (config.nixpkgs.config.allowUnfree or false);
  };
in
{
  options.ronoctua.binaryCache = {
    tool = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = null;
      internal = true;
      description = ''
        The push's program, EXTRACTED from the workspace (`rust.cachix`):
        `ronoctua-cachix push`, the update's last step. Set by the flake next
        to the import of this module.
      '';
    };

    publicRoots = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      internal = true;
      description = ''
        The distro's own outputs a PUBLIC cache may receive: the store paths
        of its ISO's seed, as the distro evaluates them, without their string
        context. Set by the flake.
      '';
    };

    cache = lib.mkOption {
      type = lib.types.nullOr (lib.types.attrsOf lib.types.unspecified);
      readOnly = true;
      default = cache;
      defaultText = "the cache the person's options.json names, judged (nix/binary-cache.nix)";
      description = ''
        The person's cache as this machine takes it — `{ name; publicKey;
        visibility; private; host; substituter; }` — or `null`: no cache, or
        one the judge refuses. Read once, here.
      '';
    };

    pusher = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      readOnly = true;
      internal = true;
      default =
        if cfg.tool == null then
          null
        else
          pkgs.runCommand "ronoctua-cachix-for-the-update" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
            makeWrapper ${cfg.tool}/bin/ronoctua-cachix $out/bin/ronoctua-cachix \
              --set-default RONOCTUA_CACHIX_NIX ${config.nix.package}/bin \
              --set-default RONOCTUA_CACHIX_MACHINE_STATE ${
                config.ronoctua.machine.stateDir or "/var/lib/ronoctua-machine"
              }/state.json ${
                # `cachix` ONLY WHERE A CACHE IS NAMED: the program is a
                # Haskell closure, and a machine with no cache never runs it
                # (the push declines without the machine's file).
                lib.optionalString (
                  cache != null
                ) "--set-default RONOCTUA_CACHIX ${lib.getExe' pkgs.cachix "cachix"}"
              }
          '';
      defaultText = "`tool`, wrapped with the machine's nix, its record, and cachix where a cache is named";
      description = ''
        What the update's push step runs: `tool`, told where the machine's
        `nix` and its record are, and — where a cache is named — `cachix`.
      '';
    };
  };

  config = lib.mkMerge [
    # THE FOLDER OF ROOT'S NETRC, always there: `0755` so the screen can say
    # whether the netrc is (the file itself is root's, `0600`).
    { systemd.tmpfiles.rules = [ "d ${shared.netrcDir} 0755 root root -" ]; }
    (lib.mkIf (cache != null) {
      nix.settings.substituters = [ cache.substituter ];
      nix.settings.trusted-public-keys = [ cache.publicKey ];
      environment.etc."ronoctua/binary-cache.json".text = builtins.toJSON systemFile;
    })
    (lib.mkIf (cache != null && cache.private) {
      nix.settings.netrc-file = shared.netrc;
    })
  ];
}
