# The person's declarations, as the system consumes them.
#
# The person's data (`software/declared.json`, `software/declined.json`) is
# validated by a narrow root unit and copied into the machine directory's
# `person/` — data, never Nix code. `lib.machine` points the options below at
# that copy (`nix/machine.nix`), and this module turns it into the machine:
#
#   nix       a part of the person's                (`declared-apps/nix.nix`)
#   appimage  a part of the person's, wrapped       (`declared-apps/appimage.nix`)
#   flatpak   nix-flatpak's list for `--user`      (`declared-apps/flatpak.nix`)
#   web       a launcher entry (`xdg.desktopEntries`) (`declared-apps/web.nix`)
#   android   a fetched apk, listed for the reconciler (`declared-apps/android.nix`)
#
# NOTHING GOES TO `systemPackages` (`person-packages-are-per-user`): these are
# the person's programs, carried from machine to machine by their
# declaration, and they are on this machine because THEY declared them — not
# because the distro ships them.
#
# # A package of the person's is a part
#
# One `abort` deep in a declared package, or an evaluation error no
# `tryEval` catches, used to stop the whole system's evaluation, and the
# update evaluated each entry alone first to hold the broken one back. Now
# the system never evaluates a declared package at all: a `nix` or
# `appimage` entry is a PART (`nix/modules/parts.nix`), which the plan names
# by the entry and its fingerprint, and the part's realizer evaluates that
# one entry in a process of its own (`ronoctua.person.parts`, the machine
# flake's `personParts`). A part that does not evaluate or build is that
# part's failure alone, tried again ever later, and says why on the parts'
# page; the one it had before stays.
#
# # It degrades, and says so
#
# An entry that cannot apply — declared for another machine, a source this
# machine lacks, a second entry with the same id, a Flatpak remote that is
# not here — is SKIPPED, and the system evaluates without it
# (`person-bad-entry-does-not-break-eval`). Each skip is a line in
# `/etc/ronoctua/person-applied.json` with its reason from a closed set, which
# is what the software centre draws. A file in a format this distro does not
# read skips everything it lists, said once per entry.
#
# What cannot be degraded is a file that is not JSON (`builtins.fromJSON` does
# not throw, it fails) — which is why only the root applier's validated copy is
# ever read here, and never the person's own staging.
{
  config,
  options,
  lib,
  pkgs,
  inputs,
  ...
}:

