# A DECLARED WEB APP — `{ source = "web"; slug; name; url; profile; icon; category; }`.
#
# The builder's contract is `nix/declared-apps/nix.nix`'s. A web app is a
# launcher entry and nothing else: Home Manager's `xdg.desktopEntries` puts it
# in the person's profile, and it says what the software centre's own entries
# say (`crates/ronoctua-software/src/web.rs`, `desktop_text`):
#
#   Exec=<the stable ronoctua-software> web-open <slug>
#   TryExec=<the stable google-chrome-stable>
#   X-Ronoctua-Web-Url / X-Ronoctua-Web-Profile
#
# The address is never in `Exec=`: `web-open` reads it back from the entry, so
# nothing a site's address holds reaches a line with quotes and `%` codes. The
# slug is the only thing that does, and it is checked here with the same rule
# (`[a-z0-9][a-z0-9-]*`).
#
# `StartupWMClass` is Chromium's window id for the address and the profile —
# how the modeline names the window. It is the arithmetic `web::app_id` does,
# done again here because a launcher entry written by Nix cannot ask the Rust;
# both follow Chromium's `GenerateApplicationNameFromURL`.
#
# # Only an address already in the form a browser gives it
#
# The Rust reads the host and the path AFTER `Url::parse`, which rewrites an
# address: it removes `.` and `..` segments (`%2e` ones too), punycodes a host
# outside ASCII, percent-encodes a path outside ASCII and `"<>`{}`, turns `\`
# into `/`, decodes `%41` in a host and writes `127.1` as `127.0.0.1`. Nix has
# no such parser, and doing half of it would be a third arithmetic. So an
# address that any of that would change is REFUSED here, said as such, and
# one that none of it changes has the same host and path on both sides — the
# software centre declares what `Url::parse` gave it. The profile is printable
# ASCII (Chrome's folders are `Default`, `Profile 1`…), so counting bytes is
# counting characters. `web-app-ids-agree` reads one table of (address,
# profile) → id with the Rust test, and the refused addresses beside it.
{ lib, pkgs }:

