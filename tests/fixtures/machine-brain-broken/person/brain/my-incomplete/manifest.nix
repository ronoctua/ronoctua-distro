# A person's module of `tests/fixtures/machine-brain-broken/` that evaluates
# alone — a manifest of plain data — and that the brain's loader refuses:
# it has no `module.rs`. The loader leaves it out with its sentence, an error
# it catches (`a-folder-that-does-not-evaluate-stops-only-itself`).
{
  what = "A fixture that evaluates alone, and that the loader refuses.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