let
  cfg = config.ronoctua.person;
  user = config.ronoctua.user;
  host = config.networking.hostName;

  # The format of the person's files (`ronoctua_person::FORMAT`).
  format = 1;

  # THE SAME nixpkgs WITH EVERY UNFREE LICENCE TAKEN: where a program the
  # person consented to is refused only for what it NEEDS (`steam` needs
  # `steam-unwrapped`), the consent covers that too (`declared-apps/nix.nix`).
  # Lazy: a machine that never meets that case never instantiates it.
  wide = import inputs.nixpkgs {
    inherit (pkgs.stdenv.hostPlatform) system;
    inherit (config.nixpkgs) overlays;
    config = config.nixpkgs.config // {
      allowUnfree = true;
    };
  };

  builders = {
    nix = import ../declared-apps/nix.nix { inherit lib pkgs wide; };
    flatpak = import ../declared-apps/flatpak.nix { inherit lib pkgs; };
    appimage = import ../declared-apps/appimage.nix { inherit lib pkgs; };
    web = import ../declared-apps/web.nix { inherit lib pkgs; };
    android = import ../declared-apps/android.nix { inherit lib pkgs; };
  };

  # What `<source>:<source-id>` is made of, per source.
  sourceIdOf = {
    nix = e: e.attr;
    flatpak = e: e."app-id";
    appimage = e: e.key;
    web = e: e.slug;
    android = e: e.package;
  };

  # WHICH SOURCES THIS MACHINE HAS, asked of the modules that turned them on
  # (the shape `nix/modules/software.nix` asks in).
  #
  # Flatpak by OUR switch, and not by `services.flatpak.enable`: a person's
  # Flatpak goes on nix-flatpak's Home Manager list, and that module is
  # imported only under `ronoctua.software.flatpak` (`nix/modules/flatpak.nix`).
  # A host that turns the system's Flatpak on by hand has no such list, and
  # defining it there would stop the whole machine evaluating.
  sourceOn = {
    nix = true;
    flatpak = config.ronoctua.software.flatpak or false;
    appimage = config.programs.appimage.enable or false;
    web = config.ronoctua.browser.enable or false;
    android = config.virtualisation.waydroid.enable or false;
  };
  # nix-flatpak installs only from a remote it declares.
  flatpakRemotes = map (r: r.name) (config.services.flatpak.remotes or [ ]);

  read = path: if path == null then null else builtins.fromJSON (builtins.readFile path);

  # ---------------------------------------------------------------------
  # declared.json
  # ---------------------------------------------------------------------
  declaredDoc = read cfg.data;
  declaredFormat = if declaredDoc == null then format else declaredDoc.format or null;
  apps = if declaredDoc == null then [ ] else declaredDoc.apps or null;
  readable = declaredFormat == format && builtins.isList apps;

  idOf = e: if builtins.isAttrs e && builtins.isString (e.id or null) then e.id else "unknown";

  # Every reason an entry's SHAPE is wrong, before anything is looked up.
  problemOf =
    e:
    if !builtins.isAttrs e then
      "an entry is not an object"
    else if !builtins.isString (e.id or null) then
      "`id` is missing"
    else if !builtins.isString (e.source or null) then
      "`source` is missing"
    else if !(builders ? ${e.source}) then
      "`${e.source}` is not a source this distro reads"
    else if
      !(
        (e.machines or null) == null
        || (builtins.isList e.machines && builtins.all builtins.isString e.machines)
      )
    then
      "`machines` is neither null nor a list of names"
    else
      let
        own = builders.${e.source}.shape e;
      in
      if own != null then
        own
      else if e.id != "${e.source}:${sourceIdOf.${e.source} e}" then
        "`id` does not name its entry"
      else
        null;

  applies = e: (e.machines or null) == null || builtins.elem host e.machines;

  # THE FINGERPRINT of an entry that is a part: the entry without its stamps,
  # and the nixpkgs it is built against. Another one is another target, which
  # the part's realizer evaluates anew (`ronoctua_parts::record`); the same
  # one keeps what it evaluated. Computed ONLY here.
  fingerprintOf =
    e:
    builtins.hashString "sha256" (
      builtins.toJSON {
        entry = removeAttrs e [
          "edited-at"
          "edited-on"
          "machines"
        ];
        nixpkgs = inputs.nixpkgs.narHash or "";
      }
    );

  # WHICH SOURCES ARE PARTS: a program Nix builds — a nixpkgs package, an
  # AppImage wrapped — realized after the system on its own
  # (`nix/modules/parts.nix`), never in the system's closure: the system
  # never evaluates what the person declared, so an entry that aborts stops
  # nothing but its own part. A Flatpak and a web app are lists and
  # launchers, and an Android app is the reconciler's.
  isPart =
    e:
    builtins.elem e.source [
      "nix"
      "appimage"
    ];

  # A PART'S ID: `person-`, eight hex digits of the entry's id (no two
  # entries share it) and as much of the entry's own name as fits the shape
  # of a part's id (`ronoctua_parts::id::is_id`).
  partIdOf =
    e:
    let
      words = builtins.filter (w: builtins.isString w && w != "") (
        builtins.split "[^a-z0-9]+" (lib.toLower (sourceIdOf.${e.source} e))
      );
      slug = lib.concatStringsSep "-" words;
      cut = lib.concatStringsSep "-" (
        builtins.filter (w: w != "") (lib.splitString "-" (builtins.substring 0 32 slug))
      );
    in
    "person-"
    + builtins.substring 0 8 (builtins.hashString "sha256" e.id)
    + lib.optionalString (cut != "") "-${cut}";

  skip = id: reason: detail: {
    ok = false;
    inherit id reason detail;
  };

  # ONE ENTRY'S FATE, in the order a person would ask about it.
  judge =
    e:
    let
      problem = problemOf e;
    in
    if problem != null then
      skip (idOf e) "format-unknown" problem
    else if !applies e then
      skip e.id "other-machine" "declared for ${lib.concatStringsSep ", " e.machines}"
    else if !sourceOn.${e.source} then
      skip e.id "source-off" "this machine has no ${e.source} support"
    # A PART IS APPLIED AS A PART: what it builds is its realizer's question.
    else if isPart e then
      {
        ok = true;
        inherit (e) id;
        part = partIdOf e;
        packages = [ ];
        flatpaks = [ ];
        desktopEntries = { };
        apks = [ ];
      }
    else
      let
        r = builders.${e.source}.build {
          entry = e;
          inherit (cfg) icons;
          remotes = flatpakRemotes;
        };
      in
      if r.ok then r // { inherit (e) id; } else skip e.id r.reason r.detail;

  # A SECOND ENTRY WITH AN ID ALREADY SEEN is the file's fault, not the
  # entry's: the first one is the declaration, and the second is said.
  firsts =
    builtins.foldl'
      (
        acc: e:
        let
          id = idOf e;
        in
        if id != "unknown" && acc.seen ? ${id} then
          acc // { dups = acc.dups ++ [ (skip id "format-unknown" "a second entry with this id") ]; }
        else
          acc
          // {
            kept = acc.kept ++ [ e ];
            seen = acc.seen // {
              ${id} = true;
            };
          }
      )
      {
        kept = [ ];
        dups = [ ];
        seen = { };
      }
      (if readable then apps else [ ]);

  results =
    if readable then
      map judge firsts.kept ++ firsts.dups
    else if builtins.isList apps then
      map (
        e:
        skip (idOf e) "format-unknown"
          "declared.json says format ${builtins.toJSON declaredFormat}; this distro reads ${toString format}"
      ) apps
    else
      [
        (skip "declared.json" "format-unknown" "declared.json has no list of apps this distro can read")
      ];

  applied = builtins.filter (r: r.ok) results;
  skipped = map (r: { inherit (r) id reason detail; }) (builtins.filter (r: !r.ok) results);

  # THE ENTRIES THAT ARE PARTS: this machine's, with a source it has.
  partEntries = builtins.filter (
    e: problemOf e == null && applies e && sourceOn.${e.source} && isPart e
  ) firsts.kept;

  # ONE PART'S TARGET, as its realizer asks it (`lib.machineFlake`'s
  # `personParts`): the entry's own build — the protected one, so a refusal
  # is said as one — or why not. Evaluated by that realizer alone, in its own
  # process: an entry that aborts stops nothing else.
  plain = builtins.unsafeDiscardStringContext;
  targetOf =
    e:
    let
      r = builders.${e.source}.build {
        entry = e;
        inherit (cfg) icons;
      };
      package =
        if builtins.length r.packages == 1 then
          builtins.head r.packages
        else
          pkgs.buildEnv {
            name = "person-${e.source}-${sourceIdOf.${e.source} e}";
            paths = r.packages;
          };
    in
    {
      fingerprint = fingerprintOf e;
    }
    // (
      if r.ok then
        {
          drv = plain package.drvPath;
          out = plain package.outPath;
        }
      else
        {
          refused = r.reason;
          inherit (r) detail;
        }
    );

  # ---------------------------------------------------------------------
  # declined.json, against the distro's registry (`ronoctua.apps`)
  # ---------------------------------------------------------------------
  declinedDoc = read cfg.declined;
  declinedList =
    if
      declinedDoc != null
      && (declinedDoc.format or null) == format
      && builtins.isList (declinedDoc.apps or null)
    then
      builtins.filter (
        d:
        builtins.isAttrs d
        && builtins.isString (d.id or null)
        && (
          (d.machines or null) == null
          || (builtins.isList d.machines && builtins.all builtins.isString d.machines)
        )
      ) declinedDoc.apps
    else
      [ ];
  declinedHere = map (d: d.id) (builtins.filter applies declinedList);

  registry = config.ronoctua.apps or { };
  # A decline names the id the person saw; a renamed app keeps it alive by
  # `formerly` (`distro-app-ids-are-stable`).
  resolve =
    id:
    if registry ? ${id} then
      id
    else
      lib.findFirst (n: builtins.elem id (registry.${n}.formerly or [ ])) null (
        builtins.attrNames registry
      );
  resolved = map (id: {
    inherit id;
    app = resolve id;
  }) declinedHere;
  known = builtins.filter (d: d.app != null) resolved;
  honoured = builtins.filter (d: registry.${d.app}.essential == null) known;
  refused = map (d: {
    inherit (d) id;
    reason = registry.${d.app}.essential;
  }) (builtins.filter (d: registry.${d.app}.essential != null) known);

  # WHAT TO LOOK FOR IN `system.path`, per honoured decline: an app pulled in
  # by another road is still there, and the report says so. The package's
  # path goes in WITHOUT its context — naming it must not build it.
  probes = map (
    d:
    let
      a = registry.${d.app};
    in
    {
      inherit (d) id;
      desktop-ids = a.desktopIds or [ ];
      package =
        if (a.package or null) == null then
          null
        else
          builtins.unsafeDiscardStringContext (toString a.package);
    }
  ) honoured;

  # ---------------------------------------------------------------------
  # the person's signature, as root's applier left it (`local/signer.json`)
  # ---------------------------------------------------------------------
  # Root takes from a staging only what the person signed under the key it
  # pinned; when it refuses one, or keeps a file back, it says so in the pin
  # — and the report carries that, so the screens say WHY a declaration did
  # not arrive. A pin in a format this distro does not read reads as none.
  signerDoc = read cfg.signer;
  signature =
    if signerDoc != null && (signerDoc.format or null) == format then
      {
        pinned = signerDoc.fingerprint or null;
        counter = signerDoc.counter or 0;
        refused = signerDoc.refused or null;
        kept-back = signerDoc."kept-back" or [ ];
      }
    else
      {
        pinned = null;
        counter = 0;
        refused = null;
        kept-back = [ ];
      };

  # ---------------------------------------------------------------------
  # the report
  # ---------------------------------------------------------------------
  report = {
    inherit format;
    machine = host;
    distro-rev = config.ronoctua.machine.distroRev;
    applied = map (r: r.id) applied;
    # Which part each applied entry that is one became, by the entry's id:
    # what the software centre and the reconciler ask the parts' records of.
    parts = lib.listToAttrs (
      map (r: lib.nameValuePair r.id r.part) (builtins.filter (r: r ? part) applied)
    );
    inherit skipped;
    declined = {
      honoured = map (d: d.id) honoured;
      inherit refused;
      unknown = map (d: d.id) (builtins.filter (d: d.app == null) resolved);
      # Measured in the build, from `system.path` (`personApplied`).
      still-present = [ ];
    };
    brain = {
      included = map baseNameOf cfg.brainModules;
      awaiting-approval = cfg.awaitingApproval;
    };
    inherit signature;
  };

  # `person-applied.json`, in the field order of `ronoctua_person::reports`.
  # `system.path` is read only when there is something to look for in it, so
  # the common machine — nothing declined — does not tie this file to the
  # profile's build.
  personApplied =
    pkgs.runCommand "person-applied.json"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        report = builtins.toJSON report;
        probes = builtins.toJSON probes;
        systemPath = if probes == [ ] then "" else "${config.system.path}";
        passAsFile = [
          "report"
          "probes"
        ];
      }
      ''
        python3 - "$reportPath" "$probesPath" "$systemPath" > $out <<'PY'
        import json, os, sys
        report = json.load(open(sys.argv[1]))
        probes = json.load(open(sys.argv[2]))
        system = sys.argv[3]

        def still(p):
            if not system:
                return False
            apps = os.path.join(system, "share", "applications")
            if any(os.path.lexists(os.path.join(apps, d)) for d in p["desktop-ids"]):
                return True
            if p["package"]:
                prefix = p["package"].rstrip("/") + "/"
                for top, dirs, files in os.walk(system):
                    for name in dirs + files:
                        here = os.path.join(top, name)
                        if os.path.islink(here) and (os.readlink(here) + "/").startswith(prefix):
                            return True
            return False

        report["declined"]["still-present"] = [p["id"] for p in probes if still(p)]
        order = ["format", "machine", "distro-rev", "applied", "parts", "skipped", "declined", "brain", "signature"]
        declined = ["honoured", "refused", "unknown", "still-present"]
        out = {k: report[k] for k in order}
        out["declined"] = {k: report["declined"][k] for k in declined}
        out["skipped"] = [{k: s[k] for k in ("id", "reason", "detail")} for s in report["skipped"]]
        out["brain"] = {
            "included": report["brain"]["included"],
            "awaiting-approval": [{"id": a["id"], "hash": a["hash"]} for a in report["brain"]["awaiting-approval"]],
        }
        sig = report["signature"]
        refused = sig["refused"]
        out["signature"] = {
            "pinned": sig["pinned"],
            "counter": sig["counter"],
            "refused": None if refused is None else {k: refused[k] for k in ("at", "why", "detail")},
            "kept-back": [{k: b[k] for k in ("at", "why")} for b in sig["kept-back"]],
        }
        json.dump(out, sys.stdout, indent=2, ensure_ascii=False)
        print()
        PY
      '';
