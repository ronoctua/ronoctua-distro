# A person's module of the machine fixture `tests/fixtures/machine-brain/`:
# the smallest folder the brain's loader takes. Approved: compiled into the
# brain, and its NixOS half imported. (`person-root-module-needs-approval`,
# `machine-compiles-approved-person-modules`).
{
  what = "A person's fixture with a NixOS half an approval names as it is.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
