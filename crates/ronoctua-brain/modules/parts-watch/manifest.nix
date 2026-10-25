# THE MACHINE'S PARTS, while any is on its way: a mark on the modeline, and a
# word when one arrives or needs the person.
#
# A watch and nothing else: the parts' realizer (root, `ronoctua-parts`)
# writes one record per part and renames it into place, so the kernel tells
# the brain (inotify) and reading the records is a `readdir` — nothing runs,
# nothing at rest. Why each is so is in `why.md`, beside this.
{
  what = "Mark the modeline while any of the machine's parts is on its way or needs you, and say when one arrives or needs you.";

  needs = {
    root = false;
    network = false;
    # WHERE THE RECORDS ARE, and what one says, come from the one crate that
    # knows the parts; the module reads a folder and runs nothing.
    boundaries = [ "ronoctua-parts" ];
  };

  # A WATCH: nothing runs, so nothing is scheduled.
  schedule = null;

  glyphs = {
    arriving = "md-download";
    needs-you = "md-download_off";
  };

  # ONE MARK AT A TIME, and only while it says something (invariant 14): a
  # part that needs the person, in the colour of a warning; else one on its
  # way, in the colour of a watch; each with how many. A click opens the
  # parts' page (`distro-parts-open`, `LEADER s u d`).
  marks = {
    arriving = {
      glyph = "arriving";
      hint = "parts-watch-mark-arriving";
    };
    needs-you = {
      glyph = "needs-you";
      hint = "parts-watch-mark-needs-you";
    };
  };

  # THE TWO MOMENTS worth a word on the minibuffer: a part arrived (it is
  # ready to use), and a part needs the person (it failed five times with a
  # network, and the page has the ways in). Neither is said at the first
  # reading, which is no news (AGENTS.md, "Um evento do sistema").
  events = {
    arrived = {
      notification = {
        show = "full";
        life = "quick";
        loud = false;
        level = 30;
        icon = "md-download";
        tags = [ "system" ];
        actions = [ ];
      };
    };
    needs-you = {
      notification = {
        show = "full";
        life = "long";
        loud = false;
        level = 60;
        icon = "md-download_off";
        tags = [ "system" ];
        actions = [ ];
      };
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the parts' crate and their module.
  uses = [
    "crates/ronoctua-parts"
    "nix/modules/parts.nix"
  ];

  namedBy = { };
}
