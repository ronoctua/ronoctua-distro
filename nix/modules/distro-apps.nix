# The distro's registry of user-facing programs.
#
# # What an entry is
#
# Every program the distro installs for the person to choose to use is an
# entry, `ronoctua.apps.<id>`, written beside the code that owns it
# (`browser.nix` writes Chrome's, `media.nix` VLC's, `cli.nix` tmux's): where
# it comes from, which group of the profile carries it, whether it is
# essential (and why, from a closed set of reasons), the launcher entries it
# owns and the ids it was known by. The registry, and nothing else, puts
# those programs on the machine — so a person who declines one
# (`software/declined.json`, here or on every machine of theirs) finds it
# gone at the next rebuild, and `/etc/ronoctua/distro-apps.json` says what
# the registry did.
#
# What is NOT an entry: what the system itself runs (the compositor, the
# footer, the tools a unit calls by store path — invariant: a program the
# system calls is called by its store path, so leaving `PATH` never breaks
# the system), what NixOS or the base already install (`less`, `curl`,
# `git`), the equipment of one of the distro's features (the formats behind
# `extract`, the secrets' `sops`, Tor's transports) and the plumbing a
# program of the person needs (a zsh plugin rides with zsh; GStreamer's
# plugins with the media group). A launcher entry that belongs to none of
# these is listed in `assets/distro-apps.nix` with its reason;
# `distro-apps-are-registered` refuses one that is in neither place.
#
# # Why a decline turns a SWITCH off for two programs
#
# For most entries the registry is the whole story: the package leaves
# `systemPackages`. Chrome and Emacs are not packages but features — the
# kiosk, the native-messaging host, the extension and the browser's state
# directory hang off `ronoctua.browser.enable`; the daemon and the file
# manager's way to `emacsclient` off `ronoctua.editor.emacs`. Taking only the
# package away would leave all of that pointing at nothing. So their modules
# declare the switch with an `apply` that turns it off when the person
# declined the program (`honoured`), and every reader of the switch follows.
# The GROUP reads the same switch as the profile and the host set it, before
# the `apply` (see `carried`): what `on-this-machine` says is whether the
# machine carries the program, and the decline is said apart. Atuin and
# Starship do the same with their Home Manager halves, OrcaSlicer with the
# model types it opens, Claude Code with the lifeline's variable and the
# Android SDK and Flutter with the session's variables.
#
# # Why every entry is defined OUTSIDE any `mkIf`
#
# `honoured` reads the registry's ids, and a switch reads `honoured`. An entry
# defined inside `mkIf <switch>` would make the registry's very names depend
# on the switch, and evaluation would go round in a circle. Presence on a
# machine is `when` (the group's switch by default), never the definition.
#
# # The distro's other sources
#
# Flatpaks, AppImages, web apps and Android apps of the distro come from
# `assets/distro-apps.nix` (the crafts group's AppImages, today), pinned in
# `assets/distro-pins.json` (`nix run .#distro-pins`). A Flatpak goes to the
# person's `--user` installation through nix-flatpak (`flatpak.nix`), an
# AppImage is wrapped (`appimageTools.wrapType2`) in a part of its own, a web
# app is a launcher entry in `systemPackages`, and an Android app's apk is a
# part of data, handed to the reconciler through `declared-android.json` by
# its part and its file (`ronoctua.distroApps.android`), which the person's
# declarations' module writes.
#
# # The AppImages the person follows past the distro's pins
#
# The owner's decision (2026-10-20): the person may let the routine update
# move the AppImages whose table entry follows a GitHub repository (today
# the ArtCraft Launcher and SolidCraft) past the release the gate tested. Their option
# (`distro-apps-follow-releases` in `options.json`) and their followed pins
# (`software/distro-pins.json`) are the person's signed data, root's
# validated copy (`ronoctua.person.options`, `ronoctua.person.distroPins`).
# A followed pin is built ONLY while the option is on and it is newer than
# the distro's (`builtins.compareVersions`) — when the owner moves the
# distro's pins past it, the distro's wins again. IT IS THE APPIMAGE'S PART
# that takes it (every distro AppImage is a part, and one that follows must
# be): its bytes are the person's own download, and a part is never in the
# system's closure, so never in a public cache either. A download that fails
# is a part that does not arrive: it keeps its previous generation — the
# last pin that arrived, or the distro's — and says what it is missing on
# the parts' page, as any part does. A pin the judge refuses
# (`nix/distro-pins.nix`) is dropped and said in `distro-apps.json`, never a
# reason to stop.
#
# # Every refusal is a sentence in one list
#
# A malformed entry, table line or pin is never an attribute-missing error
# from deep inside a builder: `problems` collects each one by name, an entry
# of the table that is malformed never reaches the registry, and the system's
# packages and `distro-apps.json` refuse to evaluate while the list is not
# empty. `distro-app-sources-are-checked` plants every shape, good and bad.
{
  config,
  options,
  lib,
  pkgs,
  ...
}:

