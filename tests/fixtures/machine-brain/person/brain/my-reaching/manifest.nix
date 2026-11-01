# A person's module of the machine fixture `tests/fixtures/machine-brain/`:
# the smallest folder the brain's loader takes. Approved: compiled, and its
# NixOS half imported from its own folder alone.
# (`person-root-module-needs-approval`,
# `machine-compiles-approved-person-modules`).
{
  what = "A person's fixture, approved, whose NixOS half reaches for a sibling.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
