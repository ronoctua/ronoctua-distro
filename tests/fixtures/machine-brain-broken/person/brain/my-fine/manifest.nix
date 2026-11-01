# A person's module of `tests/fixtures/machine-brain-broken/` that evaluates
# alone and loads: the one the machine compiles
# (`a-folder-that-does-not-evaluate-stops-only-itself`).
{
  what = "A fixture that evaluates alone and loads.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
