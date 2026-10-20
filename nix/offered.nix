# The programs the installer offers to leave out, from the ledger to Rust.
#
# `assets/distro-apps.nix`'s `offered` is the one place the list lives: which
# programs, in which order, and which come marked. This turns it into the
# table `crates/ronoctua-installer` includes (`RONOCTUA_OFFERED_RS`), and
# REFUSES THE BUILD when a row could not mean what the screen says.
#
# # Why the ledger and not the machine
#
# The installer is built for the ISO, and evaluating a whole machine to learn
# twenty names would make its build wait on every module of the distro. The
# ledger is a plain file; what the list promises about the REGISTRY (each id a
# program the `full` profile carries and `minimal` does not, declinable, and
# gone when declined) is asked of the evaluated machines by
# `offered-programs-are-the-registrys`.
#
# # What is refused, and why each
#
# - No row: the screen would be a "continue" with nothing to mark.
# - An id that is not a slug, is repeated, or is `none`: the answer is the
#   unmarked ids joined by commas, or `none` — a comma inside an id, the same
#   id twice or a program called `none` would make two answers read alike.
# - An id the ledger's `ids` does not have: a decline of it would name no
#   program the registry ever published.
# - A `checked` that is not a boolean, a field the row does not take.
# - A name that is empty, spans lines, holds a control character, or is the
#   name of another row: the screen draws it on one line, and two rows that
#   read alike cannot be told apart.
#
# A row's `name` may be left out when the ledger's `apps` table has the
# program with a `name` of its own (an AppImage, a web app): it is then that
# one, and the build refuses a row with neither.
{
  lib,
  writeTextFile,
  ledgerFile,
}:

let
  ledger = import ledgerFile;
  rows = ledger.offered or null;

  isSlug = s: builtins.isString s && builtins.match "[a-z0-9][a-z0-9-]{0,63}" s != null;

  # The name a row is drawn with: its own, or the table's.
  nameOf = row: row.name or ((ledger.apps or { }).${row.id or ""}.name or null);

  # ONE LINE OF PRINTABLE TEXT: no line break, no tab, no other control
  # character (the console's font draws none of them, and a row would move).
  isLine = s: builtins.isString s && builtins.match "[^[:cntrl:]]+" s != null && lib.trim s == s;

  fields = [
    "id"
    "checked"
    "name"
  ];

  rowProblems =
    i: row:
    let
      at = "assets/distro-apps.nix: offered row ${toString i}";
      id = row.id or null;
      extra = if builtins.isAttrs row then lib.subtractLists fields (builtins.attrNames row) else [ ];
    in
    if !builtins.isAttrs row then
      [ "${at} is not an attribute set" ]
    else
      lib.optional (extra != [ ]) "${at} has fields no row takes: ${toString extra}"
      ++ lib.optional (!isSlug id) "${at}: the id ${builtins.toJSON id} is not a slug ([a-z0-9-])"
      ++ lib.optional (
        id == "none"
      ) "${at}: `none` is the answer that leaves nothing out, and no program's id"
      ++
        lib.optional (isSlug id && !(builtins.elem id (ledger.ids or [ ])))
          "${at}: `${id}` is not in the ledger's `ids`, so no program of the registry was ever published by it"
      ++ lib.optional (!builtins.isBool (row.checked or null)) "${at}: `checked` is true or false"
      ++
        lib.optional (!isLine (nameOf row))
          "${at}: the name ${builtins.toJSON (nameOf row)} is not one line of text (and the `apps` table names no `${toString id}`)";

  repeated = l: lib.unique (builtins.filter (x: lib.count (y: y == x) l > 1) l);

  problems =
    if !builtins.isList rows then
      [ "assets/distro-apps.nix: `offered` is a list of rows" ]
    else if rows == [ ] then
      [ "assets/distro-apps.nix: `offered` has no row; the installer would offer nothing to leave out" ]
    else
      lib.concatLists (lib.imap0 rowProblems rows)
      ++ map (id: "assets/distro-apps.nix: `${toString id}` is offered twice") (
        repeated (map (r: r.id or null) rows)
      )
      ++ map (n: "assets/distro-apps.nix: two offered rows are called ${builtins.toJSON n}") (
        repeated (map nameOf rows)
      );

  # The unmarked ids, in the list's order — or `none`: what a person who
  # presses Enter through the screen answers.
  unchecked = map (r: r.id) (builtins.filter (r: !r.checked) rows);
  defaultLeftOut = if unchecked == [ ] then "none" else lib.concatStringsSep "," unchecked;

  # A JSON string is a Rust string literal for every text this can hold: the
  # control characters (whose JSON escapes Rust does not read) are refused
  # above, and `"` and `\` escape the same way in both.
  row = r: ''
    Offered {
        id: ${builtins.toJSON r.id},
        name: ${builtins.toJSON (nameOf r)},
        checked: ${lib.boolToString r.checked},
    },'';
in
assert lib.assertMsg (problems == [ ]) (
  "The installer's offered programs refuse:\n  " + lib.concatStringsSep "\n  " problems
);
writeTextFile {
  name = "ronoctua-offered-rs";
  destination = "/offered.rs";
  text = ''
    // Generated from `assets/distro-apps.nix` (`offered`). Do not edit.
    //
    // What the build refuses is in `nix/offered.nix`.

    /// The programs a `full` installation lists, in the order shown.
    pub const OFFERED: &[Offered] = &[
    ${lib.concatMapStringsSep "\n" row rows}
    ];

    /// The unmarked ids joined by `,`, or `none`: the answer of whoever
    /// presses Enter through the screen.
    pub const DEFAULT_LEFT_OUT: &str = ${builtins.toJSON defaultLeftOut};
  '';
}
