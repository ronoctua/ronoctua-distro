# A PERSON'S SAMPLE MODULE — the fixture `brain-person-modules-load` hands
# the loader as `personModulesDir`, the way a machine hands it the person's
# own folder (`$XDG_CONFIG_HOME/ronoctua/brain-modules/`).
#
# It lives under `tests/person/`, which no loader reads unless told: the
# distro never compiles it. It is the smallest folder a person's module may
# be — `manifest.nix`, `module.rs` and its texts — plus the NixOS half the
# machine imports only once approved, and one mark with instances, so the
# generator writes `MarkId::declared` for it.
{
  what = "A person's sample: one mark per seat that is taken.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
  glyphs = {
    seat = "md-cube_outline";
  };
  marks = {
    seat = {
      glyph = "seat";
      hint = "my-sample-mark-seat";
      instances = 3;
    };
  };
}
