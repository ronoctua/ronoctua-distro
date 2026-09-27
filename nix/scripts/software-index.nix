# THE SEARCH INDEX OF THE SYSTEM'S OWN nixpkgs — evaluated, never downloaded.
#
# It is nixpkgs' own recipe (`pkgs/top-level/packages-info.nix`, the one Hydra
# runs to publish `packages.json.br`), cut down to the fields a search row
# draws. Evaluating it here instead of fetching the published file is what
# makes "what the search shows" and "what `nix profile add` installs" the SAME
# revision: the published file is per channel release, and this machine's
# nixpkgs moves with its own `flake update`.
#
# Every record is guarded by `tryEval`: one package that throws must cost one
# row, never the index.
{ src, system }:
let
  pkgs = import src {
    inherit system;
    # The configuration nixpkgs itself uses for the search database: no
    # aliases (each would be a second row for one package) and no variants.
    config = {
      allowAliases = false;
      allowVariants = false;
      recursionMode = "search";
    };
  };
  inherit (pkgs) lib;

  # A field that a tab or a newline would split. Descriptions are data and lose
  # nothing by being flattened; the length is capped because a row shows one
  # line and the index is read on every keystroke.
  flat =
    s:
    let
      one = lib.replaceStrings [ "\t" "\n" "\r" ] [ " " " " " " ] (toString s);
    in
    if lib.stringLength one > 200 then lib.substring 0 200 one else one;

  first = v: if lib.isList v then (if v == [ ] then "" else lib.head v) else v;

  spdx =
    l:
    let
      one = first l;
    in
    if lib.isAttrs one then (one.spdxId or one.shortName or "") else toString one;

  record =
    path: value:
    let
      meta = value.meta or { };
      r = {
        attr = lib.showAttrPath path;
        pname = flat (value.pname or value.name or "");
        version = flat (value.version or "");
        description = flat (meta.description or "");
        program = flat (meta.mainProgram or "");
        unfree = meta.unfree or false;
        broken = meta.broken or false;
        insecure = (meta.knownVulnerabilities or [ ]) != [ ];
        # The PLATFORM's answer, and not `meta.available`: that one is also
        # false for every unfree package under this config, which would draw
        # Chrome as "not for this machine" when it only needs a yes.
        available = lib.meta.availableOn pkgs.stdenv.hostPlatform value;
        homepage = flat (first (meta.homepage or ""));
        license = flat (spdx (meta.license or ""));
        library = lib.length path > 1;
      };
      tried = builtins.tryEval (builtins.deepSeq r r);
    in
    if tried.success then [ tried.value ] else [ ];

  walk =
    path: value:
    let
      tried = builtins.tryEval (
        if path == [ "AAAAAASomeThingsFailToEvaluate" ] || !(lib.isAttrs value) then
          [ ]
        else if lib.isDerivation value then
          record path value
        else if value.recurseForDerivations or false then
          lib.concatLists (lib.mapAttrsToList (n: v: walk (path ++ [ n ]) v) value)
        else
          [ ]
      );
    in
    if tried.success then tried.value else [ ];
in
builtins.toJSON (walk [ ] (lib.recurseIntoAttrs pkgs))
