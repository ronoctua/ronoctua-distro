# A PERSON'S SAMPLE MODULE — the fixture `brain-person-modules-load` hands
# the loader as `personModulesDir`, the way a machine hands it the person's
# own folder (`$XDG_CONFIG_HOME/ronoctua/brain-modules/`).
#
# It lives under `tests/person/`, which no loader reads unless told: the
# distro never compiles it. It is the smallest folder a person's module may
# be — `manifest.nix`, `module.rs` and its texts — plus the NixOS half the
# machine imports only once approved, one mark with instances, so the
# generator writes `MarkId::declared` for it, and a `shared.rs` beside its
# `module.rs`: in a person's folder a file like any other, never a shared half
# of the library's. And one action and one event with a notification (its
# icon its own glyph's token), so the brain's catalogue carries a person's
# action, event and notification to the footer — never asked of the fixture
# at run time.
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
  actions = {
    free = { };
  };
  events = {
    taken = {
      notification = {
        show = "brief";
        life = "quick";
        level = 20;
        icon = "my-sample-seat";
        tags = [ "seats" ];
      };
    };
  };
}
