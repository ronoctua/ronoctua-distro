# THE PERSON'S BINARY CACHE — its two paths and the one judge of what the
# person named, for the three readers that have no NixOS configuration in
# hand: the machine's module (`nix/modules/binary-cache.nix`), the
# persisted folders (`nix/modules/persist.nix`) and the check that holds this
# judge and the Rust one to one table (`binary-cache-declared-once`).
# Builtins only, so `import` needs nothing.
#
# The Rust side spells the same two paths (`ronoctua_cachix::SYSTEM_FILE`,
# `ronoctua_cachix::NETRC`); the check compares them.
rec {
  # WHAT THE MACHINE WAS BUILT WITH, for the push: the cache, and the roots a
  # public one may receive. Absent when the person names no cache.
  systemFile = "/etc/ronoctua/binary-cache.json";

  # ROOT'S NETRC FOR A PRIVATE CACHE (`nix.settings.netrc-file`), written
  # through the sudo door and by the installer, in a folder of its own that
  # survives the wipe (`nix/modules/persist.nix`).
  netrc = "/var/lib/ronoctua-binary-cache/netrc";
  netrcDir = builtins.dirOf netrc;

  # The format of the person's files (`ronoctua_person::FORMAT`).
  format = 1;

  # A Cachix name: lower-case letters, digits and dashes, starting with a
  # letter or a digit, at most 63 — the host label of `<name>.cachix.org`.
  isName = name: builtins.isString name && builtins.match "[a-z0-9][a-z0-9-]{0,62}" name != null;

  # The key cachix.org shows: `<name>.cachix.org-1:` and an ed25519 public
  # key in base64 — 43 characters and one `=`. The name is judged first, and
  # a name holds no character a regular expression reads as anything but
  # itself.
  isKeyOf =
    name: key:
    isName name
    && builtins.isString key
    && builtins.match "${name}\\.cachix\\.org-1:[A-Za-z0-9+/]{43}=" key != null;

  # THE PERSON'S `options.json`, read as JSON (`doc`), judged: the cache it
  # names — `{ name; publicKey; visibility; private; host; substituter; }` —
  # or `null` for none and for anything that is not exactly one: another
  # format, a field this distro does not know, a name that is not Cachix's,
  # a key that is not the name's, a visibility that is neither. The same
  # questions `ronoctua_person::options::BinaryCache::fault` asks, held to
  # the same table (`crates/ronoctua-person/fixtures/binary-cache-cases.json`).
  judge =
    doc:
    let
      given = if builtins.isAttrs doc then doc.binary-cache or null else null;
      fields = builtins.attrNames given;
    in
    if !(builtins.isAttrs doc) || (doc.format or null) != format || !(builtins.isAttrs given) then
      null
    else if
      fields == [
        "name"
        "public-key"
        "visibility"
      ]
      && isName given.name
      && isKeyOf given.name given.public-key
      && builtins.elem given.visibility [
        "public"
        "private"
      ]
    then
      {
        inherit (given) name visibility;
        publicKey = given.public-key;
        private = given.visibility == "private";
        host = "${given.name}.cachix.org";
        substituter = "https://${given.name}.cachix.org";
      }
    else
      null;
}
