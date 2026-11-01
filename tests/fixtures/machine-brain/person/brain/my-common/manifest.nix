# A person's module of the machine fixture `tests/fixtures/machine-brain/`:
# the smallest folder the brain's loader takes. No NixOS half: compiled
# without an approval, and its helper never reached.
# (`person-root-module-needs-approval`,
# `machine-compiles-approved-person-modules`).
{
  what = "A person's fixture with no NixOS half, and code beside it no approval covers.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
