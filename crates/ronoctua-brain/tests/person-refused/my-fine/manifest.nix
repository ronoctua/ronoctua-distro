# A PERSON'S FOLDERS, SOME OF THEM BROKEN — the fixture
# `brain-person-modules-load` hands the loader to prove that one bad folder
# is left out and said, never the machine's whole evaluation stopped.
#
# This one is FINE: the smallest folder a person's module may be. Its
# neighbours are refused each for one reason — `my-fine-too` for a text
# prefix that overlaps this one's, `my-2fa` for an id outside the grammar,
# `stray` for an id that is not the person's — and `my-unreadable` is an
# error no evaluation catches, which only `personModulesOnly` keeps unread.
{
  what = "A person's fixture that loads beside folders that do not.";
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
      hint = "my-fine-mark-seat";
    };
  };
}
