# THE DISTRO'S APPIMAGES THE PERSON FOLLOWS PAST ITS PINS — the one judge of a
# followed pin (`software/distro-pins.json`, `ronoctua_person::distro_pins`)
# for the readers that have no NixOS configuration in hand: the registry
# (`nix/modules/distro-apps.nix`) and the check that holds this judge and the
# Rust one to one table of cases (`distro-apps-follow-when-on-and-newer`, over
# `crates/ronoctua-person/fixtures/distro-pin-cases.json`). Builtins only, so
# `import` needs nothing.
#
# A pin is judged against the table entry of its id; each rule answers the
# word of `ronoctua_person::distro_pins::Fault`, in the same order:
#
#   id       a slug naming an AppImage of the table that follows a repository
#   url      exactly https://github.com/<that repo>/releases/download/<tag>/<asset>,
#            tag and asset one segment of [A-Za-z0-9._+-] each, no `..`
#   asset    the asset matches the table's `assetPattern` (`*` globs only)
#   version  the tag without one leading v before a digit, with a digit, and a
#            store name's version: `<key>-<version>.AppImage` is the name the
#            distro's builder fetches it under (`nix/distro-downloads.nix`)
#   hash     sha256- and 44 characters of base64
#
# A refused pin is never a reason for the machine to stop evaluating: the
# registry drops it, says it in `/etc/ronoctua/distro-apps.json` and builds
# the distro's pin — which is also what it does with every value a pin
# could hold that is not text.
rec {
  # The format of the person's files (`ronoctua_person::FORMAT`).
  format = 1;

  # The field of `options.json` that turns following on
  # (`ronoctua_person::options::Options::distro_apps_follow_releases`).
  option = "distro-apps-follow-releases";

  isText = builtins.isString;
  isSlug = s: isText s && builtins.match "[a-z0-9][a-z0-9-]{0,63}" s != null;
  isSegment =
    s: isText s && builtins.stringLength s <= 255 && builtins.match "[A-Za-z0-9._+-]+" s != null;
  isStoreVersion =
    v:
    isText v
    && builtins.match "[A-Za-z0-9._+-]{1,64}" v != null
    && builtins.match ".*[0-9].*" v != null;
  isSha256 = h: isText h && builtins.match "sha256-[A-Za-z0-9+/]{43}=" h != null;

  # `v1.2.3` is `1.2.3`; any other tag is its own version — the distro's own
  # tool's `version_of_tag` (`nix/scripts/distro-pins.py`).
  versionOfTag =
    tag:
    if builtins.match "[vV][0-9].*" tag != null then
      builtins.substring 1 (builtins.stringLength tag) tag
    else
      tag;

  # A `*`-only glob over a whole name, as a regular expression whose every
  # other character is itself.
  glob =
    pattern: name:
    let
      special = [
        "\\"
        "."
        "+"
        "?"
        "("
        ")"
        "["
        "]"
        "{"
        "}"
        "^"
        "$"
        "|"
      ];
      escape = builtins.replaceStrings special (map (c: "\\" + c) special);
      pieces = builtins.filter builtins.isString (builtins.split "[*]" pattern);
    in
    builtins.match (builtins.concatStringsSep ".*" (map escape pieces)) name != null;

  # What the table's entry `id` follows — `{ repo; assetPattern; }` — or
  # `null` when the table has no AppImage of that id or it follows nothing.
  followsOf =
    table: id:
    let
      t = table.${id} or null;
      f = if builtins.isAttrs t then t.follows or null else null;
      g = if builtins.isAttrs f then f.github or null else null;
    in
    if
      builtins.isAttrs t
      && (t.source or null) == "appimage"
      && builtins.isAttrs g
      && isText (g.repo or null)
      && isText (g.assetPattern or null)
    then
      g
    else
      null;

  # The owner, the name, the tag and the asset of a release download, or
  # `null` for an address that is not exactly one.
  partsOf =
    url:
    let
      p =
        if isText url && builtins.replaceStrings [ ".." ] [ "" ] url == url then
          builtins.match "https://github\\.com/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)/releases/download/([^/]*)/([^/]*)" url
        else
          null;
    in
    if p != null && isSegment (builtins.elemAt p 2) && isSegment (builtins.elemAt p 3) then
      {
        repo = "${builtins.elemAt p 0}/${builtins.elemAt p 1}";
        tag = builtins.elemAt p 2;
        asset = builtins.elemAt p 3;
      }
    else
      null;

  # THE JUDGE: the word of the first rule `pin` breaks against `table`, or
  # `null` for a pin. Total: a value of any shape is judged, never thrown on.
  judge =
    table: pin:
    let
      at = if builtins.isAttrs pin then pin else { };
      follows = if isSlug (at.id or null) then followsOf table at.id else null;
      parts = partsOf (at.url or null);
    in
    if follows == null then
      "id"
    else if parts == null || parts.repo != follows.repo then
      "url"
    else if !(glob follows.assetPattern parts.asset) then
      "asset"
    else if (at.version or null) != versionOfTag parts.tag || !(isStoreVersion at.version) then
      "version"
    else if !(isSha256 (at.hash or null)) then
      "hash"
    else
      null;

  # The registry's sentence for a refused pin, English, for the journal.
  sentence =
    id: fault:
    {
      id = "the followed pin `${id}` names no AppImage of the distro's table that follows a repository";
      url = "the followed pin of ${id} is not a release download of the repository its table entry follows";
      asset = "the followed pin of ${id} names an asset its table's pattern does not match";
      version = "the followed pin of ${id} says a version that is not its tag's, has no digit or cannot name a store path";
      hash = "the followed pin of ${id} has no SRI SHA-256 hash";
      duplicate = "a second followed pin of ${id}: the first one is the pin";
    }
    .${fault};

  # THE PERSON'S FOLLOWED PINS, read: `{ by; refused; }` — the pins the judge
  # took, by id (the first of an id; a second is refused), and every refused
  # one with its rule and its sentence. Nothing while the person does not
  # follow (`on`), and nothing from a document of another format (the
  # registry says that one apart).
  read =
    {
      on,
      doc,
      table,
    }:
    let
      pins =
        if
          on && builtins.isAttrs doc && (doc.format or null) == format && builtins.isList (doc.pins or null)
        then
          doc.pins
        else
          [ ];
      step =
        acc: p:
        let
          fault = judge table p;
          id = if builtins.isAttrs p && isText (p.id or null) then p.id else "unknown";
          refuse =
            word:
            acc
            // {
              refused = acc.refused ++ [
                {
                  inherit id;
                  fault = word;
                  detail = sentence id word;
                }
              ];
            };
        in
        if fault != null then
          refuse fault
        else if acc.by ? ${id} then
          refuse "duplicate"
        else
          acc
          // {
            by = acc.by // {
              ${id} = p;
            };
          };
    in
    builtins.foldl' step {
      by = { };
      refused = [ ];
    } pins;
}