let
  ledger = import ../../assets/distro-apps.nix;
  # THE PINNED FILES, named once (`nix/distro-downloads.nix`): the same name
  # `nix run .#distro-pins` downloads them under, so its download is the
  # build's source.
  downloads = import ../distro-downloads.nix { inherit (pkgs) fetchurl; };
  # THE ONE SHAPE of an apk's part and of its line for the reconciler, the
  # person's declared apps' too.
  androidApps = import ../declared-apps/android.nix { inherit pkgs; };
  # THE ONE JUDGE of a followed pin, shared with the check that holds it and
  # the Rust one to one table (`distro-apps-follow-when-on-and-newer`).
  follow = import ../distro-pins.nix;
  cfg = config.ronoctua.distroApps;
  table = cfg.table;
  pins = cfg.pins;

  apps = config.ronoctua.apps;
  ids = builtins.attrNames apps;

  # A SWITCH AS THE PROFILE AND THE HOST SET IT, before any decline: the
  # option's winning definitions, which an `apply` never touches. Chrome's
  # and Emacs's switches also answer for their program — an `apply` in their
  # modules turns them off when the person declined it — and a group read
  # after that would say the profile lacks a program the person only
  # declined. `false` where the option does not exist: a test machine that
  # imports one module and not the others has no such switch, and no entry
  # of that group either.
  carried =
    path:
    let
      o = lib.attrByPath path null options;
      defined = if o == null then [ ] else o.definitions or [ ];
    in
    defined != [ ] && builtins.all (v: v == true) defined;

  # THE GROUPS ARE THE PROFILE'S SWITCHES (`nix/modules/profile.nix`), named
  # once here.
  groups = {
    base = true;
    browser = carried [
      "ronoctua"
      "browser"
      "enable"
    ];
    editor = carried [
      "ronoctua"
      "editor"
      "emacs"
    ];
    vscode = carried [
      "ronoctua"
      "editor"
      "vscode"
    ];
    media = carried [
      "ronoctua"
      "media"
      "enable"
    ];
    workshop = carried [
      "ronoctua"
      "workshop"
      "enable"
    ];
    office = carried [
      "ronoctua"
      "office"
      "enable"
    ];
    crafts = carried [
      "ronoctua"
      "crafts"
      "enable"
    ];
    dev-toolchains = carried [
      "ronoctua"
      "dev"
      "toolchains"
    ];
    dev-android = carried [
      "ronoctua"
      "dev"
      "android"
    ];
    dev-agents = carried [
      "ronoctua"
      "dev"
      "agents"
    ];
  };

  # WHAT THIS MACHINE CAN RUN, asked of the modules that turned the support
  # on (the shape of `software.nix`), never of a switch of this module. These
  # are the switches as they ARE: a web app opens in Chrome, and a declined
  # Chrome is no support for one.
  supported = {
    nix = true;
    appimage = true;
    flatpak = config.services.flatpak.enable or false;
    # Without Chrome the entry would hide itself (`TryExec`), and a hidden
    # entry is not a program on this machine.
    web = config.ronoctua.browser.enable or false;
    android = config.virtualisation.waydroid.enable or false;
  };

  # THE FIELDS EACH SOURCE TAKES, closed: an unknown one is refused by name
  # rather than ignored. `optionalFieldsOf` are the ones an entry may leave
  # out.
  optionalFieldsOf = {
    appimage = [ "fallback" ];
  };
  fieldsOf = {
    flatpak = [
      "appId"
      "origin"
    ];
    appimage = [
      "key"
      "name"
      "url"
      "hash"
      "version"
    ];
    web = [
      "slug"
      "name"
      "url"
      "profile"
      "category"
    ];
    android = [
      "package"
      "url"
      "hash"
      "version"
      "versionCode"
      "image"
    ];
  };

  # Where an entry comes from: its sources that are set. Exactly one is a
  # well-formed entry (`problems` refuses the rest).
  sourcesSet =
    e:
    lib.optional (e.package != null) "nix"
    ++ lib.filter (s: e.${s} != null) [
      "flatpak"
      "appimage"
      "web"
      "android"
    ];
  sourceOf =
    id: e:
    let
      set = sourcesSet e;
    in
    if builtins.length set == 1 then
      builtins.head set
    else
      throw "ronoctua.apps.${id}: an entry comes from exactly one source, and this one names ${toString (builtins.length set)}";

  # THE LAUNCHER ENTRIES A SOURCE MAKES, by the names the person's own
  # programs of that source get (`crates/ronoctua-software`): one program has
  # one launcher id, whoever installed it.
  derivedDesktopIds =
    e:
    if e.flatpak != null then
      [ "${e.flatpak.appId or ""}.desktop" ]
    else if e.appimage != null then
      # `toString`: a key that is not text is refused in `problems`, and
      # must reach the sentence rather than stop the evaluation here.
      [ "ronoctua-appimage-${toString (e.appimage.key or "")}.desktop" ]
    else if e.web != null then
      [ "ronoctua-web-${e.web.slug or ""}.desktop" ]
    else if e.android != null then
      [ "waydroid.${e.android.package or ""}.desktop" ]
    else
      [ ];

  entry = lib.types.submodule (
    { config, ... }:
    {
      options = {
        package = lib.mkOption {
          type = lib.types.nullOr lib.types.package;
          default = null;
          description = "A nixpkgs package (`source` nix): the program itself.";
        };
        extraPackages = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [ ];
          description = ''
            What comes and goes with the program: a plugin it loads, the
            tools its editor calls, a launcher entry of ours that opens it.
          '';
        };
        part = lib.mkOption {
          type = lib.types.bool;
          # EVERY DISTRO APPIMAGE AND APK IS A PART: its download is the
          # release's own (GitHub, F-Droid), which no public cache holds.
          default = config.appimage != null || config.android != null;
          defaultText = lib.literalMD "`true` for an AppImage and an Android app, `false` otherwise";
          description = ''
            A PART (`nix/modules/parts.nix`): the program and its companions
            are realized after the installation and kept out of the system's
            closure — what is not free (its package from the parts' one door,
            `config.ronoctua.partsPkgs`), or needs a download no public cache
            holds. The session finds it in the parts' view once it arrived;
            whoever runs it names its stable path
            (`config.ronoctua.parts.<id>.path`). A nixpkgs package, an
            AppImage (its wrapper) or an Android app (its apk, data the
            reconciler reads at the part's path); never a web app or a
            Flatpak, which have ways of their own.
          '';
        };
        heavy = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = ''
            A part that takes long to arrive (gigabytes, an hour of building):
            realized after the light ones, so a machine gets its small parts
            first.
          '';
        };
        flatpak = lib.mkOption {
          type = lib.types.nullOr (lib.types.attrsOf lib.types.str);
          default = null;
          description = "A Flatpak: `{ appId; origin; }`.";
        };
        appimage = lib.mkOption {
          type = lib.types.nullOr (lib.types.attrsOf (lib.types.either lib.types.str lib.types.bool));
          default = null;
          description = ''
            An AppImage: `{ key; name; url; hash; version; }`, and `fallback`
            (a boolean, `true` when left out): `false` writes
            `X-Ronoctua-Fallback=false` in its launcher entry, so the file
            manager's opener never hands it a type by name order — only a
            default or a person's choice does.
          '';
        };
        web = lib.mkOption {
          type = lib.types.nullOr (lib.types.attrsOf lib.types.str);
          default = null;
          description = "A web app: `{ slug; name; url; profile; category; }`.";
        };
        android = lib.mkOption {
          type = lib.types.nullOr (lib.types.attrsOf lib.types.str);
          default = null;
          description = "An Android app: `{ package; url; hash; version; versionCode; image; }`.";
        };
        when = lib.mkOption {
          type = lib.types.bool;
          default = groups.${config.group} && supported.${lib.head (sourcesSet config ++ [ "nix" ])};
          defaultText = lib.literalMD "its group's switch, and the support its source needs";
          description = ''
            Whether this machine carries it (tied to `profile.nix`'s groups),
            whatever the person declined: that is said apart.
          '';
        };
        essential = lib.mkOption {
          type = lib.types.nullOr (lib.types.enum ledger.essential);
          default = null;
          description = ''
            `null` when the person may decline it; else the id of the reason it
            may not (`software-essential-why-<reason>`), from the closed set in
            `assets/distro-apps.nix`.
          '';
        };
        desktopIds = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = derivedDesktopIds config;
          defaultText = lib.literalMD "the name its source gives the entry, or none";
          description = "The launcher entries (desktop-file ids) it owns.";
        };
        formerly = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          description = "The ids it was known by, so a decline survives a rename.";
        };
        group = lib.mkOption {
          type = lib.types.enum (builtins.attrNames groups);
          default = "base";
          description = "The `profile.nix` group it belongs to; `when` follows that group's switch.";
        };
        name = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default =
            if config.appimage != null then
              config.appimage.name or null
            else if config.web != null then
              config.web.name or null
            else
              null;
          defaultText = lib.literalMD "the table's `name` for an AppImage or a web app, else none";
          description = ''
            The program's name, one line (`distro-apps.json`'s `name`): what
            the software centre says of a program whose launcher entry is not
            on the machine — one declined before it was ever installed, which
            would otherwise be named by its id. Data, like a `.desktop`'s
            `Name=`, never a text key.
          '';
        };
      };
    }
  );

  # ---------------------------------------------------------------------
  # What the person declined, for THIS machine
  # ---------------------------------------------------------------------
  host = config.networking.hostName;
  # THE ROOT COPY, validated before it reached the machine directory (the
  # person-apply unit), so it is JSON of a known shape — or a format this
  # distro does not read, which is then honoured as nothing and said.
  declinedFile = config.ronoctua.person.declined or null;
  declinedDoc =
    if declinedFile == null || !(builtins.pathExists declinedFile) then
      null
    else
      builtins.fromJSON (builtins.readFile declinedFile);
  formatKnown = declinedDoc == null || (declinedDoc.format or null) == 1;
  # THE SAME FILTER AS THE PERSON'S MODULE (`nix/modules/person.nix`): a line
  # that is not a decline of a known shape is left out, never a reason for
  # the machine's evaluation to stop.
  declinings =
    if declinedDoc == null || !formatKnown || !builtins.isList (declinedDoc.apps or null) then
      [ ]
    else
      builtins.filter (
        d:
        builtins.isAttrs d
        && builtins.isString (d.id or null)
        && (
          (d.machines or null) == null
          || (builtins.isList d.machines && builtins.all builtins.isString d.machines)
        )
      ) declinedDoc.apps;
  appliesHere = d: (d.machines or null) == null || builtins.elem host d.machines;
  # The ids as the PERSON wrote them, in their order: the report speaks in
  # these, so the software view can match each line of its own file. ONE
  # READER: the person's module's list when it has one (it reads the same
  # file for `person-applied.json`), this copy of its filter until then.
  declinedHere = lib.unique (
    config.ronoctua.person.declinedHere or (map (d: d.id) (builtins.filter appliesHere declinings))
  );

  # A RENAMED PROGRAM KEEPS ITS DECLINES: an id it was known by names it.
  formerlyOf = builtins.listToAttrs (
    lib.concatMap (id: map (old: lib.nameValuePair old id) apps.${id}.formerly) ids
  );
  resolve = d: if apps ? ${d} then d else formerlyOf.${d} or null;

  honouredIds = lib.unique (
    builtins.filter (id: id != null && apps.${id}.essential == null) (map resolve declinedHere)
  );
  declinedEntries = lib.unique (builtins.filter (id: id != null) (map resolve declinedHere));

  installed = lib.mapAttrs (id: e: e.when && !(builtins.elem id honouredIds)) apps;
  installedIds = builtins.filter (id: installed.${id}) ids;

  # ── anchor: L25 ──
  # THE ENTRIES THAT ARE PARTS: realized after the installation, never in
  # `systemPackages`. Every one is declared, wanted or not — a part the
  # person declined is UNWANTED, and its record says so.
  partIds = builtins.filter (id: apps.${id}.part) ids;
  partOf =
    id:
    let
      e = apps.${id};
      # What the entry puts on a machine, of whichever source it is.
      paths = packagesOf id e;
      main = builtins.head paths;
    in
    {
      wanted = installed.${id};
      inherit (e) heavy;
      name = if e.name != null then e.name else id;
    }
    // (
      # AN APK IS DATA, which the reconciler reads at the part's path
      # (`apkOf`): nothing of it is in the view.
      if e.android != null then
        {
          kind = "data";
          package = androidApps.inPart {
            inherit (e.android) package versionCode;
            src = downloads.apk {
              inherit (e.android)
                package
                versionCode
                url
                hash
                ;
            };
          };
        }
      else
        {
          kind = "program";
          unfree = lib.any (l: !(l.free or true)) (lib.toList (main.meta.license or [ ]));
          # ONE PATH per part: the program alone, or with what comes and goes
          # with it, joined.
          package =
            if builtins.length paths == 1 then
              main
            else
              pkgs.buildEnv {
                name = "ronoctua-part-${id}";
                inherit paths;
                # AS `system.path` JOINS THEM, where these went before: the
                # first wins — the program over a companion that brings the
                # same file (Flutter's own Dart over `dart`).
                ignoreCollisions = true;
              };
        }
    );

  # ---------------------------------------------------------------------
  # The builders of the distro's other sources
  # ---------------------------------------------------------------------
  # AN APPIMAGE, WRAPPED: the image runs inside the FHS environment
  # `appimageTools` builds, as `ronoctua-appimage-<key>` — the name the
  # person's declared AppImages get, so a key never shadows a program of the
  # same name. Its launcher entry and icon come out of the image at BUILD
  # time (no import-from-derivation), in a derivation of their own: the
  # wrapper's environment is hundreds of packages, and the entry needs only
  # the unpacked image.
  #
  # The entry is WRITTEN, not copied: an AppImage is somebody else's file,
  # and its entry does not choose what runs. Only the keys a launcher needs
  # survive from its `[Desktop Entry]` group; `Name=` is the table's, `Exec=`
  # is the wrapper's stable path plus the one field code the image asked for,
  # and its actions are left behind (each names a program inside the image,
  # which no path outside it reaches). The icon is the image's `.DirIcon`,
  # followed only while it stays inside the image and only when it IS a PNG
  # or an SVG.
  #
  # `fallback = false` adds one key of ours, `X-Ronoctua-Fallback=false`: the
  # file manager's opener then never hands the program a type by name order
  # (`ronoctua_apps::DesktopEntry::fallback`) — a default or a person's choice
  # still reaches it, and its `MimeType=` keeps it in "open with". The
  # image's own `X-` keys never survive the writing, so the key is always
  # this table's.
  #
  # AND THE IMAGE NEVER WRITES ITS OWN LAUNCHER: the launcher is the
  # registry's (`entry`, below). An `AppRun` may write its own launcher and
  # icons into `~/.local/share` when it believes it runs from an AppImage
  # file — the AppImage convention is that `$APPIMAGE` names the file — and
  # that launcher would name a program the person cannot run and the
  # registry cannot decline. The wrapper runs the image already
  # unpacked and never sets `$APPIMAGE`, but it would pass one through from
  # whoever started it; so the program the system runs (`wrapper`) unsets it
  # and sets the opt-out an `AppRun` may read,
  # `<KEY>_NO_DESKTOP_INTEGRATION=1`, from the key — a variable an image does
  # not read costs nothing. Measured in a VM on the thirteen craft images of
  # 2026-10-20 (`PHOTOCRAFT_…` and `FIGCRAFT_…` were the two read): with
  # `$APPIMAGE` pointing at a file, nothing was written. The ArtCraft
  # Launcher's and SolidCraft's images were not searched for it.
  appimageOf =
    {
      key,
      name,
      version,
      src,
      fallback ? true,
      # WHERE ITS LAUNCHER RUNS IT: the system's profile, or — for an
      # AppImage that is a part — the parts' view, the one place the session
      # finds a part's programs.
      program ? "/run/current-system/sw/bin/ronoctua-appimage-${key}",
    }:
    let
      pname = "ronoctua-appimage-${key}";
      # The same arguments `wrapType2` hands its own `extract`, so the two
      # are one derivation and the image is unpacked once.
      contents = pkgs.appimageTools.extract { inherit pname version src; };
      fhs = pkgs.appimageTools.wrapType2 { inherit pname version src; };
      optOut = "${lib.toUpper (builtins.replaceStrings [ "-" ] [ "_" ] key)}_NO_DESKTOP_INTEGRATION";
      wrapper =
        pkgs.runCommand "${pname}-guarded-${version}" { nativeBuildInputs = [ pkgs.makeBinaryWrapper ]; }
          ''
            makeBinaryWrapper ${fhs}/bin/${pname} $out/bin/${pname} \
              --unset APPIMAGE \
              --set ${optOut} 1
          '';
      entry =
        pkgs.runCommand "${pname}-entry-${version}"
          {
            NAME = name;
            PROGRAM = program;
            FALLBACK = if fallback then "true" else "false";
          }
          ''
            set -- ${contents}/*.desktop
            if [ "$#" != 1 ] || [ ! -f "$1" ]; then
              echo "ERROR: the AppImage ${key} has $# launcher entries at its root, and one is the entry" >&2
              exit 1
            fi
            if ! grep -qx '\[Desktop Entry\]' "$1"; then
              echo "ERROR: the AppImage ${key}'s launcher entry has no [Desktop Entry] group" >&2
              exit 1
            fi
            ICON=
            picture=$(readlink -f ${contents}/.DirIcon || true)
            case "$picture" in
              ${contents}/*) ;;
              *) picture= ;;
            esac
            if [ -n "$picture" ] && [ -f "$picture" ]; then
              if [ "$(head -c 4 "$picture" | od -An -tx1 | tr -d ' \n')" = 89504e47 ]; then
                ICON=png
              elif head -c 512 "$picture" | grep -q '<svg'; then
                ICON=svg
              fi
            fi
            if [ -n "$ICON" ]; then
              install -Dm444 "$picture" "$out/share/pixmaps/${pname}.$ICON"
              ICON=${pname}
            fi
            export ICON
            mkdir -p $out/share/applications
            awk '
              /^[ \t]*\[/ {
                if (main) exit
                main = ($0 == "[Desktop Entry]")
                next
              }
              !main { next }
              /^Exec[ \t]*=/ && code == "" {
                n = split(substr($0, index($0, "=") + 1), word, " ")
                for (i = 2; i <= n; i++) if (word[i] ~ /^%[fFuU]$/) { code = " " word[i]; break }
                next
              }
              /^(GenericName|Comment|Keywords)(\[[^]]*\])?[ \t]*=/ { kept = kept $0 "\n"; next }
              /^(Categories|MimeType|StartupWMClass)[ \t]*=/ { kept = kept $0 "\n"; next }
              /^Terminal[ \t]*=[ \t]*(true|false)[ \t]*$/ { kept = kept $0 "\n"; next }
              END {
                printf "[Desktop Entry]\nType=Application\nName=%s\n%sExec=%s%s\n", ENVIRON["NAME"], kept, ENVIRON["PROGRAM"], code
                if (ENVIRON["ICON"] != "") printf "Icon=%s\n", ENVIRON["ICON"]
                if (ENVIRON["FALLBACK"] == "false") printf "X-Ronoctua-Fallback=false\n"
              }
            ' "$1" > $out/share/applications/${pname}.desktop
          '';
    in
    {
      inherit fhs wrapper entry;
      package = pkgs.symlinkJoin {
        name = "${pname}-${version}";
        paths = [
          wrapper
          entry
        ];
        # The launcher entry, for a check to read what it runs.
        passthru = { inherit entry; };
      };
    };

  # A WEB APP is a launcher entry in the shape the software view writes for
  # the person's (`crates/ronoctua-software/src/web.rs`): `web-open <slug>`
  # by the stable path, the address in `X-Ronoctua-Web-Url` and never in
  # `Exec`. No `StartupWMClass`: that is Chromium's arithmetic, which lives
  # in one place (`web::app_id`), and a second copy here would drift.
  webOf =
    w:
    pkgs.writeTextDir "share/applications/ronoctua-web-${w.slug}.desktop" ''
      [Desktop Entry]
      Type=Application
      Version=1.5
      Name=${w.name}
      Exec=/run/current-system/sw/bin/ronoctua-software web-open ${w.slug}
      TryExec=${(import ../parts-names.nix).viewBin "google-chrome-stable"}
      Terminal=false
      StartupNotify=true
      Categories=${w.category};
      X-Ronoctua-Web-Url=${w.url}
      X-Ronoctua-Web-Profile=${w.profile}
    '';

  # AN ANDROID APP is an apk in its part (`partOf`); the reconciler installs
  # it into Waydroid from there, once it arrived. Its line names the part,
  # never a store path.
  apkOf =
    id:
    androidApps.record {
      inherit (apps.${id}.android) package image;
      versionCode = lib.toInt apps.${id}.android.versionCode;
      part = id;
      from = "distro";
    };

  # What an installed entry puts on a machine: in `systemPackages`, or in
  # its part. An AppImage that is a part runs from the parts' view, and is
  # built at the pin the person follows when they follow one newer
  # (`followState`).
  packagesOf =
    id: e:
    let
      pin = if e.part && followedHere id then followState.${id}.use else e.appimage;
    in
    {
      nix = [ e.package ] ++ e.extraPackages;
      appimage = [
        (appimageOf (
          {
            inherit (e.appimage) key name;
            inherit (pin) version;
            fallback = e.appimage.fallback or true;
            src = downloads.appimage {
              inherit (e.appimage) key;
              inherit (pin) version url hash;
            };
          }
          // lib.optionalAttrs e.part {
            program = (import ../parts-names.nix).viewBin "ronoctua-appimage-${e.appimage.key}";
          }
        )).package
      ];
      web = [ (webOf e.web) ];
      flatpak = [ ];
      android = [ ];
    }
    .${sourceOf id e};

  # The ONE package that says the program is there, for `still-present`: an
  # entry's companions may be shared with others (or with the base), and
  # finding one of them is not finding the program.
  mainOf =
    id: e:
    let
      p = packagesOf id e;
    in
    if p == [ ] then null else builtins.head p;

  # ---------------------------------------------------------------------
  # The distro's other sources, out of the table
  # ---------------------------------------------------------------------
  tableSources = [
    "flatpak"
    "appimage"
    "web"
    "android"
  ];
  tableFields = {
    common = [
      "source"
      "group"
      "essential"
      "formerly"
    ];
    flatpak = [
      "appId"
      "origin"
    ];
    appimage = [
      "key"
      "name"
      "follows"
      "fallback"
    ];
    web = [
      "slug"
      "name"
      "url"
      "profile"
      "category"
    ];
    android = [
      "package"
      "from"
      "image"
    ];
  };
  # What a table line of each source cannot do without.
  tableNeeds = {
    flatpak = [ "appId" ];
    appimage = [
      "key"
      "name"
    ];
    web = [
      "slug"
      "name"
      "url"
    ];
    android = [
      "package"
      "from"
    ];
  };
  # A pin's fields, by kind — what `nix run .#distro-pins` writes.
  pinFields = {
    appimage = [
      "url"
      "hash"
      "version"
      "pinned-at"
    ];
    android = [
      "url"
      "hash"
      "version"
      "version-code"
    ];
  };
  pinOf = kind: id: (pins.${kind} or { }).${id} or null;
  pinProblems =
    kind: id:
    let
      p = pinOf kind id;
      at = "assets/distro-pins.json: `${id}`";
      have = if builtins.isAttrs p then builtins.attrNames p else [ ];
      missing = lib.subtractLists have pinFields.${kind};
      extra = lib.subtractLists pinFields.${kind} have;
      text = builtins.filter (f: f != "version-code") pinFields.${kind};
    in
    if p == null then
      [
        "assets/distro-apps.nix: `${id}` has no pin in assets/distro-pins.json — run `nix run .#distro-pins`"
      ]
    else if !builtins.isAttrs p then
      [ "${at} is not a pin" ]
    else
      lib.optional (missing != [ ]) "${at} lacks ${toString missing}"
      ++ lib.optional (extra != [ ]) "${at} has fields no ${kind} pin takes: ${toString extra}"
      ++ lib.optionals (missing == [ ]) (
        map (f: "${at}: `${f}` is not a line of text") (
          builtins.filter (f: !(builtins.isString p.${f})) text
        )
        ++ lib.optional (
          kind == "android" && !(builtins.isInt p.version-code && p.version-code >= 0)
        ) "${at}: `version-code` is not a number"
      );
  # `follows` is null (the pin moves by hand) or one GitHub repository and
  # the pattern of its release's asset (`nix/scripts/distro-pins.py`).
  followsProblems =
    id: f:
    let
      at = "assets/distro-apps.nix: `${id}`.follows";
      g = if builtins.isAttrs f then f.github or null else null;
    in
    if f == null then
      [ ]
    else if !builtins.isAttrs f || builtins.attrNames f != [ "github" ] || !builtins.isAttrs g then
      [ "${at} is neither null nor `{ github = { repo; assetPattern; }; }`" ]
    else
      lib.optional (
        builtins.attrNames g != [
          "assetPattern"
          "repo"
        ]
      ) "${at}.github takes `repo` and `assetPattern`, and has ${toString (builtins.attrNames g)}"
      ++ lib.optional (
        !(builtins.isString (g.repo or null))
        || builtins.match "[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+" g.repo == null
      ) "${at}.github.repo is not `owner/name`"
      ++ lib.optional (
        !(builtins.isString (g.assetPattern or null)) || !(isLine g.assetPattern)
      ) "${at}.github.assetPattern is empty or spans lines";
  tableProblemsOf =
    id: t:
    let
      source = t.source or null;
      at = "assets/distro-apps.nix: `${id}`";
    in
    if !builtins.isAttrs t then
      [ "${at} is not an attribute set" ]
    else if !builtins.elem source tableSources then
      [
        "${at} has the source ${builtins.toJSON source}; the table holds flatpak, appimage, web and android"
      ]
    else
      let
        wrong = lib.subtractLists (tableFields.common ++ tableFields.${source}) (builtins.attrNames t);
        lacking = lib.subtractLists (builtins.attrNames t) tableNeeds.${source};
        text = builtins.filter (f: t ? ${f} && !(builtins.isString t.${f})) (
          lib.subtractLists [
            "follows"
            "fallback"
          ] tableFields.${source}
        );
      in
      lib.optional (wrong != [ ]) "${at} has fields no ${source} entry takes: ${toString wrong}"
      ++ lib.optional (lacking != [ ]) "${at} lacks ${toString lacking}"
      ++ map (f: "${at}: `${f}` is not a line of text") text
      ++ lib.optionals (source == "appimage") (
        lib.optional (
          t ? fallback && !(builtins.isBool t.fallback)
        ) "${at}: `fallback` is neither true nor false"
        ++ followsProblems id (t.follows or null)
        ++ pinProblems "appimage" id
      )
      ++ lib.optionals (source == "android") (
        lib.optional (
          !builtins.elem (t.from or null) [
            "f-droid"
            "direct"
          ]
        ) "${at}: `from` is ${builtins.toJSON (t.from or null)}, not f-droid or direct"
        ++ pinProblems "android" id
      );
  tableProblems = lib.concatLists (lib.mapAttrsToList tableProblemsOf table);
  # Only a well-formed line becomes an entry: a malformed one is said in
  # `problems`, and a builder never meets it.
  wellFormed = lib.filterAttrs (id: t: tableProblemsOf id t == [ ]) table;

  # ---------------------------------------------------------------------
  # The pins the person follows past the distro's
  # ---------------------------------------------------------------------
  # ROOT'S VALIDATED COPIES, read as JSON: the person's options and their
  # followed pins. Absent options, another format, a switch that is not
  # `true`: nothing is followed.
  readJson =
    path:
    if path == null || !(builtins.pathExists path) then
      null
    else
      builtins.fromJSON (builtins.readFile path);
  personOptions = readJson (config.ronoctua.person.options or null);
  followOn =
    builtins.isAttrs personOptions
    && (personOptions.format or null) == follow.format
    && (personOptions.${follow.option} or false) == true;
  followedDoc = if followOn then readJson (config.ronoctua.person.distroPins or null) else null;
  followedFormatKnown = followedDoc == null || (followedDoc.format or null) == follow.format;
  followed = follow.read {
    on = followOn;
    doc = followedDoc;
    table = wellFormed;
  };
  newerThan = distro: p: builtins.compareVersions p.version distro.version > 0;

  # EVERY APPIMAGE OF THE TABLE and what the person's pins do to it: the
  # distro's pin, the followed one and whether it is newer — and the pin its
  # part is built at, the followed one when it is newer, else the distro's.
  followState = lib.mapAttrs (
    id: t:
    let
      distro = pinOf "appimage" id;
      f = followed.by.${id} or null;
      newer = f != null && newerThan distro f;
    in
    {
      inherit distro f newer;
      follows = (t.follows or { }).github or null;
      # The followed pin its part is built at, or `null`: the distro's.
      use = if newer then f else null;
    }
  ) (lib.filterAttrs (_: t: t.source == "appimage") wellFormed);
  # WHETHER THE PART OF `id` IS BUILT AT THE PERSON'S PIN.
  followedHere = id: (followState.${id} or { use = null; }).use != null;
  fromTable =
    id: t:
    let
      fields = {
        flatpak = {
          inherit (t) appId;
          origin = t.origin or "flathub";
        };
        # THE DISTRO'S PIN, always: the registry's. A followed pin is taken
        # where the part is built (`packagesOf`).
        appimage =
          let
            p = pinOf "appimage" id;
          in
          {
            inherit (t) key name;
            inherit (p) url hash version;
          }
          // lib.optionalAttrs (t ? fallback) { inherit (t) fallback; };
        web = {
          inherit (t) slug name url;
          profile = t.profile or "Default";
          category = t.category or "Network";
        };
        android =
          let
            p = pinOf "android" id;
          in
          {
            inherit (t) package;
            image = t.image or "vanilla";
            inherit (p) url hash version;
            versionCode = toString p.version-code;
          };
      };
    in
    {
      group = t.group or "base";
      essential = t.essential or null;
      formerly = t.formerly or [ ];
      ${t.source} = fields.${t.source};
    };

  # ---------------------------------------------------------------------
  # The registry's own refusals
  # ---------------------------------------------------------------------
  isSlug = s: builtins.match "[a-z0-9][a-z0-9-]{0,63}" s != null;
  isLine = s: builtins.match "[^\n\r]*" s != null && s != "";
  # A NAME A SCREEN DRAWS — ONE RULE with `ronoctua_person::reports::is_name`
  # (`nix/names.nix`, which the installer's offered rows read too).
  inherit (import ../names.nix { inherit lib; }) isName;
  isSri = h: builtins.match "(sha256|sha512)-[A-Za-z0-9+/]+=*" h != null;
  isHttps = u: builtins.match "https://[^[:space:]]+" u != null;
  isWeb = u: builtins.match "https?://[^[:space:]]+" u != null;
  sourceProblems =
    id: e:
    let
      s = sourceOf id e;
      a = e.${s};
      missing = lib.subtractLists (builtins.attrNames a) fieldsOf.${s};
      extra = lib.subtractLists (fieldsOf.${s} ++ optionalFieldsOf.${s} or [ ]) (builtins.attrNames a);
      at = "ronoctua.apps.${id}.${s}";
    in
    lib.optionals (s != "nix") (
      lib.optional (missing != [ ]) "${at} lacks ${toString missing}"
      ++ lib.optional (extra != [ ]) "${at} has fields no ${s} entry takes: ${toString extra}"
      ++ lib.optionals (missing == [ ]) (
        {
          flatpak = lib.optional (
            builtins.length (lib.splitString "." a.appId) < 3
            || builtins.match "[A-Za-z0-9_.-]+" a.appId == null
          ) "${at}.appId `${a.appId}` is not a Flatpak id";
          # THE ONE FIELD THAT IS NOT TEXT is `fallback`; a text field that
          # is a boolean is said, and never reaches the matches below.
          appimage =
            let
              notText = builtins.filter (f: !(builtins.isString a.${f})) fieldsOf.appimage;
            in
            lib.optional (!(builtins.isBool (a.fallback or true))) "${at}.fallback is neither true nor false"
            ++ (
              if notText != [ ] then
                [ "${at}: ${toString notText} is not a line of text" ]
              else
                lib.optional (!isSlug a.key) "${at}.key `${a.key}` is not a slug"
                ++ lib.optional (!isLine a.name) "${at}.name is empty or spans lines"
                ++ lib.optional (!isHttps a.url) "${at}.url is not an https address"
                ++ lib.optional (!isSri a.hash) "${at}.hash is not an SRI hash"
                ++ lib.optional (
                  builtins.match "[A-Za-z0-9._+-]{1,64}" a.version == null
                ) "${at}.version `${a.version}` cannot name a store path"
            );
          web =
            lib.optional (!isSlug a.slug) "${at}.slug `${a.slug}` is not a slug"
            ++ lib.optional (!isLine a.name) "${at}.name is empty or spans lines"
            ++ lib.optional (!isWeb a.url) "${at}.url is not an http(s) address"
            ++ lib.optional (!isLine a.profile) "${at}.profile is empty or spans lines"
            ++ lib.optional (builtins.match "[A-Za-z]+" a.category == null) "${at}.category is not a category";
          android =
            lib.optional (
              builtins.match "[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z][A-Za-z0-9_]*)+" a.package == null
            ) "${at}.package `${a.package}` is not an Android package"
            ++ lib.optional (!isHttps a.url) "${at}.url is not an https address"
            ++ lib.optional (!isSri a.hash) "${at}.hash is not an SRI hash"
            ++ lib.optional (builtins.match "[0-9]+" a.versionCode == null) "${at}.versionCode is not a number"
            ++ lib.optional (
              !builtins.elem a.image [
                "vanilla"
                "gapps"
              ]
            ) "${at}.image is `${a.image}`, not vanilla or gapps";
        }
        .${s}
      )
    );
  entryProblems =
    id:
    let
      e = apps.${id};
      n = builtins.length (sourcesSet e);
    in
    lib.optional (!isSlug id) "ronoctua.apps.${id}: an id is a slug ([a-z0-9-], at most 64)"
    ++ lib.optional (
      e.name != null && !isName e.name
    ) "ronoctua.apps.${id}.name is not a name: a control character, or white space alone"
    ++ lib.concatMap (
      old: lib.optional (!isSlug old) "ronoctua.apps.${id}.formerly: `${old}` is not a slug"
    ) e.formerly
    ++ (
      if n == 1 then
        sourceProblems id e
      else
        [
          "ronoctua.apps.${id}: an entry comes from exactly one source (`package`, `flatpak`, `appimage`, `web` or `android`), and this one names ${toString n}"
        ]
    );
  allFormerly = lib.concatMap (id: apps.${id}.formerly) ids;
  allDesktopIds = lib.concatMap (id: apps.${id}.desktopIds) ids;
  repeated = l: lib.unique (builtins.filter (x: lib.count (y: y == x) l > 1) l);
  stalePins =
    kind:
    lib.subtractLists (builtins.attrNames (
      lib.filterAttrs (_: t: builtins.isAttrs t && (t.source or null) == kind) table
    )) (builtins.attrNames (pins.${kind} or { }));
  problems =
    lib.concatMap entryProblems ids
    ++ map (old: "ronoctua.apps: `${old}` is both an id and a former id") (
      builtins.filter (old: apps ? ${old}) allFormerly
    )
    ++ map (old: "ronoctua.apps: `${old}` is the former id of two entries") (repeated allFormerly)
    ++ map (d: "ronoctua.apps: the launcher entry `${d}` is owned by two entries") (
      repeated allDesktopIds
    )
    ++ tableProblems
    ++
      lib.optional ((pins.format or null) != 1)
        "assets/distro-pins.json: format ${builtins.toJSON (pins.format or null)}, and this distro reads 1"
    ++ map (id: "assets/distro-pins.json: `${id}` is pinned and the table has no AppImage by that id") (
      stalePins "appimage"
    )
    ++ map (
      id: "assets/distro-pins.json: `${id}` is pinned and the table has no Android app by that id"
    ) (stalePins "android")
    # A PIN THE PERSON FOLLOWS is their own download: it rides in the
    # AppImage's part, never in the system.
    ++
      map
        (
          id:
          "ronoctua.apps.${id} follows its releases and is no part: a followed pin never goes in the system"
        )
        (
          builtins.filter (id: followState.${id}.follows != null && !(apps.${id}.part or false)) (
            builtins.attrNames followState
          )
        );
  checked =
    problems == [ ]
    || throw ("The distro's program registry refuses:\n  " + lib.concatStringsSep "\n  " problems);

  # ---------------------------------------------------------------------
  # What the registry says
  # ---------------------------------------------------------------------
  # `on-this-machine` is whether the machine CARRIES the program (`when`, as
  # `ronoctua_person::reports::DistroApp` freezes it), and `declined-here`
  # whether the person declined it for this machine: two facts, so the
  # software view can tell a program the profile lacks from one the person
  # only declined — and knows that undeclining the second brings it back.
  # Installed is the first without an honoured second. `name` is the
  # registry's name or null: a program declined before it was ever
  # installed has no launcher entry to be named by.
  #
  # An AppImage says which pin built it (`appimage`, the shape of
  # `ronoctua_person::reports::DistroAppImage`): the distro's or the one the
  # person follows, both versions, the followed one the registry took and
  # why the verification held it. `followed` says whether the person follows
  # and every followed pin the judge refused, with its rule.
  appimageReport =
    id:
    let
      a = apps.${id}.appimage;
      s = followState.${id} or null;
    in
    if sourceOf id apps.${id} != "appimage" then
      null
    else
      {
        inherit (a) key;
        follows =
          if s == null || s.follows == null then
            null
          else
            {
              inherit (s.follows) repo;
              asset-pattern = s.follows.assetPattern;
            };
        pin = if s != null && s.use != null then "followed" else "distro";
        version = if s != null && s.use != null then s.use.version else a.version;
        distro-version = if s == null then a.version else s.distro.version;
        distro-url = if s == null then a.url else s.distro.url;
        followed-version = if s == null || s.f == null then null else s.f.version;
      };
  report = {
    format = 1;
    apps = map (id: {
      inherit id;
      name = apps.${id}.name;
      source = sourceOf id apps.${id};
      desktop-ids = apps.${id}.desktopIds;
      essential = apps.${id}.essential;
      group = apps.${id}.group;
      on-this-machine = apps.${id}.when;
      declined-here = builtins.elem id declinedEntries;
      appimage = appimageReport id;
    }) ids;
    followed = {
      on = followOn;
      inherit (followed) refused;
    };
  };

  # STILL PRESENT: declined, honoured, and the program is in `systemPackages`
  # through another way in — the list `system.path` is built from.
  systemOutPaths = map (p: p.outPath or null) config.environment.systemPackages;
  stillPresent = builtins.filter (
    d:
    let
      id = resolve d;
      main = mainOf id apps.${id};
    in
    id != null
    && apps.${id}.essential == null
    && main != null
    && builtins.elem main.outPath systemOutPaths
  ) declinedHere;
in
{
  options.ronoctua.apps = lib.mkOption {
    type = lib.types.attrsOf entry;
    default = { };
    description = ''
      The distro's user-facing programs, by stable id (`ronoctua.apps.<id>`).
      Defined by the module that owns each program, never inside an `mkIf`:
      presence on a machine is `when`.
    '';
  };

  options.ronoctua.distroApps = {
    table = lib.mkOption {
      type = lib.types.raw;
      internal = true;
      default = ledger.apps;
      defaultText = lib.literalMD "the `apps` of `assets/distro-apps.nix`";
      description = ''
        The distro's Flatpaks, AppImages, web apps and Android apps, as
        declared by hand. An option only so a check can plant a table of its
        own; a machine never sets it.
      '';
    };
    pins = lib.mkOption {
      type = lib.types.raw;
      internal = true;
      default = builtins.fromJSON (builtins.readFile ../../assets/distro-pins.json);
      defaultText = lib.literalMD "`assets/distro-pins.json`";
      description = ''
        The table's pins, as `nix run .#distro-pins` wrote them. An option
        only so a check can plant pins of its own; a machine never sets it.
      '';
    };
    problems = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      readOnly = true;
      internal = true;
      default = problems;
      defaultText = lib.literalMD "every refusal of the registry, the table and the pins";
      description = ''
        Every malformed entry, table line and pin, one sentence each. The
        system's packages and `distro-apps.json` refuse to evaluate while
        this is not empty.
      '';
    };
    builders = lib.mkOption {
      type = lib.types.raw;
      readOnly = true;
      internal = true;
      default = {
        appimage = appimageOf;
        web = webOf;
        apk = apkOf;
      };
      defaultText = lib.literalMD "the builders of the table's sources";
      description = ''
        The builders the table's entries go through, for a check to build
        on a fixture: `appimage { key; name; version; src; fallback ? true; }`
        gives `{ fhs; wrapper; entry; package; }` (`wrapper` is the guarded
        program, `fhs` the environment it runs), `web` a launcher entry,
        `apk <id>` the `declared-android.json` record of a table's Android app.
      '';
    };
    honoured = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      readOnly = true;
      default = honouredIds;
      defaultText = lib.literalMD "the registry ids the person declined for this machine, essential ones left out";
      description = ''
        The registry ids declined for this machine and honoured: a module
        whose program is a feature turns its switch off when its id is here.
      '';
    };
    installed = lib.mkOption {
      type = lib.types.attrsOf lib.types.bool;
      readOnly = true;
      default = installed;
      defaultText = lib.literalMD "each entry's `when`, and not declined";
      description = "Whether each program of the registry is on this machine.";
    };
    declinedReport = lib.mkOption {
      type = lib.types.raw;
      readOnly = true;
      default =
        assert checked;
        {
          honoured = builtins.filter (
            d: resolve d != null && apps.${resolve d}.essential == null
          ) declinedHere;
          refused = map (d: {
            id = d;
            reason = apps.${resolve d}.essential;
          }) (builtins.filter (d: resolve d != null && apps.${resolve d}.essential != null) declinedHere);
          unknown = builtins.filter (d: resolve d == null) declinedHere;
          still-present = stillPresent;
        };
      defaultText = lib.literalMD "what the person's declines did on this machine";
      description = ''
        The `declined` part of `/etc/ronoctua/person-applied.json`, in the
        person's own ids: honoured, refused (an essential program, with its
        reason), unknown to the registry, and still present through another
        way in.
      '';
    };
    flatpaks = lib.mkOption {
      type = lib.types.raw;
      readOnly = true;
      default =
        assert checked;
        map (id: {
          inherit (apps.${id}.flatpak) appId origin;
        }) (builtins.filter (id: apps.${id}.flatpak != null) installedIds);
      defaultText = lib.literalMD "the distro's Flatpaks on this machine";
      description = "The distro's Flatpaks this machine installs, for nix-flatpak (`flatpak.nix`).";
    };
    android = lib.mkOption {
      type = lib.types.raw;
      readOnly = true;
      default =
        assert checked;
        map apkOf (builtins.filter (id: apps.${id}.android != null) installedIds);
      defaultText = lib.literalMD "the distro's Android apps on this machine";
      description = ''
        The distro's Android apps this machine installs, as the records of
        `/etc/ronoctua/declared-android.json` (`from` = `distro`).
      '';
    };
  };

  config = lib.mkMerge [
    # ── anchor: L25 ──
    # THE ENTRIES THAT ARE PARTS, declared as parts (`partOf`): where the
    # parts' module is imported — every machine; a test that leaves it out
    # carries none of them.
    (lib.optionalAttrs (options.ronoctua ? parts) {
      ronoctua.parts = lib.genAttrs partIds partOf;
    })
    {
      # The table's well-formed lines join the modules' own entries.
      ronoctua.apps = lib.mapAttrs fromTable wellFormed;

      # WHAT IS NO PART, at the distro's pins.
      environment.systemPackages =
        assert checked;
        lib.concatMap (id: packagesOf id apps.${id}) (builtins.filter (id: !apps.${id}.part) installedIds);

      environment.etc."ronoctua/distro-apps.json".text =
        assert checked;
        builtins.toJSON report;

      warnings =
        lib.optional (!formatKnown) ''
          The person's declined programs (${toString declinedFile}) say format
          ${builtins.toJSON (declinedDoc.format or null)}, which this distro does not read: nothing
          declined there was honoured.
        ''
        ++ lib.optional (!followedFormatKnown) ''
          The distro pins the person follows (${toString (config.ronoctua.person.distroPins or null)})
          say format ${builtins.toJSON (followedDoc.format or null)}, which this distro does not
          read: the distro's own pins are built.
        '';
    }
  ];

  # ── anchor: L04 ──
}