let
  # Nothing here can fail to evaluate once the shape is right, so this builder
  # never answers `ok = false`.
  done =
    what:
    {
      ok = true;
      packages = [ ];
      flatpaks = [ ];
      desktopEntries = { };
      apks = [ ];
    }
    // what;

  # The two stable paths `web.rs` names (`SOFTWARE`, `CHROME`): a store path
  # in a launcher entry would open a deleted program after a collection.
  software = "/run/current-system/sw/bin/ronoctua-software";
  chrome = "/run/current-system/sw/bin/google-chrome-stable";
  prefix = "ronoctua-web-";

  # The freedesktop main categories a web app may take (`web::CATEGORIES`);
  # anything else is `Network`, a web app's own.
  categories = [
    "AudioVideo"
    "Audio"
    "Video"
    "Development"
    "Education"
    "Game"
    "Graphics"
    "Network"
    "Office"
    "Science"
    "Settings"
    "System"
    "Utility"
  ];

  isSlug = slug: builtins.isString slug && builtins.match "[a-z0-9][a-z0-9-]{0,63}" slug != null;

  # WHY AN ADDRESS IS REFUSED, or `null` (see the header).
  urlProblem =
    url:
    let
      parts = builtins.match "https?://([^/?#]*)([^?#]*).*" url;
      authority = builtins.elemAt parts 0;
      path = builtins.elemAt parts 1;
      hostPort = builtins.match "([^:]*)(:([0-9]*))?" authority;
      host = builtins.elemAt hostPort 0;
      port = builtins.elemAt hostPort 2;
      label = "[A-Za-z0-9_-]+";
      named = builtins.match "${label}([.]${label})*[.]?" host != null;
      labels = lib.splitString "." (lib.removeSuffix "." host);
      # WHATWG: a host whose last label is a number is an IPv4 address, and
      # only the dotted quad of plain decimals comes back as it went in.
      numeric = builtins.match "[0-9]+|0[xX][0-9A-Fa-f]*" (lib.last labels) != null;
      octet = "(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])";
      quad = builtins.match "${octet}[.]${octet}[.]${octet}[.]${octet}" host != null;
      portNumber = builtins.match "0*([0-9]{1,5})" port;
      dotted = s: builtins.match "([.]|%2[eE])([.]|%2[eE])?" s != null;
    in
    if !builtins.isString url then
      "`url` is not text"
    else if builtins.stringLength url > 2048 then
      "`url` is longer than 2048 characters"
    else if builtins.match "[!-~]*" url == null then
      "`url` holds a space, a control or a character outside ASCII: a browser would rewrite it"
    else if parts == null then
      "`url` is not an http or https address"
    else if lib.hasInfix "@" authority then
      "`url` carries a user name"
    else if hostPort == null || !named then
      "`url` has no host a browser keeps as written"
    else if numeric && !quad then
      "`url` names an IPv4 address in a form a browser rewrites"
    else if
      port != null
      && port != ""
      && (portNumber == null || builtins.fromJSON (builtins.head portNumber) > 65535)
    then
      "`url` has a port no browser accepts"
    else if builtins.any dotted (lib.splitString "/" path) then
      "`url` has a `.` or `..` segment, which a browser removes"
    else if builtins.match ".*[\"<>`{}\\].*" path != null then
      "`url` has a character in its path that a browser percent-encodes or turns into `/`"
    else
      null;

  # Chrome's profile folders: printable ASCII.
  isProfile =
    profile:
    builtins.isString profile
    && builtins.stringLength profile <= 256
    && builtins.match "[ -~]+" profile != null;
  isText =
    text:
    builtins.isString text
    && text != ""
    && builtins.stringLength text <= 256
    && builtins.match ".*[[:cntrl:]\\].*" text == null;

  # CHROMIUM'S WINDOW ID for an address and a profile: `chrome-<host>_<path>-<profile>`,
  # with what a file name may not hold replaced by `_` (and a space, and a
  # dot, tilde or white space at either end). For an address `urlProblem`
  # takes and an ASCII profile: there a byte is a character, and the host and
  # the path are what `Url::parse` gives the Rust (the host lower-cased).
  appId =
    url: profile:
    let
      parts = builtins.match "https?://([^/?#]*)([^?#]*).*" url;
      authority = if parts == null then "" else builtins.elemAt parts 0;
      host = lib.toLower (builtins.head (lib.splitString ":" (lib.last (lib.splitString "@" authority))));
      path = if parts == null || builtins.elemAt parts 1 == "" then "/" else builtins.elemAt parts 1;
      chars = lib.stringToCharacters "chrome-${host}_${path}-${profile}";
      last = builtins.length chars - 1;
      illegal =
        c:
        builtins.elem c [
          "\""
          "*"
          "/"
          ":"
          "<"
          ">"
          "?"
          "\\"
          "|"
          " "
        ]
        || builtins.match "[[:cntrl:]]" c != null;
      atEnd = c: c == "." || c == "~" || builtins.match "[[:space:]]" c != null;
    in
    lib.concatStrings (
      lib.imap0 (i: c: if illegal c || ((i == 0 || i == last) && atEnd c) then "_" else c) chars
    );

  hostOf =
    url:
    let
      parts = builtins.match "https?://([^/?#]*).*" url;
    in
    if parts == null then "" else builtins.head parts;

  # The person's own picture, when the declaration names one and the file is
  # the one it names (content-addressed: `<sha256>.png`).
  iconOf =
    { entry, icons }:
    let
      sha = entry.icon or null;
      file = icons + "/${sha}.png";
    in
    if icons == null || !builtins.isString sha || builtins.match "[0-9a-f]{64}" sha == null then
      null
    else if !builtins.pathExists file then
      null
    else if builtins.hashFile "sha256" file != sha then
      null
    else
      file;

  entryOf =
    { entry, icons }:
    let
      icon = iconOf { inherit entry icons; };
      category = if builtins.elem (entry.category or null) categories then entry.category else "Network";
    in
    {
      name = entry.name;
      comment = hostOf entry.url;
      exec = "${software} web-open ${entry.slug}";
      terminal = false;
      startupNotify = true;
      categories = [ category ];
      settings = {
        TryExec = chrome;
        StartupWMClass = appId entry.url entry.profile;
        X-Ronoctua-Web-Url = entry.url;
        X-Ronoctua-Web-Profile = entry.profile;
        X-Ronoctua-Declared = "web:${entry.slug}";
      };
    }
    // lib.optionalAttrs (icon != null) { inherit icon; };
in
{
  inherit appId urlProblem;

  shape =
    entry:
    if !isSlug (entry.slug or null) then
      "`slug` is not a slug"
    else if !isText (entry.name or null) then
      "`name` is not a name"
    else if urlProblem (entry.url or null) != null then
      urlProblem entry.url
    else if !isProfile (entry.profile or null) then
      "`profile` is not a Chrome profile's folder"
    else
      null;

  build =
    {
      entry,
      icons ? null,
      ...
    }:
    done { desktopEntries.${prefix + entry.slug} = entryOf { inherit entry icons; }; };

  # The entry as text, for the verification: a web app has nothing to build
  # but the file the launcher reads.
  raw =
    {
      entry,
      icons ? null,
      ...
    }:
    pkgs.writeText "${prefix}${entry.slug}.json" (
      builtins.toJSON (entryOf {
        inherit entry icons;
      })
    );

}
