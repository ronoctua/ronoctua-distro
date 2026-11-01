# A person's module of the machine fixture `tests/fixtures/machine-brain/`:
# the smallest folder the brain's loader takes. No NixOS half: compiled
# without an approval. (`person-root-module-needs-approval`,
# `machine-compiles-approved-person-modules`).
{
  what = "A person's fixture with no NixOS half.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
