# The camera's effects, from Nix to Rust.
#
# `assets/camera-effects.nix` is the one place they are declared. This turns
# the list into the Rust table `ronoctua-effects` includes, and REFUSES THE
# BUILD for a declaration that cannot be drawn or turned: a default outside its
# range, a step that does not move, an id twice, a family nobody draws.
#
# No Python, for `nix/cache.nix`'s reason: the checks are membership and
# arithmetic, and a generator would be one more file to read before believing
# the table.
{
  lib,
  writeTextFile,
  effectsFile,
}:

let
  declared = (import effectsFile).effects;

  families = [
    "colour"
    "tone"
    "shape"
    "key"
    "background"
  ];

  kebab = s: builtins.match "[a-z][a-z0-9]*(-[a-z0-9]+)*" s != null;

  ids = map (e: e.id) declared;

  checkParam =
    e: p:
    assert lib.assertMsg (kebab p.name)
      "assets/camera-effects.nix: `${e.id}` has a parameter named ${builtins.toJSON p.name}, which is not kebab-case";
    assert lib.assertMsg (lib.all lib.isInt [
      p.min
      p.max
      p.default
      p.step
    ]) "assets/camera-effects.nix: every number of `${e.id}.${p.name}` must be a whole number";
    assert lib.assertMsg (p.min < p.max)
      "assets/camera-effects.nix: `${e.id}.${p.name}` has min ${toString p.min} not below max ${toString p.max}";
    assert lib.assertMsg (p.min <= p.default && p.default <= p.max) ''
      assets/camera-effects.nix: `${e.id}.${p.name}` defaults to ${toString p.default},
      outside its own range ${toString p.min}..${toString p.max}. The settings view would open
      on a value no key can reach again.
    '';
    assert lib.assertMsg (p.step > 0 && p.step <= p.max - p.min)
      "assets/camera-effects.nix: `${e.id}.${p.name}` has a step of ${toString p.step}, which does not move inside its range";
    true;

  checkEffect =
    e:
    assert lib.assertMsg (kebab e.id)
      "assets/camera-effects.nix: ${builtins.toJSON e.id} is not a kebab-case id";
    assert lib.assertMsg (builtins.elem e.family families)
      "assets/camera-effects.nix: `${e.id}` is in family ${builtins.toJSON e.family}, which is not one of ${builtins.toJSON families}";
    assert lib.all (checkParam e) e.params;
    assert lib.assertMsg (
      lib.length (lib.unique (map (p: p.name) e.params)) == lib.length e.params
    ) "assets/camera-effects.nix: `${e.id}` names one parameter twice";
    true;

  rsParam =
    p:
    ''Param { name: "${p.name}", key: "camera-param-${p.name}", min: ${toString p.min}, max: ${toString p.max}, default: ${toString p.default}, step: ${toString p.step} },'';

  rsEffect = e: ''
    Declared {
        id: "${e.id}",
        key: "camera-effect-${e.id}",
        family: "${e.family}",
        mask: ${lib.boolToString (e.mask or false)},
        params: &[
            ${lib.concatMapStringsSep "\n            " rsParam e.params}
        ],
    },'';

  paramNames = lib.unique (lib.concatMap (e: map (p: p.name) e.params) declared);
in
assert lib.assertMsg (declared != [ ]) "assets/camera-effects.nix: no effects";
assert lib.all checkEffect declared;
assert lib.assertMsg (
  lib.length (lib.unique ids) == lib.length ids
) "assets/camera-effects.nix: an id is declared twice";

writeTextFile {
  name = "ronoctua-camera-effects-rs";
  destination = "/effects.rs";
  text = ''
    // Generated from `assets/camera-effects.nix`. Do not edit.
    //
    // What the build refuses is in `nix/camera-effects.nix`.

    /// Every effect, in the order of the ring.
    pub const DECLARED: &[Declared] = &[
        ${lib.concatMapStringsSep "\n    " rsEffect declared}
    ];

    /// Every parameter name any effect takes, once — the text keys
    /// `camera-param-<name>` both languages owe.
    pub const PARAM_NAMES: &[&str] = &[${lib.concatMapStringsSep ", " (n: ''"${n}"'') paramNames}];
  '';
}
