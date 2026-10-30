# WHICH STORE NAMES ARE UNFREE, for `nix run .#release` (`nix/release.nix`):
# what of the reference systems' closure may never go to the public cache.
# That closure is the SUBSTRATE — what remains a Nix build. The apps are no
# longer distro Nix builds: they self-update outside the generation, so there
# is no public cache of apps for an unfree one to reach (the owner's amendment
# of 2026-10-30, replacing 'nothing imperative').
#
# NOT ONE PREDICATE: three questions, and a name any of them calls unfree is
# unfree. The system's `allowUnfreePredicate` is not the only door an unfree
# package comes in by — the parts' one door (`nix/parts-pkgs.nix`) admits
# `uade`, `unrar` and the rest through a nixpkgs of its own, whose predicate
# the system never sees, and with Android on the system lets EVERY unfree
# package in (`allowUnfree`) without asking its predicate at all. So:
#
# 1. THE LICENCE, as nixpkgs itself judges it (`meta.license`, a licence
#    whose `free` is false — `check-meta.nix`'s own test): asked of the
#    package of that name in the reference machine's `pkgs`, whichever door
#    it came in by. This is what catches `uade`, and anything a module, a
#    separate nixpkgs or `allowUnfree` lets in under its own name. A
#    package that only shares the name with an unfree one is left out with
#    it: a free build left out is only built by its machine.
# 2. THE PREDICATE the released distro uses (`nix/modules/nixpkgs-config.nix`,
#    `config` being the reference machine's `nixpkgs.config`), never a copy
#    of its list: for what has no top-level name to ask the licence of — the
#    SDK's pieces are named after the ZIP they came from.
# 3. With Android on, ANYTHING NAMED `android…`, for the same reason.
#
# What none of the three can see is an unfree package that is neither
# top-level by its own name, nor in the predicate, nor named `android…`.
# Every place that admits an unfree package is listed in
# `release-cache-holds-only-free-builds`, with what covers it; a new one
# fails that check until it says.
#
# A function of the configuration, the packages and the names, so the check
# asks it exactly what the release asks.
{
  config,
  pkgs,
  names,
}:

let
  inherit (pkgs) lib;
  pnameOf = name: (builtins.parseDrvName name).name;

  # nixpkgs' own judgement (`check-meta.nix`, `isUnfree`): a licence is
  # unfree when it says `free = false`; one that says nothing is free.
  unfreeLicence = l: builtins.isAttrs l && !(l.free or true);

  # THE LICENCE OF THE PACKAGE OF THAT NAME, or `false` when there is none
  # to ask: no such attribute, not a derivation, or an attribute that only
  # throws (a removed package's alias). Forced inside `tryEval`, so a throw
  # is "nothing to ask" and not the end of the release.
  licenceUnfree =
    pname:
    builtins.hasAttr pname pkgs
    && (
      let
        tried = builtins.tryEval (
          let
            p = pkgs.${pname};
            verdict = lib.isDerivation p && lib.any unfreeLicence (lib.toList (p.meta.license or [ ]));
          in
          builtins.seq verdict verdict
        );
      in
      tried.success && tried.value
    );

  android = name: builtins.substring 0 7 (pnameOf name) == "android";

  unfree =
    name:
    licenceUnfree (pnameOf name)
    || (config.allowUnfreePredicate or (_: false)) { inherit name; }
    || ((config.allowUnfree or false) && android name);
in
builtins.filter unfree names
