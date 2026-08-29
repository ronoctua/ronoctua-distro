# Colour tokens — section 5.1.
#
# INVARIANT 15: no cards, no shadow, no elevation. **There is no shadow token
# in this file, and none must come to exist.** If someone needs one,
# the design was violated earlier.
#
# Separation is by WHITESPACE and a ONE-LINE RULE, never by a
# container. Emphasis is by WEIGHT AND COLOUR, never by a box.
#
# Format: 0xRRGGBBAA.
{
  # -------------------------------------------------------------------
  # Surfaces
  #
  # There is no "elevation": `base` is the content's background, `chrome` is the
  # footer's. They are two colours, not a stack.
  # -------------------------------------------------------------------
  surface = {
    # PURE BLACK. On an OLED screen black is a pixel switched off:
    # maximum contrast and less battery, which is section 2's target
    # (laptops). And the blue-grey from before only existed out of editor-theme habit.
    base = "0x000000FF"; # the system's background
    # Dark grey for whatever IS a surface: the footer and the buffers. The
    # separation between content and chrome becomes the difference between
    # black and grey, with no rule and no box (invariant 15).
    chrome = "0x121212FF"; # the two-line footer
    # The buffers' background. Distinct from `base` so the window stands out from
    # the void with no border at all.
    buffer = "0x161616FF";
    # The background of the focused row inside an expanded list (4.3). It is a
    # change of colour, not an outline and not a box.
    selection = "0x2A2A2AFF";
  };

  # -------------------------------------------------------------------
  # Text — the hierarchy is by colour and weight, only
  # -------------------------------------------------------------------
  # THE BODY TEXT IS SLIGHTLY WARM, and not blue-white.
  #
  # The owner's decision, and the reason is the same as pure black: a screen one looks at
  # all day. Cold white (`0xD8DEE9`, from an editor theme) pulls towards
  # blue, and blue is precisely the colour the system reserves for the ACCENT —
  # with the body text close to it, the accent loses force.
  #
  # The whole family was warmed together: `primary`, `secondary` and `dim` are
  # the SAME hue at three luminances. Warming only one of them would give a greyscale
  # ramp that changes temperature halfway, and the eye reads that as dirt.
  text = {
    primary = "0xE9DEC8FF";
    secondary = "0xA59B87FF";
    # Lower-priority modeline segments, before they vanish in the
    # elision (section 7 of the plan).
    dim = "0x6E6656FF";
    inverse = "0x000000FF";
    # The label of a SUBMENU in the shortcut list — which-key's `+window`.
    #
    # It needs a tone of its own: a submenu does not DO anything, it leads to another
    # level, and with no distinction the list mixes "this happens" with "this opens
    # more keys". The `+` in front says the same thing in text; the colour says it
    # without taking up a column. Purple for being the only hue in the palette not yet
    # spent: blue is the accent, green/yellow/red are state.
    group = "0xBB9AF7FF";
  };

  # -------------------------------------------------------------------
  # Rules — the ONLY form of separation besides whitespace
  # -------------------------------------------------------------------
  rule = {
    # One line, one logical pixel. It separates the content from the footer.
    default = "0x2A2A2AFF";
    strong = "0x3D3D3DFF";
  };

  # -------------------------------------------------------------------
  # Window border — drawn by the COMPOSITOR, with these colours
  # -------------------------------------------------------------------
  #
  # The active one is `state.accent`: a single accent colour, and it marks focus.
  #
  # The INACTIVE one is dark red at the owner's request, and the reason is the moment
  # the footer takes the keyboard: there no window is focused as far as the
  # compositor is concerned, and the blue disappears. With the inactive one in grey, the outline looked
  # SWITCHED OFF; with it here, it CHANGES COLOUR — which is what was asked for.
  #
  # Dark enough not to shout "error": it is `state.error`'s red
  # dimmed until it becomes an outline, and not a warning.
  border = {
    inactive = "0x5C2E33FF";
  };

  # -------------------------------------------------------------------
  # State
  # -------------------------------------------------------------------
  state = {
    accent = "0x7AA2F7FF";
    success = "0x9ECE6AFF";
    warning = "0xE0AF68FF";
    error = "0xF7768EFF";
    # A notification's critical urgency (4.2): it does not disappear on its own, and the colour
    # has to say so before the text is read.
    critical = "0xFF5370FF";
  };

  # -------------------------------------------------------------------
  # THE NOTIFICATION BAND — the line (or three) ABOVE the modeline
  # -------------------------------------------------------------------
  #
  # It is not a toast and not a card: it is the footer's own surface growing
  # UPWARDS, exactly as the expansion already grows downwards. One surface,
  # invariants 14 and 15 intact. What separates it from the modeline is the
  # background, which is the same device that already separates the modeline
  # from the minibuffer — a difference in colour, no rule and no box.
  #
  # The owner's decision, and it replaced two earlier readings of the ask: a
  # floating toast (forbidden) and the modeline itself (which would push the
  # buffer's name off the one line that says where you are).
  notification = {
    # ONE TONE ABOVE the modeline's, which is itself one above the chrome. The
    # three together are a ramp the eye reads as "content, footer, and the
    # thing that just arrived", from bottom to top.
    background = "0x242424FF";

    # THE TWO FORMATTINGS, and the difference between them is the whole point
    # of having two: a copy that finished and a disk that is full must not
    # look alike.
    #
    # `quiet` is the same warm grey as `text.secondary`: present, readable,
    # and it does not pull the eye off what is being read.
    quiet = "0xA59B87FF";
    # `loud` is the WARM alert, and deliberately NOT the accent. Blue marks
    # focus and only focus (see the border block above); a shouting
    # notification in blue would be a second blue thing on screen, and then
    # neither of them says "the keyboard goes here".
    loud = "0xE0AF68FF";

    # HOW MUCH TIME IS LEFT, drawn as an eighths pie in one cell. It has to be
    # findable and forgettable at once, so it takes the quietest tone in the
    # family — the same as `text.dim`.
    countdown = "0x6E6656FF";

    # ALREADY READ, in the manager's left column. It stays in the list — the
    # list is the record of what happened — and it recedes.
    read = "0x6E6656FF";
    # SUPPRESSED BY A FILTER, in the manager's right column. The same tone as
    # `read` on purpose: both are "here, and not asking for anything". What
    # tells them apart is which column they are in, and the column has a
    # heading.
    suppressed = "0x6E6656FF";
  };

  # -------------------------------------------------------------------
  # THE BRAIN'S LINE — below the minibuffer, above the expansion
  # -------------------------------------------------------------------
  #
  # It is the footer's own surface growing DOWNWARDS, the mirror of the
  # notification band growing upwards. One surface, no popup, no card
  # (invariants 14 and 15).
  #
  # NOT another rung of the band's ramp. `surface.base` (0x000000) →
  # `surface.chrome` (0x121212) → `modeline.background` (0x1E1E1E) →
  # `notification.background` (0x242424) is a ladder the eye climbs upwards,
  # and every rung means "one more layer of chrome". This one is a step
  # SIDEWAYS: the same luminance neighbourhood as the modeline, with a blue
  # cast, because it is not one more thing above — it is a different KIND of
  # line. It is the only background in this file that carries a hue, and that
  # is the whole point: the machine is about to do something to itself, and
  # the line saying so must not read as a modeline that changed its mind.
  #
  # The owner asked for it in those words: a dark, slightly blue-ish grey,
  # with white letters.
  brain = {
    background = "0x161C28FF";

    # THE ONE NEAR-WHITE IN THIS FILE, and it is confined to this line.
    #
    # The body text is warm on purpose — see the `text` block, where the whole
    # family was warmed together so a greyscale ramp would not change
    # temperature halfway. Over a blue-cast background a warm white reads as
    # dirty, so this family is neutral instead. Two temperatures on one screen
    # is exactly what that block argues against, which is why this one may not
    # leave this line.
    text = "0xF4F6FAFF";

    # The bracketed keys, and the quieter half of a button's label. The same
    # hue at a lower luminance — the `text.dim` relation, inside this family.
    dim = "0x93A0B8FF";

    # THE PIE, and it is brighter than `dim` on purpose: it is the only thing
    # on the line that MOVES, and it is what a person looks at to decide
    # whether to reach for the keyboard.
    countdown = "0xC3CEE2FF";
  };

  # -------------------------------------------------------------------
  # Modeline
  # -------------------------------------------------------------------
  modeline = {
    # ONE TONE ABOVE `surface.chrome`. The footer's two lines play
    # different roles — one says where you are, the other is where you
    # type — and the difference in background is what separates them with no rule and no
    # box (invariant 15).
    background = "0x1E1E1EFF";
    # Badge counting pending notifications (4.2).
    badge = "0xF7768EFF";
    badgeText = "0x000000FF";

    # -----------------------------------------------------------------
    # The buffer's STATE — two tokens, and the second is one on purpose
    # -----------------------------------------------------------------
    #
    # A buffer declares which state it is in; the modeline draws the name. There
    # are exactly two colours here and not one per state, because a state that
    # is not the base one is drawn the SAME regardless of which buffer it
    # belongs to: the reader learns one colour, and a theme changes one line.
    #
    # A buffer MAY override this per state, by naming a role in its descriptor
    # (`tone`) — for the day a file manager waiting on a permission answer needs
    # to say something stronger than "not the usual one". None does today.
    #
    # THE ACCENT IS NOT USED HERE, and that is deliberate. Blue marks focus and
    # nothing else: it lives on the window's border and moves to the buffer's
    # name when the footer takes the keyboard. A state in blue would be a second
    # blue thing on screen, and then neither of them says "this is where the
    # keyboard goes".
    #
    # NESTED, and not `stateNormal`: a flattened `modeline.stateNormal` comes out
    # of the generator as `modeline-statenormal`, because the conversion to a WIT
    # identifier lowercases before it splits. Two levels give
    # `modeline-state-normal`, which is the name a person would have guessed.
    state = {
      #
      # THE RAMP, from the top: the buffer's NAME is `text.primary`, the mode is
      # `text.secondary`, and the base state is the quietest of the three —
      # `normal` is the one that says "nothing in particular", so it recedes.
      # Anything that is NOT normal jumps straight past all of them into green.
      normal = "0x6E6656FF"; # the same as `text.dim`
      alternate = "0x9ECE6AFF"; # the same green as `state.success`
    };
  };
}
