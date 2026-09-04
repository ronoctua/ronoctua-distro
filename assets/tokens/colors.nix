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
    # THE FOCUSED ROW of an expanded list (4.3) — what `Enter` is going to run.
    # A change of colour, not an outline and not a box (invariant 15).
    #
    # It said this from the first day and, for the file manager, it was not
    # true: both doors spent it on the row under the POINTER and left the
    # keyboard's row with nothing but the name in `state.accent`. The owner
    # found it from the screen — *"ao trocar de item com as setas nenhuma
    # alteração/indicador visual é feita"* — and in the action menu, whose rows
    # carry no accent at all, the cursor was invisible outright.
    #
    # STRONGER THAN IT WAS, and measured on screen against `hover` below rather
    # than picked: at `0x2A2A2A` it was byte for byte the value of
    # `rule.default`, a band as dark as the hairline drawn beside it.
    #
    # AND THEN IT LEFT THE GREYS ALTOGETHER, because there is no seat for it
    # there. The owner reported the band and the scrollbar as "the same colour,
    # or nearly", and the measurement agrees to an embarrassing degree: at
    # `0x383838` this sat CIEDE2000 1.63 from `rule.strong`, which is the
    # thumb — below the 2.3 that two colours have to differ by before anybody
    # can tell them apart at all. Aligned on the same row they were one shape.
    #
    # A darker grey does not fix it. The scale is FULL between L*5.5 and L*25.8
    # — chrome, hover, rule.default, modeline.background, this, rule.strong —
    # and a search of every neutral in that band came back with nothing further
    # than 4.8 from the thumb and 1.3 from `rule.default`. Six greys is what
    # this palette has room for; a seventh cannot be told from its neighbours.
    #
    # So the band takes the hue that ALREADY means "the keyboard is here": the
    # focused row's name is drawn in `state.accent`, and this is that family as
    # a GROUND. L*16.4 C25.2 h300. Measured against everything it can appear
    # beside: thumb 18.52 (was 1.63), hover 17.45, chrome 18.66, rule.default
    # 17.33, brain.background 11.96 — and that last one is the closest thing on
    # the whole screen to it.
    #
    # It is also darker, which is the second half of the same report: the
    # `modified` column is `text.dim`, and `text.dim` on `0x383838` measured
    # 2.06:1. On this it is not drawn at all — see `View::row_of`, where a
    # banded row lifts its dim fields to `text.secondary`, 5.32:1 here.
    selection = "0x282448FF";
    # THE ROW UNDER THE POINTER, which is a different question with a different
    # answer — "this is what the click would reach", not "this is chosen".
    #
    # Weaker than the focus on purpose, and that is the whole reason it is a
    # second token: with one colour for both, a pointer resting away from the
    # cursor left two rows equally lit and the question "which one is selected?"
    # unanswered. The owner chose the split: *"foco fica com o fundo; hover
    # ganha um tom mais fraco"*.
    hover = "0x222222FF";

    # THE ONE SURFACE WITH AN ALPHA, and it is the film's controls.
    #
    # The file manager's transport is painted ON the picture — the owner asked
    # for the video to keep the whole height of the preview — so its glyphs have
    # to read against whatever frame is underneath. A bright sky and a night
    # scene are both ordinary, and text alone loses one of them.
    #
    # This is NOT elevation and does not make a card (invariant 15): there is no
    # border, no radius and no shadow. It is a band the width of the panel, one
    # line tall, that darkens what it covers — the same kind of thing as the
    # modeline's background, with the picture showing through.
    #
    # `CC` is four fifths. Below about two thirds the glyphs start to fight a
    # white frame; at full opacity the band stops reading as part of the picture
    # and becomes a strip of chrome across it, which is what invariant 14 is
    # about. Four fifths keeps the film visible through it and the glyphs
    # legible over it.
    transport = "0x121212CC";

    # THE MATERIAL A SHAPE IS DRAWN IN — `.stl` and `.3mf`.
    #
    # One colour and not a palette: the file carries no colours (neither format
    # has any here), so any variety would be invented, and a preview that
    # invents is a preview that lies about the piece. What shows the form is the
    # shading, which is why this is near-white rather than mid-grey — the darker
    # the material, the less range the light has left to work in, and a piece
    # whose far faces are already black has no silhouette between them.
    #
    # Warm rather than neutral, by a hair, because the panel's own surface is
    # cold and a shape that shares its temperature reads as part of the
    # background rather than as an object sitting in it.
    #
    # There is NO background token beside it: what is behind the shape is
    # transparent, so each door's own surface shows through — the same argument
    # the film's letterbox already had to make, and for the same reason. One
    # texture, two doors.
    mesh = "0xD8D4CEFF";

    # THE WAVEFORM DRAWN FROM A SOUND, and one colour for the same reason the
    # mesh has one: a sound carries no colours, so any variety would be
    # invented.
    #
    # It is drawn by ffmpeg, not by us — the only token in the system that
    # leaves the process — and it has to read over two different grounds: an
    # album's cover, which can be anything, and the panel's own surface showing
    # through where there is no cover. So it is near-white with the accent's
    # temperature: bright enough to survive a pale sleeve, and not the flat
    # white that would read as a scratch on a dark one.
    #
    # There is no background token beside it, and no second colour for the
    # strip it sits in: what is behind is transparent, and the transport's own
    # band already handles the one place ink has to be guaranteed legible.
    wave = "0xBFD3E6E6";
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
    # THE DISCREET REGISTER: interface text that is ABOUT the screen rather
    # than being the screen's content, in two weights.
    #
    # It is what the whole of the hint line and the whole of the minibuffer's
    # line are drawn in — a message, what the pointer is resting on, what is
    # being typed into `M-x`, the page counter, and the hint's own two halves.
    # Everything on those two lines except the pending chord, which stays
    # `state.accent` because blue is what says "this is where the next key
    # goes".
    #
    # `strong` is what is being SAID or pressed: a message, a query being
    # typed, a key notation. `default` is the prose around it and the counter.
    # They are never on the line at once except inside a hint, which is exactly
    # where the distinction is wanted.
    #
    # WHY THEY ARE NOT ON THE WARM RAMP, which is the whole point.
    #
    # The first attempt put both between `secondary` and `dim`, and the owner
    # rejected it from the screen: "the colours are practically identical, both
    # to each other and to colours that already exist". The arithmetic says why.
    # In CIEDE2000 the ramp's own steps are 17.6 (primary to secondary) and
    # 20.1 (secondary to dim); the pair squeezed between them came out 6.5
    # apart, with `strong` sitting 4.3 from `text.secondary` — a third of a
    # step, and a near-miss of an existing tone, which reads as a mistake
    # rather than as a decision. The gap is one step wide, and two more stops
    # inside one step cannot be a step each.
    #
    # So the pair leaves the ramp and takes a hue of its own: 320, chroma 10,
    # which is the warm ramp's own chroma (10-12). Against warm content, a
    # different temperature says "this line is not content" before any
    # luminance is compared, which is the thing that was asked for. The hue was
    # first 272 — the `brain` family's — and the owner asked for "a little
    # warmer"; walking it round to 320 both warmed it and moved it AWAY from
    # everything, because 272 is where `brain.dim` already lives. Its nearest
    # neighbours went from 6.3 and 9.0 to 12.1 and 12.3.
    #
    # The numbers, and they are measurements: the two are 17.7 apart, the size
    # of a step on the ramp; the nearest of all forty-eight other tokens is
    # 12.1 (`brain.dim`) and against the warm ramp itself it is 17.6. On the
    # footer's ground they read 3.4:1 and 6.3:1 — the prose is no darker than
    # `text.dim`, which the date column already uses.
    #
    # NESTED, and not `subtleStrong`: the generator lowercases a key before it
    # splits it, so that spelling would reach the WIT as `text-subtlestrong`.
    # It is the same reason `modeline.state` is nested, argued where that one
    # lives.
    #
    # AND THEN TWO MORE, because the line grew two more things to say.
    #
    # `faint` is the SEPARATOR — the `·` between one shortcut and the next. It
    # is a third register and not a decoration: the owner asked for the dot to
    # differ from both the key and the prose, and punctuation that is the same
    # tone as the words it separates is read as a word. A third step of this
    # same ramp, one below the prose: 11.9 from `default`, 30.7 from `strong`.
    # The step is shorter than the 17.7 above it on purpose — a separator only
    # has to be TOLD APART, it does not carry a register of meaning, and a dot
    # as loud as the prose competes with what it separates.
    #
    # `alternate` is a key whose MEANING changes with the section that has the
    # keyboard — `S-<arrow>` in the file manager turns a shape in the preview
    # and opens a directory in the listing. It is a key, so it is drawn at the
    # key's weight (L*59.5 against 62.1) and at the key's discretion (chroma
    # 13.1 against 9.8); it is a DIFFERENT KIND of key, so it takes a different
    # hue: 183 against 321. That is the whole allocation, and it is the one
    # thing this palette has been consistent about — hue says "another kind of
    # thing", luminance says "another importance". Measured: 24.4 from
    # `strong`, 28.8 from `default`, and its nearest neighbour among all fifty
    # other tokens is 17.8. Teal because it is the seat nobody had taken: the
    # content ramp is warm (h 89), the accent and the brain are blue-violet
    # (272-281), the group and this register are purple (306-321).
    subtle = {
      strong = "0x9F92A1FF";
      default = "0x706473FF";
      faint = "0x564250FF";
      alternate = "0x749690FF";
    };
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
  # SYNTAX — the CONTENT's palette, and it is not the chrome's
  # -------------------------------------------------------------------
  #
  # WHY THIS FAMILY MAY SPEND HUES THE CHROME RESERVES
  #
  # Because it is CONTENT. The precedent is written down and was paid for: the
  # emoji face is the one thing on screen with colour outside the tokens, and
  # the reason is that emoji are content — like the clipboard's image preview —
  # and not chrome. A code preview is the same kind of thing as an image
  # preview: it is what one came to look at, not the frame around it.
  #
  # And the blue's reservation survives whole. It is reserved so that exactly
  # one thing on a screen says "the keyboard is here": the window's border, and
  # the buffer's name when the footer takes it. A function's name in blue
  # INSIDE a preview competes with neither — it is not on a border and it is
  # not a buffer's name. What the rule forbids is a second blue piece of
  # CHROME, and there is none here.
  #
  # THE VALUES ARE TOKYO NIGHT'S OWN, which is where this palette's greens,
  # yellows and purples already came from. New hues would give a preview that
  # does not look like the system around it.
  #
  # TWELVE TOKENS FOR FOURTEEN ROLES, and that is not thrift.
  # `ronoctua_design::Palette` already maps seven roles onto six tokens, with
  # the reason: two roles that share a token today stay two roles, so the day
  # the theme parts them the change is one line in one table and not a hunt
  # through every call site. `Constant` shares `number`; `Heading` shares
  # `attribute`.
  syntax = {
    # THE ONE THAT RECEDES, and the only cool grey in this file outside
    # `brain`. A comment has to be findable and ignorable at once; at
    # `text.dim` it would sit in the same register as a disabled row.
    comment = "0x565F89FF";

    string = "0x9ECE6AFF";
    # The one thing inside a string that is NOT string: `\n`, `&`, `${HOME}`.
    # Bright, because it is the thing that stops being literal.
    escape = "0x89DDFFFF";
    # `Number` AND `Constant`. A literal `null` and a literal `42` are the same
    # kind of thing today, and the two roles stay two.
    number = "0xFF9E64FF";

    keyword = "0xBB9AF7FF";
    type = "0x2AC3DEFF";
    # See the note above about the reservation: this one is content.
    function = "0x7AA2F7FF";

    # `Attribute` AND `Heading`. Both are a marker ABOUT a block rather than
    # the block: `#[derive]` over a struct, `## Section` over a paragraph,
    # `@@ -1,4 +1,6 @@` over a hunk. In a diff this is the line between the
    # green and the red, which is how a patch is read.
    attribute = "0xE0AF68FF";

    # THE KEY SIDE of a key/value pair, and the highest-value colour in this
    # family: a file manager previews config more often than code, and without
    # it JSON is one colour of strings and a unit file is a grey wall.
    property = "0x7DCFFFFF";

    tag = "0xF7768EFF";

    # THE TWO WHOSE COLOUR IS NOT A CHOICE. The same green as `string` and the
    # same red as `tag`, and they are separate tokens for the reason
    # `notification.read`, `.suppressed` and `.countdown` are three tokens on
    # one value: a diff line is not a string, and the day the theme wants a
    # diff green that is not the string green, these are the lines to change.
    #
    # They cannot be confused on screen either: a diff line takes its role from
    # its first character and carries no highlighting inside it, so nothing
    # else in this family ever appears in the same file as these two.
    added = "0x9ECE6AFF";
    removed = "0xF7768EFF";
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
