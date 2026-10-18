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
#   shared.rs         (optional) in a DISTRO folder, the half of its Rust that
#                     a crate its `namedBy` lists reads — compiled into the
#                     library the footer links (`ronoctua-brain`), beside its
#                     manifest, while `module.rs` is the daemon's; in a
#                     person's folder, a file like any other beside
#                     `module.rs`, which the daemon alone compiles
#
# THIS FILE IS A PURE FUNCTION of `lib` and the folder. Nothing here builds:
# it reads, refuses, and hands every generator what it needs — the text
# fragments to `nix/i18n.nix`, the marks, glyphs, actions, events and
# notifications the footer reads at run time to the brain's catalogue
# (`nix/brain-catalogue.nix`), the NixOS modules to `nix/modules/brain.nix`,
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
#
# THE PERSON'S MODULES (`personModulesDir`, the 2026-10-05 wave). A machine
# may compile the person's own modules beside the distro's: folders the person
# wrote (`$XDG_CONFIG_HOME/ronoctua/brain-modules/<id>/`, carried by the
# belongings' plain part), which the machine's flake hands in as a store copy.
# Each module says whose it is (`origin`, `distro` or `person`); a person's id
# begins with `my-` and a distro's never does, so the two can never name one
# module. A person's folder needs only `manifest.nix`, `module.rs` and its
# texts — its `module.nix` reaches root, so the machine imports it only once
# approved through the sudo door (`personNixosModules`, by id), and its
# `checks.nix` is not the distro's to run. It may not draw at rest, and its
# texts must hold every key its manifest needs — asked here, at evaluation,
# where the distro's are asked by the generators in the build. And a
# person's folder this refuses is LEFT OUT and said (`personRefused`), never
# a `throw`: it travels to every machine of theirs, and one bad folder must
# not stop them all.
#
# MARKS WITH INSTANCES, AND MARKS AT REST. A mark may declare `instances = N`
# (from 1 to 9: the footer draws ONE digit beside the glyph) and
# `resting = true` — drawn while all is well, which the modeline at rest
# otherwise never is. Only the marks the owner approved may rest
# (`assets/brain-resting-marks.nix`, and AGENTS.md beside the rule).
{
  lib,
  # The folder every module lives in. A test passes the fixture's instead.
  modulesDir ? ../crates/ronoctua-brain/modules,
  # The languages, from the one list — every module owes each a `.ftl`.
  # Passed in, never defaulted: whoever imports this says which list.
  languages,
  # THE 2026-10-05 WAVE: the person's modules (`my-…`), after its anchor.
  # ── anchor: L11 ──
  # The folder of the PERSON's modules, or `null` for none — the distro's own
  # builds, the checks. The machine's flake hands in a store copy.
  personModulesDir ? null,
  # The person's folders to READ, by id, or `null` for every one. The machine
  # leaves out a folder it does not include (one whose NixOS half waits for
  # an approval), and the update's isolated verification one whose manifest
  # does not even evaluate: an error Nix cannot catch, read here, would stop
  # the whole machine's evaluation.
  personModulesOnly ? null,
  # The marks the owner approved to be drawn at rest, `<module>/<mark>`. A
  # test passes its own list.
  restingApproved ? import ../assets/brain-resting-marks.nix,
  # THE BASE TEXTS' KEY NAMES, or `null`: what a PERSON's notification may
  # name outside its own fragment (`notif-action-<id>`, the label of a
  # notification's action, is one of the base's). Handed in by the machine's
  # chain (`nix/person-brain.nix`), so a person's folder that names a key no
  # text has is left out and said here, at evaluation — never a build of the
  # brain's catalogue stopped on every machine of theirs.
  baseKeyNames ? null,
}:

let
  textNames = import ./text-names.nix { inherit lib; };
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

  # THE BASE LANGUAGE, whose `.ftl` names a person's fragment's keys
  # (`textNames`, the reader every fragment's names come from).
  baseLanguage = builtins.head languages;

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
  # AND A PERSON'S: what the brain compiles. Its NixOS half, its checks and
  # its paragraph are the person's business, so each may be there or not.
  personRequiredFiles = [
    "manifest.nix"
    "module.rs"
  ];

  # WHOSE A MODULE IS, by the first word of its id — the one rule that keeps a
  # person's module from ever being mistaken for (or shadowing) the distro's.
  #
  # ONE GRAMMAR, here and in `ronoctua-person` (`check::person_module`, which
  # validates the approvals): `my-` and an English kebab word, at most 64
  # characters. A word and not a slug because the id becomes a Rust module, a
  # WIT case (a glyph token) and a text-key prefix, and a segment that begins
  # with a digit is refused by the WIT; both sides are proved on one table,
  # `crates/ronoctua-person/fixtures/module-ids.json`.
  personPrefix = "my-";
  personIdMost = 64;
  isPersonId = id: isWord id && lib.hasPrefix personPrefix id && lib.stringLength id <= personIdMost;
  # THE PERSON'S NAMESPACE, `my` and everything after `my-`: nothing of the
  # distro's — an id, a key prefix — is named in it, so every name a
  # person's module generates (`my-…`) is apart from every name the distro's
  # do. `my` alone counts: `my` with a glyph `x-seat` would be `my-x-seat`.
  inPersonNamespace =
    s: isString s && (s == lib.removeSuffix "-" personPrefix || lib.hasPrefix personPrefix s);

  # PLAIN DATA: what a person's manifest must be, all the way down. A function
  # or a path inside it is an error the evaluation cannot catch the moment
  # something turns it into JSON (`forGenerator`), so it is refused before.
  isData =
    v:
    if isFunction v || builtins.isPath v then
      false
    else if isAttrs v then
      lib.all isData (builtins.attrValues v)
    else if isList v then
      lib.all isData v
    else
      true;

  # THE MOST INSTANCES A MARK MAY DECLARE: the footer draws the instance as
  # ONE digit beside the glyph, so a tenth would be a mark drawn as two.
  mostInstances = 9;

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
    "brainManifestsRs"
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
      origin ? "distro",
      # The key names of its base fragment (`i18n/<base>.ftl`), or `null`:
      # read by `read` for a PERSON's folder, whose keys are asked here.
      fragmentKeys ? null,
    }:
    let
      m = manifest;
      person = origin == "person";
      missingFiles = filter (f: !(elem f files)) (if person then personRequiredFiles else requiredFiles);
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
      keyProblems =
        lib.optional (
          !(isList keys && keys != [ ] && lib.all isWord keys)
        ) "keys is a non-empty list of English kebab prefixes"
        # A PERSON'S TEXTS STAY IN THE PERSON'S NAMESPACE: a prefix like
        # `agents` builds today and stops the day the distro adds a key or a
        # module under it — on every machine of that person, at the update.
        ++ lib.optional (
          person && isList keys && !(lib.all (k: isString k && lib.hasPrefix personPrefix k) keys)
        ) "keys: a person's module's prefixes begin with `${personPrefix}`"
        ++ lib.optional (
          !person && isList keys && lib.any inPersonNamespace keys
        ) "keys: `my` and `${personPrefix}…` are the person's prefixes, never the distro's";

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
              instances = mk.instances or null;
              resting = mk.resting or false;
            in
            lib.optional (!(isWord k)) "marks.${k}: the name is an English kebab word"
            ++ (
              if !(isAttrs mk) then
                [ "marks.${k} is { glyph; hint; }" ]
              else
                # A STRING FIRST: `hasAttr` handed anything else stops the
                # evaluation with an error nothing catches.
                lib.optional (
                  !(isAttrs glyphs && isString (mk.glyph or null) && hasAttr mk.glyph glyphs)
                ) "marks.${k}.glyph names a glyph `glyphs` does not declare"
                ++ lib.optional (!(isKey (mk.hint or null))) "marks.${k}.hint is a text key"
                # ONE DIGIT beside the glyph, so from 1 to 9; a mark that is
                # one says nothing.
                ++ lib.optional (
                  instances != null && !(isInt instances && instances >= 1 && instances <= mostInstances)
                ) "marks.${k}.instances is a whole number from 1 to ${toString mostInstances}"
                ++ lib.optional (!(isBool resting)) "marks.${k}.resting is a boolean"
                # THE MODELINE AT REST IS SILENT, but for the marks the owner
                # approved one by one — and never a person's.
                ++ lib.optional (
                  resting == true && person
                ) "marks.${k}.resting: a person's module draws nothing at rest"
                ++
                  lib.optional (resting == true && !person && !(elem "${id}/${k}" restingApproved))
                    "marks.${k}.resting: `${id}/${k}` is not a mark the owner approved to be drawn at rest (assets/brain-resting-marks.nix, AGENTS.md)"
                ++ map (f: "marks.${k}.${f} is not a field (glyph, hint, instances, resting)") (
                  filter (
                    f:
                    !(elem f [
                      "glyph"
                      "hint"
                      "instances"
                      "resting"
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
                    # A LIST, since the catalogue's tags are joined to it
                    # (`notifications`) — and `++` on anything else stops the
                    # evaluation with an error nothing catches.
                    ++ lib.optional (
                      !(isList (n.tags or [ ]) && lib.all isString (n.tags or [ ]))
                    ) "events.${k}.notification.tags is a list of tags"
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

      # A PERSON'S KEYS, asked of its texts at evaluation — and only once the
      # manifest's shape holds, since the keys are built from its fields.
      # What the generators demand of every module in the build
      # (`gen-brain-modules.py`'s `needed`, `gen-brain-catalogue.py`'s keys):
      # its name, its line, each action's label and question, each mark's
      # hint and each why in its own fragment; each notification's summary
      # and body there or in the base texts, and each of its actions' labels
      # (`notif-action-<id>`, never a module's to own) in the base. Asked here
      # for a person's folder so the folder is left out and said, and the
      # build never sees it.
      ownKeys = [
        "brain-task-${id}"
        "brain-task-${id}-does"
      ]
      ++ map (a: "cmd-${id}-${a}") (attrNames actions)
      ++ lib.filter isString (lib.mapAttrsToList (_: a: a.asks or null) actions)
      ++ lib.mapAttrsToList (_: mk: mk.hint) marks
      ++ map (w: "${id}-why-${w}") whys;
      notifying = lib.filterAttrs (_: e: (e.notification or null) != null) events;
      notificationKeys = lib.concatMap (e: [
        "notif-${id}-${e}-summary"
        "notif-${id}-${e}-body"
      ]) (attrNames notifying);
      notificationActionKeys = lib.concatMap (
        e:
        map (a: "notif-action-${a.id}") (
          lib.filter (a: isAttrs a && isString (a.id or null)) (
            if isList (notifying.${e}.notification.actions or null) then
              notifying.${e}.notification.actions
            else
              [ ]
          )
        )
      ) (attrNames notifying);
      inBase = k: baseKeyNames != null && elem k baseKeyNames;
      keyProblemsOf =
        own:
        map (k: "i18n/${baseLanguage}.ftl has no `${k}`, which the manifest needs") (
          filter (k: !(elem k own)) ownKeys
        )
        ++ map (k: "i18n/${baseLanguage}.ftl has no `${k}`, and neither have the base texts") (
          filter (k: !(elem k own) && !(inBase k)) notificationKeys
        )
        ++ lib.optionals (baseKeyNames != null) (
          map (k: "the base texts have no `${k}`, the label of a notification's action") (
            filter (k: !(inBase k)) notificationActionKeys
          )
        );
      shape =
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
        ++ polkitProblems;
      textProblems = lib.optionals (
        person && fragmentKeys != null && unknown == [ ] && absent == [ ] && shape == [ ]
      ) (keyProblemsOf fragmentKeys);
    in
    lib.optional (!(isWord id)) "the folder's name is the id, and `${id}` is not an English kebab word"
    # WHOSE IT IS, by its first word: `my-` is the person's, and only theirs.
    ++
      lib.optional (person && !(isPersonId id))
        "a person's module's id is `${personPrefix}` and an English kebab word, at most ${toString personIdMost} characters, and `${id}` is not"
    ++ lib.optional (
      !person && inPersonNamespace id
    ) "`my` and `${personPrefix}…` are the person's names, and `${id}` is the distro's"
    ++ lib.optional (
      !(elem origin [
        "distro"
        "person"
      ])
    ) "the origin is `distro` or `person`, not `${origin}`"
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
      else if person && !(isData m) then
        [ "manifest.nix is plain data: a person's manifest holds no function and no path" ]
      else
        map (
          f: "manifest.nix: `${f}` is not a field (${lib.concatStringsSep ", " (required ++ optional)})"
        ) unknown
        ++ map (f: "manifest.nix: `${f}` is missing") absent
        ++ map (p: "manifest.nix: ${p}") shape
        ++ textProblems
    );

  # ---------------------------------------------------------------------
  # ONE MODULE, READ AND NORMALISED: every optional field filled, so nothing
  # downstream asks `or` again.
  # ---------------------------------------------------------------------
  normalise =
    origin: id: m:
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
      inherit id origin;
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
      # EVERY MARK SAYS how many it is and whether it rests: `0` and `false`
      # for a mark that is one and says nothing at rest.
      marks = lib.mapAttrs (
        _: mk:
        {
          instances = 0;
          resting = false;
        }
        // mk
      ) (m.marks or { });
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

  # ONE FOLDER, READ: what is wrong with it, and the module it is when nothing
  # is. Never a `throw` — who reads it decides (`load` for the distro's,
  # `personSettled` for the person's).
  read =
    origin: root: id:
    let
      dir = root + "/${id}";
      entries = readDir dir;
      # A FILE IS NOT A DIRECTORY: a folder named `manifest.nix` would be
      # imported as one, and `i18n` as a file read as a folder — two errors
      # nothing catches.
      files = attrNames (lib.filterAttrs (_: t: t != "directory") entries);
      i18nFiles =
        if (entries.i18n or null) == "directory" then
          attrNames (lib.filterAttrs (_: t: t == "regular") (readDir (dir + "/i18n")))
        else
          [ ];
      manifest = if elem "manifest.nix" files then import (dir + "/manifest.nix") else null;
      # A PERSON'S BASE FRAGMENT, by its key names: its texts are asked at
      # evaluation (`problemsOf`), the distro's by the generators.
      fragmentKeys =
        if origin == "person" && elem "${baseLanguage}.ftl" i18nFiles then
          textNames.namesIn (builtins.readFile (dir + "/i18n/${baseLanguage}.ftl"))
        else
          null;
      problems = problemsOf {
        inherit
          id
          files
          i18nFiles
          manifest
          origin
          fragmentKeys
          ;
      };
    in
    {
      inherit id dir problems;
      module = normalise origin id manifest // {
        inherit dir;
        hasTables = elem "tables.nix" files;
        hasNixos = elem "module.nix" files;
        # A SHARED HALF IS A DISTRO FOLDER'S `shared.rs` alone. In a person's
        # folder the same name is a file like any other — a `mod shared;` of
        # its `module.rs`, compiled with it by the daemon — because the library
        # the footer links never reads a person's folder: what it compiles is
        # the distro's, and a person's module stays on the daemon's side.
        hasShared = origin == "distro" && elem "shared.rs" files;
      };
    };

  # THE DISTRO'S FOLDER IS REFUSED WHOLE: its modules are the distro's own,
  # and a broken one is the developer's to fix before anything builds.
  load =
    origin: root: id:
    let
      r = read origin root id;
    in
    if r.problems != [ ] then
      throw ''
        the brain module `${id}` (${toString r.dir}) is refused:
          ${lib.concatStringsSep "\n  " r.problems}''
    else
      r.module;

  foldersOf =
    root:
    if root != null && pathExists root then
      lib.sort lib.lessThan (attrNames (lib.filterAttrs (_: t: t == "directory") (readDir root)))
    else
      [ ];
  distroIds = foldersOf modulesDir;
  distroEvery = map (load "distro" modulesDir) distroIds;

  # ---------------------------------------------------------------------
  # THE PERSON'S FOLDERS ARE REFUSED ONE BY ONE. They arrive from the
  # person's home through the belongings, onto every machine of theirs: a
  # `throw` for one of them would be every rebuild of every machine stopped
  # until they fix it. So a folder that is not a module — a stray directory,
  # an id outside the grammar, a field misspelt, a text missing, a name
  # another module generates — is LEFT OUT and said (`personRefused`, by id,
  # in sentences), and the rest load. What the evaluation cannot catch at all
  # (a manifest that does not parse) is left out before it is read: the
  # update's isolated verification reads each folder alone
  # (`personProblemsOf.<id>`) and the machine hands the ones that answered
  # as `personModulesOnly`.
  # ---------------------------------------------------------------------
  personFolders = foldersOf personModulesDir;
  personAsked =
    if personModulesOnly == null then
      personFolders
    else
      filter (id: elem id personModulesOnly) personFolders;

  # EACH FOLDER'S OWN PROBLEMS, one attribute per folder and lazy: asking for
  # one reads that folder alone, never its neighbours.
  personProblemsOf = lib.genAttrs personFolders (id: (read "person" personModulesDir id).problems);

  # THE NAMES A MODULE GENERATES, by kind — the ones `refuseTwice` below
  # refuses twice, and its text prefixes, which `gen-i18n.py` refuses when
  # they overlap (`made` and `made-up`).
  namesOf = m: {
    "glyph token" = map (g: "${m.id}-${g}") (attrNames m.glyphs);
    "event tag" = map (e: "${m.id}-${e}") (attrNames m.events);
    command = map (a: "${m.id}-${a}") (attrNames m.actions);
  };
  overlap = p: q: p == q || lib.hasPrefix "${p}-" q || lib.hasPrefix "${q}-" p;
  # What a person's module would generate that a module already taken does,
  # in sentences — empty when nothing.
  clashesWith =
    taken: m:
    concatMap (
      t:
      let
        mine = namesOf m;
        theirs = namesOf t;
      in
      concatMap (
        kind:
        map (n: "its ${kind} `${n}` is also generated by `${t.id}`") (
          filter (n: elem n theirs.${kind}) mine.${kind}
        )
      ) (attrNames mine)
      ++ concatMap (
        p:
        map (q: "its text prefix `${p}` overlaps `${q}`, which is `${t.id}`'s") (
          filter (overlap p) t.prefixes
        )
      ) m.prefixes
    ) taken;

  # IN ORDER, each against the distro's and the person's already taken: of two
  # folders that clash, the first by name loads and the second is said — the
  # same answer on every machine.
  personSettled =
    lib.foldl'
      (
        acc: id:
        let
          r = read "person" personModulesDir id;
          why = if r.problems != [ ] then r.problems else clashesWith (distroEvery ++ acc.accepted) r.module;
        in
        if why == [ ] then
          acc // { accepted = acc.accepted ++ [ r.module ]; }
        else
          acc
          // {
            refused = acc.refused // {
              ${id} = why;
            };
          }
      )
      {
        accepted = [ ];
        refused = lib.genAttrs (filter (id: !(elem id personAsked)) personFolders) (_: [
          "left out unread: the machine did not ask for it (`personModulesOnly`)"
        ]);
      }
      personAsked;
  personEvery = personSettled.accepted;
  personIds = map (m: m.id) personEvery;
  personRefused = personSettled.refused;

  # THE COPY THE BUILD READS: the person's folder with only the modules that
  # load. `build.rs` compares that folder with the table's `person` lines, so
  # a refused folder left in it would stop the build the refusal kept going.
  # `null` when none loads — a build with no person's module, the distro's.
  personCopy =
    if personIds == [ ] then
      null
    else
      builtins.path {
        path = personModulesDir;
        name = "ronoctua-person-brain-modules";
        filter =
          path: _:
          elem (lib.head (lib.splitString "/" (lib.removePrefix "${toString personModulesDir}/" (toString path)))) personIds;
      };

  # ONE NAME, ONE MODULE: the prefix rule already keeps the two lists apart,
  # and this says so by name should it ever not.
  ids =
    let
      twice = filter (id: elem id personIds) distroIds;
    in
    if twice != [ ] then
      throw "the person's brain modules and the distro's are both called: ${lib.concatStringsSep ", " twice}"
    else
      distroIds ++ personIds;
  modules = listToAttrs (
    map (m: {
      name = m.id;
      value = m;
    }) (distroEvery ++ personEvery)
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

  # ONE EVENT'S NOTIFICATION, as an entry of `assets/notifications.nix`:
  # recognised by the event's own tag, tagged with it and with the module's
  # id, and saying what the manifest says.
  notificationOf =
    m: event: n:
    let
      tag = "${m.id}-${event}";
    in
    {
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

  # THE NOTIFICATIONS, by their tag: every event that declared one.
  notifications = listToAttrs (
    concatMap (
      m:
      lib.mapAttrsToList (event: e: {
        name = "${m.id}-${event}";
        value = notificationOf m event e.notification;
      }) (lib.filterAttrs (_: e: e.notification != null) m.events)
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
  # A PERSON'S HALF has no switch of its own (`nix/modules/brain.nix` makes
  # one per DISTRO module): it is on with the brain, and only once the
  # machine imported it — approved.
  gateAs =
    origin: id: dir: m:
    let
      isFile = builtins.isPath m || isString m;
      mine = lib.hasPrefix "${toString dir}/" (toString m);
      raw = if isFile then import m else m;
      inner = lib.setFunctionArgs (
        args:
        let
          r = if isFunction raw then raw args else raw;
          full = r ? config || r ? options;
          on =
            args.config.ronoctua.brain.enable
            && (origin == "person" || args.config.ronoctua.brain.modules.${id}.enable);
        in
        if (r.disabledModules or [ ]) != [ ] then
          throw "the brain module `${id}`: its NixOS half declares `disabledModules`, which no gate can turn back on; put that in `nix/modules/`"
        else
          # THE MODULE SYSTEM'S OWN SPLIT (`unifyModuleSyntax`): in the full
          # form `config` and a top-level `meta` are the configuration; in the
          # short form everything but the module system's own keys is.
          {
            imports = map (gateAs origin id dir) ((r.imports or [ ]) ++ (r.require or [ ]));
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

  gate = gateAs "distro";

  # THE DISTRO'S HALVES, which `nix/modules/brain.nix` imports.
  nixosModules = map (m: gate m.id m.dir (m.dir + "/module.nix")) distroEvery;
  # AND THE PERSON'S, by id: a person's `module.nix` reaches root, so the
  # machine imports one only once it is approved through the sudo door.
  personNixosModules = listToAttrs (
    map (m: {
      name = m.id;
      value = gateAs "person" m.id m.dir (m.dir + "/module.nix");
    }) (filter (m: m.hasNixos) personEvery)
  );

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

  # THE DISTRO'S CHECKS: a person's module is not the distro's to test.
  checksWith = arguments: mergeChecks (map (m: checksOf m arguments) distroEvery);

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
  # WHAT THE RUST GENERATOR READS, one record per module. The tables are
  # copied into the store ON THEIR OWN, so an edit of a module's Rust does not
  # regenerate its manifest; and of its texts it reads the key NAMES alone,
  # read here at evaluation (`nix/text-names.nix`) — a sentence edited leaves
  # the manifest, and the brain's crates, as they were.
  # ---------------------------------------------------------------------
  forGenerator =
    tablesOf:
    map (
      m:
      removeAttrs m [
        "dir"
        "hasTables"
        "hasNixos"
        "hasShared"
        "uses"
        "namedBy"
      ]
      // {
        names = textNames.namesIn (builtins.readFile (m.dir + "/i18n/${builtins.head languages}.ftl"));
        tables = if m.hasTables then tablesOf m else { };
        # WHETHER THE LIBRARY COMPILES A SHARED HALF of it, beside its
        # manifest — a word in the table (`shared.txt`), so the library's
        # `build.rs` watches only the files it is told of.
        shared = m.hasShared;
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
    # THE PERSON'S MODULES: whose each id is, the folders left out and why
    # (all of them, and one by one for the isolated verification), their
    # NixOS halves by id (for the machine to import the approved ones), and
    # the rules a check proves.
    distroIds
    personIds
    personRefused
    personProblemsOf
    personNixosModules
    personPrefix
    personIdMost
    mostInstances
    restingApproved
    ;
  # THE COPY THE BUILD READS — only the folders that load — or `null`.
  # `nix/brain-modules-rs.nix` carries it beside the table it generates.
  personModulesDir = personCopy;
  # AND THE FOLDER IT WAS MADE FROM, as handed in.
  personModulesSource = personModulesDir;

  # WHAT `nix/i18n.nix` HANDS `gen-i18n.py`: each module's text folder, alone
  # in the store, the prefixes its keys may begin with, and whose it is — a
  # person's keys are data the footer reads at run time, never names the
  # registry is compiled with.
  i18nFragments = map (m: {
    inherit (m) id prefixes origin;
    dir = builtins.path {
      path = m.dir + "/i18n";
      name = "brain-${m.id}-i18n";
    };
  }) every;

  # WHAT THE BRAIN'S CATALOGUE IS WRITTEN FROM (`nix/brain-catalogue.nix`):
  # one record per module (the generator orders them by id, as the daemon's
  # registry does), with what the footer reads of it at run time — its marks,
  # notices, actions, events and their notifications, its whys, its glyphs
  # and its text prefixes. Never its code, its NixOS half or its prose.
  forCatalogue = map (m: {
    inherit (m)
      id
      origin
      prefixes
      glyphs
      marks
      notices
      actions
      whys
      ;
    # Each event with the entry its notification is — `notifications`'
    # own, so the catalogue and that table cannot tell two stories.
    events = lib.mapAttrs (event: e: {
      notification = if e.notification == null then null else notificationOf m event e.notification;
    }) m.events;
  }) every;

  # THE WHOLE VIEW, for a check that reads every manifest at once.
  json = toJSON (
    map (
      m:
      removeAttrs m [
        "dir"
        "hasTables"
        "hasNixos"
        "hasShared"
      ]
    ) every
  );
}
