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
# `chrome-loud`, `chrome-band`, `border`, `border-strong`, the three desks
# (`backdrop`, `near`, `absolute`), each desk's `faint-*` edge and the six
# text steps.
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
    # the theme's colour (`LEADER s t`, the second column). The one neutral
    # step off the chrome in a hue of its own, the seed's `backdrop`: the desk
    # is the single surface meant to read as the theme's colour, and it is
    # under everything, so the footer and a window both keep an edge against
    # it (gated).
    #
    # AND IT IS A PAGE, since the person may put the footer and the windows
    # on the desk's colour (the third and fourth columns): every theme is
    # solved again with each desk in the chrome's place (`Theme.on_page`),
    # and a surface on the desk wears that palette — the bands follow the
    # ground.
    backdrop = "neutral.backdrop";
    # THE OTHER DESK: the extreme the interface climbs away from — black in a
    # dark theme, white in a light one, `#000000` or `#FFFFFF` exactly. It was
    # not a token while its only reader was the compositor and its name was
    # the option's ("black"); a light theme made that name wrong, and the
    # footer and the windows may now stand on it, which makes it a ground the
    # gates have to see.
    absolute = "neutral.absolute";
    # THE THIRD DESK, A LIGHT THEME'S: near-white, halfway from the chrome to
    # the paper in the grounds' own hue and tint — the owner's middle answer
    # between the white and the theme's colour. Every theme has it, because
    # every theme has the same keys; a dark one's is the near-black halfway to
    # the pixel switched off, gated as a ground and never offered.
    near = "neutral.near";
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

    # THE GROUND OF A CONTAINER INSIDE A BUFFER WINDOW, and it exists because
    # `shelf` above is INVISIBLE there.
    #
    # A shelf is `neutral.window`, which is right by 0,018 over the footer's
    # `surface.chrome`. A buffer's own ground is `surface.buffer`, which is
    # `neutral.window` — the same recipe. So a container drawn on a shelf
    # inside a buffer is drawn in the colour of the page it sits on: the
    # command is emitted, the quad is painted, and the pixels do not move.
    # Measured, not reasoned: the brain's history rows emitted
    # `selection surface-shelf` and the right edge of every one of them read
    # `#08150A`, exactly the page. The owner asked for a container with a
    # background of its own and got a container with no background at all.
    #
    # `neutral.chrome-raised` is 0,035 above the window — the next rung that
    # exists, and the same device one step over: no border, no radius, no
    # shadow (invariant 15). The hover row (0,070 above chrome) and the focused
    # one (the accent) both still read on top of it.
    container = "neutral.chrome-raised";

    # AND THE GROUND OF A BUFFER'S HEADER LINE, one rung above that again.
    #
    # The header is chrome — a glyph, the screen's name, and every shortcut it
    # answers to — and it has to read as chrome against a page of content. The
    # container above is meant to be quiet; this one is meant to be a band.
    #
    # IT WAS `neutral.chrome-loud`, AND THAT RUNG WAS THE HOVER BAND'S.
    # Measured on 2026-09-20 across the nine themes: `surface.header` against
    # `surface.hover` came to 0,0043 in darkside — the page's name and the
    # pointer's row drawn in the same colour. `chrome-loud` could not be the
    # one to move, because it is `notification.background` too and that hex is
    # frozen in `owl.reference.nix`; so the header took a rung of its own. The
    # whole argument is written where the rung is, in `NEUTRAL_PAINT`.
    header = "neutral.chrome-band";

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
    # Nine tenths. It was four fifths, and the owner found the player's
    # shortcut line hard to read over some films — a bright, busy frame came
    # through the band and through the letters with it. Below about two
    # thirds the glyphs fight a white frame; at full opacity the band stops
    # reading as part of the picture and becomes a strip of chrome across it,
    # which is what invariant 14 is about. Nine tenths still lets the film
    # show, and the text wins.
    transport = {
      of = "neutral.chrome";
      alpha = 0.9;
    };

    # THE SECOND SURFACE WITH AN ALPHA, and it is what the POINTER has claimed.
    #
    # Two things wear it, and they are one thing said twice: the rectangle a
    # rubber band is being dragged over the listing, and the folder a drop
    # would land in. Both mean *"this is what the button is about"*, and both
    # have to be legible over a MINIATURE — a grid is pictures edge to edge,
    # and there is no depth test, so the band is pushed with the quads that are
    # drawn AFTER the images (`Quads::draw_range`), exactly as the transport is.
    #
    # THE ACCENT AND NOT A NEUTRAL, which is the difference from `transport`.
    # That one darkens a picture so glyphs can be read on it; this one has to
    # say "chosen" over a picture whose own colours are unknown, and the only
    # thing in this palette that means chosen is the accent.
    #
    # A THIRD, and it is low on purpose. The band is a claim about what is
    # underneath, so what is underneath has to go on being visible: at two
    # thirds a thumbnail inside the band becomes a coloured rectangle and the
    # person can no longer see what they are selecting. Under about a fifth it
    # stops being visible over a bright photograph, which is the case it exists
    # for.
    sweep = {
      of = "accent.element";
      alpha = 0.34;
    };

    # TEXT BEING SELECTED WITH THE POINTER, in any view, window or buffer.
    #
    # Drawn OVER the letters (the order is rectangle, image, text, and the
    # selection goes with the rectangles, so the letters stay on top), which
    # is why it is translucent: what is selected has to stay readable while it
    # is chosen. The accent, because chosen is what the accent means; lower
    # than the sweep, because a sweep is over pictures and this is over text.
    text-selection = {
      of = "accent.element";
      alpha = 0.45;
    };

    # THE FOOTER'S TWO HIDDEN MODES (`LEADER LEADER` pushes, `LEADER ALT
    # LEADER` covers — `crate::hidden`).
    #
    # NOTHING: the one-pixel edge a hidden footer keeps, and the clear under a
    # veil, where the window shows through. The chrome's hue at alpha 0 and
    # not a colour of its own, so a surface the driver cannot make
    # see-through (`Gpu::transparent`) clears with the chrome it always had.
    # A row that pushes the windows away stands on the desk, opaque: the
    # footer WEARS the desk's page there (`theme::Page`), whose chrome is the
    # desk.
    clear = {
      of = "neutral.chrome";
      alpha = 0.0;
    };
    # AND THE VEIL, where a row lies OVER a window: the page's ground at four
    # fifths, the window readable underneath and the letters winning. It is
    # not a card (invariant 15): no border, no radius, no shadow — the
    # transport's device, one band the width of the screen. Tuned by eye,
    # like the transport's nine tenths; the blend on an sRGB target makes it
    # read a little lighter than its number (`ronoctua_render::gpu::clear_of`).
    #
    # ONE RECIPE FOR EVERY GROUND: on a desk's page the chrome IS the desk,
    # so a footer that wears the desk's page lays the desk's veil.
    veil = {
      of = "neutral.chrome";
      alpha = 0.8;
    };

    # THE ON-SCREEN KEYBOARD'S KEYS (`LEADER m k`), grounds under a legend.
    #
    # A key has to read as a KEY with nothing but its ground — no border, no
    # radius, no shadow (invariant 15) — so the keys that type are one rung
    # above the chrome and the keys that do something else (a modifier, Tab,
    # the language) one rung below that: the letters are what the eye hunts
    # for, and they are the ones that stand out.
    #
    # `hover` is the rung ABOVE the keys, not the list's `surface.hover`: that
    # one sits between the chrome and a key, and a key the pointer is on would
    # go darker. `on` is a key held down or a modifier waiting for the next key
    # — the accent's ground, because the accent is what means "this is in
    # effect"; `locked` is the same modifier kept on until it is pressed again,
    # one rung brighter so the two can be told apart at a glance.
    key = {
      cap = "neutral.chrome-raised";
      action = "neutral.window";
      hover = "neutral.chrome-band";
      on = "accent.element";
      locked = "alternate.element";
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

    # THE GLOBE, and the argument the mesh makes applies whole: the world
    # carries no colours of its own, so any variety here would be invented.
    # What has to read at a glance is THREE things — sea, land, and the country
    # the coordinate is in — and that is what these four are.
    #
    # There is NO background token: what is behind the disc is each door's own
    # surface, which is the same one-texture-two-doors argument the mesh and
    # the film's letterbox already had to make.
    atlas = {
      # THE DISC, WHERE THERE IS NO LAND — and it must NOT be the chrome.
      #
      # It was `neutral.chrome` and that is the panel's own ground: the sea and
      # the panel came out the same ink, so the globe had no DISC at all —
      # land floating on the background, with the rim the only thing saying
      # where the world ended. Caught by the shots, and only because the
      # assertion that was supposed to find the sea found the whole screen
      # instead: an empty assertion and a real defect, in one line.
      #
      # `accent.element` is the theme's OWN hue at a ground's weight, and it is
      # not an invention: a sea that reads as sea is the one thing every map
      # that has ever been drawn agrees on, and taking the hue from the theme
      # is what keeps it the theme's rather than a colour this file chose.
      #
      # `neutral.window` was tried first and is wrong for a reason worth
      # keeping: it is the same rung the buffer's ground sits on, so the disc
      # would be invisible in the window door and visible in the footer — one
      # picture, two answers.
      ocean = {
        of = "accent.element";
      };
      # Land. The mesh's own reasoning at a lower chroma: warm by a hair so it
      # does not read as part of the cold surface behind it.
      land = {
        of = "neutral.text-loud";
        chroma = 0.22;
      };
      # THE COUNTRY THE COORDINATE IS IN. It is what "framed on the country"
      # SHOWS as well as frames, and it is the accent because it is the answer
      # to the question that was asked.
      here = {
        of = "accent.text-muted";
      };
      # Inland water, painted over the land. Named apart from the ocean on
      # purpose: the day one of the two moves, the other must not — and one of
      # them just did.
      lake = {
        of = "accent.element";
      };
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
    # THE VALUE OF A ROW ONE CAN ACT ON, and it is the counterpart `text.read`
    # never had.
    #
    # The pair was asymmetric: a row one cannot act on had a role of its own
    # (`read`), and a row one CAN act on borrowed the body-text token. So the
    # only way to give the answer more colour was to warm every word on every
    # screen — and measured, that does not even work: past a relative chroma of
    # 0.40 what moves is the ramp's DIM end (`text.dim`, the notification
    # tones, the modeline's normal state, all of which drift past
    # `owl.reference.nix`), while the bright step where a value lives barely
    # changes, because the ladder squeezes chroma as it approaches white.
    #
    # A MULTIPLIER ON ONE RECIPE is the whole of the fix: the same primitive,
    # the same luminance, its chroma scaled. Nothing else on any screen moves,
    # and no frozen key drifts, because this token is new.
    #
    # 2.4 AND NOT MORE, AND THE CEILING IS THE SCREEN. At 2.4 the red channel
    # sits at FE; at 2.5 it clips to FF and stays there, so every larger number
    # draws the same pixel. A chroma the report states and the display cannot
    # show is a number that has stopped describing anything — the owner picked
    # this one knowing that is where it stops.
    answer = {
      of = "neutral.text-strong";
      chroma = 2.4;
    };
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
    # green/yellow/red are state.
    #
    # AND IT IS A TEXT RUNG, not the family's SOLID — which is the whole of
    # what changed on 2026-09-21, and it was the owner reading the list on
    # screen that found it: *"estou achando que uma das cores está se
    # destacando mais e assim facilitando a leitura dela e chamando mais
    # atenção para ela, do que mantendo um mesmo padrão entre as duas"*.
    #
    # Measured, that is exactly what it was. `group.solid` is the step a
    # family fills a BUTTON with, anchored at Lc 52 plus the family's shift —
    # 56 on the chrome in all nine themes — while the command beside it is
    # `text.secondary`, the neutral ramp's own `text` rung at Lc 48. Eight Lc
    # apart, ΔL 0.05, every theme, and nothing in the gates had ever compared
    # the two: `text.group` was measured against the GROUNDS it is drawn on
    # and never against the ink it shares a line with. So the thing that
    # leads somewhere read louder than the thing that runs, which is the
    # hierarchy backwards.
    #
    # One rung, one job: a role that is TEXT takes a text step. `group.text`
    # is the same family solved at the same Lc as `text.secondary`, so the
    # two now differ by HUE alone — which is all the distinction was ever
    # supposed to be, the `+` carrying the rest.
    #
    # AND THE MULTIPLIER IS WHY IT IS STILL A HUE ONE CAN SEE. A text rung is
    # solved at the family's `text-chroma`, which is the chroma of a ramp
    # meant to be READ, and at the shared 0.35 this violet lands ΔE 0.056
    # from a grey ramp (cyber; industrial 0.065, frost 0.067). With the
    # lightness now equal, that distance is the WHOLE distinction, and a hue
    # nobody can see is not one. 1.4 is the smallest step that carries every
    # theme past the gate below — measured, the worst pair goes 0.056 → 0.079.
    #
    # WHY A MULTIPLIER ON THE RECIPE AND NOT A `text-chroma` IN THE NINE
    # SEEDS, which was the first attempt: a family's text-chroma is the whole
    # ramp's, and this family's ramp has other tenants. The accent's has four
    # — `text.mine`, the globe's dot and its country, and the waveform — and
    # raising it for the sake of a key letter moved all of them, in eight
    # themes, for a reason none of them shares. The build said so out loud:
    # frost's `surface.wave` is near-white with the accent's temperature, and
    # at the chroma the KEY needed it came within 0.198 of `#00FFFF`, which is
    # one of the four film colours no chrome token may approach. A multiplier
    # on one recipe moves one token, which is the size of this decision.
    #
    # THE SAME FAMILY as `syntax.keyword`, which keeps the solid: a keyword
    # is ink on a page of ink, not a row in a list of rows.
    group = {
      of = "group.text";
      chroma = 1.4;
    };
    # THE TITLE OF A SECTION, in every view that has sections.
    #
    # THIS TOKEN IS A SPLIT, and the split is what `text.group` moving to a
    # text rung forced into the open. One name was doing two jobs: the label
    # of a SUBMENU in the shortcut list, which is what the doc above it
    # describes and what the name means, and the TITLE OF A SECTION, which
    # twenty-six call sites across twelve views had borrowed it for — every
    # `heading(…)` builder in the system, plus the chevron of a drawer.
    #
    # They read the same until the day one of them had to move. A submenu is a
    # ROW in a list of rows and has to weigh what the rows beside it weigh; a
    # title is what those rows hang under, and hierarchy in this system is
    # colour and air (invariant 15), so a title that drops to the body's rung
    # has spent the only thing it had. Dragging every section title in the
    # system down a step was not what the owner asked for when he asked about
    # a shortcut list.
    #
    # So the title keeps the SOLID, byte for byte what it has always been —
    # nothing on any of those twelve screens changes — and the name now says
    # which of the two jobs it is. The contrast gates that were written for
    # `text.group` came here with it, because they were always measuring this.
    heading = "group.solid";
    # THE TITLE OF A HEADER — the window's name on line 0, and the brain's.
    #
    # It was `text.heading`, the ink of every title INSIDE the content, and the
    # owner read the two side by side on the manual's window: the header's
    # name and the first section under it wore the same colour, so the line
    # that frames the window read as one more heading of its body. It is the
    # hint's own family (`subtle`) on its loud rung, with a little more of its
    # hue: part of the header, and the strongest thing on it.
    header-title = {
      of = "subtle.text-loud";
      chroma = 1.6;
    };
    # THE KEY ONE PRESSES, in every list that teaches one.
    #
    # It was `state.accent` — the same token as the PENDING CHORD one line
    # above it in the minibuffer, and as the cursor in the finder and the
    # theme chooser. Three jobs, one hex, and the accent's anchor is Lc 52:
    # the offered keys were drawn louder than the command names they answer
    # for, for the same reason `text.group` was, and the letter column is on
    # every row of the list.
    #
    # The accent's own TEXT rung instead, which is `text.mine`'s argument
    # word for word — *"a step down from its solid tone reads as related
    # without competing with the cursor sitting on the same list"*. Here the
    # cursor really is on the same list: the finder paints its cursor row in
    # `state.accent` and the matched letters of a query in it too, and both
    # of those only started to read as a different thing on the day the
    # resting key stopped being the identical hex.
    #
    # WHAT STAYS AT Lc 52 IS THE CHORD. `CTRL ALT SPC-` is STATE — a sequence
    # is in progress — and it is the one thing on those lines that is not an
    # offer; draining it would leave the list with nothing saying that the
    # keyboard is inside a sequence at all.
    #
    # 2.4 AND NOT 1.4: the accent's text ramp has to cross a ramp that is
    # often the same temperature as the accent itself, and frost is where that
    # is worst — a cyan accent on a cyan-grey ramp, ΔE 0.020 apart at the
    # shared chroma, which is a key letter the eye reads as a command name.
    # The number is what frost needs and the other eight ride along; measured,
    # it leaves the key no louder than it is today, because what it replaces
    # is `accent.solid`, which is more saturated still.
    key = {
      of = "accent.text";
      chroma = 2.4;
    };
    # THE LETTERS A QUERY MATCHED, ON THE ROW THE CURSOR IS ON.
    #
    # Off the cursor the matched letters are `state.accent` over a quieter
    # name, and they stand out. On the cursor row the name itself becomes the
    # accent (the cursor is a colour), so the letters that said WHY the row is
    # in the list vanished into it — the owner read it on the finder, the
    # manual list and the app locator. The accent's own top text rung: the
    # same hue, so it still reads as "this is the match", and a step lighter,
    # so it is not the name it sits in.
    match-here = {
      of = "accent.text-loud";
      chroma = 1.8;
    };
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
      # THE KEY THAT IS ONLY HERE BECAUSE OF WHAT YOU ARE LOOKING AT.
      #
      # A film's rewind, a shape's zoom, a playlist's next track: on the line
      # now and gone a row later. The owner asked for both halves — that they
      # appear only in their context, and that they READ differently from the
      # keys that are always there.
      #
      # THE ALTERNATE'S HUE AT HALF ITS COLOUR, and not a family of its own: a
      # new family is a seed in all eleven themes plus the frozen reference, and
      # what this has to say is a DEGREE, not another kind of thing. The
      # alternate is already the register for "this key is not what it looks
      # like"; a guest is that, quieter.
      contextual = {
        of = "alternate.text-muted";
      };
      # THE SENTENCE UNDER AN OPENED LINE OF SHORTCUTS — a COOL GREY, and a
      # family of its own (`explain`, in every seed).
      #
      # It was the hint's own prose first, and the owner read it as one more
      # shortcut: *"não criando uma boa distinção visual"*. Then it was
      # ACHROMATIC at `text-loud` — no hue being the one seat left that was
      # not another family, and the loud rung because nought chroma alone
      # landed on `text.secondary` in the grey themes (ΔE 0.0000). The owner
      # read that one too: too BRIGHT, louder than the content under the bar,
      # and in the warm themes brighter than their yellowish text — *"um cinza
      # frio discreto"*, he said.
      #
      # So it takes a HUE after all, a cold one and barely drawn, and sits
      # BELOW the content's middle step: the sentence is read when asked for
      # (the line is opened for it) and does not compete with what is under
      # it. The gates against the hint's registers and the ramp keep it apart
      # from every other text.
      explain = {
        of = "explain.text";
      };
      # A KEY ON A HINT LINE THAT A CLICK CANNOT PRESS.
      #
      # Every shortcut a header teaches is clickable: a click presses the key
      # it names (`chrome::Mark::press`). A few notations are not one key —
      # `RCtrl ↑↓`, a range of digits, a held modifier — and the owner asked
      # for those to read slightly apart from the rest, so the hand does not
      # reach for a control that does nothing.
      #
      # The key's own rung with its hue taken away: the same weight, because
      # it IS a key and the eye still hunts for it, and a tone that says
      # "this one is only to be read". A degree and not another kind of
      # thing, which is why it is not a family.
      inert = {
        of = "subtle.text";
        chroma = 0.0;
      };
      # A SHORTCUT UNDER THE POINTER — the one a click would press.
      #
      # Every header of the system lights it, and the owner asked for the lit
      # tones to be SMALL variations of the resting ones: the same rung (the
      # same weight, so nothing on the line jumps) with more of its own hue. The
      # brain lit its keys in `text.once` and its words in the content's grey,
      # which were other colours altogether; these are the same colours, lit.
      lit = {
        key = {
          of = "subtle.text";
          chroma = 2.4;
        };
        prose = {
          of = "subtle.text-muted";
          chroma = 2.4;
        };
        alternate = {
          of = "alternate.text";
          chroma = 1.6;
        };
        contextual = {
          of = "alternate.text-muted";
          chroma = 1.9;
        };
      };
    };
    # THE LABEL COLUMN of the three views that are a list of a label and a
    # value — network, power and Bluetooth.
    #
    # # Why the label leaves the neutral ramp
    #
    # Because it was wearing the value's own colours. The two columns were
    # drawn from one pair of greys — the label in one step, the value in the
    # other, and swapped inside a list — so nothing said which column was
    # which except its position, and nothing at all said whether the row was a
    # DOOR. A mark in front of it said that for a while, and the owner refused
    # it: a chevron is what a tree uses to open and close, and this row does
    # neither.
    #
    # So the difference moves to where the palette's own rule puts it. HUE says
    # "another kind of thing": the label column takes a hue of its own, and no
    # value ever wears it. LUMINANCE says "another importance": inside that
    # hue, `acts` is a row that answers `RET` and `fact` a row one can only
    # read.
    #
    # A row that is reachable and REFUSING keeps `text.dim` over both: that is
    # not a kind of row, it is what is happening to one. THE CURSOR IS NOT
    # LIKE THAT, and this file claimed it was for a year — see `text.here`.
    # Under the cursor the label takes the accent and the value keeps saying
    # whether the answer can be changed, because the row being looked at is
    # the row whose question is being asked.
    #
    # AND THE PAIR IS SOBER, at the owner's word. It went out loud the first
    # time — the family's `text-loud` step at `alternate`'s chroma — and a
    # whole column of labels at that weight competes with the values it names.
    # A label is the quiet half of its line; it is the VALUE one reads.
    label = {
      acts = "label.text";
      # AND THE FACT'S LABEL IS THE SAME WORD WITH THE LIFE TAKEN OUT.
      #
      # It was `label.text-muted` — twenty-five Lc below the label beside it —
      # and the owner read it off the screen as a column half switched off. A
      # label is the quiet half of its line, not the dim half: what it says is
      # WHICH question this is, and that is worth reading whether or not the
      # answer can be changed. So the distinction is CHROMA now, at the same
      # luminance: see the `label-read` seed.
      fact = "label-read.text";
    };
    # THE VALUE OF A ROW ONE CANNOT ACT ON.
    #
    # Grey, at the weight it already had: `strong` is a fact's answer and
    # `weak` a list item's description, the same two steps the neutral ramp
    # gives an interactive row's value — with the colour taken out.
    #
    # Everything else on these screens is warm (the ramp is hue 85 at chroma
    # 0.26), so a neutral value reads as a step back from a cream one without
    # being any darker. That is the whole device, and it is why only the
    # COLOUR changes: a row one can only read should not be the loudest thing
    # on its line, and it should still be perfectly legible — it is, after
    # all, the thing one came to read.
    read = {
      strong = "grey.text-loud";
      # AND THE LIST ITEM'S KEEPS THE WEIGHT IT HAD. `grey` dropped a second
      # step to separate the pair in a sheet; this one is a different screen's
      # list, and following it there would have put it at Lc 32 — under APCA's
      # floor for text of any size. See the `grey-listed` seed.
      weak = "grey-listed.text";
    };

    # THE ROW UNDER THE CURSOR — AND IT STILL SAYS WHAT KIND OF ROW IT IS.
    #
    # This file used to say the opposite, in as many words: *"the cursor keeps
    # `state.accent` over both, and a row that is reachable and REFUSING keeps
    # `text.dim` over both — neither of those is a kind of row, they are what
    # is happening to one."* It reads well and it was wrong on the screen. The
    # owner moved the cursor down a sheet and watched a door and a fact become
    # the same row: everything the label column had been rebuilt to say went
    # out the moment the row was the one being looked at — which is the moment
    # the question is actually being asked.
    #
    # WHY THE DIFFERENCE GOES ON THE VALUE AND NOT THE LABEL. Because the
    # LABEL has to carry "the keyboard is here", and it is the half that is
    # always there. Neither window draws a band behind the cursor's row — the
    # footer's band is the POINTER's — so the accent on the text is the only
    # thing saying where the keyboard is, and a row whose value is empty would
    # lose it entirely if the value carried it. So: the label is the accent on
    # every focused row, and the value says whether the answer can be changed.
    # It is the same sentence the resting row already tells, told in the one
    # hue a focused row is allowed to wear.
    #
    # AND THE AXIS IS CHROMA, for the third time in this file and for the same
    # reason each time (`label-read`, `read`): draining colour at a fixed
    # luminance separates two things without making either of them the DIM
    # one. `acts` is the accent whole; `read` is the accent with the colour
    # taken out of it, still plainly the accent's own step, still plainly
    # "here". A multiplier on one recipe, which is what `answer` above already
    # does — no new family in any theme, and no frozen key moved.
    #
    # `acts` resolves to the same primitive `state.accent` does, and that is
    # deliberate rather than a duplicate: what this token records is the
    # DECISION that a focused answer one can change wears the focus colour
    # whole. The gate that matters measures the pair, and the pair differs.
    here = {
      acts = "accent.solid";
      read = {
        of = "accent.solid";
        chroma = 0.40;
      };
    };

    # WHAT THE PERSON WROTE THEMSELVES.
    #
    # The catalogue is forty rows the distro ships and, under them, the handful
    # the person wrote. The word beside each one says which ("your filter"), and
    # the owner asked for the colour to say it too — a word at the far end of a
    # row is not what the eye finds when it is looking for its own work.
    #
    # It is the ACCENT's family and not a new one: the accent is already what
    # this system uses for "this one is yours to act on" (the cursor, a focused
    # button), and a step down from its solid tone reads as related without
    # competing with the cursor sitting on the same list. No seed changes, so
    # no theme grows a family for one row.
    mine = "accent.text";

    # AND WHAT IS ON ITS WAY TO EVERYBODY.
    #
    # A filter of theirs that has been sent to the repository and has not come
    # back yet: still working, still theirs, and no longer only theirs. A third
    # state gets a third tone rather than a footnote — but a COOLER family than
    # the accent, because it is not something to act on, it is something that
    # is happening. The day a build brings it back it becomes one of the
    # distro's rows and this tone stops applying to it, which is the honest end
    # of the story.
    #
    # `alternate` is the family, and that is not an arbitrary pick from the
    # list. It is the one this system already spends on "the meaning of this is
    # in motion" — the hint line's key whose sentence moves under the reader —
    # and a filter halfway to everybody is exactly that. It was `cyan` first,
    # and measured: cyan sits ΔE_ok 0.046 from `text.mine`, which is above the
    # JND and below the distance this project asks of two things that must not
    # be confused. `alternate` measures 0.079 from `mine` and 0.26 from an
    # ordinary row.
    shared = "alternate.text";

    # A TASK THAT RUNS ONCE AND THEN IS GONE.
    #
    # The brain's list has two kinds of row and the owner asked for them to be
    # told apart by a glyph AND a colour: a routine keeps the ordinary ramp, a
    # one-shot does not. Same family as `shared` above, and for a reason that
    # is nearly the same one: `alternate` is what this system spends on "the
    # other kind of the thing you are looking at". A routine is the ramp; this
    # is the ramp's alternative.
    #
    # A NAME OF ITS OWN even though the recipe matches `shared`, because the
    # two say different things and the day one of them moves is the day a
    # shared name would move the other with it. They never meet on a screen —
    # `shared` is a notification filter, this is a row of the brain.
    once = "alternate.text";
  };

  # -------------------------------------------------------------------
  # Rules — the ONLY form of separation besides whitespace
  # -------------------------------------------------------------------
  rule = {
    # One line, one logical pixel. It separates the content from the footer.
    default = "neutral.border";
    strong = "neutral.border-strong";

    # THE SAME LINE, ON A GROUND THAT WAS RAISED.
    #
    # A rule is a SIGNAL drawn on a ground, and `default` is the step that
    # reads against the chrome. On a raised ground it does not: measured on
    # 2026-09-20 across the nine themes, `rule.default` on `surface.header` is
    # ΔE 0,025 and on `surface.container` 0,047 — a line one squints at, and a
    # line the owner photographed as not being there.
    #
    # Same hex as `strong`, and still its own name, for the reason
    # `state.accent` and `text.here.acts` are two names for one hex: the
    # division is about what the line SAYS. `strong` is a rule that is louder
    # on purpose; this one is a rule that had to climb to stay the same
    # loudness. The day a raised ground moves, one of them follows it and the
    # other does not.
    #
    # Nothing chooses between them by hand: `Pen::rule` resolves it from the
    # role the block's ground was filled with, so a buffer cannot get it wrong
    # — see `Role::RuleRaised` and `a-rule-follows-its-ground`.
    raised = "neutral.border-strong";

    # THE LINES OF THE GLOBE. A border is stroked OVER the fill, which is also
    # what hides the one-step seam two independently quantised countries can
    # have between them — see `baked.rs`.
    atlas = {
      border = "neutral.border-strong";
      # A REGION'S EDGE IS QUIETER THAN A COUNTRY'S, and the theme gate holds
      # the two apart in every palette: with one ink for both, a country full
      # of provinces reads as mud.
      region = "neutral.border";
      rim = "neutral.border-strong";
      # A STREET NETWORK CARRIES NO COLOURS EITHER, so one ink, for the mesh's
      # reason.
      street = {
        of = "neutral.text-muted";
      };
    };
  };

  # -------------------------------------------------------------------
  # Window border — drawn by the COMPOSITOR, with these colours
  # -------------------------------------------------------------------
  #
  # BOTH OF THEM ARE THE THEME'S OWN ACCENT, one rung apart on the paint
  # ladder — and neither of them was, which is what the owner said in one
  # line: *"não gosto nem um pouco das atuais cores de bordas das janelas;
  # ambas as cores de foco e fora de foco devem ser alteradas de acordo com o
  # tema, e podem ser mais discretas que as atuais."*
  #
  # WHAT THEY WERE. The active one was `accent.solid` — the brightest step the
  # accent has, a colour solved to be READ AS TEXT and spent here on a
  # two-pixel outline. The inactive one was the `error` family's border: a
  # dark red, in a theme whose entire identity is green. Neither followed the
  # theme; one of them was not even its hue.
  #
  # WHAT SURVIVES FROM THE OLD DECISION. The reason the inactive one was ever
  # given a colour of its own: while the footer holds the keyboard, NO window
  # is focused as far as the compositor is concerned, so whatever marks focus
  # disappears at that instant for every window at once. With a grey there the
  # outline read as SWITCHED OFF. So the inactive one still has to be a
  # colour, and it still is — the same hue, one rung down. Pressing the leader
  # now DIMS the outline instead of erasing it or turning it red.
  #
  # WHY A PAINT STEP AND NOT A SOLID. Because that is what these two are: an
  # outline around a window, not a word to be read. The paint ladder is the
  # register built for exactly that, and moving to it is the whole of "more
  # discreet".
  #
  # AND THEN A RUNG LOWER AGAIN, with the pair spread wider — the owner asked
  # for both at once: *"a diferença entre a janela em foco e a que não está em
  # foco deve ser um pouco maior; e ambos, foco e fora de foco, devem ser um
  # pouco mais discretos."*
  #
  # THE LADDER CAPS HOW MUCH OF THAT CAN BE LIGHTNESS. The rungs sit at 0.044,
  # 0.102, 0.178 and 0.26 above the chrome, and the window's own ground is at
  # 0.018 — so an inactive outline on the bottom rung is a couple of
  # hundredths from the surface it is drawn around, and measured, it stops
  # reading as an outline at all. The lowest pair that still frames a window
  # is `border` over `element`, and that pair is NARROWER than the one it
  # replaces. So the rest of the difference has to come from chroma.
  #
  # IT COMES OUT OF THE INACTIVE ONE, and not into the active one. Pushing the
  # focused outline's chroma up widens the pair too — measured, it nearly
  # doubles it in owl — but a more saturated outline is the opposite of what
  # was asked for, and in forest it does nothing at all, because a green that
  # dark is already against the edge of what sRGB can show. Draining the
  # unfocused one moves the same distance in the direction the request
  # points: quieter, not louder.
  #
  # WHAT IT KEEPS. Enough hue to not read as SWITCHED OFF, which is the
  # constraint the inactive outline has carried since it was first given a
  # colour of its own: while the footer holds the keyboard no window is
  # focused, so every outline changes at once, and a grey there says the
  # screen turned off rather than that the keyboard moved.
  #
  # `accent.border` is also `context.accent.border`, and that is one primitive
  # under two names rather than an accident — the same arrangement
  # `syntax.keyword` and `text.group` already have.
  border = {
    active = "accent.border";
    # A SELECTED WINDOW'S OUTLINE (the compositor's `client.selected`), which
    # was the compositor's own fixed green in every theme: the theme's green
    # now, at the rung the focused outline wears.
    selected = "success.solid";
    inactive = {
      of = "accent.element";
      chroma = 0.20;
    };
    # THE DISCREET BORDERS (`LEADER s t`, the borders column): the owner asked
    # for the focused window's outline to be "very discreet" and for every
    # other one to be "EXTREMELY faint — never transparent: the person still
    # needs a reference to resize with the mouse".
    #
    # The focused one is the accent's ELEMENT rung, the selection band's: the
    # accent still says "the keyboard is here", a rung quieter than the
    # standard outline, and the gates hold it between the two it sits between
    # (`DISCREET_ORDER`). It is also the right column's focus line when the
    # borders are discreet (`theme::focus_line`), so it is measured against
    # every ground that line is drawn on.
    # EVEN QUIETER (the owner, 2026-09-28): the element rung with less than
    # half its colour — still the accent's hue, still above the faint outline
    # and below the standard one (`DISCREET_ORDER`).
    discreet = {
      of = "accent.element";
      chroma = 0.45;
    };
    # The unfocused one is the DESK'S OWN colour one faint step up the ladder
    # (`FAINT_RATIO`), so there is one per desk: an outline is measured
    # against what it lies on, and black and a grey desk are not the same
    # ground. Opaque, like every outline: an alpha would be elevation
    # (invariant 15).
    faint-backdrop = "neutral.faint-backdrop";
    faint-near = "neutral.faint-near";
    faint-absolute = "neutral.faint-absolute";
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
  # -------------------------------------------------------------------
  # THE GLANCE — `LEADER g`, one line above the modeline
  # -------------------------------------------------------------------
  #
  # It draws several PLACES side by side on one line, and where one ends and
  # the next begins has to be readable. A separator alone was not enough: with
  # four places the line is a row of words and the eye has nothing to group
  # them by.
  #
  # So the ground ALTERNATES, place by place. Not a border, not a box, not a
  # radius (invariant 15) — a change of ground, which is the one device this
  # system already uses for exactly this (`surface.shelf`, and the modeline
  # against the band).
  #
  # The FIRST place keeps the band's own ground, because the glance sits under
  # the band and a first place that changed colour would read as a third rung
  # of that ladder rather than as a stripe.
  #
  # The alternate is the CHROME, two rungs down and not one. One rung — the
  # modeline's — measured six values out of 255 on the screen, which is the
  # `surface.shelf` register: right for a container that is merely present,
  # too quiet for a stripe whose whole job is to say where one place ends. The
  # owner asked for the second place to GAIN a background, and at one rung it
  # had not gained a visible one. Two rungs is still two dark greys with no
  # hue between them — quiet, and legible.
  # -------------------------------------------------------------------
  # Marks — a point drawn ON something, rather than a surface or a line
  # -------------------------------------------------------------------
  #
  # ONE FAMILY FOR TWO, and it is a family rather than two strays under
  # `surface` because a dot on a globe is neither a surface nor a rule: it is a
  # mark, and the next one that needs a colour will be too.
  mark = {
    atlas = {
      # WHERE THE COORDINATE IS. The accent at its loudest, because the whole
      # picture exists to answer where this is.
      dot = "accent.text-strong";
      # AND A RING ROUND IT, so the dot reads over land and over sea both —
      # two draws and no branch. The chrome's own colour, which is what is
      # furthest from both of them.
      halo = {
        of = "neutral.chrome";
        alpha = 0.85;
      };
    };
  };

  # THE SCREEN CAPTURE'S OVERLAY (`ronoctua-shell capture`), drawn over a
  # frozen picture of the screen whose colours are anybody's.
  #
  # `dim` darkens what would NOT be kept, so the rectangle or the window a
  # click would take is the bright part — the page's own ground, translucent,
  # because what is dimmed still has to be seen to be chosen from. `guide` is
  # the two lines through the pointer and every outline: a family of its own
  # (`guide`, the accent's complement in every seed), so it is never the
  # focused window's border the frozen picture already carries, and bright
  # enough that a dotted line one pixel wide still reads over a photograph.
  capture = {
    dim = {
      of = "neutral.ground";
      alpha = 0.55;
    };
    guide = {
      of = "guide.text-loud";
      chroma = 1.9;
    };
  };

  glance = {
    ground = "neutral.chrome-loud";
    ground-alternate = "neutral.chrome";
  };

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

    # AN ACTION THE NOTIFICATION CARRIES, on the band and in the open record.
    #
    # The owner read the screen: an action was drawn in `quiet`, the same tone
    # as the sentence above it, so the one thing on the card that can be
    # PRESSED looked like one more line of the message. Nothing said it was a
    # button — no box, no border and no rule, because there are none here
    # (invariant 15) — which leaves colour as the only thing that could.
    #
    # `label.text` is the palette's own answer to "this answers `RET`": the
    # hue that no value ever wears, which is what the label column of the
    # network, power and Bluetooth views already uses. The band gets a recipe
    # of its own rather than borrowing `text.label.acts`, for the reason
    # `read`, `suppressed` and `countdown` are three recipes on one primitive
    # — the day a theme wants the band's buttons apart from a label column,
    # this is the line to change.
    acts = "label.text";
    # THE SAME ACTION UNDER THE POINTER — where a click would land.
    #
    # An event said on the minibuffer's line carries its entry's actions at the
    # end of the line (`chrome::Piece::action`), drawn in `acts` so an action
    # looks the same on the band and on the line, and on that line every
    # shortcut lights under the pointer (`chrome::lit`). So the action lights
    # too, the way the hint's keys do: the same rung and more of its own hue
    # — a small step, which is what the owner asked the lit tones to be.
    acts-lit = {
      of = "label.text";
      chroma = 1.6;
    };

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
    # A CONNECTED BLUETOOTH DEVICE — every one of its marks, in a family of
    # its own (`device`, in every seed): a faded blue, at the same rung as
    # `text.secondary`, which every other mark wears. The owner asked for the
    # devices to read apart from the rest of the line, and a hue at the same
    # luminance is a KIND of thing, not a louder one. Not the accent, which is
    # the keyboard's; and a device whose battery is at the danger level still
    # takes `state.error` over this.
    device = "device.text";
    # THE CAMERA IS LOOKING — the camera-in-use mark and the gestures' —, in a
    # family of its own (`watching`, in every seed): a faded pink leaning to
    # red, at the device's rung. The owner asked for it beside the Bluetooth
    # blue: the same quietness, another KIND of thing. Not the danger red,
    # which is the RECORDING mark's: looking is not yet keeping.
    watching = "watching.text";
    # THE SELECTION — windows marked to be acted on together. The green of
    # their outline (`border.selected`), at the rung every other mark reads
    # at: the mark and the windows it counts are told apart by nothing else.
    selection = "success.solid";
    # THE DRAWER, while it holds a window — a family of its own (`drawer`, in
    # every seed): a faded green at the device's rung, the owner's request.
    # Not the selection's green, which is the outline's and louder.
    drawer = "drawer.text";
  };
}
