# The brain's modules, read out of their folders — brain.md §1 and §3.
#
# A module is a FOLDER, `crates/ronoctua-brain/modules/<id>/`, and its id is
# the folder's name, written nowhere else (invariant 16):
#
#   manifest.nix      a plain attrset — what it is, its schedule, its marks,
#                     notices, actions, events, the files it uses and the files
#                     allowed to name it
#   module.rs         `pub fn new(birth: &crate::Birth) -> impl Module + use<>`
#   module.nix        its NixOS half, gated here by
#                     `ronoctua.brain.modules.<id>.enable`
#   i18n/<lang>.ftl   one per declared language, its own keys only
#   checks.nix        `{ <arguments> }: { checks = { … }; slow = [ … ]; }`
#   why.md            its paragraph of documentation, in pt-BR
#   tables.nix        (optional) Rust tables other generators make for it
#
# THIS FILE IS A PURE FUNCTION of `lib` and the folder. Nothing here builds:
# it reads, refuses, and hands every generator what it needs — the text
# fragments to `nix/i18n.nix`, the glyphs to the measured icon table, the
# notifications to the catalogue, the NixOS modules to `nix/modules/brain.nix`,
# the checks to `tests/checks.nix`, and the manifests, as JSON, to
# `nix/scripts/gen-brain-modules.py`. The flake imports it ONCE and hands each
# generator its part; `nix/modules/brain.nix` imports it again only because a
# NixOS module's `imports` cannot come from anywhere else.
#
# WHAT IT REFUSES, at evaluation and with the folder's name: a folder that is
# not an English kebab id, a required file missing, a language without its
# `.ftl`, a manifest that is a function, a field it does not know, a field with
# the wrong shape, a mark whose glyph is not declared, and two modules whose
# generated names (glyph tokens, event tags, commands) come out the same.
{
  lib,
  # The folder every module lives in. A test passes the fixture's instead.
  modulesDir ? ../crates/ronoctua-brain/modules,
  # The languages, from the one list — every module owes each a `.ftl`.
  # Passed in, never defaulted: whoever imports this says which list.
  languages,
}:

