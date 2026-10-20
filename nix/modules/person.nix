# The person's declarations, as the system consumes them.
#
# The person's data (`software/declared.json`, `software/declined.json`) is
# validated by a narrow root unit and copied into the machine directory's
# `person/` — data, never Nix code. `lib.machine` points the options below at
# that copy (`nix/machine.nix`), and this module turns it into the machine:
#
#   nix       the person's `home.packages`        (`declared-apps/nix.nix`)
#   flatpak   nix-flatpak's list for `--user`      (`declared-apps/flatpak.nix`)
#   appimage  a wrapped AppImage in `home.packages` (`declared-apps/appimage.nix`)
#   web       a launcher entry (`xdg.desktopEntries`) (`declared-apps/web.nix`)
#   android   a fetched apk, listed for the reconciler (`declared-apps/android.nix`)
#
# EVERYTHING GOES TO THE PERSON, never to `systemPackages`
# (`person-packages-are-per-user`): these are the person's programs, carried
# from machine to machine by their declaration, and they are on this machine
# because THEY declared them — not because the distro ships them.
#
# # It degrades, and says so
#
# An entry that does not evaluate — a name nixpkgs dropped, an alias that
# throws, an unfree package declared without consent, a source this machine
# lacks, an entry the update held back — is SKIPPED, and the system evaluates
# without it (`person-bad-entry-does-not-break-eval`). Each skip is a line in
# `/etc/ronoctua/person-applied.json` with its reason from a closed set, which
# is what the software centre draws. A file in a format this distro does not
# read skips everything it lists, said once per entry.
#
# What cannot be degraded is a file that is not JSON (`builtins.fromJSON` does
# not throw, it fails) — which is why only the root applier's validated copy is
# ever read here, and never the person's own staging.
{
  config,
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

  # THE FINGERPRINT the update's verification holds an entry back by
  # (`local/held.json`): the entry without its stamps, and the nixpkgs it is
  # built against. Re-declaring the same thing keeps it held; a new pin, an
  # edit or a new nixpkgs gives it another try. Computed ONLY here, and handed
  # out as the machine flake's `personFingerprints`.
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

  heldDoc = read cfg.held;
  heldList =
    if
      heldDoc != null && (heldDoc.format or null) == format && builtins.isList (heldDoc.held or null)
    then
      builtins.filter (
        h:
        builtins.isAttrs h && builtins.isString (h.id or null) && builtins.isString (h.fingerprint or null)
      ) heldDoc.held
    else
      [ ];
  heldBy = lib.listToAttrs (map (h: lib.nameValuePair h.id h) heldList);
  # THE FOLLOWED DISTRO APPIMAGES ROOT'S VERIFY BUILT, by id, in a format
  # this distro reads — nothing else is let in.
  verifiedDoc = read cfg.verified;
  verifiedBy =
    if
      verifiedDoc != null
      && (verifiedDoc.format or null) == format
      && builtins.isList (verifiedDoc.verified or null)
    then
      lib.listToAttrs (
        map (v: lib.nameValuePair v.id v) (
          builtins.filter (v: builtins.isAttrs v && builtins.isString (v.id or null)) verifiedDoc.verified
        )
      )
    else
      { };
  # Held by the fingerprint it has NOW: an edit, a new pin or a new nixpkgs
  # gives it another try. JSON alone answers it — nothing of the package.
  isHeld = e: heldBy ? ${e.id} && heldBy.${e.id}.fingerprint == fingerprintOf e;

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
    else if isHeld e then
      skip e.id "held" (
        if builtins.isString (heldBy.${e.id}.reason or null) then heldBy.${e.id}.reason else ""
      )
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

  # The entries the verification evaluates one at a time — this machine's,
  # with a source it has, held or not (a held entry is verified again, and
  # comes back when it builds).
  verifiable = builtins.filter (
    e: problemOf e == null && applies e && sourceOn.${e.source}
  ) firsts.kept;

  # THE UNFREE NAMES THE PERSON SAID YES TO — which READS each package, to
  # learn its name. So never a held one: the entry the verification held back
  # may be exactly the one whose reading `abort`s or fails in a way no
  # `tryEval` catches, and through the predicate it would take the whole
  # machine with it.
  #
  # `alone` is the verification's question (`lib.machineFlake`'s
  # `personEntries`): ONE entry, held or not — it is verified again so it can
  # come back — and no other, so a sibling that breaks cannot break it.
  consented = lib.concatMap builders.nix.consented (
    builtins.filter (
      e: e.source == "nix" && (if cfg.alone == null then !isHeld e else e.id == cfg.alone)
    ) verifiable
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
        order = ["format", "machine", "distro-rev", "applied", "skipped", "declined", "brain", "signature"]
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

    unfreeNames = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        The unfree packages the person said yes to when declaring them —
        joined to the one unfree predicate (`nixpkgs-config.nix`).
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

    verified = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        `local/verified.json` of the machine directory: the followed distro
        AppImages the update's verification built, each at its fingerprint
        and with its pin — the allow-list the registry builds one by.
      '';
    };

    verifiedBy = lib.mkOption {
      type = lib.types.raw;
      readOnly = true;
      internal = true;
      default = verifiedBy;
      defaultText = "the entries `local/verified.json` names, by id";
      description = ''
        `local/verified.json` as read here, by id: what the registry asks of a
        followed distro AppImage before it builds it.
      '';
    };

    heldBy = lib.mkOption {
      type = lib.types.raw;
      readOnly = true;
      internal = true;
      default = heldBy;
      defaultText = "the entries `local/held.json` holds, by id";
      description = ''
        `local/held.json` as read here, by id (`{ id; fingerprint; reason; … }`):
        what the registry asks of a followed distro AppImage
        (`distro-appimage:<id>`), the one reader of the file.
      '';
    };

    # ── anchor: L03 ──
    held = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        `local/held.json` of the machine directory: the entries the update's
        verification held back, each with the fingerprint it failed with.
        An entry is skipped while its fingerprint is still that one.
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

    alone = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      internal = true;
      description = ''
        One entry's id: the machine as the update's verification evaluates
        that entry (`lib.machineFlake`'s `personEntries`), where the unfree
        consent is that entry's alone — held or not — so neither a held
        entry nor a broken sibling decides whether it evaluates. `null` is
        the machine itself.
      '';
    };

    entries = lib.mkOption {
      # LAZY: a value here may be exactly the entry that throws, and the
      # names must be readable without reading it.
      type = lib.types.lazyAttrsOf lib.types.unspecified;
      readOnly = true;
      internal = true;
      # AND THE DISTRO'S APPIMAGES THE PERSON FOLLOWS past its pins, each one
      # the registry would build (`distro-appimage:<id>`,
      # `nix/modules/distro-apps.nix`): a release the gate never built is
      # tried like a declaration before the rebuild meets it.
      default =
        lib.listToAttrs (
          map (
            e:
            lib.nameValuePair e.id (
              builders.${e.source}.raw {
                entry = e;
                inherit (cfg) icons;
              }
            )
          ) verifiable
        )
        // (config.ronoctua.distroApps.followedEntries or { });
      defaultText = "one derivation per entry of this machine, and per followed distro AppImage";
      description = ''
        Every entry this machine would build, by id, with NO protection. The
        machine flake's `personEntries` reads each one from the machine as
        `alone` makes it for that entry, and the update evaluates and builds
        them one at a time, so that an entry that breaks evaluation is held
        back before the system ever reads it.
      '';
    };

    fingerprints = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      readOnly = true;
      internal = true;
      default =
        lib.listToAttrs (map (e: lib.nameValuePair e.id (fingerprintOf e)) verifiable)
        // (config.ronoctua.distroApps.followedFingerprints or { });
      defaultText = "one fingerprint per entry of this machine, and per followed distro AppImage";
      description = ''
        The machine flake's `personFingerprints`: what `local/held.json`
        names an entry by.
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
        (`rust.person`): `ronoctua-person apply`, `verify`, `approve`. The
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

  config = {
    ronoctua.person.unfreeNames = consented;
    ronoctua.declaredAndroid = lib.concatMap (r: r.apks) applied;

    # THE PERSON'S PROFILE, through Home Manager — never `systemPackages`.
    home-manager.users.${user} = {
      # AND THE DISTRO'S APPIMAGES THE PERSON FOLLOWS past its pins, at the
      # pin root's verify built: their programs, from their own download
      # (`nix/modules/distro-apps.nix`, `personPackages`).
      home.packages =
        lib.concatMap (r: r.packages) applied ++ (config.ronoctua.distroApps.personPackages or [ ]);
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
  };
  # ── anchor: L06 ──
}
