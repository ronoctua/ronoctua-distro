# Colour tokens — section 5.1. RECIPES, not values.
#
# INVARIANT 15: no cards, no shadow, no elevation. **There is no shadow token
# in this file, and none must come to exist.** If someone needs one, the
# design was violated earlier. Separation is by WHITESPACE and a ONE-LINE
# RULE, never by a container. Emphasis is by WEIGHT AND COLOUR, never by a box.
#
# # What a line in this file says
#
# It names a PRIMITIVE of a FAMILY: `text.primary = "neutral.text-strong"`.
# The primitive is solved per theme by `nix/scripts/gen-theme.py` from the
# theme's seeds (`assets/themes/<id>.nix`) through one ladder that is the same
# for every theme — a hue and a relative chroma in, a lightness solved to a
# contrast or a separation out. A recipe may also carry an `alpha` (the one
# translucent band) or scale the step's chroma (`{ of = …; chroma = 0.3; }`).
# No hex lives here, and `colours-are-computed` refuses one.
#
# Every relationship this file used to promise in prose — "measured 17
# CIEDE2000 from the thumb", "6.3:1 on the footer's ground" — is now a GATE in
# the generator, measured on every theme at every build, and the numbers are
# in `report.md` of `nix build .#themes`. What stays below is the DECISION
# each token records: what it is for, why that family, why it is or is not
# shared. The numbers moved out because a number in a comment is a number
# nobody re-measures.
#
# The families: `neutral` (the greys, and the warm text ramp), `accent` (blue,
# and it means "the keyboard is here"), `success`, `warning`, `error`, `group`
# (purple), `subtle` (the discreet register), `alternate` (the key whose
# meaning changes with the section), `brain` (the blue-cast line), and the
# content hues the syntax palette spends — `slate`, `sky`, `azure`, `cyan`,
# `orange`. Which hue each is, per theme, is the seed file's business.
#
# The steps: `ground`, `element`, `border`, `border-strong` (paint, climbing
# from the footer's grey), `solid`, `solid-hover`, `vivid` (solved to a
# contrast on the footer), `text-faint`, `text-muted`, `text`, `text-loud`,
# `text-strong`, `text-bright` (text, the same), and `on-solid`. The neutral
# family has `ground`, `chrome`, `window`, `chrome-raised`, `hover`,
# `chrome-loud`, `border`, `border-strong` and the six text steps.
#
# NESTED, and never camelCase: the generator lowercases a key before it splits
# it, so `subtleStrong` would reach the WIT as `text-subtlestrong`. Two levels
# give `text-subtle-strong`, which is the name a person would have guessed.
{
  # -------------------------------------------------------------------
  # Surfaces
  #
  # There is no "elevation": `base` is the content's background, `chrome` is
  # the footer's. They are two colours, not a stack.
  # -------------------------------------------------------------------
  surface = {
    # PURE BLACK in owl (the seed's `ground` is 0): on an OLED screen black is
    # a pixel switched off — maximum contrast and less battery, which is
    # section 2's target. The blue-grey from before only existed out of
    # editor-theme habit.
    base = "neutral.ground";
    # Dark grey for whatever IS a surface: the footer and the buffers. The
    # separation between content and chrome is the difference between black
    # and grey, with no rule and no box (invariant 15).
    chrome = "neutral.chrome";
    # The buffers' background. Distinct from `base` so the window stands out
    # from the void with no border at all.
    buffer = "neutral.window";
    # What the compositor paints where no window is, when the person chose
    # the theme's colour over black (`LEADER s t`, second column). The one
    # neutral step below the chrome, in the hue the seed's `backdrop` names:
    # the desk is the single surface meant to read as the theme's colour, and
    # it is under everything, so the footer and a window both keep an edge
    # against it (gated). Black itself is not a token: it is the option's name.
    backdrop = "neutral.backdrop";
    # THE FOCUSED ROW of an expanded list (4.3) — what `Enter` is going to run.
    # A change of colour, not an outline and not a box (invariant 15).
    #
    # NOT A GREY. The owner reported the band and the scrollbar's thumb as
    # "the same colour, or nearly", and the measurement agreed: the grey
    # scale between the chrome and the strong rule is full, and a seventh grey
    # cannot be told from its neighbours. So the band takes the hue that
    # ALREADY means "the keyboard is here" — the focused row's name is drawn
    # in `state.accent`, and this is that family as a GROUND. The accent's
    # `torsion` in the seed is what turns the ground a few degrees from the
    # solid. The `distance` gates keep it apart from every grey it can appear
    # beside; measured: report.md, `surface.selection vs …`.
    #
    # It is also dark, which is the second half of the same report: the file
    # manager lifts a banded row's dim fields to `text.secondary`
    # (`lift_onto_a_band`); the notification manager and the update window
    # draw dim on the band as it is, and the `contrast` gate holds it above
    # the dim floor there.
    selection = "accent.element";
    # THE ROW UNDER THE POINTER, which is a different question with a different
    # answer — "this is what the click would reach", not "this is chosen".
    #
    # Weaker than the focus on purpose, and that is the whole reason it is a
    # second token: with one colour for both, a pointer resting away from the
    # cursor left two rows equally lit and the question "which one is
    # selected?" unanswered. The owner chose the split: *"foco fica com o
    # fundo; hover ganha um tom mais fraco"*.
    hover = "neutral.hover";

    # THE GROUND OF A SHELF — the fixed-height lists inside the network view.
    #
    # The owner asked for it: *"quero que coloque um background diferente e
    # discreto para esses containers"*. A shelf is a list inside a list, and
    # nothing on screen said where one ended — the air before a heading says
    # where a SECTION starts, and a shelf is not a section.
    #
    # THE SMALLEST STEP THERE IS, `neutral.window` at 0,018 ΔL above the
    # chrome. Discreet was the word, and it is also what invariant 15 leaves
    # room for: a change of ground, with no border, no radius and no shadow —
    # the same device the modeline and the notification band already use, one
    # rung lower. The hover row is at 0,070 and the focused one takes the
    # accent, so both still read on top of it.
    shelf = "neutral.window";

    # THE ONE SURFACE WITH AN ALPHA, and it is the film's controls.
    #
    # The file manager's transport is painted ON the picture — the owner asked
    # for the video to keep the whole height of the preview — so its glyphs
    # have to read against whatever frame is underneath. A bright sky and a
    # night scene are both ordinary, and text alone loses one of them.
    #
    # This is NOT elevation and does not make a card (invariant 15): there is
    # no border, no radius and no shadow. It is a band the width of the panel,
    # one line tall, that darkens what it covers — the same kind of thing as
    # the modeline's background, with the picture showing through.
    #
    # Four fifths. Below about two thirds the glyphs start to fight a white
    # frame; at full opacity the band stops reading as part of the picture and
    # becomes a strip of chrome across it, which is what invariant 14 is about.
    transport = {
      of = "neutral.chrome";
      alpha = 0.8;
    };

    # THE MATERIAL A SHAPE IS DRAWN IN — `.stl` and `.3mf`.
    #
    # One colour and not a palette: the file carries no colours (neither
    # format has any here), so any variety would be invented, and a preview
    # that invents is a preview that lies about the piece. What shows the form
    # is the shading, which is why this is near-white rather than mid-grey —
    # the darker the material, the less range the light has left to work in,
    # and a piece whose far faces are already black has no silhouette between
    # them.
    #
    # Warm rather than neutral, by a hair (a third of the text ramp's chroma),
    # because the panel's own surface is cold and a shape that shares its
    # temperature reads as part of the background rather than as an object
    # sitting in it.
    #
    # There is NO background token beside it: what is behind the shape is
    # transparent, so each door's own surface shows through — the same
    # argument the film's letterbox already had to make. One texture, two
    # doors.
    mesh = {
      of = "neutral.text-loud";
      chroma = 0.3;
    };

    # THE WAVEFORM DRAWN FROM A SOUND, and one colour for the same reason the
    # mesh has one: a sound carries no colours, so any variety would be
    # invented.
    #
    # It is drawn by ffmpeg, not by us — the only token in the system that
    # leaves the process — and it has to read over two different grounds: an
    # album's cover, which can be anything, and the panel's own surface
    # showing through where there is no cover. So it is near-white with the
    # accent's temperature: bright enough to survive a pale sleeve, and not
    # the flat white that would read as a scratch on a dark one.
    #
    # There is no background token beside it, and no second colour for the
    # strip it sits in: what is behind is transparent, and the transport's own
    # band already handles the one place ink has to be guaranteed legible.
    wave = {
      of = "accent.text-loud";
      alpha = 0.9;
    };
  };

  # -------------------------------------------------------------------
  # Text — the hierarchy is by colour and weight, only
  # -------------------------------------------------------------------
  # THE BODY TEXT IS SLIGHTLY WARM, and not blue-white.
  #
  # The owner's decision, and the reason is the same as pure black: a screen
  # one looks at all day. Cold white pulls towards blue, and blue is precisely
  # the colour the system reserves for the ACCENT — with the body text close
  # to it, the accent loses force.
  #
  # The whole family is warmed together: `primary`, `secondary` and `dim` are
  # the SAME hue at ONE relative chroma and three luminances (the seed's
  # `text`). Warming only one of them would give a greyscale ramp that
  # changes temperature halfway, and the eye reads that as dirt.
  text = {
    primary = "neutral.text-strong";
    secondary = "neutral.text";
    # Lower-priority modeline segments, before they vanish in the elision
    # (section 7 of the plan).
    dim = "neutral.text-muted";
    inverse = "neutral.ground";
    # The label of a SUBMENU in the shortcut list — which-key's `+window`.
    #
    # It needs a tone of its own: a submenu does not DO anything, it leads to
    # another level, and with no distinction the list mixes "this happens"
    # with "this opens more keys". The `+` in front says the same thing in
    # text; the colour says it without taking up a column. Purple for being
    # the only hue in the palette not yet spent: blue is the accent,
    # green/yellow/red are state. THE SAME PRIMITIVE as `syntax.keyword`, on
    # purpose: one purple, two names.
    group = "group.solid";
    # THE DISCREET REGISTER: interface text that is ABOUT the screen rather
    # than being the screen's content, in three weights and a variant.
    #
    # It is what the whole of the hint line and the whole of the minibuffer's
    # line are drawn in — a message, what the pointer is resting on, what is
    # being typed into `M-x`, the page counter, and the hint's own two halves.
    # Everything on those two lines except the pending chord, which stays
    # `state.accent` because blue is what says "this is where the next key
    # goes".
    #
    # `strong` is what is being SAID or pressed: a message, a query being
    # typed, a key notation. `default` is the prose around it and the
    # counter. They are never on the line at once except inside a hint, which
    # is exactly where the distinction is wanted.
    #
    # WHY THEY ARE NOT ON THE WARM RAMP, which is the whole point. The first
    # attempt put both between `secondary` and `dim`, and the owner rejected
    # it from the screen: "the colours are practically identical, both to
    # each other and to colours that already exist". The ramp's gap is one
    # step wide, and two more stops inside one step cannot be a step each. So
    # the pair leaves the ramp and takes a hue of its own (the `subtle`
    # family, at the ramp's own chroma): against warm content, a different
    # temperature says "this line is not content" before any luminance is
    # compared. The owner asked for it "a little warmer" than the brain's
    # hue, and walking it round both warmed it and moved it AWAY from
    # everything. The `distance` gates hold the cross-register separations
    # (subtle against the ramp, against the brain); measured: report.md.
    #
    # `faint` is the SEPARATOR — the `·` between one shortcut and the next. It
    # is a third register and not a decoration: the owner asked for the dot
    # to differ from both the key and the prose, and punctuation that is the
    # same tone as the words it separates is read as a word. A third step of
    # this same family, one below the prose, and a shorter step on purpose — a
    # separator only has to be TOLD APART, it does not carry a register of
    # meaning, and a dot as loud as the prose competes with what it separates.
    #
    # `alternate` is a key whose MEANING changes with the section that has the
    # keyboard — `S-<arrow>` in the file manager turns a shape in the preview
    # and opens a directory in the listing. It is a key, so it is drawn at the
    # key's weight; it is a DIFFERENT KIND of key, so it takes a different hue
    # (the `alternate` family, teal in owl: the seat nobody had taken). That
    # is the whole allocation, and it is the one thing this palette has been
    # consistent about — hue says "another kind of thing", luminance says
    # "another importance".
    subtle = {
      strong = "subtle.text";
      default = "subtle.text-muted";
      faint = "subtle.text-faint";
      alternate = "alternate.text";
    };
  };

  # -------------------------------------------------------------------
  # Rules — the ONLY form of separation besides whitespace
  # -------------------------------------------------------------------
  rule = {
    # One line, one logical pixel. It separates the content from the footer.
    default = "neutral.border";
    strong = "neutral.border-strong";
  };

  # -------------------------------------------------------------------
  # Window border — drawn by the COMPOSITOR, with these colours
  # -------------------------------------------------------------------
  #
  # The active one is `state.accent`: a single accent colour, and it marks
  # focus.
  #
  # The INACTIVE one is dark red at the owner's request, and the reason is the
  # moment the footer takes the keyboard: there no window is focused as far as
  # the compositor is concerned, and the blue disappears. With the inactive
  # one in grey, the outline looked SWITCHED OFF; with it here, it CHANGES
  # COLOUR — which is what was asked for.
  #
  # Dark enough not to shout "error": it is the `error` family as a BORDER —
  # its red dimmed until it becomes an outline, and not a warning.
  border = {
    inactive = "error.border";
  };

  # -------------------------------------------------------------------
  # State
  # -------------------------------------------------------------------
  state = {
    accent = "accent.solid";
    success = "success.solid";
    warning = "warning.solid";
    error = "error.solid";
    # A notification's critical urgency (4.2): it does not disappear on its
    # own, and the colour has to say so before the text is read. The error's
    # red at FULL chroma and a little darker — hotter, not another hue.
    critical = "error.vivid";
  };

  # -------------------------------------------------------------------
  # SYNTAX — the CONTENT's palette, and it is not the chrome's
  # -------------------------------------------------------------------
  #
  # WHY THIS FAMILY MAY SPEND HUES THE CHROME RESERVES
  #
  # Because it is CONTENT. The precedent is written down and was paid for: the
  # emoji face is the one thing on screen with colour outside the tokens, and
  # the reason is that emoji are content — like the clipboard's image preview
  # — and not chrome. A code preview is the same kind of thing as an image
  # preview: it is what one came to look at, not the frame around it.
  #
  # And the blue's reservation survives whole. It is reserved so that exactly
  # one thing on a screen says "the keyboard is here": the window's border,
  # and the buffer's name when the footer takes it. A function's name in blue
  # INSIDE a preview competes with neither — it is not on a border and it is
  # not a buffer's name. What the rule forbids is a second blue piece of
  # CHROME, and there is none here.
  #
  # THE HUES ARE TOKYO NIGHT'S OWN in owl, which is where this palette's
  # greens, yellows and purples already came from; a theme moves them with
  # its seeds. New hues would give a preview that does not look like the
  # system around it.
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
    comment = "slate.text-muted";

    string = "success.solid";
    # The one thing inside a string that is NOT string: `\n`, `&`, `${HOME}`.
    # Bright, because it is the thing that stops being literal.
    escape = "sky.solid";
    # `Number` AND `Constant`. A literal `null` and a literal `42` are the
    # same kind of thing today, and the two roles stay two.
    number = "orange.solid";

    keyword = "group.solid";
    type = "cyan.solid";
    # See the note above about the reservation: this one is content.
    function = "accent.solid";

    # `Attribute` AND `Heading`. Both are a marker ABOUT a block rather than
    # the block: `#[derive]` over a struct, `## Section` over a paragraph,
    # `@@ -1,4 +1,6 @@` over a hunk. In a diff this is the line between the
    # green and the red, which is how a patch is read.
    attribute = "warning.solid";

    # THE KEY SIDE of a key/value pair, and the highest-value colour in this
    # family: a file manager previews config more often than code, and
    # without it JSON is one colour of strings and a unit file is a grey wall.
    property = "azure.solid";

    tag = "error.solid";

    # THE TWO WHOSE COLOUR IS NOT A CHOICE. The same green as `string` and the
    # same red as `tag`, and they are separate recipes for the reason
    # `notification.read`, `.suppressed` and `.countdown` are three recipes on
    # one primitive: a diff line is not a string, and the day the theme wants
    # a diff green that is not the string green, these are the lines to
    # change.
    #
    # They cannot be confused on screen either: a diff line takes its role
    # from its first character and carries no highlighting inside it, so
    # nothing else in this family ever appears in the same file as these two.
    added = "success.solid";
    removed = "error.solid";
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
    background = "neutral.chrome-loud";

    # THE TWO FORMATTINGS, and the difference between them is the whole point
    # of having two: a copy that finished and a disk that is full must not
    # look alike.
    #
    # `quiet` is the same warm grey as `text.secondary`: present, readable,
    # and it does not pull the eye off what is being read.
    quiet = "neutral.text";
    # `loud` is the WARM alert, and deliberately NOT the accent. Blue marks
    # focus and only focus (see the border block above); a shouting
    # notification in blue would be a second blue thing on screen, and then
    # neither of them says "the keyboard goes here".
    loud = "warning.solid";

    # HOW MUCH TIME IS LEFT, drawn as an eighths pie in one cell. It has to be
    # findable and forgettable at once, so it takes the quietest tone in the
    # family — the same as `text.dim`.
    countdown = "neutral.text-muted";

    # ALREADY READ, in the manager's left column. It stays in the list — the
    # list is the record of what happened — and it recedes.
    read = "neutral.text-muted";
    # SUPPRESSED BY A FILTER, in the manager's right column. The same tone as
    # `read` on purpose: both are "here, and not asking for anything". What
    # tells them apart is which column they are in, and the column has a
    # heading.
    suppressed = "neutral.text-muted";
  };

  # -------------------------------------------------------------------
  # THE BRAIN'S LINE — below the minibuffer, above the expansion
  # -------------------------------------------------------------------
  #
  # It is the footer's own surface growing DOWNWARDS, the mirror of the
  # notification band growing upwards. One surface, no popup, no card
  # (invariants 14 and 15).
  #
  # NOT another rung of the band's ramp. `surface.base` → `surface.chrome` →
  # `modeline.background` → `notification.background` is a ladder the eye
  # climbs upwards, and every rung means "one more layer of chrome". This one
  # is a step SIDEWAYS: the same luminance neighbourhood as the modeline, with
  # a blue cast, because it is not one more thing above — it is a different
  # KIND of line. It is the only background in this file that carries a hue,
  # and that is the whole point: the machine is about to do something to
  # itself, and the line saying so must not read as a modeline that changed
  # its mind. The owner asked for it in those words: a dark, slightly
  # blue-ish grey, with white letters.
  brain = {
    background = "brain.ground";

    # THE ONE NEAR-WHITE IN THIS FILE, and it is confined to this line.
    #
    # The body text is warm on purpose — see the `text` block, where the whole
    # family was warmed together so a greyscale ramp would not change
    # temperature halfway. Over a blue-cast background a warm white reads as
    # dirty, so this family is neutral instead — the brain's own hue at the
    # top of the text ladder, where the gamut leaves almost no chroma. Two
    # temperatures on one screen is exactly what that block argues against,
    # which is why this one may not leave this line.
    text = "brain.text-bright";

    # The bracketed keys, and the quieter half of a button's label. The same
    # hue at a lower luminance — the `text.dim` relation, inside this family.
    dim = "brain.text";

    # THE PIE, and it is brighter than `dim` on purpose: it is the only thing
    # on the line that MOVES, and it is what a person looks at to decide
    # whether to reach for the keyboard.
    countdown = "brain.text-loud";
  };

  # -------------------------------------------------------------------
  # Modeline
  # -------------------------------------------------------------------
  modeline = {
    # ONE TONE ABOVE `surface.chrome`. The footer's two lines play different
    # roles — one says where you are, the other is where you type — and the
    # difference in background is what separates them with no rule and no
    # box (invariant 15).
    background = "neutral.chrome-raised";
    # -----------------------------------------------------------------
    # The buffer's STATE — two tokens, and the second is one on purpose
    # -----------------------------------------------------------------
    #
    # A buffer declares which state it is in; the modeline draws the name.
    # There are exactly two colours here and not one per state, because a
    # state that is not the base one is drawn the SAME regardless of which
    # buffer it belongs to: the reader learns one colour, and a theme changes
    # one line.
    #
    # A buffer MAY override this per state, by naming a role in its descriptor
    # (`tone`) — for the day a file manager waiting on a permission answer
    # needs to say something stronger than "not the usual one". None does
    # today.
    #
    # THE ACCENT IS NOT USED HERE, and that is deliberate. Blue marks focus
    # and nothing else: it lives on the window's border and moves to the
    # buffer's name when the footer takes the keyboard. A state in blue would
    # be a second blue thing on screen, and then neither of them says "this
    # is where the keyboard goes".
    state = {
      # THE RAMP, from the top: the buffer's NAME is `text.primary`, the mode
      # is `text.secondary`, and the base state is the quietest of the three —
      # `normal` is the one that says "nothing in particular", so it recedes.
      # Anything that is NOT normal jumps straight past all of them into green.
      normal = "neutral.text-muted"; # the same as `text.dim`
      alternate = "success.solid"; # the same green as `state.success`
    };
  };
}