let
  inherit (builtins)
    attrNames
    concatMap
    elem
    filter
    functionArgs
    hasAttr
    isAttrs
    isBool
    isFunction
    isInt
    isList
    isString
    listToAttrs
    match
    pathExists
    readDir
    replaceStrings
    toJSON
    ;

  # AN ENGLISH KEBAB NAME WHOSE EVERY SEGMENT BEGINS WITH A LETTER.
  #
  # A letter and not a digit because every name here becomes an identifier
  # somewhere: `STILL_SHORT` in the generated Rust, and a glyph token becomes a
  # case of the WIT enum, where a segment that begins with a digit is refused.
  word = "[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*";
  isWord = s: isString s && match word s != null;
  # A text key: kebab, and a digit may begin a segment (`notif-2fa-…`).
  isKey = s: isString s && match "[a-z][a-z0-9-]*" s != null;
  isUnit = s: isString s && match "[a-z0-9@_.-]+\\.(service|timer|socket|path|target)" s != null;

  snakeOf = id: replaceStrings [ "-" ] [ "_" ] id;

  # THE FIELDS A MANIFEST MAY HAVE. A field outside this list is refused, so a
  # typo (`schedul`) is an error and not a module quietly on its defaults.
  required = [
    "what"
    "needs"
    "schedule"
    "uses"
    "namedBy"
  ];
  optional = [
    "units"
    "keys"
    "whys"
    "glyphs"
    "marks"
    "notices"
    "actions"
    "events"
    "polkit"
  ];
  scheduleRequired = [
    "kind"
    "calendar"
    "partOfDay"
    "countdownSeconds"
    "runawayMinutes"
    "gated"
  ];
  # `intervalDays` is required only of a module that keeps a calendar: one
  # that does not is never due by it (brain.md §1's example has none).
  scheduleOptional = [
    "everyMinutes"
    "intervalDays"
  ];
  # WHAT A CALENDAR-LESS SCHEDULE CARRIES as its interval, inert: it is never
  # due by it, the core refuses a cadence for it, and its page draws none. The
  # shared default's (`TaskPrefs::default`), so the row it lands in a person's
  # preferences with reads like every other — and the golden sample's.
  intervalWithoutCalendar = 7;
  partsOfDay = [
    "any"
    "night"
    "morning"
    "afternoon"
    "evening"
  ];
  levels = [
    "info"
    "warn"
    "danger"
    "alarm"
  ];
  # What an event's notification may say: the verdict fields of an entry of
  # `assets/notifications.nix`, and `tags` (extra ones — an old tag a saved
  # filter still names) and `actions`. `when` and `rewrite` are the loader's:
  # a module's notification is recognised by its own tag.
  notificationFields = [
    "show"
    "life"
    "loud"
    "level"
    "auto-read"
    "icon"
    "blink"
    "vibrate"
    "audio"
    "tags"
    "actions"
  ];

  # THE FILES EVERY FOLDER HAS. `tables.nix` is the one optional file.
  requiredFiles = [
    "manifest.nix"
    "module.rs"
    "module.nix"
    "checks.nix"
    "why.md"
  ];

  # What a module's `checks.nix` may ask for, by name — `tests/checks.nix`
  # passes exactly these, plus `module` (this file's view of it) and `dir`
  # (its folder).
  checkArguments = [
    "pkgs"
    "lib"
    "self"
    "system"
    "rust"
    "buffers"
    "mkCheck"
    "pyPrelude"
    "texts"
    "languages"
    "brainModules"
    "brainModulesRs"
    "module"
    "dir"
  ];

  bools =
    where: fields: attrs:
    concatMap (f: lib.optional (!(isBool (attrs.${f} or null))) "${where}.${f} is a boolean") fields;

  # ---------------------------------------------------------------------
  # THE REFUSALS, as a list of sentences. Pure, so a check can hand it a
  # broken folder that exists nowhere and read what it says.
  # ---------------------------------------------------------------------
  problemsOf =
    {
      id,
      files,
      i18nFiles,
      manifest,
    }:
    let
      m = manifest;
      missingFiles = filter (f: !(elem f files)) requiredFiles;
      missingTexts = filter (l: !(elem "${l}.ftl" i18nFiles)) languages;
      strayTexts = filter (f: !(elem (lib.removeSuffix ".ftl" f) languages)) i18nFiles;

      fields = attrNames m;
      unknown = filter (f: !(elem f (required ++ optional))) fields;
      absent = filter (f: !(hasAttr f m)) required;

      needs = m.needs or null;
      needsProblems =
        if !(isAttrs needs) then
          [ "needs is an attrset: { root; network; boundaries; }" ]
        else
          bools "needs" [
            "root"
            "network"
          ] needs
          ++ lib.optional (
            !(isList (needs.boundaries or null) && lib.all isWord needs.boundaries)
          ) "needs.boundaries is a list of crate names (`ronoctua-memory`)"
          ++ map (f: "needs.${f} is not a field (root, network, boundaries)") (
            filter (
              f:
              !(elem f [
                "root"
                "network"
                "boundaries"
              ])
            ) (attrNames needs)
          );

      s = m.schedule or null;
      scheduleProblems =
        if s == null then
          [ ]
        else if !(isAttrs s) then
          [ "schedule is an attrset, or null for a module that only watches" ]
        else
          map (f: "schedule.${f} is missing") (filter (f: !(hasAttr f s)) scheduleRequired)
          ++ lib.optional (
            (s.calendar or false) == true && !(hasAttr "intervalDays" s)
          ) "schedule.intervalDays is missing (a module that keeps a calendar has an interval)"
          ++ map (f: "schedule.${f} is not a field") (
            filter (f: !(elem f (scheduleRequired ++ scheduleOptional))) (attrNames s)
          )
          ++ lib.optional (
            hasAttr "kind" s
            && !(elem s.kind [
              "routine"
              "once"
            ])
          ) "schedule.kind is `routine` or `once` (a watch has `schedule = null`)"
          ++ lib.optional (
            hasAttr "partOfDay" s && !(elem s.partOfDay partsOfDay)
          ) "schedule.partOfDay is one of ${lib.concatStringsSep ", " partsOfDay}"
          ++ bools "schedule" (filter (f: hasAttr f s) [
            "calendar"
            "gated"
          ]) s
          ++
            concatMap
              (
                f:
                lib.optional (
                  hasAttr f s && !(isInt s.${f} && s.${f} >= 0 && s.${f} < 65536)
                ) "schedule.${f} is a whole number below 65536"
              )
              [
                "intervalDays"
                "countdownSeconds"
                "runawayMinutes"
              ]
          ++ lib.optional (
            (s.everyMinutes or null) != null && !(isInt s.everyMinutes && s.everyMinutes > 0)
          ) "schedule.everyMinutes is null or a positive number of minutes";

      units = m.units or { };
      unitProblems =
        if !(isAttrs units) then
          [ "units is an attrset: { run = \"x.service\"; … }" ]
        else
          concatMap (
            w:
            let
              u = units.${w};
            in
            lib.optional (!(isWord w)) "units.${w}: the name is an English kebab word"
            ++ (
              if isString u then
                lib.optional (!(isUnit u)) "units.${w} = ${u} is not a unit name"
              else if isAttrs u then
                lib.optional (!(isUnit (u.name or null))) "units.${w}.name is a unit name"
                ++ lib.optional (!(isBool (u.sessionPath or false))) "units.${w}.sessionPath is a boolean"
                ++ map (f: "units.${w}.${f} is not a field (name, sessionPath)") (
                  filter (
                    f:
                    !(elem f [
                      "name"
                      "sessionPath"
                    ])
                  ) (attrNames u)
                )
              else
                [ "units.${w} is a unit name or { name; sessionPath; }" ]
            )
          ) (attrNames units);

      keys = m.keys or [ id ];
      keyProblems = lib.optional (
        !(isList keys && keys != [ ] && lib.all isWord keys)
      ) "keys is a non-empty list of English kebab prefixes";

      whys = m.whys or [ ];
      whyProblems =
        lib.optional (!(isList whys && lib.all isWord whys)) "whys is a list of English kebab words"
        ++ lib.optional (isList whys && lib.unique whys != whys) "whys repeats a word";

      glyphs = m.glyphs or { };
      glyphProblems =
        if !(isAttrs glyphs) then
          [ "glyphs is an attrset: { <name> = \"<glyph name>\"; }" ]
        else
          concatMap (
            g:
            lib.optional (!(isWord g)) "glyphs.${g}: the name is an English kebab word"
            ++ lib.optional (
              !(isString glyphs.${g} && glyphs.${g} != "")
            ) "glyphs.${g} is a glyph name (`md-harddisk`)"
          ) (attrNames glyphs);

      marks = m.marks or { };
      markProblems =
        if !(isAttrs marks) then
          [ "marks is an attrset: { <name> = { glyph; hint; }; }" ]
        else
          concatMap (
            k:
            let
              mk = marks.${k};
            in
            lib.optional (!(isWord k)) "marks.${k}: the name is an English kebab word"
            ++ (
              if !(isAttrs mk) then
                [ "marks.${k} is { glyph; hint; }" ]
              else
                lib.optional (
                  !(isAttrs glyphs && hasAttr (mk.glyph or "") glyphs)
                ) "marks.${k}.glyph names a glyph `glyphs` does not declare"
                ++ lib.optional (!(isKey (mk.hint or null))) "marks.${k}.hint is a text key"
                ++ map (f: "marks.${k}.${f} is not a field (glyph, hint)") (
                  filter (
                    f:
                    !(elem f [
                      "glyph"
                      "hint"
                    ])
                  ) (attrNames mk)
                )
            )
          ) (attrNames marks);

      notices = m.notices or { };
      noticeProblems =
        if !(isAttrs notices) then
          [ "notices is an attrset: { <id> = { level; dismiss; snooze; }; }" ]
        else
          concatMap (
            k:
            let
              n = notices.${k};
            in
            lib.optional (!(isWord k)) "notices.${k}: the name is an English kebab word"
            ++ (
              if !(isAttrs n) then
                [ "notices.${k} is { level; dismiss; snooze; }" ]
              else
                lib.optional (
                  !(elem (n.level or null) levels)
                ) "notices.${k}.level is one of ${lib.concatStringsSep ", " levels}"
                ++ bools "notices.${k}" [ "dismiss" ] n
                ++ lib.optional (
                  !(isList (n.snooze or [ ]) && lib.all (x: isInt x && x > 0 && x < 65536) (n.snooze or [ ]))
                ) "notices.${k}.snooze is a list of minutes"
                ++ map (f: "notices.${k}.${f} is not a field (level, dismiss, snooze)") (
                  filter (
                    f:
                    !(elem f [
                      "level"
                      "dismiss"
                      "snooze"
                    ])
                  ) (attrNames n)
                )
            )
          ) (attrNames notices);

      actions = m.actions or { };
      actionProblems =
        if !(isAttrs actions) then
          [ "actions is an attrset: { <id> = { row; asks; }; }" ]
        else
          concatMap (
            k:
            let
              a = actions.${k};
            in
            lib.optional (!(isWord k)) "actions.${k}: the name is an English kebab word"
            ++ (
              if !(isAttrs a) then
                [ "actions.${k} is { row; asks; }" ]
              else
                lib.optional (!(isBool (a.row or false))) "actions.${k}.row is a boolean"
                ++ lib.optional (
                  (a.asks or null) != null && !(isKey a.asks)
                ) "actions.${k}.asks is null or the key of a question"
                ++ map (f: "actions.${k}.${f} is not a field (row, asks)") (
                  filter (
                    f:
                    !(elem f [
                      "row"
                      "asks"
                    ])
                  ) (attrNames a)
                )
            )
          ) (attrNames actions);

      events = m.events or { };
      eventProblems =
        if !(isAttrs events) then
          [ "events is an attrset: { <name> = { notification = null | { … }; }; }" ]
        else
          concatMap (
            k:
            let
              e = events.${k};
              n = e.notification or null;
            in
            lib.optional (!(isWord k)) "events.${k}: the name is an English kebab word"
            ++ (
              if !(isAttrs e && hasAttr "notification" e) then
                [ "events.${k} is { notification = null | { … }; }" ]
              else
                map (f: "events.${k}.${f} is not a field (notification)") (
                  filter (f: f != "notification") (attrNames e)
                )
                ++ (
                  if n == null then
                    [ ]
                  else if !(isAttrs n) then
                    [ "events.${k}.notification is null or an attrset" ]
                  else
                    map (
                      f: "events.${k}.notification.${f} is not a field (${lib.concatStringsSep ", " notificationFields})"
                    ) (filter (f: !(elem f notificationFields)) (attrNames n))
                )
            )
          ) (attrNames events);

      uses = m.uses or null;
      usesProblems = lib.optional (
        !(isList uses && lib.all isString uses)
      ) "uses is a list of the repository's paths it relies on";

      namedBy = m.namedBy or null;
      namedByProblems =
        if !(isAttrs namedBy) then
          [ "namedBy is an attrset: { \"<path>\" = \"*\" | [ <names> ]; }" ]
        else
          concatMap (
            p:
            let
              v = namedBy.${p};
            in
            lib.optional (
              !(v == "*" || (isList v && v != [ ] && lib.all isString v))
            ) "namedBy.\"${p}\" is \"*\" or a non-empty list of the names that file may say"
          ) (attrNames namedBy);

      polkit = m.polkit or [ ];
      polkitProblems = lib.optional (
        !(isList polkit && lib.all isString polkit)
      ) "polkit is a list of polkit action ids";

      what = m.what or null;
      whatProblems = lib.optional (
        !(isString what && what != "" && match ".*\n.*" what == null)
      ) "what is one English line";
    in
    lib.optional (!(isWord id)) "the folder's name is the id, and `${id}` is not an English kebab word"
    ++ map (f: "${f} is missing") missingFiles
    ++ map (l: "i18n/${l}.ftl is missing") missingTexts
    ++ map (f: "i18n/${f} is not a declared language") strayTexts
    ++ (
      if !(elem "manifest.nix" files) then
        [ ]
      else if isFunction m then
        [ "manifest.nix is a plain attrset, not a function" ]
      else if !(isAttrs m) then
        [ "manifest.nix is an attrset" ]
      else
        map (
          f: "manifest.nix: `${f}` is not a field (${lib.concatStringsSep ", " (required ++ optional)})"
        ) unknown
        ++ map (f: "manifest.nix: `${f}` is missing") absent
        ++ map (p: "manifest.nix: ${p}") (
          whatProblems
          ++ needsProblems
          ++ scheduleProblems
          ++ unitProblems
          ++ keyProblems
          ++ whyProblems
          ++ glyphProblems
          ++ markProblems
          ++ noticeProblems
          ++ actionProblems
          ++ eventProblems
          ++ usesProblems
          ++ namedByProblems
          ++ polkitProblems
        )
    );

  # ---------------------------------------------------------------------
  # ONE MODULE, READ AND NORMALISED: every optional field filled, so nothing
  # downstream asks `or` again.
  # ---------------------------------------------------------------------
  normalise =
    id: m:
    let
      unitOf =
        u:
        if isString u then
          {
            name = u;
            sessionPath = false;
          }
        else
          { sessionPath = false; } // u;
    in
    {
      inherit id;
      snake = snakeOf id;
      inherit (m) what uses namedBy;
      needs = {
        inherit (m.needs) root network boundaries;
      };
      schedule =
        if m.schedule == null then
          null
        else
          {
            everyMinutes = null;
            intervalDays = intervalWithoutCalendar;
          }
          // m.schedule;
      units = lib.mapAttrs (_: unitOf) (m.units or { });
      # THE ID IS ALWAYS ONE OF ITS PREFIXES: `cmd-<id>-<action>`,
      # `<id>-why-<word>` and `notif-<id>-<event>-…` are built from it.
      prefixes = lib.unique ([ id ] ++ (m.keys or [ ]));
      whys = m.whys or [ ];
      glyphs = m.glyphs or { };
      marks = m.marks or { };
      notices = lib.mapAttrs (_: n: { snooze = [ ]; } // n) (m.notices or { });
      actions = lib.mapAttrs (
        _: a:
        {
          row = false;
          asks = null;
        }
        // a
      ) (m.actions or { });
      events = m.events or { };
      polkit = m.polkit or [ ];
    };

  load =
    id:
    let
      dir = modulesDir + "/${id}";
      files = attrNames (readDir dir);
      i18nFiles =
        if pathExists (dir + "/i18n") then
          attrNames (lib.filterAttrs (_: t: t == "regular") (readDir (dir + "/i18n")))
        else
          [ ];
      manifest = if elem "manifest.nix" files then import (dir + "/manifest.nix") else null;
      problems = problemsOf {
        inherit
          id
          files
          i18nFiles
          manifest
          ;
      };
    in
    if problems != [ ] then
      throw ''
        the brain module `${id}` (${toString dir}) is refused:
          ${lib.concatStringsSep "\n  " problems}''
    else
      normalise id manifest
      // {
        inherit dir;
        hasTables = elem "tables.nix" files;
      };

  folders =
    if pathExists modulesDir then
      lib.filterAttrs (_: t: t == "directory") (readDir modulesDir)
    else
      { };
  ids = lib.sort lib.lessThan (attrNames folders);
  modules = listToAttrs (
    map (id: {
      name = id;
      value = load id;
    }) ids
  );
  every = map (id: modules.${id}) ids;

  # ---------------------------------------------------------------------
  # NAMES TWO MODULES COULD BOTH GENERATE. `made` with a glyph `up-full` and
  # `made-up` with a glyph `full` are two modules and one token.
  # ---------------------------------------------------------------------
  refuseTwice =
    what: pairs:
    let
      names = map (p: p.name) pairs;
      twice = lib.unique (filter (n: lib.count (x: x == n) names > 1) names);
    in
    if twice == [ ] then
      listToAttrs pairs
    else
      throw "two brain modules generate the same ${what}: ${lib.concatStringsSep ", " twice}";

  glyphs = refuseTwice "glyph token" (
    concatMap (
      m:
      lib.mapAttrsToList (name: glyph: {
        name = "${m.id}-${name}";
        value = glyph;
      }) m.glyphs
    ) every
  );

  eventTags = refuseTwice "event tag" (
    concatMap (
      m:
      map (e: {
        name = "${m.id}-${e}";
        value = m.id;
      }) (attrNames m.events)
    ) every
  );

  commands = refuseTwice "command" (
    concatMap (
      m:
      map (a: {
        name = "${m.id}-${a}";
        value = m.id;
      }) (attrNames m.actions)
    ) every
  );

  # THE NOTIFICATIONS, as entries of `assets/notifications.nix`: recognised by
  # the event's own tag, tagged with it and with the module's id, and saying
  # what the manifest says. Only the events that declared one.
  notifications = listToAttrs (
    concatMap (
      m:
      lib.mapAttrsToList (
        event: e:
        let
          tag = "${m.id}-${event}";
          n = e.notification;
        in
        {
          name = tag;
          value = {
            when = [
              {
                field = "tag";
                is = tag;
              }
            ];
            rewrite = false;
            actions = [ ];
          }
          // removeAttrs n [ "tags" ]
          // {
            tags = lib.unique (
              [
                tag
                m.id
              ]
              ++ (n.tags or [ ])
            );
          };
        }
      ) (lib.filterAttrs (_: e: e.notification != null) m.events)
    ) every
  );

  # ---------------------------------------------------------------------
  # THE NIXOS HALF, GATED. A module's `module.nix` is written as any NixOS
  # module is; the gate is put around its `config` here, so turning one off
  # with `ronoctua.brain.modules.<id>.enable = false` takes every unit it
  # declares with it, and no module has to remember to ask. Its `options` stay
  # declared either way — an option that exists only while its module is on
  # is an evaluation error for whoever sets it.
  #
  # AND AROUND EVERYTHING IT IMPORTS, down the tree: a module that splits its
  # units into `imports = [ ./units.nix ];` is gated in each file, or the
  # units would stay with the module off. What it may import is its own:
  # a file of its FOLDER, or a module written inline. A path outside the
  # folder is refused — it is somebody else's file (and a module the system
  # already imports, imported again under a new key, is two declarations of
  # every option it has). `disabledModules` is refused too: what it takes
  # away is resolved before any `config`, so no gate can put it back.
  # ---------------------------------------------------------------------
  gate =
    id: dir: m:
    let
      isFile = builtins.isPath m || isString m;
      mine = lib.hasPrefix "${toString dir}/" (toString m);
      raw = if isFile then import m else m;
      inner = lib.setFunctionArgs (
        args:
        let
          r = if isFunction raw then raw args else raw;
          full = r ? config || r ? options;
          on = args.config.ronoctua.brain.enable && args.config.ronoctua.brain.modules.${id}.enable;
        in
        if (r.disabledModules or [ ]) != [ ] then
          throw "the brain module `${id}`: its NixOS half declares `disabledModules`, which no gate can turn back on; put that in `nix/modules/`"
        else
          # THE MODULE SYSTEM'S OWN SPLIT (`unifyModuleSyntax`): in the full
          # form `config` and a top-level `meta` are the configuration; in the
          # short form everything but the module system's own keys is.
          {
            imports = map (gate id dir) ((r.imports or [ ]) ++ (r.require or [ ]));
            options = r.options or { };
            config = lib.mkIf on (
              if full then
                lib.mkMerge [
                  (r.config or { })
                  (lib.optionalAttrs (r ? meta) { inherit (r) meta; })
                ]
              else
                removeAttrs r [
                  "_class"
                  "_file"
                  "key"
                  "disabledModules"
                  "require"
                  "imports"
                  "freeformType"
                ]
            );
          }
      ) (if isFunction raw then functionArgs raw else { });
    in
    if isFile && !mine then
      throw "the brain module `${id}`: its NixOS half imports ${toString m}, outside its folder; a module imports only its own files"
    else
      {
        imports = [ inner ];
      }
      # A FILE KEEPS ITS NAME, for the errors that point at it and so that
      # one file imported twice is still one module.
      // lib.optionalAttrs isFile {
        _file = toString m;
        key = "ronoctua-brain-module:${id}:${toString m}";
      };

  nixosModules = map (m: gate m.id m.dir (m.dir + "/module.nix")) every;

  # ---------------------------------------------------------------------
  # THE CHECKS. Every module's own, merged, and a name two of them give — or
  # a name outside its own — is refused before anything is built.
  # ---------------------------------------------------------------------
  checksOf =
    m: arguments:
    let
      f = import (m.dir + "/checks.nix");
      given = arguments // {
        module = m;
        dir = m.dir;
      };
      asked = functionArgs f;
      r = f (if asked == { } then given else builtins.intersectAttrs asked given);
    in
    if !(isAttrs r && isAttrs (r.checks or null)) then
      throw "the brain module `${m.id}`: checks.nix answers { checks = { … }; slow = [ … ]; }"
    else
      {
        inherit (m) id prefixes;
        checks = r.checks;
        slow = r.slow or [ ];
      };

  # PURE, and the refusal is its whole point: a list of `{ id; prefixes;
  # checks; slow; }`, merged, or an error naming what collided.
  mergeChecks =
    answers:
    let
      named = concatMap (
        a:
        map (n: {
          inherit (a) id prefixes;
          name = n;
        }) (attrNames a.checks)
      ) answers;
      outside = filter (c: !(lib.any (p: c.name == p || lib.hasPrefix "${p}-" c.name) c.prefixes)) named;
      names = map (c: c.name) named;
      twice = lib.unique (filter (n: lib.count (x: x == n) names > 1) names);
      ghosts = concatMap (a: map (s: "${a.id}: ${s}") (filter (s: !(hasAttr s a.checks)) a.slow)) answers;
    in
    if outside != [ ] then
      throw "a brain module's check is named outside its own id and key prefixes: ${
        lib.concatMapStringsSep ", " (c: "${c.name} (in ${c.id})") outside
      }"
    else if twice != [ ] then
      throw "two brain modules declare the same check: ${lib.concatStringsSep ", " twice}"
    else if ghosts != [ ] then
      throw "a brain module's `slow` names a check it does not declare: ${lib.concatStringsSep ", " ghosts}"
    else
      {
        checks = lib.foldl' (acc: a: acc // a.checks) { } answers;
        slow = concatMap (a: a.slow) answers;
      };

  checksWith = arguments: mergeChecks (map (m: checksOf m arguments) every);

  # THE SLOW NAMES, read without the arguments: the flake needs them to split
  # the batteries, and the batteries are built long before any check is. A
  # module whose `slow` list or check NAMES depend on an argument is refused
  # by the argument itself, which throws when it is looked at.
  slowChecks =
    (checksWith (
      lib.genAttrs checkArguments (
        n:
        throw "a brain module's check names and `slow` list are read without arguments; `${n}` was looked at"
      )
    )).slow;

  # ---------------------------------------------------------------------
  # WHAT THE RUST GENERATOR READS, one record per module. The text fragment
  # and the tables are copied into the store ON THEIR OWN, so an edit of a
  # module's Rust does not regenerate its manifest.
  # ---------------------------------------------------------------------
  forGenerator =
    tablesOf:
    map (
      m:
      removeAttrs m [
        "dir"
        "hasTables"
        "uses"
        "namedBy"
      ]
      // {
        i18n = "${builtins.path {
          path = m.dir + "/i18n";
          name = "brain-${m.id}-i18n";
        }}";
        tables = if m.hasTables then tablesOf m else { };
      }
    ) every;
in
# THE CLASHES ARE REFUSED WHOEVER ASKS. Nothing downstream reads the tags or
# the commands as a table, so without this a clash between two modules would
# surface only in the Rust — as two constants of one name, far from the cause.
builtins.deepSeq (attrNames glyphs ++ attrNames eventTags ++ attrNames commands) {
  inherit
    ids
    modules
    every
    glyphs
    notifications
    nixosModules
    checksWith
    slowChecks
    forGenerator
    checkArguments
    # The pure halves, for the checks that prove the refusals and the gate.
    problemsOf
    mergeChecks
    gate
    ;

  # WHAT `nix/i18n.nix` HANDS `gen-i18n.py`: each module's text folder, alone
  # in the store, and the prefixes its keys may begin with.
  i18nFragments = map (m: {
    inherit (m) id prefixes;
    dir = builtins.path {
      path = m.dir + "/i18n";
      name = "brain-${m.id}-i18n";
    };
  }) every;

  # THE WHOLE VIEW, for a check that reads every manifest at once.
  json = toJSON (
    map (
      m:
      removeAttrs m [
        "dir"
        "hasTables"
      ]
    ) every
  );
}
