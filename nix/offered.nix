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
# - An id the ledger's `ids` does not have (or `ids` that is no list): a
#   decline of it would name no program the registry ever published.
# - A `checked` that is not a boolean, a field the row does not take.
# - A name that is missing, not text, empty, not one the registry takes
#   (`nix/names.nix`: a control character, white space alone), spans lines
#   (U+0085 and U+2028 too), has white space at an edge (U+00A0 too), or is
#   the name of another row: the screen draws it on one line, and two rows
#   that read alike cannot be told apart.
#
# Each defect is said in a sentence of its own, naming the row, and a guard
# that never refused is not tested: `offered-refuses-each-malformed-row`
# plants every one of them in a ledger of its own and asks for its sentence.
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
  isSlug = s: builtins.isString s && builtins.match "[a-z0-9][a-z0-9-]{0,63}" s != null;

  # THE REGISTRY'S RULE FOR A NAME (`nix/names.nix`), and its characters.
  names = import ./names.nix { inherit lib; };

  fields = [
    "id"
    "checked"
    "name"
  ];

  # WHAT A LEDGER'S `offered` IS REFUSED FOR: a sentence a defect, and one
  # defect one sentence — the build asks it of the tree's ledger, and the
  # negative fixture (`offered-refuses-each-malformed-row`) of a planted
  # ledger per defect, in one evaluation.
  problemsOf =
    ledger:
    let
      rows = ledger.offered or null;
      # The ids the registry published; a ledger whose `ids` is no list is
      # said once, and no row is judged against it.
      ids = ledger.ids or [ ];
      idsKnown = builtins.isList ids;

      # The name a row is drawn with: its own, or the table's (by an id that
      # is text: any other is refused on its own).
      nameOf =
        row:
        let
          id = row.id or null;
        in
        row.name or ((ledger.apps or { }).${if builtins.isString id then id else ""}.name or null);

      # A NAME OF THE REGISTRY ON ONE LINE: what `isName` refuses (a
      # control character, white space alone), and a line break past ASCII
      # (U+0085, U+2028) or white space at an edge (U+00A0) besides — the
      # console draws the row on one line, and two names that read alike
      # cannot be told apart. One sentence a defect, in this order.
      nameProblem =
        id: name:
        if name == null then
          "has no name, and the `apps` table names no ${builtins.toJSON id}"
        else if !builtins.isString name then
          "the name ${builtins.toJSON name} is not text"
        else if name == "" then
          "the name is empty"
        else if names.spansLines name then
          "the name ${builtins.toJSON name} spans lines"
        else if names.hasControl name then
          "the name ${builtins.toJSON name} holds a control character"
        else if names.isWhiteAlone name then
          "the name ${builtins.toJSON name} is white space alone"
        else if names.hasWhiteAtAnEdge name then
          "the name ${builtins.toJSON name} begins or ends with white space"
        else
          assert names.isName name;
          null;

      rowProblems =
        i: row:
        let
          at = "assets/distro-apps.nix: offered row ${toString i}";
          id = row.id or null;
          extra = lib.subtractLists fields (builtins.attrNames row);
          name = nameProblem id (nameOf row);
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
            lib.optional (isSlug id && id != "none" && idsKnown && !(builtins.elem id ids))
              "${at}: `${id}` is not in the ledger's `ids`, so no program of the registry was ever published by it"
          ++ lib.optional (!builtins.isBool (row.checked or null)) "${at}: `checked` is true or false"
          ++ lib.optional (name != null) "${at}: ${name}";

      repeated = l: lib.unique (builtins.filter (x: lib.count (y: y == x) l > 1) l);
    in
    if !builtins.isList rows then
      [ "assets/distro-apps.nix: `offered` is a list of rows" ]
    else if rows == [ ] then
      [ "assets/distro-apps.nix: `offered` has no row; the installer would offer nothing to leave out" ]
    else
      lib.optional (
        !idsKnown
      ) "assets/distro-apps.nix: `ids` is the list of the registry's ids, not ${builtins.toJSON ids}"
      ++ lib.concatLists (lib.imap0 rowProblems rows)
      ++ map (id: "assets/distro-apps.nix: `${toString id}` is offered twice") (
        repeated (builtins.filter isSlug (map (r: r.id or null) rows))
      )
      ++ map (n: "assets/distro-apps.nix: two offered rows are called ${builtins.toJSON n}") (
        repeated (builtins.filter builtins.isString (map nameOf rows))
      );

  ledger = import ledgerFile;
  rows = ledger.offered;
  problems = problemsOf ledger;

  # The name a row of the tree's ledger is drawn with.
  nameOf = row: row.name or ((ledger.apps or { }).${row.id}.name or null);

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
  # The refusals, for the negative fixture.
  passthru = { inherit problemsOf; };
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
