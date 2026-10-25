# A DECLARED nixpkgs PACKAGE — the person's `{ source = "nix"; attr; unfree; }`.
#
# One of the five builders the person's declarations go through
# (`nix/modules/person.nix`), and the distro's registry through its own table
# (`nix/modules/distro-apps.nix`). Each takes ONE entry in the shape of
# `declared.json` and answers either what to put where, or why not:
#
#   { ok = true; packages; flatpaks; desktopEntries; apks; }
#   { ok = false; reason; detail; }     reason ∈ the report's closed set
#
# plus `raw`, the same thing with no protection at all, for the update's
# isolated verification (`personEntries`), which has to SEE an entry fail.
#
# # Why a bad entry cannot take the system down
#
# A name that nixpkgs dropped, an alias that `throw`s when read, an unfree
# package nobody consented to, a broken or insecure one: each is caught here
# and becomes a line in `/etc/ronoctua/person-applied.json`, and the system
# evaluates without it. `lib.attrByPath` answers `null` for a missing name and
# `builtins.tryEval` catches the `throw` of an alias and of the meta check
# (which runs when `drvPath` is read).
#
# What it cannot catch is an evaluation ERROR deep inside a dependency
# (`attribute missing` is not a `throw`, and `tryEval` lets it through), nor an
# `abort`. That is why the update verifies every entry in an evaluation of its
# own first, and holds back the one that breaks (`local/held.json`) before the
# system ever sees it — and why the consumer reads nothing of a held entry,
# its name included.
#
# # What a consent covers
#
# `unfree = true` is the person saying yes to THIS program, and its unfree
# name joins the one predicate (`nix/modules/nixpkgs-config.nix`). A program
# is not always one package: `steam` needs `steam-unwrapped`, unfree under
# another name the predicate never heard of, and the meta check of that one
# refuses the whole. So when the machine's nixpkgs refuses a consented
# package and the package's OWN meta lets it in, it is taken from `wide` —
# the same nixpkgs with every unfree licence taken — and only if it evaluates
# there: what it needs being broken, insecure or for another platform still
# refuses it, with that said. The predicate stays the list of what the person
# named; `wide` is instantiated only on that road.
{
  lib,
  pkgs,
  # The same nixpkgs (overlays and all) with `allowUnfree`, or `null` where
  # nobody consents to anything (the distro's own registry): then a package
  # refused for what it needs is skipped, said as such.
  wide ? null,
}:

let
  skip = reason: detail: {
    ok = false;
    inherit reason detail;
  };
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

  # The shape `ronoctua_person` validates, checked again: a Nix path is code,
  # and `pkgs.${…}` of a string that is not one would be the wrong question.
  isAttr =
    attr:
    builtins.isString attr
    && builtins.stringLength attr <= 200
    && builtins.match "[A-Za-z_][A-Za-z0-9_'-]*([.][A-Za-z_][A-Za-z0-9_'-]*)*" attr != null;

  lookupIn = set: attr: lib.attrByPath (lib.splitString "." attr) null set;
  lookup = lookupIn pkgs;

  # `true` only when the question was answered and the answer is yes.
  yes =
    question:
    let
      r = builtins.tryEval question;
    in
    r.success && r.value == true;

  # WHY THE PACKAGE ITSELF IS REFUSED ON THIS MACHINE, or `null` when its own
  # meta lets it in and what refused it was something it needs. The questions
  # nixpkgs' meta check asks of one package, asked of this one alone, so the
  # report can say which — and so a consent is never stretched over a
  # package whose own meta refuses it.
  ownRefusal =
    pkg:
    let
      config = pkgs.config;
      meta = pkg.meta or { };
      available = builtins.tryEval (lib.meta.availableOn pkgs.stdenv.hostPlatform pkg);
      insecurePermitted =
        (config.allowInsecurePredicate
          or (p: builtins.elem (p.name or "") (config.permittedInsecurePackages or [ ]))
        )
          pkg;
      unfreeTaken = (config.allowUnfree or false) || (config.allowUnfreePredicate or (_: false)) pkg;
    in
    if yes (meta.broken or false) && !(config.allowBroken or false) then
      "it is marked broken"
    else if yes ((meta.knownVulnerabilities or [ ]) != [ ]) && !yes insecurePermitted then
      "it is marked insecure"
    else if available.success && !available.value && !(config.allowUnsupportedSystem or false) then
      "it is not built for this platform"
    else if yes (meta.unfree or false) && !yes unfreeTaken then
      "it is unfree, and this machine's unfree list does not take it"
    else
      null;

  # The package from `wide`, when it evaluates there; else `null`.
  widened =
    attr:
    if wide == null then
      null
    else
      let
        p = builtins.tryEval (lookupIn wide attr);
      in
      if p.success && p.value != null && (builtins.tryEval p.value.drvPath).success then
        p.value
      else
        null;
