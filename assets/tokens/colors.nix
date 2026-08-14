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
    # The label of a SUBMENU in the shortcut list — which-key's `+janela`.
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
  };
}
