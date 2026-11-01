# A person's module of `tests/fixtures/machine-brain-broken/` that would
# load, and that root never evaluated alone at what it is now: the machine
# leaves it out, unchecked, until an apply or an approval checks it
# (`a-folder-that-does-not-evaluate-stops-only-itself`).
{
  what = "A fixture nobody evaluated alone yet.";
  needs = {
    root = false;
    network = false;
    boundaries = [ ];
  };
  schedule = null;
  uses = [ ];
  namedBy = { };
}