in
{
  # `null` when the entry has this source's fields, else what is wrong.
  shape =
    entry:
    if !isAttr (entry.attr or null) then
      "`attr` is not a nixpkgs attribute path"
    else if !builtins.isBool (entry.unfree or null) then
      "`unfree` is not true or false"
    else
      null;

  # THE NAMES THE PERSON SAID YES TO, for the one unfree predicate
  # (`nix/modules/nixpkgs-config.nix`). By `lib.getName`, which is what the
  # predicate compares. Reading a name never runs the meta check, so asking
  # here does not loop through the predicate it feeds. It DOES read the
  # package, so the consumer asks it only of entries nothing holds back.
  consented =
    entry:
    let
      named = builtins.tryEval (
        let
          p = lookup entry.attr;
        in
        if p != null && lib.isDerivation p then lib.getName p else null
      );
    in
    if entry.unfree && named.success && named.value != null then [ named.value ] else [ ];

  build =
    { entry, ... }:
    let
      found = builtins.tryEval (lookup entry.attr);
      pkg = found.value;
      isPackage = builtins.tryEval (lib.isDerivation pkg);
      unfree = builtins.tryEval (pkg.meta.unfree or false);
      realised = builtins.tryEval pkg.drvPath;
      own = ownRefusal pkg;
      covered = widened entry.attr;
    in
    if !found.success then
      skip "attr-missing" "reading `${entry.attr}` throws: it was renamed or removed from nixpkgs"
    else if pkg == null then
      skip "attr-missing" "nixpkgs has no `${entry.attr}`"
    else if !(isPackage.success && isPackage.value) then
      skip "not-a-package" "`${entry.attr}` is not a package"
    else if !unfree.success then
      skip "refused-by-meta" "the meta of `${entry.attr}` cannot be read"
    # UNFREE IS ASKED HERE, of the person: the system admits every unfree
    # package (the owner, 2026-10-24), and one the person declared without
    # saying yes to its licence must not ride in on that. (What it NEEDS
    # rides in: nothing unfree refuses a machine.)
    else if unfree.value && !entry.unfree then
      skip "refused-by-meta" "`${entry.attr}` is unfree, and its declaration carries no consent"
    else if realised.success then
      done { packages = [ pkg ]; }
    else if own != null then
      skip "refused-by-meta" "the meta of `${entry.attr}` refuses it on this machine: ${own}"
    # Its own meta lets it in, so what refused it is something it needs.
    else if covered != null && entry.unfree then
      done { packages = [ covered ]; }
    else if covered != null then
      skip "refused-by-meta" "`${entry.attr}` needs unfree packages, and its declaration carries no consent"
    else
      skip "refused-by-meta" (
        "`${entry.attr}` itself is allowed, but a package it needs is refused on this machine: "
        + (
          if wide == null then
            "broken, insecure, not for this platform or unfree"
          else
            "broken, insecure or not for this platform"
        )
      );

  # THE PACKAGE `build` WOULD APPLY, unprotected: the machine's own, or the
  # one from `wide` exactly where `build` takes that one. Anything else that
  # is wrong stays wrong here, so the verification sees it fail.
  raw =
    { entry, ... }:
    let
      pkg = lib.attrByPath (lib.splitString "." entry.attr) (throw "nixpkgs has no `${entry.attr}`") pkgs;
      covered = widened entry.attr;
    in
    if
      entry.unfree == true
      && !(builtins.tryEval pkg.drvPath).success
      && ownRefusal pkg == null
      && covered != null
    then
      covered
    else
      pkg;
}
