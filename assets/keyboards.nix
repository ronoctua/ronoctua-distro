# WHAT A KEY CAN BE MADE INTO — one place, and generated from here.
#
# `keyd` maps a key to an action, and its vocabulary is a small language:
# `overload(control, esc)` is "control when held, escape when tapped". Written
# free-form, a person editing a keyboard on screen could type anything, and
# anything keyd does not understand takes the DAEMON down — which on this
# system takes the leader with it (`nix/modules/keyboard.nix`).
#
# So the screen offers a RING and never a field: these keys, those meanings,
# and nothing else. The same decision the visibility timeout took, and for a
# harder reason — the cost of a typo there is a number nobody wanted, and here
# it is a keyboard that stops answering.
#
# # THE THREE FIELDS
#
#   key      what keyd calls it. Never drawn: it is the daemon's word.
#   text     the key BOTH languages owe. It is what the screen says.
#   holds    true when the meaning is "this when held, that when tapped".
#            The screen says so, because a key that behaves differently
#            depending on how long you press it is worth a warning.
#
# The technical word still reaches the person — under the cursor, in the
# discreet second line the format buffer already uses. A name nobody can search
# for is a name nobody can fix.
{
  # THE LEADER'S OWN RULE, and it lives here because TWO things need it.
  #
  # `nix/modules/keyboard.nix` writes it into the daemon's config, and the
  # shell writes it into EVERY per-keyboard file it renders — because keyd
  # takes a device id in one file only, so a keyboard with a remap of its own
  # stops being covered by the catch-all. A rule copied into the second place
  # is a rule that drifts, and what drifts here is the key that opens
  # everything.
  #
  # `f13` and not a real key: F13 exists on no keyboard, so nothing else can
  # emit it by accident. See the module for the rest of the reasoning.
  leader = {
    key = "leftmeta";
    action = "overload(meta, f13)";
  };

  # WHICH KEYS ARE OFFERED. Not every key: these are the ones people actually
  # move, and a list of a hundred and five is a list nobody reads. The two
  # modifiers by the space bar, the one under the little finger, and the three
  # that most keyboards waste.
  keys = [
    {
      key = "capslock";
      text = "keyboard-key-capslock";
    }
    {
      key = "leftalt";
      text = "keyboard-key-leftalt";
    }
    {
      key = "leftmeta";
      text = "keyboard-key-leftmeta";
    }
    {
      key = "rightalt";
      text = "keyboard-key-rightalt";
    }
    {
      key = "rightmeta";
      text = "keyboard-key-rightmeta";
    }
    {
      key = "menu";
      text = "keyboard-key-menu";
    }
    {
      key = "insert";
      text = "keyboard-key-insert";
    }
  ];

  # AND WHAT THEY CAN BECOME. The first is the one that means "leave it
  # alone", and it is first because it is the way back: somebody who has made
  # a mess of a keyboard reaches it by pressing left until the list stops.
  choices = [
    {
      key = "";
      text = "keyboard-choice-default";
    }
    {
      key = "esc";
      text = "keyboard-choice-esc";
    }
    {
      key = "control";
      text = "keyboard-choice-control";
    }
    {
      key = "overload(control, esc)";
      text = "keyboard-choice-control-esc";
      holds = true;
    }
    {
      key = "meta";
      text = "keyboard-choice-meta";
    }
    {
      key = "alt";
      text = "keyboard-choice-alt";
    }
    {
      key = "backspace";
      text = "keyboard-choice-backspace";
    }
    {
      key = "capslock";
      text = "keyboard-choice-capslock";
    }
    {
      key = "noop";
      text = "keyboard-choice-noop";
    }
  ];
}