in
{
  options.ronoctua.person = {
    data = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The person's `declared.json` (the root copy under the machine
        directory's `person/`); `null` for none.
      '';
    };

    declined = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The person's `declined.json`; `null` for none.
      '';
    };

    brainModules = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [ ];
      description = ''
        The person's brain modules (`my-…`) compiled into this machine's
        brain; a root part only when approved through the sudo door.
      '';
    };

    signer = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        `local/signer.json` of the machine directory: the person's signing
        key root pinned, and what the last apply made of their signature —
        a refusal, what it kept back — which the report carries. `null`:
        no key is pinned.
      '';
    };

    distroPins = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The person's `software/distro-pins.json` (the root copy, validated by
        the root applier): the distro's AppImages they follow past its pins.
        The registry builds one only while their options say to follow, and
        only when it is newer than the distro's (`nix/modules/distro-apps.nix`).
        `null` for none.
      '';
    };

    icons = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The person's content-addressed icons (`software/icons/<sha256>.png`)
        a web app or an AppImage names; a file whose hash is not its name is
        not used.
      '';
    };

    awaitingApproval = lib.mkOption {
      type = lib.types.listOf (lib.types.attrsOf lib.types.str);
      default = [ ];
      description = ''
        The person's brain modules with a NixOS half that no approval names
        with their current hash (`{ id; hash; }`): `lib.machine` did not
        import them, and the report says which hash an approval must name.
      '';
    };

    declinedHere = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      readOnly = true;
      internal = true;
      default = declinedHere;
      defaultText = "the ids `declined.json` declines on this machine";
      description = ''
        The registry ids the person declined on THIS machine, read once here
        for the registry (`nix/modules/distro-apps.nix`) and the report.
      '';
    };

    # ── anchor: L25 ──
    parts = lib.mkOption {
      # LAZY: a value here may be exactly the entry that aborts, and the
      # names must read without it.
      type = lib.types.lazyAttrsOf lib.types.unspecified;
      readOnly = true;
      internal = true;
      default = lib.listToAttrs (map (e: lib.nameValuePair e.id (targetOf e)) partEntries);
      defaultText = "one target per entry of this machine that is a part";
      description = ''
        What each entry that is a part builds, by the entry's id: its
        derivation and output, or why its build refuses it — always with
        its fingerprint. The machine flake's `personParts` hands it out,
        and the part's realizer evaluates its own entry alone, in its own
        process: the system's evaluation never reads one.
      '';
    };

    report = lib.mkOption {
      type = lib.types.attrsOf lib.types.unspecified;
      readOnly = true;
      internal = true;
      default = report;
      defaultText = "what was applied and skipped, and why";
      description = ''
        `/etc/ronoctua/person-applied.json` as evaluation knows it — every
        field but `declined.still-present`, which the build measures.
      '';
    };

    # ── anchor: L06 ──
    tool = lib.mkOption {
      type = lib.types.package;
      description = ''
        Root's half of the person's data, EXTRACTED from the workspace
        (`rust.person`): `ronoctua-person apply`, `approve`, `pin`. The
        units that run it as root must not carry the desktop, so the flake
        sets this next to the import of `person-apply.nix`.
      '';
    };
    # ── anchor: L10 ──
    options = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        The person's `options.json` (the root copy under the machine
        directory's `person/`, validated by the root applier): the opt-ins
        a restore of the belongings offers. `null` for none: nothing is
        turned on. Whether a security key answers sudo is not among them:
        it is root's list of the person's keys, written through the sudo
        door (`nix/modules/yubikey.nix`).
      '';
    };
  };

  # ── anchor: L03 ──
  # THE APKS EVERY DECLARATION FETCHED, the distro's registry and the
  # person's alike: `/etc/ronoctua/declared-android.json` is one list, and the
  # reconciler reads only that file.
  options.ronoctua.declaredAndroid = lib.mkOption {
    type = lib.types.listOf (lib.types.attrsOf lib.types.unspecified);
    default = [ ];
    internal = true;
    description = ''
      The declared apks (`{ id; package; apk; version-code; from; image; }`,
      the shape of `declared-android.json`), from `nix/declared-apps/android.nix`.
    '';
  };

  config = lib.mkMerge [
    # ── anchor: L25 ──
    # THE ENTRIES THAT ARE PARTS, each by its declaration: the plan names
    # the entry, never a derivation (`nix/modules/parts.nix`), and its
    # realizer evaluates it. Where the parts' module is imported.
    (lib.optionalAttrs (options.ronoctua ? parts) {
      ronoctua.parts = lib.listToAttrs (
        map (
          e:
          lib.nameValuePair (partIdOf e) {
            wanted = true;
            entry = {
              inherit (e) id;
              fingerprint = fingerprintOf e;
            };
            name = if builtins.isString (e.name or null) then e.name else sourceIdOf.${e.source} e;
          }
        ) partEntries
      );
    })
    {
      ronoctua.declaredAndroid = lib.concatMap (r: r.apks) applied;

      # THE PERSON'S PROFILE, through Home Manager — never `systemPackages`:
      # their web apps' launchers and their Flatpaks. Their programs are parts.
      home-manager.users.${user} = {
        home.packages = lib.concatMap (r: r.packages) applied;
        xdg.desktopEntries = builtins.foldl' (all: r: all // r.desktopEntries) { } applied;
      }
      # nix-flatpak's option exists only where Flatpak is on
      # (`nix/modules/flatpak.nix` imports its Home Manager module there).
      // lib.optionalAttrs sourceOn.flatpak {
        services.flatpak.packages = lib.concatMap (r: r.flatpaks) applied;
      };

      environment.etc."ronoctua/person-applied.json".source = personApplied;
      environment.etc."ronoctua/declared-android.json".text = builtins.toJSON {
        inherit format;
        apps = config.ronoctua.declaredAndroid;
      };
    }
  ];
  # ── anchor: L06 ──
}
