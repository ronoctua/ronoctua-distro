#!/usr/bin/env python3
"""Seeds and recipes in, one palette per theme out — every colour SOLVED.

`assets/tokens/colors.nix` no longer holds a colour. It holds RECIPES: each
token names a primitive of a family (`text.primary = "neutral.text-strong"`),
optionally with an alpha or a chroma scale. The primitives are solved here,
per theme, from the theme's SEEDS (`assets/themes/<id>.nix`: hues, relative
chromas, contrast shifts) through one LADDER that is the same for every theme.
So a theme is a dozen numbers, the ladder is the palette the system was
designed on made into a rule (each table says which hand value it was read
from), and the palette is arithmetic — and every relationship the palette
used to promise in prose is a GATE below, measured on every theme at every
build.

# The ladder

Every family has fourteen named steps (names, never numbers: a WIT identifier
starts with a letter). Four PAINT steps climb from the footer's grey by a
lightness separation — a ground, a band, a border, a strong border — with a
low relative chroma and the family's hue TORSION applied (a hue as a ground is
not the same hue as a solid). Three SOLID steps are solved to an APCA contrast
against the footer's grey: `solid` (Lc 52), `solid-hover` (60) and `vivid`
(44 at full chroma), each plus the family's `shift` — where that hue sits;
yellows read lighter, reds darker. Six TEXT steps are solved to Lc 11, 23,
48, 79, 87 and 100 at the family's text chroma. `on-solid` is the text that
goes ON the solid: the ground if it reads at Lc 45 there, else the strongest
text. The neutral family has the two fixed anchors (`ground`, `chrome`) and
climbs from `chrome` in the same way.

Why the footer's grey is the reference for everything: it is what nearly
everything is drawn on. The buffer window and the black ground are GATED
against, not solved against.

# What comes out

`palettes.bin` — what the binaries draw with, read at run time
(`ronoctua_tokens::palettes`, `RONOCTUA_THEMES`): a header (the theme ids in
the chooser's order, the default, each theme's mode, the pages, and an FNV-1a
hash of the role names) and then every theme's every page's every role as
RGBA, roles in the order of `--roles` — the list the Rust is compiled against.

`themes.json` — `[{"id", "mode", "colors": {flat key: "0xRRGGBBAA"}}, …]` in
the order of `assets/themes/themes.nix` (the default is declared apart, by
name); every theme has the identical key set or this exits 1. Every theme carries its
`pages` too: the same roles solved on each desk (`PAGES`, `Theme.on_page`).
Per theme: `<id>.json` (the flat map of the base), `<id>.<page>.json` (each
desk's page), `<id>.primitives.json` (every primitive with L, C, h, its anchor and
any shortfall), the fragments consumers outside the shell read
(`<id>/compositor-*`, `<id>/scroll-<desk>-<borders>.conf`,
`<id>/ghostty-<ground>.conf`), and `report.md` + `report.json` with every
gate's margin and the regression against the frozen reference.

Any Fail exits 1 with the finding on stderr. Warn and Note are in the report.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import os
import pathlib
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import colorspace as cs  # noqa: E402

# ---------------------------------------------------------------------------
# The ladder. One table; every theme is solved through it.
#
# WHERE THE NUMBERS COME FROM. They are the `owl` palette's own measurements
# (2026-09-05, frozen in `assets/themes/owl.reference.nix`), made into the rule
# every theme follows — not the reference project's dark separations (which
# climb from a single `bg-app`; this system has a black ground AND a grey
# footer, and its greys were already spaced by hand). Each table says which
# hand value each row was read from; a row read from nothing says so.
# ---------------------------------------------------------------------------

#: Paint steps of a chromatic family: (name, ΔL from the chrome, relative chroma).
#: Read from owl: `ground` is `brain.background` (ΔL 0.044, rel C 0.20),
#: `element` is `surface.selection` (0.102, 0.39), `border` is the step the
#: window's inactive outline was read from when it was still a hand value
#: (0.178, 0.46). `border-strong` was on no screen: it is the
#: next rung at the neutral ladder's own spacing (rule.strong sits at 0.178
#: above the chrome, so a family's strong border sits one step above its
#: border), with the chroma continuing the row.
PAINT_STEPS = (
    ("ground", 0.044, 0.20),
    ("element", 0.102, 0.39),
    ("border", 0.178, 0.46),
    ("border-strong", 0.260, 0.50),
)
#: Solid steps: (name, Lc target before the family's shift, relative chroma or
#: None for the family's own). Read from owl: `solid` is `state.accent` (Lc
#: 52.1 on the chrome); `vivid` is `state.critical` (44.1, at the boundary);
#: `solid-hover` was on no screen and sits 8 Lc above the solid — the step the
#: reference keeps between its solid and its hover.
SOLID_STEPS = (
    ("solid", 52.0, None),
    ("solid-hover", 60.0, None),
    ("vivid", 44.0, 1.0),
)
#: Text steps: (name, Lc target before the family's text shift). Read from
#: owl: `text-faint` is `text.subtle.faint` (10.7, rounded — read WITHOUT
#: undoing the register's −3 shift, which is why that key is the one listed
#: deviation in `owl.reference.nix`), `text-muted` is `text.dim` (22.8),
#: `text` is `text.secondary` (48.1),
#: `text-loud` is `brain.countdown` (75.9) and the wave (78), `text-strong` is
#: `text.primary` (86.6), `text-bright` is `brain.text` (101).
TEXT_STEPS = (
    ("text-faint", 11.0),
    ("text-muted", 23.0),
    ("text", 48.0),
    ("text-loud", 79.0),
    ("text-strong", 87.0),
    ("text-bright", 100.0),
)
#: The neutral family's paint steps above the chrome: (name, ΔL from the
#: chrome). Owl's greys, byte for byte: the buffer window, the modeline, the
#: hover row, the notification band, the rule and the strong rule.
#: THE NEUTRAL STEPS THAT ARE A BACKGROUND, and not paint drawn ON one.
#:
#: They take the seed's `ground-tint` instead of its `tint`, and the division
#: is the answer to a question the owner asked of `forest`: make the grounds
#: greener — *"quero que esse tema seja mais verde nos backgrounds, como os
#: das janelas dos app, minibuffer e modeline"*. One tint for the whole ramp
#: cannot do it. In a theme whose greys and whose ACCENT share a hue (forest
#: is 150° on both), chroma in the greys walks them straight at the accent,
#: and the first thing to give is `surface.selection` against `surface.hover`
#: and `rule.default` — measured, those two sit at ΔE 0.063 and 0.054 with a
#: floor of 0.05, while the same band against `surface.chrome` has 0.115.
#:
#: Which is the palette telling us something true: a GROUND is where the theme
#: is allowed to be the theme, and a BAND or a RULE is a signal drawn on top
#: of it, so its colour has to stay its own. The tint falls off as the ladder
#: climbs out of the background and into the signals.
NEUTRAL_GROUNDS = frozenset({"ground", "chrome", "window", "chrome-raised", "chrome-loud", "chrome-band", "near"})
NEUTRAL_PAINT = (
    ("window", 0.018),
    ("chrome-raised", 0.053),
    ("hover", 0.070),
    ("chrome-loud", 0.078),
    # THE BAND ACROSS THE TOP OF A PAGE, and it is a step of its own because
    # `chrome-loud` could not move.
    #
    # `chrome-loud` sits 0.008 above `hover`, and until 2026-09-20 nothing
    # measured the two against each other: the GROUNDS climb in one chain
    # (`ground → chrome → window → chrome-raised → chrome-loud`) and the
    # SIGNALS in another (`chrome → hover → border → border-strong`), and the
    # ladder gate walks each chain on its own. Where the two chains CROSS, no
    # rule looked — and they cross here. Measured across the nine themes, a
    # header and a hover band were 0.0043 apart in darkside: two bands on one
    # page, the same colour, one meaning "this is the page's name" and the
    # other "your pointer is here".
    #
    # `chrome-loud` could not be the thing that moved, because it is also
    # `notification.background` and `glance.ground`, and the first of those is
    # one of the 52 keys frozen in `owl.reference.nix`. So the header takes a
    # rung of its own, 0.017 above it — the same size of step that already
    # separates a container from a header, and which the page reads as two
    # surfaces rather than one.
    #
    # It sits BELOW `border` by 0.008, which is the other half of this change:
    # a rule drawn on this ground is `rule.raised`, never `rule.default`. See
    # `Role::RuleRaised`.
    ("chrome-band", 0.095),
    ("border", 0.103),
    ("border-strong", 0.178),
)
#: The one neutral step BELOW the chrome: the desk the compositor paints where
#: no window is, when the person asks for the theme's own instead of the
#: absolute one (`LEADER s t`). The window's step, mirrored: a window stands
#: one step above the chrome, the desk one step below it, so a window on the
#: desk is two steps up and the footer's chrome one. Its hue and relative chroma are the
#: seed's `backdrop` — the only neutral step with a hue of its own, because
#: this is the one surface that is MEANT to read as the theme's colour — or
#: not to, which is why the STEP is the seed's too. A desk with no chroma has
#: only its lightness left to be told from the footer's chrome, and one step is
#: not enough of it: owl asked for a grey desk and, at the mirrored step, it
#: measured ΔE 0.026 from the chrome — legal and marginal, which is not a place
#: to leave a surface the eye is meant to read as another thing. And in a
#: light theme the step has no grey rung left to land between — the chrome's
#: signals are all below it — so there it is the CHROMA that tells the desk
#: from the bands drawn on it (see the light seeds).
NEUTRAL_BACKDROP = "backdrop"
#: THE OTHER DESK, and the one neutral step that is not solved at all: the
#: extreme the interface climbs AWAY from — black in a dark theme, white in a
#: light one — with no chroma, so it is `#000000` or `#FFFFFF` byte for byte.
#: It was a literal here, spelled as the option's name ("black"), and a light
#: theme made the name a lie: the pixel switched off is not the far end of a
#: ladder that descends from the paper. One word for both polarities, and the
#: mode says which extreme it is.
NEUTRAL_ABSOLUTE = "absolute"
#: THE THIRD DESK, NEAR THE ABSOLUTE ONE, and it is a light theme's: the
#: owner asked for a near-white between the chrome and the paper, in the
#: grounds' own hue and tint (2026-09-28). It sits `NEAR_SHARE` of the way
#: from the chrome to the ground — the paper in a light theme, and black in a
#: dark one, where it exists only because every theme has the same keys: the
#: chooser does not offer it there, so it is gated as a GROUND and never as
#: an option (`NEAR_GATES`).
#:
#: WHY IT IS NOT A SECOND SIDE OF `backdrop`: the coloured desk keeps its
#: place below the chrome, and the near-white is a desk of its own beside it,
#: so a light theme offers three and a dark one two.
NEUTRAL_NEAR = "near"
#: Halfway: a light chrome sits 0.043 below the paper, and each half of that
#: is over the ladder's minimum step (`LADDER_MIN_STEP`) on both sides.
NEAR_SHARE = 0.5
#: THE EDGE OF A WINDOW NOBODY IS LOOKING AT, when the person asked for the
#: borders to be discreet — one per desk, because it is measured against the
#: desk it lies on: `faint-<desk>` is that desk moved one faint step up the
#: interface's own ladder, in the desk's hue and chroma.
NEUTRAL_FAINT = {"faint-backdrop": NEUTRAL_BACKDROP, "faint-near": NEUTRAL_NEAR, "faint-absolute": NEUTRAL_ABSOLUTE}
#: HOW FAINT, and it is a LUMINANCE RATIO and not a ΔE — the one step in this
#: file solved against the WCAG ratio, which is written for text and does not
#: model it; an outline is not text.
#:
#: The owner's words were "extremely faint, and never transparent: the person
#: still needs a reference to resize with the mouse". ΔE_ok cannot say that at
#: both ends: its cube root has no linear toe, so the first byte above black is
#: already 0.067 away and a step the eye barely finds there measures 0.14,
#: while the same step off white measures 0.018. The ratio, with its flare of
#: 0.05, is one number at both extremes and on every grey between: 1.05 is
#: `#090909` on black, `#F9F9F9` on white, and about the ladder's own smallest
#: rung (the chrome against a window is 1.035 in owl) — the quietest step this
#: palette already asks the eye to see. `FAINT_EDGE_FLOOR` and
#: `FAINT_EDGE_CEILING` gate it from both sides.
FAINT_RATIO = 1.05
#: The neutral steps a page of the base does NOT solve again (`Theme.on_page`):
#: the ground and the desks, which are what a page stands on and beside.
NOT_ON_THE_PAGE = frozenset({"ground", NEUTRAL_BACKDROP, NEUTRAL_NEAR, NEUTRAL_ABSOLUTE, *NEUTRAL_FAINT})
#: How far under the most a page can pay its light ladder tops out
#: (`Theme._ceiling`): one of the ladder's small steps.
PAGE_CEILING_MARGIN = 2.0
#: Below this |Lc| of the ground on a solid, `on-solid` switches to the
#: strongest text instead.
ON_SOLID_FLOOR = 45.0

#: What a family may leave unsaid. `text-chroma` 0.35 is the brain family's
#: (its text steps were the only family text on screen). `chroma` 0.60 is a
#: CHOICE for a family that never defined a solid — quiet enough that a
#: register's unused solid does not land near the film colours (the `subtle`
#: family at 0.8 did) — and every family whose solid is on screen sets its own.
FAMILY_DEFAULTS = {
    "chroma": 0.60,
    "shift": 0.0,
    "torsion": 0.0,
    "paint-chroma": 1.0,
    "text-chroma": 0.35,
    "text-shift": 0.0,
}
FAMILY_FIELDS = frozenset(("hue", *FAMILY_DEFAULTS))
SEED_FIELDS = frozenset(("mode", "lift", "ground", "chrome", "neutral", "text", "backdrop", "chroma", "families", "overrides"))
#: Which way the ladder climbs. `dark` is the interface every theme was until
#: 2026-09-21: the ground is the darkest thing and every rung above it walks
#: toward white. `light` is the mirror — the ground is the paper and the rungs
#: walk toward black.
#:
#: DECLARED AND NOT DERIVED, which was the first thing tried. `ground` versus
#: `chrome` looks like it says the direction and does not: a light theme may
#: legitimately want the desk DARKER than the footer (a grey desk with white
#: windows on it), and a rule reading the pair would call that dark. A field
#: cannot be inferred wrong; it can only be written wrong, and then the gates
#: say so on the first build.
MODES = ("dark", "light")
#: THE LOUDEST AN APCA CONTRAST CAN HONESTLY BE ASKED FOR ON A LIGHT GROUND.
#:
#: APCA is not symmetric, and the asymmetry is the whole reason this constant
#: exists. Measured: the most contrast obtainable against a grey chrome is
#: 107.3 Lc at L 0.182 (owl's, white text) but only 96.1 at L 0.95 and 86.6 at
#: L 0.90 (black text). `_soft_clamp_black` lifts black text off zero, and the
#: BoW exponent is far more sensitive to a background stepping down from white
#: than the WoB one is to a background stepping up from black.
#:
#: So `TEXT_STEPS`' top rungs — 87 and 100, read off a DARK theme — are not
#: reachable on any light ground that is not white itself, and asking for them
#: returns `L = 0.0` with a shortfall: pure black text, and a ladder whose last
#: rung is twice the size of the one before it.
#:
#: AND IT IS A REMAP, NOT A CLAMP, which is the correction. Clamped at 90, the
#: top three rungs of every ladder (79, 87, 100) all landed on 90 — ΔE 0.0000
#: between them, thirty-four of them across the families, which is three names
#: for one colour. So the light ladder is the dark one's SHAPE laid into the
#: range this interface can pay: it runs from the theme's `lift` to this
#: ceiling, and each rung keeps its place along the way.
LIGHT_LC_CEILING = 96.0

#: Families the chrome spends, each of which gets a `context.*` group.
CONTEXT_FAMILIES = ("accent", "success", "warning", "error", "group", "subtle", "alternate", "brain")
#: Families that only content (the syntax palette) spends.
CONTENT_FAMILIES = ("slate", "sky", "azure", "cyan", "orange")
REQUIRED_FAMILIES = CONTEXT_FAMILIES + CONTENT_FAMILIES

#: THE FAMILIES DRAWN ON A FILM, and never on a page.
#:
#: A subtitle lane is read over a picture whose colours are anybody's, with an
#: outline round every letter — not on the chrome, a desk or a window. So its
#: colours are solved against ONE FIXED PICTURE (`PICTURE_GROUND_L`) and not
#: against the theme's chrome: the theme's `lift`, `chroma` multiplier and
#: page play no part, and a lane is the same colour in every theme and on
#: every page (`IDENTICAL_IN_EVERY_THEME`, `IDENTICAL_ON_EVERY_PAGE`). The
#: player's daemon, which draws them into the film with no door open and
#: knows no theme, resolves them itself.
#:
#: A FAMILY IN EVERY SEED all the same, which is the rule every role follows
#: (a seed without it is refused `no family 'subtitle' in the seed`), and the
#: same one in every seed, which the build refuses otherwise. Only `hue` (lane
#: one's; the other five are spaced from it) and `chroma` (the share of the
#: gamut every lane takes) mean anything on a film.
PICTURE_FAMILIES = ("subtitle",)
PICTURE_FIELDS = frozenset(("hue", "chroma"))
#: The picture the lanes are solved against: achromatic, at the lightness of
#: a dark scene — where a subtitle is hardest to lose and easiest to make
#: garish. A bright scene is the outline's job (`SUBTITLE_ON_EDGE`).
PICTURE_GROUND_L = 0.25
#: How many lanes one picture carries (`ronoctua_player::session::
#: SUBTITLES_PER_PICTURE`), each `subtitle.lane-<word>`.
SUBTITLE_LANES = 6
#: Every lane at ONE contrast on the picture — the colour is what tells two
#: lanes apart, and a ramp would let them differ mostly by how light they are
#: (the cursors' argument). 75 is body text's floor on a page, measured
#: without the outline; with it every letter has a black rim besides.
SUBTITLE_LC = 75.0
#: The lightness the six hues are spaced at, in equal ΔE arcs from the
#: family's hue (`cs.even_delta_e_hues`): where the solved lanes land.
SUBTITLE_MEASURE_L = 0.85
#: The outline, `subtitle.edge`: black. A rim that reads on a white sky and
#: melts into a night.
SUBTITLE_EDGE_L = 0.0
#: The five whose hues have to stay apart on the footer.
CHROME_HUES = ("accent", "success", "warning", "error", "group")
#: The four solids the colour-vision report compares.
STATE_SOLIDS = ("accent", "success", "warning", "error")

#: The accent hues that collapse onto red under protanopia (measured in the
#: reference project, 2.5° steps: worst at 165°, 0.0018 apart).
PROTAN_BAND = (158.0, 172.0)

#: The regression tolerance: a generated value further than this (ΔE_ok, one
#: and a half just-noticeable differences) from the frozen reference fails,
#: unless the reference lists the key under `deviations` with its own ceiling
#: and reason. The list lives beside the reference it annotates, so this file
#: knows no theme by name.
REGRESSION_TOLERANCE = 0.03

#: The colours a test film is drawn in (ffmpeg's `testsrc2`), which a
#: photograph (`nix run .#shot`) looks for on screen to find the film: no token
#: may come near them. Distance is Euclidean in encoded RGB, divided by √3; a
#: scene matches with an 18% fuzz, so a quarter keeps a margin over it, and the
#: content floor sits just above the fuzz itself.
FILM_COLOURS = ("#FF00FF", "#00FFFF", "#FFFF00", "#FF6000")
FILM_FLOOR, FILM_CONTENT_FLOOR = 0.25, 0.18
#: Groups whose tokens are CONTENT (a code preview, a chart) and may come
#: closer to the film colours: a Warn, not a Fail. The terminal palette is
#: exempt outright — an ANSI palette has to contain a cyan and a yellow.
CONTENT_GROUPS = ("syntax", "chart", "level")

#: The number words the categorical and ordinal groups are named with — a WIT
#: segment has to start with a letter.
WORDS = ("zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten")

#: The WIT identifier rule, refused HERE with the key's name — the same
#: guard `gen-tokens.py` keeps, one wall earlier.
VALID_SEGMENT = re.compile(r"^[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*$")


class Refusal(Exception):
    """A seed, a recipe or a gate the build cannot accept."""


# ---------------------------------------------------------------------------
# A solved colour.
# ---------------------------------------------------------------------------


class Swatch:
    """One primitive: its OKLCH after mapping, its bytes, and how it was found."""

    __slots__ = ("lch", "rgb8", "anchor", "shortfall")

    def __init__(self, lch, rgb8, anchor: str, shortfall: float | None = None):
        self.lch = lch
        self.rgb8 = rgb8
        self.anchor = anchor
        self.shortfall = shortfall

    @property
    def rgb(self):
        return cs.dequantize(self.rgb8)

    @property
    def lab(self):
        return cs.rgb_to_oklab(self.rgb)

    def describe(self) -> dict:
        l, c, h = self.lch
        return {
            "hex": cs.format_hex(self.rgb8),
            "l": round(l, 4),
            "c": round(c, 4),
            "h": round(h, 1),
            "anchor": self.anchor,
            "shortfall": None if self.shortfall is None else round(self.shortfall, 2),
        }


def _number(value, what: str, low: float, high: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise Refusal(f"{what} must be a number, got {value!r}")
    v = float(value)
    if not low <= v <= high:
        raise Refusal(f"{what} must be within [{low}, {high}], got {v}")
    return v


class Theme:
    """A theme's seeds, validated, and the primitives solved from them."""

    def __init__(self, id: str, seed: dict, source: str):
        self.id = id
        self.source = source
        if not isinstance(seed, dict):
            raise Refusal(f"{source}: the seed is not an attribute set")
        unknown = set(seed) - SEED_FIELDS
        if unknown:
            raise Refusal(f"{source}: unknown field(s) {sorted(unknown)}; the schema is {sorted(SEED_FIELDS)}")
        missing = SEED_FIELDS - set(seed)
        if missing:
            raise Refusal(f"{source}: missing field(s) {sorted(missing)}")
        self.mode = seed["mode"]
        if self.mode not in MODES:
            raise Refusal(f"{source}: mode is {self.mode!r}; it has to be one of {list(MODES)}")
        #: `+1` toward white, `-1` toward black. Every anchor in this file is
        #: written as a distance FROM the chrome, and this is what turns a
        #: distance into a direction — the one number the whole mirror hangs on.
        self.rising = 1.0 if self.mode == "dark" else -1.0
        self.ground = _number(seed["ground"], f"{source}: ground", 0.0, 1.0)
        self.chrome = _number(seed["chrome"], f"{source}: chrome", 0.0, 1.0)
        if not 0.0 < self.chrome < 1.0:
            raise Refusal(f"{source}: chrome must be strictly inside (0, 1)")
        if (self.chrome - self.ground) * self.rising <= 0.0:
            toward = "lighter" if self.rising > 0 else "darker"
            raise Refusal(
                f"{source}: in a {self.mode} interface the chrome ({self.chrome}) has to be {toward} "
                f"than the ground ({self.ground}) — the ladder climbs from it"
            )
        for group, fields in (
            ("neutral", ("hue", "tint", "ground-tint")),
            ("text", ("hue", "chroma")),
            ("backdrop", ("hue", "chroma", "step")),
        ):
            if not isinstance(seed[group], dict) or set(seed[group]) != set(fields):
                raise Refusal(f"{source}: {group} must be an attribute set with exactly {list(fields)}")
        self.neutral_hue = _number(seed["neutral"]["hue"], f"{source}: neutral.hue", 0.0, 360.0) % 360.0
        self.tint = _number(seed["neutral"]["tint"], f"{source}: neutral.tint", 0.0, 1.0)
        #: The tint of the steps a window, the footer and the modeline are
        #: PAINTED ON — see `NEUTRAL_GROUNDS` for why it is a second number.
        self.ground_tint = _number(seed["neutral"]["ground-tint"], f"{source}: neutral.ground-tint", 0.0, 1.0)
        self.text_hue = _number(seed["text"]["hue"], f"{source}: text.hue", 0.0, 360.0) % 360.0
        self.text_chroma = _number(seed["text"]["chroma"], f"{source}: text.chroma", 0.0, 1.0)
        self.backdrop_hue = _number(seed["backdrop"]["hue"], f"{source}: backdrop.hue", 0.0, 360.0) % 360.0
        self.backdrop_chroma = _number(seed["backdrop"]["chroma"], f"{source}: backdrop.chroma", 0.0, 1.0)
        #: How far BELOW the chrome the desk sits, in L. A theme that paints it
        #: grey needs more of this than one that paints it a colour: with no
        #: chroma, the lightness is the whole of what tells it from the chrome.
        self.backdrop_step = _number(seed["backdrop"]["step"], f"{source}: backdrop.step", 0.0, 1.0)
        if not 0.0 < self.backdrop_step:
            raise Refusal(f"{source}: backdrop.step has to be positive — it is a DISTANCE, and the mode gives it its sign")
        # The desk is `chrome − step` in both interfaces (see NEUTRAL_BACKDROP),
        # so what it must not reach is the darkest thing the theme has: the
        # ground in a dark interface, and the bottom of the range in a light one.
        floor = self.ground if self.rising > 0.0 else 0.0
        if self.chrome - self.backdrop_step <= floor:
            raise Refusal(
                f"{source}: backdrop.step ({self.backdrop_step}) takes the desk to or past {floor} — "
                "a desk the eye cannot tell from the darkest thing on the screen"
            )
        self.multiplier = _number(seed["chroma"], f"{source}: chroma", 0.0, 2.0)
        #: EVERY Lc ANCHOR IN THIS THEME, LIFTED — the counterpart of `chroma`
        #: one line up, and the first light theme is what asked for it.
        #:
        #: APCA compresses the whole ladder near white. Lc 23 above owl's
        #: chrome costs ΔL 0.33; below a chrome at L 0.955 it costs ΔL 0.135 —
        #: while the surface steps are a fixed ΔL in both. So on a light ground
        #: the raised grounds eat the room every solved tone had, and measured,
        #: `text.dim` on `surface.header` came to Lc 0.0000: text that is not
        #: there. Forty-seven tones failed that way, spread over every family.
        #:
        #: IT IS THE THEME'S AND NOT THE RAMP'S, which was the first attempt.
        #: Putting it under `text` lifted the six neutral rungs and left the
        #: seventeen families where they were — and what stayed failing was
        #: `state.accent`, `text.group`, `state.error`, `text.subtle.*`, every
        #: one of them a family tone on a raised ground. The compression is a
        #: property of the POLARITY, so it is answered once, where every anchor
        #: passes, rather than seventeen times over in a seed.
        self.lift = _number(seed["lift"], f"{source}: lift", -60.0, 60.0)
        if not isinstance(seed["overrides"], dict):
            raise Refusal(f"{source}: overrides must be an attribute set")
        self.overrides: dict = seed["overrides"]

        families = seed["families"]
        if not isinstance(families, dict):
            raise Refusal(f"{source}: families must be an attribute set")
        self.families: dict[str, dict[str, float]] = {}
        #: The families drawn on a film (`PICTURE_FAMILIES`): apart from
        #: `families`, so no ladder, gate or report of a page ever reads one.
        self.pictures: dict[str, dict[str, float]] = {}
        for name, fields in families.items():
            if not VALID_SEGMENT.match(name):
                raise Refusal(f"{source}: the family name {name!r} is not a WIT identifier")
            if not isinstance(fields, dict) or "hue" not in fields:
                raise Refusal(f"{source}: family {name!r} needs at least a hue")
            if name in PICTURE_FAMILIES:
                unknown = set(fields) - PICTURE_FIELDS
                if unknown or "chroma" not in fields:
                    raise Refusal(
                        f"{source}: the picture family {name!r} is a hue and a chroma and nothing else "
                        f"(unknown: {sorted(unknown)}) — it is solved on a film, where no page or ladder reaches"
                    )
                self.pictures[name] = {
                    "hue": _number(fields["hue"], f"{source}: {name}.hue", 0.0, 360.0) % 360.0,
                    "chroma": _number(fields["chroma"], f"{source}: {name}.chroma", 0.0, 1.0),
                }
                continue
            unknown = set(fields) - FAMILY_FIELDS
            if unknown:
                raise Refusal(f"{source}: family {name!r} has unknown field(s) {sorted(unknown)}")
            resolved = dict(FAMILY_DEFAULTS)
            resolved["hue"] = _number(fields["hue"], f"{source}: {name}.hue", 0.0, 360.0) % 360.0
            for key in FAMILY_DEFAULTS:
                if key in fields:
                    low, high = (-60.0, 60.0) if key in ("shift", "text-shift") else (-180.0, 180.0) if key == "torsion" else (0.0, 4.0) if key == "paint-chroma" else (0.0, 1.0)
                    resolved[key] = _number(fields[key], f"{source}: {name}.{key}", low, high)
            self.families[name] = resolved
        absent = [f for f in REQUIRED_FAMILIES if f not in self.families]
        if absent:
            raise Refusal(f"{source}: the recipes need the families {absent}, and the seed does not define them")

        self._memo: dict[tuple[str, str, float], Swatch] = {}
        #: The base palette this one is a PAGE of, and which desk is its page
        #: — `None` and `"chrome"` for the base itself (`on_page`).
        self.base: Theme | None = None
        self.page_name = "chrome"
        self._page: Swatch | None = None

    # -- the page: the ground every role is solved against ----------------

    def on_page(self, desk: str) -> "Theme":
        """This theme solved again with `desk` as its PAGE — the chrome's
        place, which every text anchor, band and rule is solved against.

        The owner's answer (2026-09-28) to a footer or a window standing on a
        desk: the bands follow the ground. A hover row, a selection, a rule,
        the text itself are measured from the ground they are drawn on, so on
        a desk they are solved from the desk — not the chrome's tones laid on
        a colour they were never measured on. What must NOT move is taken
        from the base unchanged: the ground and the desks themselves (they
        are what the page IS), and the roles `IDENTICAL_ON_EVERY_PAGE` names.
        """
        page = copy.copy(self)
        page._memo = {}
        page.base = self
        page.page_name = desk
        page._page = self.primitive("neutral", desk)
        page.chrome = page._page.lch[0]
        return page

    # -- the chrome, the reference of every solve --------------------------

    def chrome_swatch(self) -> Swatch:
        return self.primitive("neutral", "chrome")

    def _rel(self, base: float, scale: float) -> float:
        return min(1.0, max(0.0, base * self.multiplier * scale))

    def _reach(self, lc: float) -> float:
        """An anchor as this theme actually asks for it.

        A dark interface is asked for the Lc that is written down, plus the
        theme's lift. A light one has a SHORTER range to spend (see
        `LIGHT_LC_CEILING`), so the ladder is laid into it proportionally —
        the quiet end comes up off the grounds that were eating it, the loud
        end comes down to what the interface can actually pay, and the six
        rungs stay six.
        """
        if self.rising > 0.0:
            return lc + self.lift
        return self.lift + (self._ceiling() - self.lift) * lc / 100.0

    def _ceiling(self) -> float:
        """The top of a light ladder: `LIGHT_LC_CEILING`, or — on a page
        darker than the chrome, where black ink cannot pay for the SECOND
        highest rung — lowered until it can, so the two top rungs stay two
        instead of one black (measured: on a desk page at L 0.907 every
        family's `text-strong` and `text-bright` were the same `#000000`).
        Only as far as that: every rung lowered is contrast taken from every
        ink on the page."""
        if self.base is None:
            return LIGHT_LC_CEILING
        assert self._page is not None
        reachable = cs.apca_magnitude((0.0, 0.0, 0.0), self._page.rgb) - PAGE_CEILING_MARGIN
        second = TEXT_STEPS[-2][1] / 100.0
        return min(LIGHT_LC_CEILING, self.lift + (reachable - self.lift) / second)

    def _toward(self, delta: float) -> float:
        """A distance from the chrome, signed by the mode — what the anchor PRINTS.

        Without this the report says `chrome -0.05` for a primitive that sits
        at `chrome +0.05`, which is the audit trail disagreeing with the pixel.
        """
        return delta * self.rising

    def _solve_text(self, hue: float, rel: float, lc: float, anchor: str) -> Swatch:
        chrome = self.chrome_swatch()
        l, shortfall = cs.solve_lightness(
            ("apca", self._reach(lc)), lambda _: hue, rel,
            reference_lightness=chrome.lch[0], reference_rgb=chrome.rgb, direction=self.rising,
        )
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        solved = Swatch(lch, rgb8, anchor, shortfall)
        if self.base is None:
            return solved
        # ON A PAGE, NEVER WEAKER THAN THE BASE. An ink solved from a page
        # with MORE room than the chrome — white under a light theme, black or
        # a dark desk under a dark one — comes out paler, because it only has
        # to reach the same Lc; and the grounds that do not follow the page
        # (the notification band, the brain's line: `IDENTICAL_ON_EVERY_PAGE`)
        # then carry an ink weaker than the one they were measured with.
        # Measured on papyrus's white page: `text.primary` on the band 63.7
        # under a floor of 65. So a page takes whichever of the two reads more
        # on it — its own where it has LESS room than the chrome, the base's
        # where it has more.
        kept = self.base._solve_text(hue, rel, lc, anchor)
        if cs.apca_magnitude(kept.rgb, chrome.rgb) > cs.apca_magnitude(solved.rgb, chrome.rgb):
            return Swatch(kept.lch, kept.rgb8, f"{anchor}, the base's: it reads more on the {self.page_name} page", kept.shortfall)
        return solved

    def _paint(self, hue: float, rel: float, delta: float, anchor: str) -> Swatch:
        if self.base is not None:
            return self._paint_on_the_page(hue, rel, delta, anchor)
        l, shortfall = cs.solve_lightness(
            ("delta-l", delta), lambda _: hue, rel, reference_lightness=self.chrome, direction=self.rising
        )
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        return Swatch(lch, rgb8, anchor, shortfall)

    def _paint_on_the_page(self, hue: float, rel: float, delta: float, anchor: str) -> Swatch:
        """A paint step on a desk's page: the SAME STEP OF LIGHT the base takes
        from its chrome, taken from the page.

        Not the same step of lightness. A paint step is a ΔL from the chrome,
        which is right where the base's chrome is — a grey the ladder was
        designed on — and wrong at the ends: on a black page the window's
        0.018 is the byte 0, and on a white one a hover band's 0.07 is a
        smaller step of light than it is off a mid grey. A LUMINANCE RATIO,
        with the flare the faint outline is solved with (`FAINT_RATIO`), is
        one measure at both ends: the page's hover band is as far from the
        page, in light, as the base's is from the chrome.
        """
        assert self.base is not None

        def luminance(l: float) -> float:
            return max(0.0, min(1.0, l)) ** 3

        base_from = self.base.chrome
        base_to = max(0.0, min(1.0, base_from + self.rising * delta))
        up = base_to >= base_from
        yf, yt = luminance(base_from), luminance(base_to)
        ratio = (max(yf, yt) + 0.05) / (min(yf, yt) + 0.05)
        yp = luminance(self.chrome)
        target = (yp + 0.05) * ratio - 0.05 if up else (yp + 0.05) / ratio - 0.05
        shortfall = None
        if not 0.0 <= target <= 1.0:
            target = min(1.0, max(0.0, target))
            shortfall = ratio
        l = target ** (1.0 / 3.0)
        # AND THE SAME COLOUR: the base's CHROMA, not its relative chroma. How
        # much chroma a hue can carry moves with lightness, so the same share
        # of it is a paler band on a white page than on the chrome — measured,
        # the selection came to 0.043 from the rule on papyrus's white page
        # (floor 0.05), the pair being told apart by colour alone.
        kept = cs.screen_colour(base_to, hue, rel)[0][1]
        room = cs.max_chroma(l, hue)
        lch, rgb8 = cs.screen_colour(l, hue, min(1.0, kept / room) if room > 0.0 else 0.0)
        return Swatch(lch, rgb8, f"{anchor}, as light on the {self.page_name} page", shortfall)

    # -- primitives --------------------------------------------------------

    def primitive(self, family: str, step: str, scale: float = 1.0) -> Swatch:
        """The solved colour of `family.step`, with the step's relative chroma
        multiplied by `scale` (a recipe's `chroma`)."""
        key = (family, step, scale)
        if key in self._memo:
            return self._memo[key]
        if family == "neutral":
            swatch = self._neutral(step, scale)
        elif family in PICTURE_FAMILIES:
            swatch = self._picture(family, step, scale)
        else:
            swatch = self._chromatic(family, step, scale)
        self._memo[key] = swatch
        return swatch

    def _picture(self, family: str, step: str, scale: float) -> Swatch:
        """A colour drawn ON A FILM (`PICTURE_FAMILIES`): solved against the
        fixed dark picture and nothing of this theme — not its chrome, its
        page, its `lift` or its `chroma` multiplier — so the same seed gives
        the same bytes in every theme and on every page."""
        if family not in self.pictures:
            raise Refusal(f"{self.source}: no family {family!r} in the seed")
        fam = self.pictures[family]
        if step == "edge":
            lch, rgb8 = cs.screen_colour(SUBTITLE_EDGE_L, fam["hue"], 0.0)
            return Swatch(lch, rgb8, f"fixed L {SUBTITLE_EDGE_L:g}, no chroma: the rim of every lane")
        lanes = [f"lane-{WORDS[i + 1]}" for i in range(SUBTITLE_LANES)]
        if step not in lanes:
            raise Refusal(f"{self.source}: the picture family {family!r} has no step {step!r}; it has {lanes} and 'edge'")
        rel = min(1.0, fam["chroma"] * scale)
        hue = cs.even_delta_e_hues(SUBTITLE_LANES, fam["hue"], SUBTITLE_MEASURE_L, rel)[lanes.index(step)]
        picture = cs.screen_colour(PICTURE_GROUND_L, 0.0, 0.0)[1]
        l, shortfall = cs.solve_lightness(
            ("apca", SUBTITLE_LC), lambda _: hue, rel,
            reference_lightness=PICTURE_GROUND_L, reference_rgb=cs.dequantize(picture), direction=1.0,
        )
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        return Swatch(lch, rgb8, f"Lc {SUBTITLE_LC:g} on the picture's L {PICTURE_GROUND_L:g}, h {hue:.0f}", shortfall)

    def _neutral(self, step: str, scale: float) -> Swatch:
        if self.base is not None:
            # A PAGE OF THE BASE: the chrome is the desk, and the steps that are
            # not solved from the chrome are the base's own.
            if step == "chrome":
                assert self._page is not None
                return self._page
            if step in NOT_ON_THE_PAGE:
                return self.base.primitive("neutral", step, scale)
        hue = self.neutral_hue
        tint = self._rel(self.ground_tint if step in NEUTRAL_GROUNDS else self.tint, scale)
        if step == "ground":
            lch, rgb8 = cs.screen_colour(self.ground, hue, tint)
            return Swatch(lch, rgb8, f"fixed L {self.ground}")
        if step == "chrome":
            lch, rgb8 = cs.screen_colour(self.chrome, hue, tint)
            return Swatch(lch, rgb8, f"fixed L {self.chrome}")
        for name, delta in NEUTRAL_PAINT:
            if name == step:
                return self._paint(hue, tint, delta, f"chrome {self._toward(delta):+}")
        if step == NEUTRAL_BACKDROP:
            # A STEP DARKER THAN THE FOOTER, AND IN BOTH INTERFACES — which is
            # the one rung of this ladder that does NOT mirror.
            #
            # In a dark theme the desk sits between the ground and the chrome
            # and that is also the way away from the light. Mirrored literally,
            # a light theme would put it between the chrome and the paper — and
            # there is no room there: the chrome has to stay near white for the
            # text to be readable at all (see `LIGHT_LC_CEILING`), so the whole
            # gap is 0.045 and the desk measured ΔE 0.0155 from the paper. A
            # desk the eye cannot tell from the page is the thing the distance
            # gate exists to refuse.
            #
            # Written with the sign, the two collapse to one expression:
            # `chrome − step`, either way. Which is also what a desk IS —
            # every desktop ever made puts the windows on top of something
            # darker than they are.
            delta = -self.backdrop_step * self.rising
            return self._paint(self.backdrop_hue, self._rel(self.backdrop_chroma, scale), delta, f"chrome {self._toward(delta):+}, h {self.backdrop_hue:.0f}")
        if step == NEUTRAL_NEAR:
            l = self.chrome + (self.ground - self.chrome) * NEAR_SHARE
            lch, rgb8 = cs.screen_colour(l, hue, tint)
            return Swatch(lch, rgb8, f"L {l:.4f}, {NEAR_SHARE:g} of the way from the chrome to the ground")
        if step == NEUTRAL_ABSOLUTE:
            extreme = 0.0 if self.rising > 0.0 else 1.0
            lch, rgb8 = cs.screen_colour(extreme, hue, 0.0)
            return Swatch(lch, rgb8, f"fixed L {extreme:g}, no chroma")
        if step in NEUTRAL_FAINT:
            return self._faint(NEUTRAL_FAINT[step], scale)
        for name, lc in TEXT_STEPS:
            if name == step:
                return self._solve_text(self.text_hue, self._rel(self.text_chroma, scale), lc, f"Lc {lc} on chrome")
        raise Refusal(f"{self.source}: the neutral family has no step {step!r}")

    def _faint(self, desk_step: str, scale: float) -> Swatch:
        """`desk_step` moved up the interface's ladder until its luminance is
        `FAINT_RATIO` away — measured on the bytes, as the APCA solve is."""
        desk = self.primitive("neutral", desk_step)
        if desk_step == NEUTRAL_BACKDROP:
            hue, rel = self.backdrop_hue, self._rel(self.backdrop_chroma, scale)
        elif desk_step == NEUTRAL_NEAR:
            hue, rel = self.neutral_hue, self._rel(self.ground_tint, scale)
        else:
            hue, rel = self.neutral_hue, 0.0

        def ratio(l: float) -> float:
            return cs.wcag_ratio(cs.dequantize(cs.screen_colour(l, hue, rel)[1]), desk.rgb)

        near, far = desk.lch[0], 1.0 if self.rising > 0.0 else 0.0
        shortfall = None
        if ratio(far) < FAINT_RATIO:
            near, shortfall = far, ratio(far)
        else:
            for _ in range(40):
                middle = (near + far) / 2.0
                if ratio(middle) < FAINT_RATIO:
                    near = middle
                else:
                    far = middle
            near = far
        lch, rgb8 = cs.screen_colour(near, hue, rel)
        return Swatch(lch, rgb8, f"{desk_step} × {FAINT_RATIO} in luminance", shortfall)

    def _chromatic(self, family: str, step: str, scale: float) -> Swatch:
        if family not in self.families:
            raise Refusal(f"{self.source}: no family {family!r} in the seed")
        fam = self.families[family]
        hue = fam["hue"]
        for name, delta, rel in PAINT_STEPS:
            if name == step:
                # The torsion turns the PAINT steps only: a ground is not a solid.
                turned = (hue + fam["torsion"]) % 360.0
                return self._paint(turned, self._rel(rel * fam["paint-chroma"], scale), delta, f"chrome {self._toward(delta):+}, h {turned:.0f}")
        for name, lc, rel in SOLID_STEPS:
            if name == step:
                target = lc + fam["shift"]
                return self._solve_text(hue, self._rel(fam["chroma"] if rel is None else rel, scale), target, f"Lc {target:g} on chrome")
        for name, lc in TEXT_STEPS:
            if name == step:
                target = lc + fam["text-shift"]
                return self._solve_text(hue, self._rel(fam["text-chroma"], scale), target, f"Lc {target:g} on chrome")
        if step == "on-solid":
            solid = self.primitive(family, "solid")
            ground = self.primitive("neutral", "ground")
            if cs.apca_magnitude(ground.rgb, solid.rgb) >= ON_SOLID_FLOOR:
                return Swatch(ground.lch, ground.rgb8, f"neutral.ground (Lc {cs.apca_magnitude(ground.rgb, solid.rgb):.0f} on solid)")
            strong = self.primitive("neutral", "text-strong")
            return Swatch(strong.lch, strong.rgb8, "neutral.text-strong (the ground would not read on the solid)")
        raise Refusal(f"{self.source}: no step {step!r} in the ladder (family {family!r})")

    def every_primitive(self) -> dict[str, Swatch]:
        out: dict[str, Swatch] = {}
        for step in ("ground", "chrome", NEUTRAL_BACKDROP, NEUTRAL_NEAR, NEUTRAL_ABSOLUTE, *NEUTRAL_FAINT, *(n for n, _ in NEUTRAL_PAINT), *(n for n, _ in TEXT_STEPS)):
            out[f"neutral.{step}"] = self.primitive("neutral", step)
        for family in self.families:
            for step in (*(n for n, _, _ in PAINT_STEPS), *(n for n, _, _ in SOLID_STEPS), *(n for n, _ in TEXT_STEPS), "on-solid"):
                out[f"{family}.{step}"] = self.primitive(family, step)
        for family in self.pictures:
            for step in (*(f"lane-{WORDS[i + 1]}" for i in range(SUBTITLE_LANES)), "edge"):
                out[f"{family}.{step}"] = self.primitive(family, step)
        return out


# ---------------------------------------------------------------------------
# Recipes: the tokens' names, pointing at primitives.
# ---------------------------------------------------------------------------


class Resolved:
    """A token: its bytes, alpha, and which primitive it came from."""

    __slots__ = ("rgb8", "alpha8", "source", "swatch")

    def __init__(self, swatch: Swatch, alpha8: int, source: str):
        self.swatch = swatch
        self.rgb8 = swatch.rgb8
        self.alpha8 = alpha8
        self.source = source

    @property
    def rgb(self):
        return cs.dequantize(self.rgb8)

    @property
    def lab(self):
        return cs.rgb_to_oklab(self.rgb)

    @property
    def token(self) -> str:
        return cs.format_token(self.rgb8, self.alpha8)


def flatten_recipes(obj: dict, prefix: str = "") -> dict[str, object]:
    """Nested attribute sets to dotted keys; a set with `of` in it is a recipe,
    not a group."""
    out: dict[str, object] = {}
    for key, value in obj.items():
        name = f"{prefix}.{key}" if prefix else key
        if isinstance(value, dict) and "of" not in value:
            out.update(flatten_recipes(value, name))
        else:
            out[name] = value
    return out


def resolve_recipe(theme: Theme, key: str, recipe, where: str) -> Resolved:
    if isinstance(recipe, str):
        recipe = {"of": recipe}
    if not isinstance(recipe, dict) or "of" not in recipe:
        raise Refusal(f"{where}: the recipe of `{key}` must be \"family.step\" or {{ of = …; }}, got {recipe!r}")
    unknown = set(recipe) - {"of", "alpha", "chroma"}
    if unknown:
        raise Refusal(f"{where}: the recipe of `{key}` has unknown field(s) {sorted(unknown)}")
    of = recipe["of"]
    if not isinstance(of, str) or "." not in of:
        raise Refusal(f"{where}: the recipe of `{key}` must name a primitive as `family.step`, got {of!r}")
    family, step = of.split(".", 1)
    scale = _number(recipe.get("chroma", 1.0), f"{where}: `{key}` chroma", 0.0, 4.0)
    alpha = _number(recipe.get("alpha", 1.0), f"{where}: `{key}` alpha", 0.0, 1.0)
    swatch = theme.primitive(family, step, scale)
    return Resolved(swatch, int(math.floor(alpha * 255.0 + 0.5)), of if scale == 1.0 else f"{of} × chroma {scale}")


def check_key(key: str, where: str) -> None:
    for segment in key.split("."):
        piece = re.sub(r"[^a-z0-9]+", "-", segment.lower()).strip("-")
        if not VALID_SEGMENT.match(piece):
            raise Refusal(f"{where}: the key `{key}` would not be a WIT identifier ({segment!r})")
    if re.search(r"shadow|elevat", key, re.I):
        raise Refusal(f"{where}: `{key}` — no shadow, no elevation (invariant 15)")


# ---------------------------------------------------------------------------
# The shapes: groups every theme gets for the contexts that do not exist yet.
# ---------------------------------------------------------------------------

CONTEXT_SHAPE = (("solid", "solid"), ("hover", "solid-hover"), ("ground", "ground"), ("border", "border"), ("on", "on-solid"), ("text", "text-strong"))
NEUTRAL_SHAPE = (
    ("surface", "chrome"),
    ("surface-raised", "chrome-raised"),
    ("fg", "text-strong"),
    ("fg-muted", "text-muted"),
    ("border", "border"),
    ("border-strong", "border-strong"),
)
TERMINAL = (
    ("background", "neutral", "window"),
    ("foreground", "neutral", "text-strong"),
    ("cursor", "accent", "solid"),
    ("cursor-text", "accent", "on-solid"),
    ("selection-background", "accent", "element"),
    ("selection-foreground", "neutral", "text-strong"),
    ("ansi.black", "neutral", "border-strong"),
    ("ansi.red", "error", "solid"),
    ("ansi.green", "success", "solid"),
    ("ansi.yellow", "warning", "solid"),
    ("ansi.blue", "accent", "solid"),
    ("ansi.magenta", "group", "solid"),
    ("ansi.cyan", "cyan", "solid"),
    ("ansi.white", "neutral", "text"),
    ("ansi.bright-black", "neutral", "text-muted"),
    ("ansi.bright-red", "error", "solid-hover"),
    ("ansi.bright-green", "success", "solid-hover"),
    ("ansi.bright-yellow", "warning", "solid-hover"),
    ("ansi.bright-blue", "accent", "solid-hover"),
    ("ansi.bright-magenta", "group", "solid-hover"),
    ("ansi.bright-cyan", "cyan", "solid-hover"),
    ("ansi.bright-white", "neutral", "text-strong"),
)
#: WHAT A LIGHT TERMINAL FLIPS, and it is the only table in this file that
#: names a polarity in its own WORDS.
#:
#: Four of the ANSI sixteen are greys, and the table above spends four rungs of
#: the neutral ramp on them: `border-strong` for black, `text-muted` for bright
#: black, `text` for white and `text-strong` for bright white. That ordering is
#: "away from a dark background", so on a light one it draws a `black` that is
#: a pale grey and a `white` that is nearly black — the two names inverted. The
#: mirror is the same four rungs read the other way.
#:
#: AND `bright` MEANS LESS CONTRAST, not a higher rung. `solid-hover` is Lc 60
#: against the solid's 52, which is lighter on a dark ground and DARKER on a
#: light one — a bright red darker than red. `vivid` is the step below (Lc 44)
#: at the family's full chroma, which on paper is what a bright colour is:
#: lighter than its base and more saturated.
TERMINAL_LIGHT = {
    "ansi.black": ("neutral", "text-strong"),
    "ansi.bright-black": ("neutral", "text"),
    "ansi.white": ("neutral", "text-muted"),
    "ansi.bright-white": ("neutral", "border-strong"),
    "ansi.bright-red": ("error", "vivid"),
    "ansi.bright-green": ("success", "vivid"),
    "ansi.bright-yellow": ("warning", "vivid"),
    "ansi.bright-blue": ("accent", "vivid"),
    "ansi.bright-magenta": ("group", "vivid"),
    "ansi.bright-cyan": ("cyan", "vivid"),
}


def terminal_table(theme: "Theme") -> tuple[tuple[str, str, str], ...]:
    """The ANSI sixteen for this interface: the dark table, with the entries a
    light one has to read the other way swapped in."""
    if theme.rising > 0.0:
        return TERMINAL
    return tuple((name, *TERMINAL_LIGHT.get(name, (family, step))) for name, family, step in TERMINAL)


#: The ANSI entry that is DELIBERATELY near the background, per polarity, and
#: is therefore the one the contrast gate below exempts. In a dark terminal it
#: is black; in a light one it is white, and exempting black there would gate
#: the ink and excuse the paper.
NEAR_THE_PAPER = {"dark": "ansi.black", "light": "ansi.white"}
#: THE TERMINAL'S BACKGROUND on each page: the base palette's own window, or
#: — on a desk's page, when the person puts the windows on the desk — the desk
#: itself, which is that page's `surface.chrome` (`ghostty-<page>.conf`, one
#: per page, each with that page's palette).
TERMINAL_BACKGROUND = {"chrome": "terminal.background", "backdrop": "surface.chrome", "near": "surface.chrome", "absolute": "surface.chrome"}
#: The fragment each page's terminal palette is written to.
TERMINAL_FRAGMENT = {"chrome": "ghostty-theme.conf", "backdrop": "ghostty-backdrop.conf", "near": "ghostty-near.conf", "absolute": "ghostty-absolute.conf"}
#: The ANSI order Ghostty's `palette = N=` wants.
ANSI_ORDER = ("black", "red", "green", "yellow", "blue", "magenta", "cyan", "white")
#: The categorical set: eight hues (the reference ships six by default and
#: measures eight as the most that stay a JND apart under dichromacy), spaced
#: at the lightness a chart's solid sits at (the accent's, 0.72) with the
#: reference's chart chroma less a tenth for the darker grounds here. Their
#: lightness climbs 0.50 → 0.90 with the index, the reference's device:
#: lightness is what survives dichromacy, hue is not.
CHART_COUNT, CHART_MEASURE_L, CHART_CHROMA = 8, 0.72, 0.75
#: The ordinal ramp climbs from the accent's ground to just under the text's
#: brightest step; its chroma is muted at both ends and richest in the middle,
#: the shape the reference gives every ramp ("backgrounds carrying full chroma
#: look garish and text carrying it looks cheap").
LEVEL_TOP_L = 0.90
#: THE CURSORS: one per seat, eight colours, and a ninth cursor reuses the
#: first (the owner's choice — past eight, colours stop being told apart at
#: the size of a cursor). The inside is BRIGHT and the outline DARK in both
#: polarities, because a cursor is drawn over anything — a photograph, a
#: terminal, the other theme's windows — and the pair that reads on every
#: ground is the classic one: light inside, dark rim. The hues are spaced at
#: equal ΔE from the accent, like the chart set, but at ONE lightness: here
#: the colour is the only thing telling two cursors apart, and a ramp would
#: let two of them differ mostly by how light they are.
CURSOR_COUNT, CURSOR_L, CURSOR_CHROMA, CURSOR_OUTLINE_L = 8, 0.80, 0.70, 0.16
#: THE SAME EIGHT IN EVERY THEME, from a fixed hue rather than the accent. A
#: cursor's colour is how a person tells WHICH HAND is which, and a theme
#: switch that repainted every cursor would unlearn that; it also makes the
#: cursor images eight builds instead of eight per theme (`nix/cursors.nix`).
#: Only the hover band is per theme — it lies on the theme's own surfaces.
CURSOR_START_HUE = 265.0
#: And the band each cursor's hover paints in the shell: its hue at the
#: lightness the neutral hover row already sits at, with enough chroma to say
#: WHOSE hover it is. PER POLARITY, measured: near black the chroma a hue can
#: carry is small, so a dark theme needs nearly all of it for eight bands to
#: be told apart (evilside's closest pair was ΔE 0.019 at 0.80); near white
#: the same share carries the yellow band onto the film's `#FFFF00`.
CURSOR_HOVER_CHROMA = {"dark": 0.95, "light": 0.60}
#: AND THE SELECTION each keyboard moves in one of our apps, when several
#: keyboards type into one window (`ronoctua_design::Carets`): its cursor's
#: hue at the lightness `surface.selection` sits at — the band that says
#: "the keyboard is here", once per keyboard. A single keyboard keeps the
#: accent's own selection, the rule the hover bands follow.
CURSOR_SELECTION_CHROMA = {"dark": 0.95, "light": 0.60}
#: AND THE MINIBUFFER'S OWNER BAND (`cursor.band.*`, read by the footer): the
#: cursor's hue at the hover row's lightness, like the hover band — but drained
#: to a WHISPER. The hover band is loud because eight of them may light eight
#: rows at once and each must be told from the others; this one is never more
#: than ONE band at a time (the minibuffer the owner's keyboard opened), so it
#: owes nothing to the other seven and everything to the grey it lies on. The
#: owner asked for "um azul bem desaturado e próximo do cinza usado no
#: minibuffer": a tone that reads as the same bar, leaning the cursor's way.
#: NOT gated against the other bands or the ground — the distinguishability the
#: hover band is held to (`CURSOR_HOVER_GAP`) is exactly what would make this
#: loud again, and with one cursor the footer paints no band at all.
CURSOR_BAND_CHROMA = {"dark": 0.22, "light": 0.14}


def shaped_tokens(theme: Theme) -> dict[str, Resolved]:
    out: dict[str, Resolved] = {}

    def put(key: str, swatch: Swatch, source: str) -> None:
        out[key] = Resolved(swatch, 255, source)

    for family in CONTEXT_FAMILIES:
        for name, step in CONTEXT_SHAPE:
            put(f"context.{family}.{name}", theme.primitive(family, step), f"{family}.{step}")
    for name, step in NEUTRAL_SHAPE:
        put(f"context.neutral.{name}", theme.primitive("neutral", step), f"neutral.{step}")
    for name, family, step in terminal_table(theme):
        put(f"terminal.{name}", theme.primitive(family, step), f"{family}.{step}")

    accent = theme.families["accent"]
    rel = min(1.0, CHART_CHROMA * theme.multiplier)
    # AND THE HUES ARE SPACED WHERE THE SET ACTUALLY SITS. `CHART_MEASURE_L` is
    # "the lightness a chart's solid sits at" — a dark theme's. A light one's
    # sit at the mirrored end, and chroma is not the same size there.
    measure = CHART_MEASURE_L if theme.rising > 0.0 else 1.0 - CHART_MEASURE_L
    hues = cs.even_delta_e_hues(CHART_COUNT, accent["hue"], measure, rel)
    for i, hue in enumerate(hues):
        # THE RAMP RUNS THE WAY THE INTERFACE DOES. Written as `0.50 + 0.40·t`
        # it climbed toward white in both polarities, and on a light ground the
        # top half of a categorical set is unreadable — measured, `chart.eight`
        # came to Lc 17.3 on white. In a dark theme `rising` is `+1` and the
        # expression is the one that was here, byte for byte.
        l = 0.50 + theme.rising * 0.40 * i / (CHART_COUNT - 1)
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        put(f"chart.{WORDS[i + 1]}", Swatch(lch, rgb8, f"h {hue:.0f}, L {l:.2f}, equal ΔE arcs from the accent"), "computed")

    cursor_rel = CURSOR_CHROMA
    cursor_hues = cs.even_delta_e_hues(CURSOR_COUNT, CURSOR_START_HUE, CURSOR_L, cursor_rel)
    hover_l = theme.primitive("neutral", "hover").lch[0]
    selection_l = theme.primitive("accent", "element").lch[0]
    polarity = "dark" if theme.rising > 0.0 else "light"

    def band(l: float, hue: float, rel: float, base_l: float | None) -> tuple:
        # ON A PAGE, THE BASE'S CHROMA — see `Theme._paint_on_the_page`: eight
        # bands told apart by hue alone lose that at a lightness with less room.
        if base_l is None:
            return cs.screen_colour(l, hue, rel)
        kept = cs.screen_colour(base_l, hue, rel)[0][1]
        room = cs.max_chroma(l, hue)
        return cs.screen_colour(l, hue, min(1.0, kept / room) if room > 0.0 else 0.0)

    base_hover_l = theme.base.primitive("neutral", "hover").lch[0] if theme.base is not None else None
    base_selection_l = theme.base.primitive("accent", "element").lch[0] if theme.base is not None else None
    for i, hue in enumerate(cursor_hues):
        lch, rgb8 = cs.screen_colour(CURSOR_L, hue, cursor_rel)
        put(f"cursor.fill.{WORDS[i + 1]}", Swatch(lch, rgb8, f"h {hue:.0f}, L {CURSOR_L:.2f}, equal ΔE arcs from the accent"), "computed")
        lch, rgb8 = band(hover_l, hue, CURSOR_HOVER_CHROMA[polarity], base_hover_l)
        put(f"cursor.hover.{WORDS[i + 1]}", Swatch(lch, rgb8, f"h {hue:.0f}, at the neutral hover's L {hover_l:.3f}"), "computed")
        lch, rgb8 = band(selection_l, hue, CURSOR_SELECTION_CHROMA[polarity], base_selection_l)
        put(f"cursor.selection.{WORDS[i + 1]}", Swatch(lch, rgb8, f"h {hue:.0f}, at the selection's L {selection_l:.3f}"), "computed")
        lch, rgb8 = band(hover_l, hue, CURSOR_BAND_CHROMA[polarity], base_hover_l)
        put(f"cursor.band.{WORDS[i + 1]}", Swatch(lch, rgb8, f"h {hue:.0f}, a whisper at the neutral hover's L {hover_l:.3f}"), "computed")
    lch, rgb8 = cs.screen_colour(CURSOR_OUTLINE_L, CURSOR_START_HUE, 0.0)
    put("cursor.outline", Swatch(lch, rgb8, f"achromatic, L {CURSOR_OUTLINE_L:.2f}"), "computed")

    bottom = theme.primitive("accent", "ground").lch[0]
    # AND THE FAR END OF THE ORDINAL RAMP IS MIRRORED WITH IT. `LEVEL_TOP_L` is
    # "just under the text's brightest step", which in a light interface is
    # just over its darkest. Left alone, both ends of a light ramp landed in
    # the same place: measured at a chrome of 0.95 the eleven levels spanned
    # 0.006 of L and every adjacent pair failed the ordinal gate.
    top = LEVEL_TOP_L if theme.rising > 0.0 else 1.0 - LEVEL_TOP_L
    for k in range(11):
        t = k / 10.0
        l = bottom + (top - bottom) * t
        rel_k = min(1.0, (0.3 + 0.6 * math.sin(math.pi * t)) * theme.multiplier)
        hue = (accent["hue"] + accent["torsion"] * (1.0 - t)) % 360.0
        lch, rgb8 = cs.screen_colour(l, hue, rel_k)
        put(f"level.{WORDS[k]}", Swatch(lch, rgb8, f"accent hue, L {l:.2f}, rel C {rel_k:.2f}"), "computed")
    return out


# ---------------------------------------------------------------------------
# Gates. Data first; the code only measures.
# ---------------------------------------------------------------------------

SIX_GROUNDS = ("surface.chrome", "modeline.background", "notification.background", "surface.hover", "surface.buffer", "surface.base")

#: THE TWO RAISED GROUNDS, which no gate measured at all until 2026-09-20.
#
# `surface.container` is what a region shaped like a card is filled with and
# `surface.header` is the band across the top of a buffer's page. Fifteen tones
# are drawn on one or the other in the nine themes, and NOTHING in this file
# had ever looked at one of them — the brain's page was the first screen built
# out of both, and what it showed was a rule on the header measuring ΔL 0.025,
# which is a line nobody can see.
#
# They are a tuple of their own and not appended to `SIX_GROUNDS` because that
# name is sliced (`[:5]`), and a list whose length is part of another rule is a
# list nobody may add to.
RAISED_GROUNDS = ("surface.container", "surface.header")

#: THE DESKS, AND THE PALETTE EACH ONE IS THE PAGE OF (2026-09-28).
#
# The person may put the footer and our windows on the desk's colour
# (`LEADER s t`). The first answer to that laid the chrome's own tones on the
# desk and gated every one of them there too, and it forced the light themes'
# desks into a colour: a grey desk sits among the chrome's bands, and a band
# on it could not be told from it. The owner chose the other way: THE BANDS
# FOLLOW THE GROUND. Each desk is the PAGE of a palette of its own
# (`Theme.on_page`) — the chrome's place taken by the desk, and every text,
# band and rule solved again from it — and a surface standing on a desk wears
# that palette. So no tone SOLVED for the base is drawn on a desk, and a desk
# is measured as the page of its palette, where it is `surface.chrome`. The
# roles copied from the base (`IDENTICAL_ON_EVERY_PAGE`) are the one way a
# base tone reaches a desk's page — the notification manager draws `read`,
# `suppressed` and `loud` on its own ground — and they are gated there like
# every other tone, since every page runs every contrast gate.
#
# THE ORDER IS THE RUNTIME TABLE'S (`ronoctua_tokens::theme::Ground`): the
# base palette first, whose page is the chrome.
PAGES = ("chrome", "backdrop", "near", "absolute")
#: The desks, by the page name each one gives its palette.
DESKS = {"backdrop": "surface.backdrop", "near": "surface.near", "absolute": "surface.absolute"}
#: THE ROLES THAT ARE THE SAME ON EVERY PAGE, copied from the base into each
#: page's palette. The notification band and the brain's line keep their own
#: grounds and inks whatever the footer stands on (the owner: they never change
#: with these options), and so do the glance's two stripes and the maximum
#: alert bar that takes the band's place; the desks are what
#: the pages ARE; the outlines are the compositor's, whose fragments carry the
#: base; and the subtitle lanes are drawn on a film, never on a page.
IDENTICAL_ON_EVERY_PAGE = ("notification.", "brain.", "glance.", "border.", "subtitle.", "alert.")
IDENTICAL_KEYS = frozenset(DESKS.values())
#: THE ROLES THAT ARE THE SAME IN EVERY THEME, byte for byte, or the build is
#: refused: the subtitle lanes, which the player draws into a film with no
#: door open and no theme to ask (`PICTURE_FAMILIES`). Their seeds must agree,
#: and this is what says so when one is edited alone.
IDENTICAL_IN_EVERY_THEME = ("subtitle.",)


def identical_on_every_page(key: str) -> bool:
    return key in IDENTICAL_KEYS or key.startswith(IDENTICAL_ON_EVERY_PAGE)


#: THE GATES THAT BELONG TO THE BASE ALONE, because they measure the chrome
#: against the ground and the desks it stands beside — the footer's chrome and
#: a window's page over the desk under them. On a page of a desk the "chrome"
#: IS that desk, and these would measure it against itself. Every other gate
#: runs on every page.
BASE_ONLY_PAIRS = frozenset(
    {
        ("surface.base", "surface.chrome"),
        ("surface.backdrop", "surface.chrome"),
        ("surface.backdrop", "surface.buffer"),
        ("surface.near", "surface.chrome"),
        ("surface.near", "surface.buffer"),
        ("surface.absolute", "surface.chrome"),
        ("surface.absolute", "surface.buffer"),
        # AND THE MODELINE'S OWN BAND: on a desk's page the modeline wears
        # the page (the owner: it has the desk's colour), so the page's
        # `modeline.background` IS its `surface.chrome` (`page_roles`).
        ("surface.chrome", "modeline.background"),
    }
)


def page_roles(page_tokens: dict[str, "Resolved"], base_tokens: dict[str, "Resolved"]) -> None:
    """What a page's palette says that is not solved from the page: the roles
    `IDENTICAL_ON_EVERY_PAGE` names, copied from the base; and the modeline's
    ground, which on a desk's page is the page itself."""
    for key in page_tokens:
        if identical_on_every_page(key):
            page_tokens[key] = base_tokens[key]
    page_tokens["modeline.background"] = page_tokens["surface.chrome"]

#: (text key, ground keys, Fail below, Warn below). Lc = |APCA|.
CONTRAST_GATES = (
    ("text.primary", (*SIX_GROUNDS, "surface.selection", *RAISED_GROUNDS), 75, 82),
    # THE ON-SCREEN KEYBOARD: every key's legend is `text.primary`, whatever
    # ground the key is on at that moment, and the small second legend (the
    # shifted and the AltGr character) is `text.dim` on the two resting grounds.
    ("text.primary", ("surface.key.cap", "surface.key.action", "surface.key.hover", "surface.key.on", "surface.key.locked"), 60, 70),
    ("text.dim", ("surface.key.cap", "surface.key.action"), 15, 20),
    ("text.secondary", (*SIX_GROUNDS, *RAISED_GROUNDS), 43, 45),
    ("text.secondary", ("surface.selection",), 40, 45),
    ("text.dim", (*SIX_GROUNDS[:5], *RAISED_GROUNDS), 15, 20),
    # ON THE BAND TOO: the file manager lifts its dim fields to secondary there
    # (`lift_onto_a_band`), but the notification manager's summary column and
    # the update window's step glyphs are dim on a banded row and lift nothing.
    ("text.dim", ("surface.selection",), 15, 20),
    ("state.accent", ("surface.selection",), 45, 48),
    # THE SHORTCUT LIST'S TWO COLOURED INKS HOLD `text.secondary`'S FLOORS,
    # because since 2026-09-21 they ARE its rung: `text.group` is `group.text`
    # and `text.key` is `accent.text`, both solved to Lc 48 on the chrome, the
    # same number the neutral ramp's `text` step is solved to.
    #
    # They are the SAME floors and not floors of their own on purpose: three
    # inks on one line, one rung, one promise. The day the ramp's rung moves,
    # all three move with it.
    #
    # THE OLD NUMBERS DID NOT DIE, THEY MOVED TO `text.heading` — which is the
    # other half of the same day's split. `text.group` was one name for two
    # jobs, a submenu in a list and the title of a section, and these floors
    # (45/50 on the band, 45/55 on the chrome) were always measuring the
    # title: they are written for `group.solid`, Lc 52 plus the family's
    # shift. The title kept that step and the gates came with it.
    ("text.group", ("surface.selection",), 40, 45),
    ("text.key", ("surface.selection",), 40, 45),
    # THE MATCHED LETTERS ON THE CURSOR ROW: they have to read on the list's
    # grounds and be told apart from the accent they sit in (distance below).
    ("text.match-here", (*SIX_GROUNDS[:4],), 60, 65),
    ("text.heading", ("surface.selection",), 45, 50),
    ("text.subtle.strong", ("surface.chrome",), 40, 45),
    ("text.subtle.default", ("surface.chrome",), 18, 20),
    ("text.subtle.faint", ("surface.chrome",), 7, 8),
    ("text.subtle.alternate", ("surface.chrome",), 38, 42),
    # THE KEY A CLICK CANNOT PRESS, and the key that is only a guest. The
    # second had no contrast gate at all; both are drawn on every hint line.
    # The guest sits on the muted rung by design (Lc 16,5-17,3 measured in
    # every theme), so its floor is the prose's, one step under what it gets.
    ("text.subtle.inert", ("surface.chrome", "surface.buffer"), 36, 38),
    ("text.subtle.lit.key", ("surface.chrome", "surface.buffer"), 38, 40),
    ("text.subtle.lit.prose", ("surface.chrome", "surface.buffer"), 15, 16),
    ("text.subtle.contextual", ("surface.chrome", "surface.buffer"), 15, 16),
    # AND ON THE TRANSPORT BAND, which is where the camera and the player draw
    # their header over a picture. Only `text.primary` was measured there.
    ("text.subtle.strong", ("surface.transport",), 38, 40),
    ("text.subtle.default", ("surface.transport",), 15, 18),
    ("text.subtle.faint", ("surface.transport",), 6, 7),
    ("text.subtle.alternate", ("surface.transport",), 36, 38),
    ("text.subtle.contextual", ("surface.transport",), 15, 16),
    ("text.subtle.explain", ("surface.transport",), 26, 28),
    ("text.subtle.inert", ("surface.transport",), 34, 36),
    # THE WINDOW'S HINT LINE draws the same register on the buffer's ground,
    # which is a hair lighter than the chrome by construction; the floors hold
    # and there is no warn band, because the hair is the ladder's own.
    ("text.subtle.strong", ("surface.buffer",), 40, 40),
    ("text.subtle.default", ("surface.buffer",), 18, 18),
    ("text.subtle.faint", ("surface.buffer",), 7, 7),
    ("text.subtle.alternate", ("surface.buffer",), 38, 38),
    # AND ON THE TWO RAISED GROUNDS, because a buffer's HEADER is where the
    # hint family is drawn now: the brain's header line carries its glyph, its
    # name and every shortcut it answers to, in `Role::Key` and `Role::Prose`,
    # which are this family. Not one of those four was measured against the
    # ground they are drawn on until the raised rungs were gated.
    #
    # The floors are `surface.buffer`'s, with no warn band, for the reason
    # written there: the difference between these grounds is the ladder's own
    # rung, and a warn band that fires on the rung itself is a warning about
    # the design rather than about a theme.
    ("text.subtle.strong", RAISED_GROUNDS, 40, 40),
    # THE RUNG COSTS ABOUT 1,6 Lc, AND THE FLOOR SAYS SO OUT LOUD. On the
    # chrome this tone is floored at 18 and measures 19-21; on a raised ground
    # it measures 16,0-17,4, because the ground climbed and the tone did not.
    # The floor here is not 18 lowered to fit — it is 15, which is a whole
    # step of the subtle ladder below what the rung gives, so a theme that
    # loses a step is still caught and the rung itself is not reported as a
    # defect every time this file is read.
    ("text.subtle.default", RAISED_GROUNDS, 15, 15),
    # `text.subtle.faint` IS NOT HERE, AND THAT IS THE FINDING.
    #
    # Measured on every one of the nine themes: on `surface.header` and on
    # `surface.container` it comes to Lc 0,0000 — APCA's way of saying there
    # is no contrast at all. The quietest step of the subtle ladder sits a
    # hair over the CHROME, and a raised ground eats the hair whole.
    #
    # So it is not floored here, it is made unreachable: `Pen` lifts
    # `Role::Faint` to `Role::Prose` inside a block whose ground was raised,
    # the same move `files::lift_onto_a_band` makes for a dim field on a band.
    # A buffer cannot draw invisible text by choosing the wrong role, and the
    # line above is what measures what it gets instead.
    ("text.subtle.alternate", RAISED_GROUNDS, 38, 38),
    # THE SENTENCE UNDER AN OPENED LINE, on every ground a line of shortcuts is
    # drawn on — the footer's chrome, a window's buffer ground and the brain's
    # raised header. It had only distance gates, which say it is not another
    # tone and nothing about whether it reads; a cool grey the owner asked to
    # be DISCREET is exactly the tone that could slide into not reading.
    ("text.subtle.explain", ("surface.chrome", "surface.buffer"), 30, 32),
    ("text.subtle.explain", RAISED_GROUNDS, 26, 26),
    # THE ACCENT AND THE ALARM ON THEM TOO. A container is where the brain
    # draws a run that FAILED and a header is where the accent marks the page,
    # so both are drawn on a raised ground on the first screen built out of
    # these two rungs.
    ("state.accent", RAISED_GROUNDS, 45, 45),
    # THE SHELF'S OWN GROUND. `surface.shelf` is the network view's box — a
    # list inside a list — and it is drawn under rows of `text.primary` and
    # `text.dim`. It was never measured because it resolves to
    # `neutral.window`, the same rung as `surface.buffer`, and "the same as
    # something that IS measured" is not a measurement: the day the rung moves
    # nothing would say so.
    ("text.primary", ("surface.shelf",), 75, 82),
    ("text.dim", ("surface.shelf",), 15, 20),
    # AND THE FILM'S CONTROLS, which are text on the one surface in the system
    # with an alpha. The gate measures it OPAQUE — its own hex over nothing —
    # which is the worst case it can be composited onto: the token is 90%
    # opaque over a frame of film, and a frame of film can be any colour.
    ("text.primary", ("surface.transport",), 60, 70),
    # AND THE RUBBER BAND, which is the system's SECOND alpha and the same
    # reasoning one line up: measured OPAQUE, its own hex over nothing, because
    # at 34% it is composited over a thumbnail and a thumbnail can be any
    # colour. What is read through it is what the band exists to say — the NAME
    # of each file it is about to take, and the dim columns beside it.
    #
    # MEASURED: the name reads 49-56 Lc and the dim columns 18,5-19,6 in the
    # three themes whose accent is darkest — over the warn line's 20 by a hair,
    # and the reason that is accepted rather than tuned is that the worst case
    # measured here CANNOT HAPPEN: 34% of the accent over the listing's own
    # ground is lighter than the accent alone, so what the eye gets is better
    # than what the gate reads. The floor is what matters, and it is 15.
    ("text.primary", ("surface.sweep",), 45, 55),
    ("text.dim", ("surface.sweep",), 15, 20),
    # A SWEEP OF TEXT lies under the letters it chose, and they have to go
    # on being read through it — the sweep's own floors, for the same reason.
    ("text.primary", ("surface.text-selection",), 45, 55),
    ("text.dim", ("surface.text-selection",), 15, 20),
    ("state.error", RAISED_GROUNDS, 45, 45),
    # 36 AND NOT 40 for the same reason as the subtle default above: measured
    # 38,2-39,5 on the header, which is the rung's own cost and not a theme
    # drifting.
    ("text.once", RAISED_GROUNDS, 36, 36),
    ("text.group", (*SIX_GROUNDS, *RAISED_GROUNDS), 43, 45),
    ("text.key", (*SIX_GROUNDS, *RAISED_GROUNDS), 43, 45),
    ("text.heading", RAISED_GROUNDS, 45, 45),
    ("text.heading", ("surface.chrome",), 45, 55),
    ("brain.text", ("brain.background",), 90, 95),
    ("brain.dim", ("brain.background",), 44, 46),
    ("brain.countdown", ("brain.background",), 60, 70),
    # THE CURSOR ON A LINE THAT APPEARS BY ITSELF: the action of the band's
    # card, and the control of the brain's strip, a click picked — the one the
    # next click runs. It is the cursor's own ink (`text.here.acts`), so it is
    # read where those two grounds put it, at the floor every cursor holds.
    ("text.here.acts", ("notification.background", "brain.background"), 45, 50),
    ("modeline.state.normal", ("modeline.background",), 15, 20),
    ("modeline.state.alternate", ("modeline.background",), 45, 60),
    # A BLUETOOTH DEVICE'S MARK, at the rung every other mark reads at.
    ("modeline.device", ("modeline.background",), 40, 45),
    # AND THE CAMERA'S, at the same rung.
    ("modeline.watching", ("modeline.background",), 40, 45),
    # AND ON A WINDOW'S GROUNDS, where the brain's page draws a watch in the
    # colour the modeline marks one with — its rows, a module's row toned
    # `watching` — on the page, under the cursor's band and the pointer's, and
    # on the two raised grounds the page is framed in.
    ("modeline.watching", ("surface.buffer", "surface.selection", "surface.hover", *RAISED_GROUNDS), 40, 45),
    # THE TONES A BRAIN MODULE GIVES A MARK, on the modeline's own band, and
    # the levels of its notices on the brain's page — the page, the cursor's
    # band and the pointer's. `text.secondary`, the quiet one, already is.
    ("state.warning", ("modeline.background", "surface.buffer", "surface.selection", "surface.hover"), 45, 50),
    ("state.error", ("modeline.background", "surface.buffer", "surface.selection"), 45, 50),
    ("state.accent", ("modeline.background",), 45, 50),
    # AND THE SELECTION'S, at the same rung.
    ("modeline.selection", ("modeline.background",), 40, 45),
    # AND THE DRAWER'S.
    ("modeline.drawer", ("modeline.background",), 40, 45),
    ("notification.quiet", ("notification.background",), 43, 45),
    ("notification.loud", ("notification.background",), 45, 60),
    # AND THE MANAGER'S TONES, on the grounds the manager draws them on.
    #
    # `read`, `suppressed` and `loud` are the same on every page
    # (`IDENTICAL_ON_EVERY_PAGE`), but the notification MANAGER — the region,
    # a window, a column — draws them on its own ground, which wears the
    # footer's or the windows' page: its chrome, its hover band, its
    # selection. Until these rows nothing measured `read` or `suppressed` on
    # any ground at all (they were `text.dim`'s primitive, and "the same as
    # something measured" is not a measurement), and `loud` only on the band.
    # Every page runs these, so a seed that sank one of them on a desk's
    # page fails here. The floors are the tones they sit beside: `text.dim`'s
    # for the two that recede, `state.warning`'s for the one that shouts.
    ("notification.read", ("surface.chrome", "surface.hover", "surface.selection"), 15, 20),
    ("notification.suppressed", ("surface.chrome", "surface.hover", "surface.selection"), 15, 20),
    ("notification.loud", ("surface.chrome", "surface.hover", "surface.selection"), 45, 50),
    ("notification.countdown", ("notification.background",), 15, 20),
    # AN ACTION IS READ AND PRESSED, so it holds the same floor as the message
    # it sits under — a button one has to lean in to read is worse than one
    # drawn in the message's own colour.
    ("notification.acts", ("notification.background",), 43, 45),
    # AND ON THE MINIBUFFER'S LINE, where an event said there offers its
    # entry's actions after its sentence — at rest and under the pointer.
    ("notification.acts", ("surface.chrome",), 43, 45),
    ("notification.acts-lit", ("surface.chrome", "notification.background"), 43, 45),
    ("state.accent", ("surface.chrome",), 45, 50),
    ("state.success", ("surface.chrome",), 45, 50),
    ("state.warning", ("surface.chrome",), 45, 50),
    ("state.error", ("surface.chrome",), 45, 50),
    ("state.critical", ("surface.chrome",), 38, 40),
    # A resting cursor row after the pane loses focus is hover-grey with the
    # state colour on it (`files.rs`, `banded`).
    ("state.error", ("surface.hover",), 45, 45),
    ("state.success", ("surface.hover",), 45, 45),
    ("syntax.comment", ("surface.buffer",), 15, 20),
    # THE GIT BROWSER'S DIFF BANDS (`surface.diff.*`): the code on them is
    # the body text, the diff's own quiet lines and every ink of the syntax
    # palette (measured by `SYNTAX_GROUNDS` below), at the floors each holds
    # on the window's ground.
    ("text.primary", ("surface.diff.added", "surface.diff.removed"), 75, 82),
    ("text.dim", ("surface.diff.added", "surface.diff.removed"), 15, 20),
    ("syntax.comment", ("surface.diff.added", "surface.diff.removed"), 15, 20),
    # THE MAXIMUM ALERT BAR's two phases (`crate::alert`): each ink on its own
    # ground, at the floor every solid's ink is solved to (`ON_SOLID_FLOOR`,
    # the `on-solid` step) — a bar whose words cannot be read in one of its
    # two colours is a bar that shouts nothing half of the time.
    ("alert.text", ("alert.ground",), 45, 50),
    ("alert.text-alt", ("alert.ground-alt",), 45, 50),
)
SYNTAX_LOUD_FLOOR = (45, 48)
#: WHERE THE SYNTAX PALETTE'S INKS ARE DRAWN: a buffer's page, and the git
#: browser's two diff bands, where a line's code keeps its language's colours.
SYNTAX_GROUNDS = ("surface.buffer", "surface.diff.added", "surface.diff.removed")
CONTEXT_TEXT_FLOOR, CONTEXT_ON_FLOOR = (75, 80), 45
TERMINAL_FLOOR, TERMINAL_DIM_FLOOR = (40, 45), (15, 20)

#: WHAT A LIGHT INTERFACE CANNOT PAY, AND THE MEASUREMENT THAT SAYS SO.
#:
#: Every floor in this file was read off a DARK theme, and most of them mean
#: the same thing either way — `apca_magnitude` does not care which side the
#: ink is on. These do not, and each one is here because the light ladder is
#: structurally SHORTER, not because a seed is badly tuned:
#:
#:   - a light interface spends its whole Lc range between the paper and about
#:     96 (see `LIGHT_LC_CEILING`), against a dark one's 0 to 107, so every
#:     rung of it is roughly six tenths as far from its neighbours; and
#:   - the chrome has to stay near the paper for the text to be legible at
#:     all, which leaves the desk and the footer a smaller gap than a dark
#:     theme has between the desk and ITS footer.
#:
#: Keyed by the exact pair, never by the token, so a floor is loosened only
#: where it was measured to be unreachable — a gate that is looser than it
#: needs to be is a gate that has stopped saying anything.
LIGHT_CONTRAST_FLOORS = {
    # Measured 68.6 / 70.2 / 73.5 / 74.8 against a floor of 75. These four are
    # the grounds furthest from the chrome, and the body text is already as
    # dark as this interface can make it — `text-strong` sits at the top of
    # the remapped ladder. 65 is APCA's own line for text of this size.
    ("text.primary", "surface.selection"): (65, 70),
    ("text.primary", "surface.header"): (65, 70),
    ("text.primary", "surface.hover"): (65, 70),
    ("text.primary", "notification.background"): (65, 70),
    # Measured 87.8 against a floor of 90. `brain.text` is the ladder's
    # brightest rung and `brain.background` is a paint step below the chrome;
    # the pair cannot be further apart than the interface's whole range.
    ("brain.text", "brain.background"): (85, 88),
}
LIGHT_DISTANCE_FLOORS = {
    # Measured 0.0454 against a floor of 0.10 — and this is the pair the whole
    # seed turns on. The desk is the paper and the footer is one step off it;
    # putting the footer 0.10 away would take it to L 0.90, where the ladder
    # can no longer reach the contrast the body text needs. 0.04 is a step the
    # eye reads (over a JND and a half) and it is what this interface can buy.
    ("surface.base", "surface.chrome"): (0.04, 0.05),
    # Measured 0.0478 against 0.10. The discreet register's two quiet rungs sit
    # 12 Lc apart in the written ladder, and the light remap makes that about
    # seven — the six-tenths above, applied to the one pair that had the least
    # room to begin with.
    ("text.subtle.default", "text.subtle.faint"): (0.045, 0.055),
    # THE OWNER'S EXCEPTION (2026-09-28): a light theme's three desks are
    # three close papers — white, near-white and the theme's own — and the
    # 0.10 that keeps a coloured desk from black in a dark theme would force
    # this one into a colour (it did: a peach, then a lilac). Measured 0.0940
    # (papyrus) and 0.0941 (flashbang); 0.07 is a clear step and a half of a
    # JND past what the eye splits.
    ("surface.backdrop", "surface.absolute"): (0.07, 0.08),
}

#: Pairs that must be TOLD APART: (a, b, Fail below, Warn below), in ΔE_ok.
DISTANCE_GATES = (
    ("surface.base", "surface.chrome", 0.10, 0.15),
    # The desk is the ALTERNATIVE to the absolute one (black, or white in a
    # light theme): one the eye cannot tell from it is a choice that does
    # nothing. And the footer and a window both stand on it, so each needs an
    # edge of its own against it.
    ("surface.backdrop", "surface.absolute", 0.10, 0.12),
    ("surface.backdrop", "surface.chrome", 0.02, 0.03),
    ("surface.backdrop", "surface.buffer", 0.03, 0.04),
    # AND THE ABSOLUTE DESK THE SAME TWO EDGES. It is the content's own ground
    # in every dark theme, so the first is `surface.base vs surface.buffer`
    # under another name; in a light theme it is the paper's white, which the
    # footer sits one step off.
    ("surface.absolute", "surface.chrome", 0.02, 0.03),
    ("surface.absolute", "surface.buffer", 0.03, 0.04),
    # THE DISCREET FOCUSED OUTLINE is also the right column's focus line
    # (`theme::focus_line`), and that line is drawn on the footer's page: the
    # chrome, or — on a desk's page — the desk (every page runs this row).
    ("border.discreet", "surface.chrome", 0.05, 0.06),
    # THE BAND BESIDE THE PAGE. The notification band keeps the base's ground
    # on every page (`IDENTICAL_ON_EVERY_PAGE`), and on a desk's page there is
    # no modeline band between the two: the band sits on the desk-coloured
    # rows. The floor is the one it keeps against the modeline's band.
    ("notification.background", "surface.chrome", 0.02, 0.024),
    ("surface.chrome", "modeline.background", 0.03, 0.04),
    ("modeline.background", "notification.background", 0.02, 0.024),
    ("surface.chrome", "surface.hover", 0.05, 0.06),
    # THE GIT BROWSER'S DIFF BANDS: each one seen on the two grounds it is
    # drawn over — a window's and the footer's — and the two told apart. The
    # floors are the notification band's against the chrome: a ground beside
    # a ground, and the sign in its own ink (`syntax.added`, `.removed`) says
    # which band it is besides.
    ("surface.diff.added", "surface.buffer", 0.02, 0.03),
    ("surface.diff.added", "surface.chrome", 0.02, 0.03),
    ("surface.diff.removed", "surface.buffer", 0.02, 0.03),
    ("surface.diff.removed", "surface.chrome", 0.02, 0.03),
    ("surface.diff.added", "surface.diff.removed", 0.02, 0.03),
    # THE KEYBOARD'S KEYS stand apart from the footer they are drawn on, the
    # two kinds of key from each other, the key under the pointer from the key
    # it is, and a latched modifier from a locked one.
    ("surface.key.cap", "surface.chrome", 0.02, 0.03),
    ("surface.key.action", "surface.key.cap", 0.015, 0.02),
    ("surface.key.hover", "surface.key.cap", 0.02, 0.03),
    ("surface.key.on", "surface.key.cap", 0.05, 0.06),
    ("surface.key.on", "surface.key.locked", 0.03, 0.04),
    # THE TWO BANDS ON ONE PAGE. A buffer's header is a band across the top and
    # the hover is a band on a row, and until this line nothing compared them:
    # the grounds climb in one chain and the signals in another, and the ladder
    # gate walks each on its own. Where they CROSS, this looks — and what it
    # found was 0,0043 in darkside, which is the same colour twice.
    #
    # The floor is the one `modeline.background vs notification.background`
    # already carries, and for the same reason: two chrome grounds one rung
    # apart, which is the whole of what a page needs to say they are two
    # things. See `NEUTRAL_PAINT`'s `chrome-band`.
    ("surface.header", "surface.hover", 0.02, 0.024),
    ("surface.header", "surface.container", 0.02, 0.024),
    ("surface.container", "surface.chrome", 0.05, 0.06),
    # A RULE ON A RAISED GROUND IS THE STRONG ONE, and this is what says the
    # default one would be invisible there: measured, `surface.header` against
    # `rule.default` is 0,025 and against `rule.strong` is 0,098. The `Pen`
    # resolves it from the ground the region was filled with, so a buffer
    # cannot choose wrong; these two are what keep the ANSWER worth having.
    ("surface.header", "rule.raised", 0.05, 0.06),
    ("surface.container", "rule.raised", 0.05, 0.06),
    ("surface.hover", "rule.default", 0.02, 0.03),
    ("rule.default", "rule.strong", 0.05, 0.06),
    ("surface.selection", "rule.strong", 0.05, 0.06),
    ("surface.selection", "surface.hover", 0.05, 0.06),
    ("surface.selection", "surface.chrome", 0.05, 0.06),
    ("surface.selection", "rule.default", 0.05, 0.06),
    ("surface.selection", "modeline.background", 0.05, 0.06),
    ("surface.selection", "brain.background", 0.05, 0.06),
    ("brain.background", "surface.chrome", 0.04, 0.045),
    ("text.primary", "text.secondary", 0.10, 0.12),
    ("text.secondary", "text.dim", 0.10, 0.12),
    # THE THREE INKS OF A SHORTCUT LIST, each told from the other two — and
    # this is the gate that had never existed.
    #
    # A row of the leader's list is `key : name`: `text.key` for the letter,
    # then `text.group` if it opens more keys or `text.secondary` if it runs.
    # Until 2026-09-21 the three were measured only against the GROUNDS they
    # are drawn on, never against each other, and what that let through was a
    # submenu at Lc 56 and a key at 52 beside a command at 48 — a list whose
    # weights say a thing that leads somewhere matters most. The owner read it
    # on screen.
    #
    # They are on ONE rung now, so LIGHTNESS can no longer tell them apart and
    # the whole distinction is hue and chroma. That is a promise a gate has to
    # keep, because it is the kind that breaks silently: the seeds each pick
    # their own `text-chroma` for these two families, and a theme whose grey
    # ramp drifts towards either hue turns a distinction into three greys with
    # nothing on screen saying so. At the shared default it ALREADY did —
    # measured, frost's key sat ΔE 0.020 from its command and owl's key 0.039
    # from its submenu.
    #
    # The floor is 0.06 and the warn 0.07, which is the band the nine sit in
    # with their tuned chromas (worst measured 0.076, frost's submenu). It is
    # higher than the 0.03/0.04 the discreet register carries against the ramp
    # because that pair is told apart by WEIGHT as well; here there is no
    # weight left to help.
    ("text.group", "text.secondary", 0.06, 0.07),
    ("text.key", "text.secondary", 0.06, 0.07),
    ("text.match-here", "state.accent", 0.06, 0.08),
    ("text.match-here", "text.here.read", 0.05, 0.06),
    ("text.key", "text.group", 0.06, 0.07),
    ("text.subtle.strong", "text.subtle.default", 0.10, 0.12),
    ("text.subtle.default", "text.subtle.faint", 0.10, 0.11),
    ("text.subtle.strong", "text.secondary", 0.03, 0.04),
    ("text.subtle.default", "text.dim", 0.03, 0.04),
    ("text.subtle.strong", "brain.dim", 0.03, 0.04),
    ("text.subtle.alternate", "text.subtle.strong", 0.03, 0.04),
    # THE THREE KINDS OF KEY ON A HINT LINE, each told from the other two.
    #
    # `strong` is a key that is always there, `alternate` one whose MEANING
    # moves under the reader, `contextual` one that is only on the line because
    # of what is under the cursor. They sit side by side in one sentence, so a
    # theme that solved any pair to one tone would be a line that says three
    # things in one voice.
    ("text.subtle.contextual", "text.subtle.strong", 0.03, 0.04),
    ("text.subtle.contextual", "text.subtle.alternate", 0.03, 0.04),
    # AND THE KEY A CLICK CANNOT PRESS, told from the one it can: slightly,
    # which is what the owner asked for, and not so slightly it is the same.
    ("text.subtle.inert", "text.subtle.strong", 0.03, 0.04),
    ("text.subtle.inert", "text.subtle.default", 0.03, 0.04),
    ("text.subtle.inert", "text.subtle.contextual", 0.03, 0.04),
    # A LIT SHORTCUT IS ITS OWN TONE, a little lit: told apart from the resting
    # one (the pointer is on it) by a small step, which is what the owner asked
    # for — the floors are low on purpose, and the warn band says "barely".
    ("text.subtle.lit.key", "text.subtle.strong", 0.012, 0.02),
    ("text.subtle.lit.prose", "text.subtle.default", 0.012, 0.02),
    ("text.subtle.lit.alternate", "text.subtle.alternate", 0.012, 0.02),
    ("text.subtle.lit.contextual", "text.subtle.contextual", 0.012, 0.02),
    # THE SENTENCE UNDER THE HINT LINE, against everything it is drawn between.
    #
    # The owner asked for exactly this and named both sides: a tone told apart
    # from the shortcuts above it AND from the content that begins on the row
    # below. So it is gated against both — the two registers of the hint, and
    # the three steps of the content ramp — because a theme that solved it to
    # any one of them would put the explanation back into the line it is
    # supposed to be distinguishable from.
    ("text.subtle.explain", "text.subtle.strong", 0.03, 0.04),
    ("text.subtle.explain", "text.subtle.default", 0.03, 0.04),
    ("text.subtle.explain", "text.primary", 0.03, 0.04),
    ("text.subtle.explain", "text.secondary", 0.03, 0.04),
    ("text.subtle.explain", "text.dim", 0.03, 0.04),
    # A DEVICE'S MARK beside the others on the modeline: told from the marks'
    # own tone (the whole point of it), from the accent (the keyboard's), and
    # from the alarm a device at the danger level turns into.
    ("modeline.device", "text.secondary", 0.04, 0.05),
    ("modeline.device", "state.accent", 0.04, 0.05),
    ("modeline.device", "state.error", 0.06, 0.08),
    # THE CAMERA'S MARKS, beside the device's and the recording one: told
    # from the marks' own tone, from the accent, from the device blue they sit
    # next to, and from the danger red the recording mark wears — looking is
    # not recording, and the two can be lit at once.
    ("modeline.watching", "text.secondary", 0.04, 0.05),
    ("modeline.watching", "state.accent", 0.04, 0.05),
    ("modeline.watching", "modeline.device", 0.06, 0.08),
    ("modeline.watching", "state.error", 0.06, 0.08),
    # THE SELECTION'S MARK, among the same marks: told from their tone, the
    # accent, the device blue, the camera pink, and the state's green that
    # sits on the same line and means something else.
    ("modeline.selection", "text.secondary", 0.04, 0.05),
    ("modeline.selection", "state.accent", 0.04, 0.05),
    ("modeline.selection", "modeline.device", 0.06, 0.08),
    ("modeline.selection", "modeline.watching", 0.06, 0.08),
    ("modeline.selection", "state.error", 0.06, 0.08),
    # THE DRAWER'S MARK sits beside the selection's, and is a quieter green:
    # told from the marks' tone, the accent, the device blue, the camera pink
    # and the selection it could be confused with.
    # THE CAPTURE'S GUIDES are not the focused window's border, which the
    # frozen picture carries, nor the accent.
    ("capture.guide", "border.active", 0.08, 0.10),
    ("capture.guide", "state.accent", 0.08, 0.10),
    ("modeline.drawer", "text.secondary", 0.04, 0.05),
    ("modeline.drawer", "state.accent", 0.04, 0.05),
    ("modeline.drawer", "modeline.device", 0.06, 0.08),
    ("modeline.drawer", "modeline.watching", 0.06, 0.08),
    ("modeline.drawer", "modeline.selection", 0.04, 0.05),
    # A SELECTED OUTLINE beside the focused one: a window can be both.
    ("border.selected", "border.active", 0.05, 0.06),
    ("border.selected", "border.inactive", 0.05, 0.06),
    ("state.error", "state.critical", 0.04, 0.05),
    # THE MESSAGE AND THE BUTTON UNDER IT. They are drawn one line apart on the
    # same ground and they are not the same kind of thing — one is read, the
    # other is pressed — and for a while they were the same colour. This is
    # what says they cannot go back to it in some theme nobody looked at.
    ("notification.quiet", "notification.acts", 0.04, 0.05),
    # AN ACTION UNDER THE POINTER, told from the same action at rest by the
    # small step every lit shortcut takes (`text.subtle.lit.*` above); and on
    # the minibuffer's line it follows the sentence it offers to act on, drawn
    # in the discreet register's loud tone.
    ("notification.acts-lit", "notification.acts", 0.012, 0.02),
    ("notification.acts", "text.subtle.strong", 0.04, 0.05),
    # AND THE ACTION THE CURSOR IS ON, told from the ones beside it — and on
    # the brain's strip, the picked control from the label of the next.
    ("text.here.acts", "notification.acts", 0.04, 0.05),
    ("text.here.acts", "brain.text", 0.04, 0.05),
    # THE GLANCE'S TWO GROUNDS, which are drawn EDGE TO EDGE on one line: one
    # place ends where the next begins, with nothing between them. If a theme
    # ever solved them to the same tone the alternation would simply not be
    # there, and the line would go back to being a row of words the eye cannot
    # group. The floor is low on purpose — this is meant to be the smallest
    # step there is, the `surface.shelf` register — but it is not zero.
    ("glance.ground", "glance.ground-alternate", 0.008, 0.012),
    # THE ANSWER OF THE ROW UNDER THE CURSOR, in its two readings. The owner
    # moved the cursor down a sheet and watched a door and a fact become one
    # row; this is what says they cannot go back to being one in some theme
    # nobody looked at. The floor is the one every "these two must be told
    # apart" pair in this table carries.
    ("text.here.acts", "text.here.read", 0.05, 0.06),
    # THE WINDOW'S TWO OUTLINES. They are never on screen together on ONE
    # window, but they are on screen together constantly — the focused window
    # beside every other one — and telling them apart is the whole job of the
    # pair. One rung of the paint ladder apart, which is what this measures.
    ("border.active", "border.inactive", 0.05, 0.06),
    # THE ANSWER OF A ROW ONE CAN ACT ON, AGAINST ORDINARY TEXT. `text.answer`
    # is `text.primary`'s own primitive with its chroma multiplied, and the
    # multiplier is ONE NUMBER FOR EVERY THEME — so how far it actually moves
    # depends on how much chroma the theme's ramp had to begin with. A theme
    # with a nearly colourless ramp multiplies nearly nothing, and the value
    # of a row that answers a key goes back to wearing the body text's tone,
    # which is the exact defect the token was added to remove.
    #
    # It is not hypothetical: `cyber` was drafted at a ramp chroma of 0.16 and
    # landed at ΔE 0.010, while the three themes before it sat at 0.055.
    ("text.answer", "text.primary", 0.035, 0.045),
    # THE CHOSEN MARK ON THE GLOBE, against the marks it is chosen from and
    # the two grounds it is drawn on. Read before gating, across the 44
    # palettes at their nearest: 0.215 from the dot, 0.161 from the land and
    # 0.344 from the ocean (`colors.nix` says which recipes lost, and by how
    # much). The halo every mark wears is what the dot relies on over land
    # (0.044 at its nearest); the chosen one does not need it.
    ("mark.atlas.chosen", "mark.atlas.dot", 0.15, 0.18),
    ("mark.atlas.chosen", "surface.atlas.land", 0.10, 0.12),
    ("mark.atlas.chosen", "surface.atlas.ocean", 0.25, 0.30),
    # THE PLAYER'S DECK (`player_timeline`): three kinds of lane, a lane that
    # is off, the ghost of one being dragged and the playhead, all on the
    # chrome's track. Read before gating, over the 44 palettes at their
    # nearest: the kinds from each other 0.079 (azure and success, in
    # `flashbang`'s desk), the ghost from a kind 0.079, the playhead from a
    # kind 0.102, a kind from the lane that is off 0.183, the off lane from
    # the track's rule 0.198 and a band from the chrome 0.458.
    ("player.lane.video", "player.lane.audio", 0.06, 0.07),
    ("player.lane.video", "player.lane.subtitle", 0.06, 0.07),
    ("player.lane.audio", "player.lane.subtitle", 0.06, 0.07),
    ("player.lane.held", "player.lane.video", 0.06, 0.07),
    ("player.lane.held", "player.lane.audio", 0.06, 0.07),
    ("player.lane.held", "player.lane.subtitle", 0.06, 0.07),
    ("player.lane.off", "player.lane.video", 0.12, 0.15),
    ("player.lane.off", "player.lane.audio", 0.12, 0.15),
    ("player.lane.off", "player.lane.subtitle", 0.12, 0.15),
    ("player.lane.off", "player.lane.held", 0.12, 0.15),
    ("player.lane.off", "rule.default", 0.12, 0.15),
    ("player.lane.off", "surface.chrome", 0.20, 0.25),
    ("player.lane.video", "surface.chrome", 0.30, 0.35),
    ("player.lane.audio", "surface.chrome", 0.30, 0.35),
    ("player.lane.subtitle", "surface.chrome", 0.30, 0.35),
    ("player.lane.held", "surface.chrome", 0.30, 0.35),
    ("player.playhead", "player.lane.video", 0.08, 0.09),
    ("player.playhead", "player.lane.audio", 0.08, 0.09),
    ("player.playhead", "player.lane.subtitle", 0.08, 0.09),
    ("player.playhead", "player.lane.held", 0.10, 0.12),
    ("player.playhead", "surface.chrome", 0.40, 0.50),
    # AND EACH BAND FROM THE METER BESIDE IT: a lane's line ends in its volume
    # meter, filled in `state.accent`, and in the themes whose accent is a
    # blue the picture's band and the meter are two blues on one line. Read
    # before gating, at the nearest: the words 0.069 (`flashbang`'s desk), the
    # picture 0.082 (`industrial`'s absolute desk), the ghost 0.082, the sound
    # 0.106. A regression floor under what is there, as every row here is —
    # not a proof that the two read apart, which the glyph and the name at the
    # head of the line also carry.
    ("player.lane.video", "state.accent", 0.05, 0.06),
    ("player.lane.audio", "state.accent", 0.05, 0.06),
    ("player.lane.subtitle", "state.accent", 0.05, 0.06),
    ("player.lane.held", "state.accent", 0.05, 0.06),
    # AND THE LAYOUT'S SCHEMATIC: a tile on the chrome (0.171 at the nearest)
    # and the lit one against it (0.268).
    ("player.tile", "surface.chrome", 0.10, 0.12),
    ("player.tile-lit", "player.tile", 0.15, 0.20),
    # THE ALERT BAR'S TWO GROUNDS ALTERNATE (`crate::alert`), and an
    # alternation the eye cannot see is a bar standing still: a hue apart, and
    # well over the distance two bands of one list keep. And each stands out
    # of the footer's chrome it rises over.
    ("alert.ground", "alert.ground-alt", 0.10, 0.12),
    ("alert.ground", "surface.chrome", 0.10, 0.12),
    ("alert.ground-alt", "surface.chrome", 0.10, 0.12),
)
#: THE NEAR DESK AMONG THE OTHERS, measured where the chooser offers it: in a
#: LIGHT theme only. A dark theme has the token (every theme has the same keys)
#: and a palette whose page it is, gated like every page, but it is never
#: offered — a "near" in the file is worn as the absolute desk under a dark
#: theme — so nothing there sets it beside the others.
#:
#: THE OWNER'S EXCEPTIONS (2026-09-28): between any two other desks the floor
#: is 0.10, the "a choice that does nothing" floor. A light theme's three are
#: MEANT to be close papers: near-white and white one JND apart (0.02), and
#: the theme's own desk a clear step off the near-white (0.05) — see
#: `LIGHT_DISTANCE_FLOORS` for it against the white. The last two rows are the
#: floors every desk keeps against the chrome and a window, on the base.
NEAR_GATES = (
    ("surface.near", "surface.absolute", 0.02, 0.025),
    ("surface.near", "surface.backdrop", 0.05, 0.06),
    ("surface.near", "surface.chrome", 0.02, 0.03),
    ("surface.near", "surface.buffer", 0.03, 0.04),
)
#: Pairs reported but never gated: they are never drawn side by side.
DISTANCE_NOTES = (
    # THE RAISED GROUNDS AGAINST WHAT IS DRAWN NEAR THEM, reported before they
    # are gated, because a floor chosen before the number is read is a floor
    # chosen to fit whatever is there.
    ("surface.header", "rule.default"),
    ("surface.container", "rule.default"),
    ("surface.header", "surface.selection"),
    ("surface.chrome", "surface.buffer"),
    ("surface.hover", "notification.background"),
    ("brain.background", "modeline.background"),
)
LADDER_MIN_STEP, LADDER_MAX_STEP = 0.012, 0.26
HUE_GAP = (14.0, 20.0)
CVD_WARN = 0.05
CHART_GAP, LEVEL_GAP = (cs.JND, 0.05), 0.012
#: THE GROUNDS A CHART OR A HEAT RAMP CAN BE DRAWN ON.
#:
#: Both palettes are CONTENT — a figure inside a buffer, or inside the footer's
#: expanded region, or on a card. The three grounds those are filled with, and
#: measured they answer within a hair of each other (0.3 Lc between the buffer
#: and the chrome across the ten), so gating all three costs nothing and leaves
#: no door out.
CONTENT_GROUNDS = ("surface.buffer", "surface.chrome", "surface.container")
#: A categorical mark has to be SEEN, and until 2026-09-21 nothing said so.
#:
#: `chart.*` was measured only against the other seven (`CHART_GAP`) and
#: `level.*` only against its own neighbours (`LEVEL_GAP`) — eight colours
#: proved to be eight DIFFERENT colours, with nothing asking whether any of
#: them is visible. A set that is distinct and invisible passes both.
#:
#: 15 is the floor the quietest TEXT in this system carries (`text.dim`), and a
#: chart mark is a filled area, which needs less than a letter — so holding it
#: to the text floor is if anything strict. Measured across the ten themes on
#: the three grounds: nothing falls under 15, and nine of the 240 readings sit
#: between 15 and 20 — the dark end of the ramp on a card, in cyber, midnight,
#: industrial, owl and darkside. Those are warns, and they are true.
CHART_ON_GROUND = (15, 20)
#: Two cursors' insides: STRICTER than two chart colours (which fail only
#: under a JND), since a cursor is smaller than any chart mark — and not
#: stricter than eight hues at one lightness can be. Measured: at L 0.80 the
#: closest pair of eight equal-ΔE hues lands between 0.053 and 0.065.
CURSOR_GAP = (0.05, 0.06)
#: A cursor's inside against its own rim — the edge that makes it a shape.
CURSOR_RIM = (60, 70)
#: A hover band against the surface it is drawn on, the `surface.hover`
#: separation this family stands in for.
CURSOR_HOVER_BAND = (0.02, 0.03)
#: Two cursors' hover bands, told apart from each other.
CURSOR_HOVER_GAP = (cs.JND, 0.03)
#: The row's own words on a keyboard's selection band — the floor body text
#: carries everywhere else in this file.
CURSOR_SELECTION_TEXT = (60, 75)
#: THE LOUD END OF AN ORDINAL RAMP, which is the half of it that has to read.
#: The floor every signal in this file carries; measured 83.2 to 87.0.
LEVEL_TOP_FLOOR = (45, 50)
#: AND THE QUIET END IS GATED THE OTHER WAY ROUND.
#:
#: `level.zero` is the ramp's "none of it", and it is `accent.ground` — a
#: BACKGROUND tint. Requiring it to contrast would contradict what the bottom
#: of an ordinal ramp is, so the exemption is written as a measurement instead
#: of as a silence: it has to stay NEAR the ground. Measured Lc 0.0000 on all
#: three grounds in all ten themes — it is the ground, exactly.
LEVEL_BOTTOM_CEILING = 10.0
#: THE DISCREET BORDERS (`LEADER s t`, the borders column), per desk: the
#: faint unfocused outline and the desk it lies on.
DISCREET_DESKS = (
    ("surface.backdrop", "border.faint-backdrop"),
    ("surface.near", "border.faint-near"),
    ("surface.absolute", "border.faint-absolute"),
)
#: The faint outline against its desk, both ways. VISIBLE — ΔE_ok off the
#: bytes, so an outline quantised onto the desk's own bytes is a Fail and not a
#: rounding — and FAINT, in the ratio it is solved in (see `FAINT_RATIO`): past
#: 1.10 it is a border, not a reference for the pointer.
FAINT_EDGE_FLOOR, FAINT_EDGE_CEILING = (0.012, 0.015), (1.10, 1.08)
#: The three outlines on one desk, in order: the discreet FOCUSED one clearly
#: stronger than the faint one and clearly weaker than the standard focused
#: one — "clearly" being a JND and a half of ΔE_ok between each pair's
#: distances from the desk.
DISCREET_ORDER = (0.03, 0.04)
#: THE SUBTITLE LANES (`PICTURE_FAMILIES`), measured once, on the base: they
#: are the same on every page and in every theme.
#:
#: Two lanes told apart while READING, at a glance, a line that is gone in
#: three seconds: three JNDs, stricter than two cursors (`CURSOR_GAP`), which
#: are looked for rather than read. Measured at the seed's hue 109 and chroma
#: 0.70: the closest pair (lanes three and four) 0.1006.
SUBTITLE_GAP = (0.06, 0.08)
#: A lane on its own rim, which is all that stands between a letter and a
#: white sky: the floor body text carries. Measured 77.5 on black.
SUBTITLE_ON_EDGE = (70, 75)
#: And on the dark picture it was solved against: the anchor itself, so a
#: lane that could not reach it is a Fail and not a quieter lane.
SUBTITLE_ON_PICTURE = (SUBTITLE_LC, SUBTITLE_LC)


class Finding:
    __slots__ = ("level", "gate", "subject", "measured", "threshold", "note")

    def __init__(self, level: str, gate: str, subject: str, measured: float, threshold: str, note: str = ""):
        self.level, self.gate, self.subject, self.measured, self.threshold, self.note = level, gate, subject, measured, threshold, note

    def row(self) -> dict:
        return {"level": self.level, "gate": self.gate, "subject": self.subject, "measured": round(self.measured, 4), "threshold": self.threshold, "note": self.note}


def _level(measured: float, fail: float, warn: float, higher_is_better: bool = True) -> str:
    if higher_is_better:
        return "Fail" if measured < fail else "Warn" if measured < warn else "OK"
    return "Fail" if measured > fail else "Warn" if measured > warn else "OK"


def run_gates(theme: Theme, tokens: dict[str, Resolved], primitives: dict[str, Swatch]) -> list[Finding]:
    """Every gate, on one palette: the base (`theme.page_name == "chrome"`) or
    the page of a desk. The same tables for both; what differs is written
    here and says why — `BASE_ONLY_PAIRS`, the desks among themselves and
    their outlines (the same tokens on every page, measured once, on the
    base), the ladder rungs that start at the ground, and the terminal's
    background."""
    findings: list[Finding] = []
    base = theme.page_name == "chrome"

    def lc(text: str, ground: str) -> float:
        return cs.apca_magnitude(tokens[text].rgb, tokens[ground].rgb)

    def de(a: str, b: str) -> float:
        return cs.delta_e_ok(tokens[a].lab, tokens[b].lab)

    light = theme.rising < 0.0
    for text, grounds, fail, warn in CONTRAST_GATES:
        for ground in grounds:
            # NEW NAMES, because the pair being overridden is the INNER one: a
            # gate names several grounds, and rebinding the loop's own `fail`
            # would carry one ground's exemption to the next.
            low, band = LIGHT_CONTRAST_FLOORS.get((text, ground), (fail, warn)) if light else (fail, warn)
            v = lc(text, ground)
            findings.append(Finding(_level(v, low, band), "contrast", f"{text} on {ground}", v, f"Lc ≥ {low} (warn < {band})"))
    for key in tokens:
        if key.startswith("syntax.") and key != "syntax.comment":
            for ground in SYNTAX_GROUNDS:
                v = lc(key, ground)
                findings.append(Finding(_level(v, *SYNTAX_LOUD_FLOOR), "contrast", f"{key} on {ground}", v, f"Lc ≥ {SYNTAX_LOUD_FLOOR[0]}"))
    for family in CONTEXT_FAMILIES:
        v = lc(f"context.{family}.text", f"context.{family}.ground")
        findings.append(Finding(_level(v, *CONTEXT_TEXT_FLOOR), "contrast", f"context.{family}.text on .ground", v, f"Lc ≥ {CONTEXT_TEXT_FLOOR[0]}"))
        v = lc(f"context.{family}.on", f"context.{family}.solid")
        findings.append(Finding(_level(v, CONTEXT_ON_FLOOR, CONTEXT_ON_FLOOR), "contrast", f"context.{family}.on on .solid", v, f"Lc ≥ {CONTEXT_ON_FLOOR}"))
    near = NEAR_THE_PAPER[theme.mode]
    dim = "ansi.bright-black" if theme.rising > 0.0 else "ansi.bright-white"
    for name, _, _ in TERMINAL:
        if name.startswith("ansi.") and name != near:
            floor = TERMINAL_DIM_FLOOR if name == dim else TERMINAL_FLOOR
            # THE BACKGROUND THIS PAGE'S FRAGMENT WRITES: the base's window,
            # or the desk (`TERMINAL_BACKGROUND`, `write_fragments`).
            background = TERMINAL_BACKGROUND[theme.page_name]
            v = lc(f"terminal.{name}", background)
            findings.append(Finding(_level(v, *floor), "contrast", f"terminal.{name} on {background}", v, f"Lc ≥ {floor[0]}"))

    for a, b, fail, warn in DISTANCE_GATES:
        if not base and (a, b) in BASE_ONLY_PAIRS:
            continue
        low, band = LIGHT_DISTANCE_FLOORS.get((a, b), (fail, warn)) if light else (fail, warn)
        v = de(a, b)
        findings.append(Finding(_level(v, low, band), "distance", f"{a} vs {b}", v, f"ΔE ≥ {low} (warn < {band})"))
    for a, b, fail, warn in NEAR_GATES if light and base else ():
        v = de(a, b)
        findings.append(Finding(_level(v, fail, warn), "desk", f"{a} vs {b}", v, f"ΔE ≥ {fail} (warn < {warn})"))
    # THE DISCREET BORDERS, on each desk: the faint outline visible and faint,
    # and the three outlines in order — faint, discreet, standard. Outlines and
    # desks are the same on every page, so they are measured on the base.
    for desk, faint in DISCREET_DESKS if base else ():
        v = de(faint, desk)
        findings.append(Finding(_level(v, *FAINT_EDGE_FLOOR), "borders", f"{faint} vs {desk}", v, f"ΔE ≥ {FAINT_EDGE_FLOOR[0]} — never transparent"))
        v = cs.wcag_ratio(tokens[faint].rgb, tokens[desk].rgb)
        findings.append(Finding(_level(v, *FAINT_EDGE_CEILING, higher_is_better=False), "borders", f"{faint} on {desk}, luminance ratio", v, f"≤ {FAINT_EDGE_CEILING[0]} — extremely faint"))
        v = de("border.discreet", desk) - de(faint, desk)
        findings.append(Finding(_level(v, *DISCREET_ORDER), "borders", f"border.discreet over {faint}, on {desk}", v, f"ΔE apart ≥ {DISCREET_ORDER[0]}"))
        v = de("border.active", desk) - de("border.discreet", desk)
        findings.append(Finding(_level(v, *DISCREET_ORDER), "borders", f"border.active over border.discreet, on {desk}", v, f"ΔE apart ≥ {DISCREET_ORDER[0]}"))
    for a, b in DISTANCE_NOTES:
        findings.append(Finding("Note", "distance", f"{a} vs {b}", de(a, b), "reported only: never adjacent"))

    ladders = [("neutral", ("chrome", "window", "chrome-raised", "chrome-loud", "chrome-band")), ("neutral", ("chrome", "hover", "border", "border-strong")), ("neutral", tuple(n for n, _ in TEXT_STEPS))]
    if base:
        # THE RUNGS FROM THE GROUND, on the base alone: on a desk's page the
        # "chrome" is that desk, and a rung from the ground to it measures the
        # desk against itself or against the black it may be.
        ladders += [
            ("neutral", ("ground", "chrome")),
            # THE DESK'S CHAIN IS THE ONE THAT REORDERS, because the desk is
            # the one rung that does not mirror (see NEUTRAL_BACKDROP): in a
            # dark interface it lies between the ground and the chrome, and in
            # a light one it is past the chrome on the far side.
            ("neutral", ("ground", "backdrop", "chrome") if theme.rising > 0.0 else ("ground", "chrome", "backdrop")),
            # AND THE NEAR DESK, between the ground and the chrome in both.
            ("neutral", ("ground", "near", "chrome")),
        ]
    for family in theme.families:
        ladders.append((family, tuple(n for n, _, _ in PAINT_STEPS)))
        ladders.append((family, tuple(n for n, _ in TEXT_STEPS)))
    for family, steps in ladders:
        for lower, upper in zip(steps, steps[1:]):
            a, b = primitives[f"{family}.{lower}"], primitives[f"{family}.{upper}"]
            gap = cs.delta_e_ok(a.lab, b.lab)
            level = "Fail" if gap < LADDER_MIN_STEP else "Warn" if gap > LADDER_MAX_STEP else "OK"
            findings.append(Finding(level, "ladder", f"{family}.{lower} → {family}.{upper}", gap, f"{LADDER_MIN_STEP} ≤ ΔE ≤ {LADDER_MAX_STEP}"))
            # MONOTONE IN THE INTERFACE'S OWN DIRECTION, not upward.
            #
            # A rung is further from the ground than the one before it, and in
            # a light theme "further" is DARKER. Written as `b < a` this fired
            # on every adjacent pair of every ladder of a light theme — several
            # hundred Fails saying the mirror is upside down, which it is not.
            # The ΔE half above is unsigned and needs nothing.
            if (b.lch[0] - a.lch[0]) * theme.rising < -1e-9:
                away = "darker" if theme.rising > 0 else "lighter"
                findings.append(Finding("Fail", "ladder", f"{family}.{upper} is {away} than {family}.{lower}", b.lch[0] - a.lch[0], "L monotone"))
    for name, swatch in primitives.items():
        if swatch.shortfall is not None:
            findings.append(Finding("Warn", "reach", name, swatch.shortfall, f"the anchor {swatch.anchor} was out of reach"))

    hues = {f: theme.families[f]["hue"] for f in CHROME_HUES}
    for i, a in enumerate(CHROME_HUES):
        for b in CHROME_HUES[i + 1 :]:
            v = cs.hue_difference(hues[a], hues[b])
            findings.append(Finding(_level(v, *HUE_GAP), "hue", f"{a} vs {b}", v, f"≥ {HUE_GAP[0]}° (warn < {HUE_GAP[1]}°)"))
    accent_hue = hues["accent"]
    in_band = PROTAN_BAND[0] <= accent_hue <= PROTAN_BAND[1]
    findings.append(Finding("Fail" if in_band else "OK", "hue", "accent outside the protan band", accent_hue, f"not in [{PROTAN_BAND[0]}, {PROTAN_BAND[1]}]"))

    for i, a in enumerate(STATE_SOLIDS):
        for b in STATE_SOLIDS[i + 1 :]:
            kind, v = cs.worst_separation(tokens[f"state.{a}"].rgb, tokens[f"state.{b}"].rgb)
            findings.append(Finding("Warn" if v < CVD_WARN else "OK", "colour-vision", f"state.{a} vs state.{b} ({kind})", v, f"ΔE ≥ {CVD_WARN} under every deficiency; never a Fail — the glyph carries the meaning"))

    charts = [f"chart.{WORDS[i + 1]}" for i in range(CHART_COUNT)]
    for i, a in enumerate(charts):
        for b in charts[i + 1 :]:
            v = de(a, b)
            findings.append(Finding(_level(v, *CHART_GAP), "categorical", f"{a} vs {b}", v, f"ΔE ≥ {CHART_GAP[0]}"))
    # THE CURSORS: every pair of fills tells two cursors apart, every fill
    # reads against its own outline, and every hover band is a band — told
    # apart from the ground it lies on and from the other cursors' bands.
    fills = [f"cursor.fill.{WORDS[i + 1]}" for i in range(CURSOR_COUNT)]
    for i, a in enumerate(fills):
        for b in fills[i + 1 :]:
            v = de(a, b)
            findings.append(Finding(_level(v, *CURSOR_GAP), "cursors", f"{a} vs {b}", v, f"ΔE ≥ {CURSOR_GAP[0]}"))
        v = lc(a, "cursor.outline")
        findings.append(Finding(_level(v, *CURSOR_RIM), "cursors", f"{a} on cursor.outline", v, f"Lc ≥ {CURSOR_RIM[0]}"))
    hovers = [f"cursor.hover.{WORDS[i + 1]}" for i in range(CURSOR_COUNT)]
    for i, a in enumerate(hovers):
        for ground in ("surface.chrome", "surface.buffer"):
            v = de(a, ground)
            findings.append(Finding(_level(v, *CURSOR_HOVER_BAND), "cursors", f"{a} on {ground}", v, f"ΔE ≥ {CURSOR_HOVER_BAND[0]}"))
        for b in hovers[i + 1 :]:
            v = de(a, b)
            findings.append(Finding(_level(v, *CURSOR_HOVER_GAP), "cursors", f"{a} vs {b}", v, f"ΔE ≥ {CURSOR_HOVER_GAP[0]}"))
    # AND EACH KEYBOARD'S SELECTION: a band on the ground, apart from the
    # others, and under text that still reads — a selected row carries the
    # row's own words.
    selections = [f"cursor.selection.{WORDS[i + 1]}" for i in range(CURSOR_COUNT)]
    for i, a in enumerate(selections):
        for ground in ("surface.chrome", "surface.buffer"):
            v = de(a, ground)
            findings.append(Finding(_level(v, *CURSOR_HOVER_BAND), "cursors", f"{a} on {ground}", v, f"ΔE ≥ {CURSOR_HOVER_BAND[0]}"))
        v = lc("text.primary", a)
        findings.append(Finding(_level(v, *CURSOR_SELECTION_TEXT), "cursors", f"text.primary on {a}", v, f"Lc ≥ {CURSOR_SELECTION_TEXT[0]}"))
        for b in selections[i + 1 :]:
            v = de(a, b)
            findings.append(Finding(_level(v, *CURSOR_HOVER_GAP), "cursors", f"{a} vs {b}", v, f"ΔE ≥ {CURSOR_HOVER_GAP[0]}"))

    # THE SUBTITLE LANES, on the base alone: a film is under them on every
    # page. Each on the dark picture it was solved against, each on its own
    # rim, and every pair told apart — and, reported and never gated, the
    # pair a person without one of the three cones tells apart least: the
    # lane's place in the stack says which it is for them.
    subtitles = [f"subtitle.lane-{WORDS[i + 1]}" for i in range(SUBTITLE_LANES)]
    picture = cs.dequantize(cs.screen_colour(PICTURE_GROUND_L, 0.0, 0.0)[1])
    for i, a in enumerate(subtitles if base else ()):
        v = cs.apca_magnitude(tokens[a].rgb, picture)
        findings.append(Finding(_level(v, *SUBTITLE_ON_PICTURE), "subtitles", f"{a} on the dark picture (L {PICTURE_GROUND_L:g})", v, f"Lc ≥ {SUBTITLE_ON_PICTURE[0]:g}"))
        v = lc(a, "subtitle.edge")
        findings.append(Finding(_level(v, *SUBTITLE_ON_EDGE), "subtitles", f"{a} on subtitle.edge", v, f"Lc ≥ {SUBTITLE_ON_EDGE[0]} (warn < {SUBTITLE_ON_EDGE[1]})"))
        for b in subtitles[i + 1 :]:
            v = de(a, b)
            findings.append(Finding(_level(v, *SUBTITLE_GAP), "subtitles", f"{a} vs {b}", v, f"ΔE ≥ {SUBTITLE_GAP[0]} (warn < {SUBTITLE_GAP[1]})"))
    if base:
        (kind, v), a, b = min(
            ((cs.worst_separation(tokens[a].rgb, tokens[b].rgb), a, b) for i, a in enumerate(subtitles) for b in subtitles[i + 1 :]),
            key=lambda found: found[0][1],
        )
        findings.append(Finding("Note", "subtitles", f"{a} vs {b} ({kind}), the closest pair", v, "reported only: the stack's order tells the lanes apart"))

    levels = [f"level.{WORDS[k]}" for k in range(11)]
    for a, b in zip(levels, levels[1:]):
        v = de(a, b)
        findings.append(Finding("Fail" if v < LEVEL_GAP else "OK", "ordinal", f"{a} → {b}", v, f"ΔE ≥ {LEVEL_GAP}"))

    # AND BOTH PALETTES ARE MEASURED AGAINST A GROUND, which is the thing
    # neither of the two loops above does. They compare a chart colour to the
    # other seven and a level to its neighbour — so a set that is perfectly
    # distinct and perfectly invisible passes both of them, and for as long as
    # the ladder climbed one way that never showed. Its shape did: `chart.*`
    # was built with a hard-coded ascent, and on the first light ground the top
    # half of the set came out at Lc 17 with nothing saying a word.
    for name in charts:
        for ground in CONTENT_GROUNDS:
            v = lc(name, ground)
            findings.append(Finding(_level(v, *CHART_ON_GROUND), "categorical", f"{name} on {ground}", v, f"Lc ≥ {CHART_ON_GROUND[0]}"))
    # THE RAMP IS GATED AT ITS TWO ENDS, and in OPPOSITE directions — the loud
    # end has to read and the quiet end has to be the ground. What holds the
    # middle together is `LEVEL_GAP` above, one rung at a time.
    for ground in CONTENT_GROUNDS:
        v = lc("level.ten", ground)
        findings.append(Finding(_level(v, *LEVEL_TOP_FLOOR), "ordinal", f"level.ten on {ground}", v, f"Lc ≥ {LEVEL_TOP_FLOOR[0]}"))
        v = lc("level.zero", ground)
        findings.append(Finding("Fail" if v > LEVEL_BOTTOM_CEILING else "OK", "ordinal", f"level.zero on {ground}", v, f"Lc ≤ {LEVEL_BOTTOM_CEILING} — it IS the ground"))

    for key, tok in tokens.items():
        if key.startswith("terminal."):
            continue
        content = key.split(".", 1)[0] in CONTENT_GROUPS
        floor = FILM_CONTENT_FLOOR if content else FILM_FLOOR
        for film in FILM_COLOURS:
            f = cs.dequantize(cs.parse_token(film)[0])
            d = math.sqrt(sum((x - y) ** 2 for x, y in zip(tok.rgb, f))) / math.sqrt(3.0)
            if d < floor:
                findings.append(Finding("Warn" if content else "Fail", "film", f"{key} near {film}", d, f"RGB distance ≥ {floor}"))
    return findings


def regression(theme: Theme, tokens: dict[str, Resolved], reference: dict) -> list[Finding]:
    """Every key of the frozen reference against the generated value.

    `reference` is `{"colors": {…nested…}, "deviations": {key: {max, reason}}}`;
    a listed key may go as far as its own `max`, and is reported as a Note
    carrying the reason, so the report says WHY a value moved."""
    if not isinstance(reference, dict) or "colors" not in reference:
        raise Refusal(f"the reference for {theme.id} is not {{ colors; deviations; }}")
    colors = flatten_recipes(reference["colors"])
    deviations = reference.get("deviations", {})
    unknown = sorted(set(colors) - set(tokens))
    if unknown:
        raise Refusal(f"the reference for {theme.id} names keys the recipes do not have: {unknown}")
    stray = sorted(set(deviations) - set(colors))
    if stray:
        raise Refusal(f"the reference for {theme.id} lists deviations for keys it has no value for: {stray}")
    findings = []
    for key, literal in colors.items():
        rgb8, alpha8 = cs.parse_token(literal)
        tok = tokens[key]
        v = cs.delta_e_ok(tok.lab, cs.rgb_to_oklab(cs.dequantize(rgb8)))
        note = f"{cs.format_hex(tok.rgb8)} against {cs.format_hex(rgb8)}"
        if tok.alpha8 != alpha8:
            findings.append(Finding("Fail", "regression", f"{key} alpha", tok.alpha8 - alpha8, f"alpha {alpha8}", note))
        allowed = deviations.get(key)
        if v <= REGRESSION_TOLERANCE:
            level = "OK"
        elif allowed is not None and v <= _number(allowed.get("max"), f"deviation `{key}` max", 0.0, 1.0):
            level = "Note"
            note += f"; allowed to {allowed['max']}: {allowed.get('reason', 'no reason given')}"
        else:
            level = "Fail"
        findings.append(Finding(level, "regression", key, v, f"ΔE ≤ {REGRESSION_TOLERANCE}", note))
    return findings


# ---------------------------------------------------------------------------
# Fragments for the consumers outside the shell.
# ---------------------------------------------------------------------------


def write_fragments(directory: pathlib.Path, theme_id: str, mode: str, palettes: dict[str, dict[str, Resolved]]) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    hexes = {k: cs.format_hex(v.rgb8) for k, v in palettes["chrome"].items()}
    # THE POLARITY, AS A FRAGMENT, because a colour cannot be asked what side
    # it is on. `nix/modules/foreign.nix` dresses GTK and Qt, and until a light
    # theme existed it could answer `prefer-dark` and be right nine times out
    # of nine — its own header says so, and says this file is what has to
    # export the mode the day the tenth arrives. It goes here and not into
    # `<id>.json`, which is a flat map of token to hex that two generators and
    # the extension parse: one non-colour key there is a shape nobody expects.
    (directory / "mode").write_text(mode)
    # THE COMPOSITOR: three values for the static config, and SIX fragments
    # per theme — one per pair of answers to "what is under the windows" (the
    # desk: the theme's, the near one, the absolute one) and "how loud are
    # their outlines" (the borders). A dark theme's near fragments exist and
    # are never copied: there a "near" answer is worn as the absolute desk. The footer copies
    # the one the person chose into the configuration directory when the theme
    # or either answer changes (`theme::fragments`).
    #
    # THE STATIC THREE ARE THE DEFAULT ANSWERS' FRAGMENT, `scroll-backdrop-
    # standard.conf`, said as three values: the theme's own desk and the
    # standard borders. A machine with no `theme.json` has no fragment at all
    # (`theme::sync_with_file`), so what the static config paints IS what a
    # fresh machine shows — and the shell's default answer (`Backdrop::Theme`,
    # `Borders::Standard`) is what the footer's hidden rows wear on it. The
    # desk was `surface.base` here, a desk no answer names: the rows that push
    # the windows away were painted `surface.backdrop` over it, a strip across
    # the desk on every machine that never chose. `themes-have-fragments`
    # holds these three to that fragment.
    (directory / "compositor-border-active").write_text(hexes["border.active"])
    (directory / "compositor-border-inactive").write_text(hexes["border.inactive"])
    (directory / "compositor-background").write_text(hexes["surface.backdrop"])
    # THE CURSORS, for `nix/cursors.nix`: one line per colour, the inside and
    # the rim, in the order the numbers go. A file and not a read of the JSON
    # so the cursor build depends on exactly these bytes.
    (directory / "cursor-colours").write_text(
        "".join(f"{hexes[f'cursor.fill.{WORDS[i + 1]}']} {hexes['cursor.outline']}\n" for i in range(CURSOR_COUNT))
    )

    def five(key: str) -> str:
        return " ".join([hexes[key]] * 5)

    for desk in ("backdrop", "near", "absolute"):
        ground = f"surface.{desk}"
        for borders in ("standard", "discreet"):
            if borders == "standard":
                focused, focused_inactive, unfocused = "border.active", "border.inactive", "border.inactive"
            else:
                # DISCREET: the focused window in a quiet outline and every
                # other one in an EXTREMELY faint one — never none, because
                # the outline is what the pointer finds to resize by. The
                # faint colour is the desk's own (`border.faint-<desk>`): it
                # is measured against the desk it lies on.
                #
                # `focused_inactive` IS AN UNFOCUSED WINDOW HERE. In this
                # compositor every column is a container, so the window each
                # column last focused — nearly every window on the screen —
                # wears that class; given the discreet colour, focused and
                # unfocused looked alike (the owner saw it, 2026-09-28).
                faint = f"border.faint-{desk}"
                focused, focused_inactive, unfocused = "border.discreet", faint, faint
            (directory / f"scroll-{desk}-{borders}.conf").write_text(
                f"# GENERATED by nix/scripts/gen-theme.py for the theme `{theme_id}`. Do not edit.\n"
                f"# The desk `{desk}` and the `{borders}` borders.\n"
                "# Included by the system's compositor config; it is re-read on `reload`.\n"
                "#\n"
                "# `client.focused` takes FIVE colours and only two of them are ours to\n"
                "# pick: the border and the indicator. The rest are decorations that\n"
                "# `default_border pixel` never paints, so repeating the border colour\n"
                "# into them is honest rather than a guess.\n"
                "#\n"
                "# A LATER FILE SIMPLY WINS: `client.focused` overwrites the whole\n"
                "# setting and the desk is merged last-wins, so there is no node to\n"
                "# merge and no ordering to get right.\n"
                f"client.focused {five(focused)}\n"
                f"client.focused_inactive {five(focused_inactive)}\n"
                f"client.unfocused {five(unfocused)}\n"
                # THE SELECTED OUTLINE, the compositor's fixed green until now —
                # focused or not, the same colour: what it says is "marked", and
                # the borders' answer does not quiet it.
                f'client.selected {five("border.selected")}\n'
                f'client.selected_focused {five("border.selected")}\n'
                f"output * bg {hexes[ground]} solid_color\n"
            )
    # THE TERMINAL, one fragment per page its windows may stand on, each in
    # THAT page's palette: the base's on its own window, and a desk's page's
    # on the desk (`TERMINAL_BACKGROUND`), gated there.
    for page, fragment in TERMINAL_FRAGMENT.items():
        tones = {k: cs.format_hex(v.rgb8) for k, v in palettes[page].items()}
        lines = [
            f"# GENERATED by nix/scripts/gen-theme.py for the theme `{theme_id}`, on the `{page}` page. Do not edit.",
            f"background = {tones[TERMINAL_BACKGROUND[page]]}",
            f"foreground = {tones['terminal.foreground']}",
            f"cursor-color = {tones['terminal.cursor']}",
            f"cursor-text = {tones['terminal.cursor-text']}",
            f"selection-background = {tones['terminal.selection-background']}",
            f"selection-foreground = {tones['terminal.selection-foreground']}",
        ]
        for i, ansi in enumerate(ANSI_ORDER):
            lines.append(f"palette = {i}={tones[f'terminal.ansi.{ansi}']}")
        for i, ansi in enumerate(ANSI_ORDER):
            lines.append(f"palette = {i + 8}={tones[f'terminal.ansi.bright-{ansi}']}")
        (directory / fragment).write_text("\n".join(lines) + "\n")
    # THE WINDOWS' SPACING, two fragments — and the only compositor fragments
    # that are NOT colour, so they are the same bytes in every theme's
    # directory. The footer copies `with` or `none` (never neither) into
    # `scroll-gaps.conf`, which the compositor includes (`theme::fragments`,
    # `nix/modules/scroll.nix`). `with` RESTATES the standing gap rather than
    # leaving the file off, because a prior `none` has to be put back to it.
    #
    # THE EIGHT IS ALSO THE STATIC CONFIG'S, the fresh-machine default: a
    # machine with no `theme.json` has no fragment and shows the static `gaps`.
    # `themes-have-fragments` holds the two equal, so the generator and the
    # static config cannot drift apart on the gap.
    gaps_header = (
        f"# GENERATED by nix/scripts/gen-theme.py for the theme `{theme_id}`. Do not edit.\n"
        "# The windows' spacing; included by the system's compositor config and\n"
        "# re-read on `reload`. Held equal to the static config's own `gaps` by\n"
        "# `themes-have-fragments`.\n"
    )
    (directory / "scroll-gaps-with.conf").write_text(gaps_header + "gaps inner 8\ngaps outer 0\n")
    (directory / "scroll-gaps-none.conf").write_text(gaps_header + "gaps inner 0\ngaps outer 0\n")


# ---------------------------------------------------------------------------
# The report.
# ---------------------------------------------------------------------------


def report_markdown(results: dict[str, dict]) -> str:
    def counts(findings) -> dict[str, int]:
        return {lvl: sum(1 for f in findings if f.level == lvl) for lvl in ("Fail", "Warn", "Note", "OK")}

    def score(c: dict[str, int]) -> str:
        return f"Fail {c['Fail']} · Warn {c['Warn']} · Note {c['Note']} · OK {c['OK']}"

    every = [f for r in results.values() for pr in r["pages"].values() for f in pr["findings"]]
    palettes = sum(len(r["pages"]) for r in results.values())
    # LINE FIVE IS THE SCORE, of every palette at once: the line a person
    # reads after `nix build .#themes` (AGENTS.md).
    out = [
        "# Themes — the gates, measured",
        "",
        f"{palettes} palettes: {len(results)} themes × {', '.join(PAGES)} (the page each is solved on).",
        "",
        f"all · {score(counts(every))}",
        "",
    ]
    for theme_id, r in results.items():
        # THE POLARITY IS ON THE LINE, because every contrast row below is a
        # MAGNITUDE (`apca_magnitude`) and a magnitude cannot say which side
        # the ink is on. Without this a reviewer reading `Lc 87` has no way to
        # tell a light theme from a dark one anywhere in the report.
        out += [f"## {theme_id}", ""]
        for page, pr in r["pages"].items():
            out.append(f"{r['mode']} · on {page} · {score(counts(pr['findings']))}")
        out.append("")
        worth = [(page, f) for page, pr in r["pages"].items() for f in pr["findings"] if f.level != "OK"]
        if worth:
            out += ["| page | level | gate | subject | measured | threshold | note |", "|---|---|---|---|---|---|---|"]
            out += [f"| {page} | {f.level} | {f.gate} | {f.subject} | {f.measured:.4f} | {f.threshold} | {f.note} |" for page, f in worth]
            out.append("")
        if r["regression"]:
            out += ["### Regression against the frozen reference", "", "| key | ΔE_ok | generated | reference | level |", "|---|---|---|---|---|"]
            for f in sorted(r["regression"], key=lambda f: -f.measured):
                if f.gate == "regression" and not f.subject.endswith(" alpha"):
                    gen, _, ref = f.note.partition(" against ")
                    out.append(f"| {f.subject} | {f.measured:.4f} | {gen} | {ref.split(';')[0]} | {f.level} |")
            out.append("")
        out += ["### Colour vision (ΔE_ok between the state solids)", "", "| pair | protan | deutan | tritan |", "|---|---|---|---|"]
        for pair, values in r["cvd"].items():
            out.append(f"| {pair} | " + " | ".join(f"{values[d]:.4f}" for d in cs.DEFICIENCIES) + " |")
        out.append("")
        out += ["### Families", "", "| family | hue | solid | cusp L | cusp C | solid L | solid C |", "|---|---|---|---|---|---|---|"]
        for family, hue, solid in r["families"]:
            cusp_l, cusp_c = cs.cusp(hue, steps=200)
            out.append(f"| {family} | {hue:.0f} | {cs.format_hex(solid.rgb8)} | {cusp_l:.3f} | {cusp_c:.3f} | {solid.lch[0]:.3f} | {solid.lch[1]:.3f} |")
        out.append("")
        out += ["### Tokens", "", "| key | value | from |", "|---|---|---|"]
        out += [f"| {k} | `{t.token}` | {t.source} |" for k, t in r["tokens"].items()]
        out.append("")
        out += ["### Primitives", "", "| primitive | hex | L | C | h | anchor | shortfall |", "|---|---|---|---|---|---|---|"]
        for name, sw in r["primitives"].items():
            d = sw.describe()
            out.append(f"| {name} | {d['hex']} | {d['l']} | {d['c']} | {d['h']} | {d['anchor']} | {'' if d['shortfall'] is None else d['shortfall']} |")
        out.append("")
    return "\n".join(out)


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


#: THE PALETTES' FILE, as `ronoctua_tokens::palettes` reads it. Little-endian
#: throughout. A change of shape is a new VERSION: the reader refuses any other
#: one with a sentence, never a guess.
PALETTES_FILE = "palettes.bin"
PALETTES_MAGIC = b"RONOPAL\0"
PALETTES_VERSION = 1


def fnv1a64(data: bytes) -> int:
    """FNV-1a, 64 bits — the hash the palettes carry of their role names, and
    the one `ronoctua_tokens::palettes` computes of `color::NAMES` before it
    reads a byte of colour."""
    h = 0xCBF29CE484222325
    for byte in data:
        h ^= byte
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def palettes_bin(ids: list[str], default: str, modes: dict[str, str], palettes: dict[str, dict[str, dict[str, str]]], roles: list[str]) -> bytes:
    """`ids` in the chooser's order; `palettes[id][page][role]` the solved
    token (`0xRRGGBBAA`); `roles` in the order the Rust is compiled with.

    The header, then the data:

        magic          8 bytes, `RONOPAL\\0`
        version        u16
        pages          u16, then themes u16, roles u16, the default's index u16
        role hash      u64, FNV-1a of the role names joined by `\\n`
        each page      u8 length, its name
        each theme     u8 (1 when it is light), u8 length, its id
        data           [theme][page][role] -> R, G, B, A
    """
    out = bytearray(PALETTES_MAGIC)
    out += struct.pack("<HHHHH", PALETTES_VERSION, len(PAGES), len(ids), len(roles), ids.index(default))
    out += struct.pack("<Q", fnv1a64("\n".join(roles).encode("ascii")))
    for page in PAGES:
        out += bytes([len(page)]) + page.encode("ascii")
    for theme_id in ids:
        if modes[theme_id] not in ("dark", "light"):
            raise Refusal(f"the theme {theme_id!r} says its mode is {modes[theme_id]!r}")
        out += bytes([1 if modes[theme_id] == "light" else 0, len(theme_id)]) + theme_id.encode("ascii")
    for theme_id in ids:
        for page in PAGES:
            for role in roles:
                token = palettes[theme_id][page][role]
                m = re.fullmatch(r"0x([0-9A-Fa-f]{8})", token)
                if not m:
                    raise Refusal(f"{theme_id} on {page}: `{role}` came out as {token!r}, not 0xRRGGBBAA")
                out += int(m.group(1), 16).to_bytes(4, "big")
    return bytes(out)


def parse_pairs(items: list[str], what: str) -> dict[str, pathlib.Path]:
    out = {}
    for item in items:
        if "=" not in item:
            raise Refusal(f"--{what} wants id=path, got {item!r}")
        theme_id, path = item.split("=", 1)
        out[theme_id] = pathlib.Path(path)
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--themes", type=pathlib.Path, required=True, help="themes.json: the ordered list of ids")
    ap.add_argument("--seed", action="append", default=[], help="id=seed.json (one per theme)")
    ap.add_argument("--recipes", type=pathlib.Path, required=True, help="colors.json: the recipes")
    ap.add_argument(
        "--roles",
        type=pathlib.Path,
        required=True,
        help="token-roles.json from nix/token-names.nix: every role the Rust is compiled with, in its order",
    )
    ap.add_argument("--default", required=True, help="the id of the theme a fresh machine shows (assets/themes/themes.nix)")
    ap.add_argument("--reference", action="append", default=[], help="id=reference.json: a frozen palette to regress against")
    ap.add_argument("--out", type=pathlib.Path, required=True)
    ap.add_argument("--report-only", action="store_true", help="write the report and exit 0 even on a Fail (for iterating on seeds)")
    args = ap.parse_args()

    try:
        ids = json.loads(args.themes.read_text())
        if not isinstance(ids, list) or not ids or not all(isinstance(i, str) for i in ids):
            raise Refusal("themes.json must be a non-empty list of ids")
        for theme_id in ids:
            if not re.fullmatch(r"[a-z][a-z0-9-]*", theme_id):
                raise Refusal(f"the theme id {theme_id!r} is not kebab-case")
        if args.default not in ids:
            raise Refusal(f"--default {args.default!r} is not one of the declared themes: {ids}")
        declared = json.loads(args.roles.read_text())
        if not isinstance(declared, list) or not all(isinstance(r, str) for r in declared) or len(set(declared)) != len(declared):
            raise Refusal(f"{args.roles} is not a list of distinct role names")
        seeds = parse_pairs(args.seed, "seed")
        references = parse_pairs(args.reference, "reference")
        missing = [i for i in ids if i not in seeds]
        if missing:
            raise Refusal(f"no --seed for {missing}")
        stray = [i for i in seeds if i not in ids]
        if stray:
            raise Refusal(f"--seed for {stray}, which themes.json does not declare")

        recipes_raw = json.loads(args.recipes.read_text())
        recipes = flatten_recipes(recipes_raw)
        for key in recipes:
            check_key(key, "colors.nix")

        results: dict[str, dict] = {}
        key_sets: dict[str, list[str]] = {}
        failed = False
        for theme_id in ids:
            source = f"assets/themes/{theme_id}.nix"
            theme = Theme(theme_id, json.loads(seeds[theme_id].read_text()), source)
            merged = dict(recipes)
            for key, recipe in flatten_recipes(theme.overrides).items():
                if key not in merged:
                    raise Refusal(f"{source}: overrides `{key}`, which is not a token")
                merged[key] = recipe
            def solved(on: Theme) -> dict[str, Resolved]:
                out = {key: resolve_recipe(on, key, recipe, source) for key, recipe in merged.items()}
                for key, tok in shaped_tokens(on).items():
                    if key in out:
                        raise Refusal(f"the shape `{key}` collides with a recipe")
                    check_key(key, "gen-theme.py")
                    out[key] = tok
                return out

            tokens = solved(theme)
            primitives = theme.every_primitive()
            findings = run_gates(theme, tokens, primitives)
            regress = regression(theme, tokens, json.loads(references[theme_id].read_text())) if theme_id in references else []
            # THE PAGES: the same recipes solved again on each desk, and the
            # roles that never move copied from the base (`Theme.on_page`).
            pages = {"chrome": {"tokens": tokens, "primitives": primitives, "findings": findings + regress}}
            for page in PAGES[1:]:
                on = theme.on_page(page)
                page_tokens = solved(on)
                page_roles(page_tokens, tokens)
                page_primitives = on.every_primitive()
                pages[page] = {"tokens": page_tokens, "primitives": page_primitives, "findings": run_gates(on, page_tokens, page_primitives)}
            cvd = {}
            for i, a in enumerate(STATE_SOLIDS):
                for b in STATE_SOLIDS[i + 1 :]:
                    cvd[f"{a} vs {b}"] = {d: cs.cvd_separation(tokens[f"state.{a}"].rgb, tokens[f"state.{b}"].rgb, d) for d in cs.DEFICIENCIES}
            families = [(name, fam["hue"], theme.primitive(name, "solid")) for name, fam in theme.families.items()]
            results[theme_id] = {"mode": theme.mode, "tokens": tokens, "primitives": primitives, "findings": findings + regress, "regression": regress, "cvd": cvd, "families": families, "pages": pages}
            key_sets[theme_id] = sorted(tokens)
            for page, r in pages.items():
                if sorted(r["tokens"]) != key_sets[theme_id]:
                    raise Refusal(f"the page `{page}` of {theme_id} does not have the base's keys")
                for f in r["findings"]:
                    if f.level == "Fail":
                        failed = True
                        print(f"FAIL [{theme_id} on {page}] {f.gate}: {f.subject} = {f.measured:.4f}, wanted {f.threshold} {f.note}", file=sys.stderr)

        first = key_sets[ids[0]]
        for theme_id, keys in key_sets.items():
            if keys != first:
                raise Refusal(f"the theme {theme_id} does not have the same keys as {ids[0]}: {sorted(set(keys) ^ set(first))}")
        # THE ROLES SOLVED ARE THE ROLES THE RUST IS COMPILED WITH, both ways.
        # The evaluated list (`nix/token-names.nix`) is the attribute names of
        # colors.nix and shaped.nix; a role shaped here and named in neither
        # would be a colour no binary can index, and a name with no role
        # behind it a constant that draws the wrong bytes.
        unnamed = sorted(set(first) - set(declared))
        unsolved = sorted(set(declared) - set(first))
        if unnamed:
            raise Refusal(
                f"the generator solves {', '.join(unnamed)}, which neither assets/tokens/colors.nix nor "
                "assets/tokens/shaped.nix names: the role list is read at evaluation from those two files — "
                "a role this generator shapes is named in shaped.nix"
            )
        if unsolved:
            raise Refusal(
                f"{', '.join(unsolved)} {'is' if len(unsolved) == 1 else 'are'} named at evaluation and solved by nothing here: "
                "a name in assets/tokens/shaped.nix needs its rule in `shaped_tokens`"
            )
        if sorted(declared) != declared:
            raise Refusal(f"{args.roles} is not in sorted order, which is `color::NAMES`' order")
        # THE ROLES A THEME MUST NOT MOVE (`IDENTICAL_IN_EVERY_THEME`): the
        # same bytes in every theme, or the seeds disagree about a family that
        # has to be one.
        for theme_id in ids[1:]:
            moved = sorted(
                key
                for key in first
                if key.startswith(IDENTICAL_IN_EVERY_THEME)
                and results[theme_id]["tokens"][key].token != results[ids[0]]["tokens"][key].token
            )
            if moved:
                raise Refusal(
                    f"{', '.join(moved)} differ between {ids[0]} and {theme_id}: they are drawn on a film, the same in every theme — "
                    f"give assets/themes/{theme_id}.nix the same `{moved[0].split('.', 1)[0]}` family as assets/themes/{ids[0]}.nix"
                )

        args.out.mkdir(parents=True, exist_ok=True)
        combined = []
        for theme_id in ids:
            flat = {k: t.token for k, t in results[theme_id]["tokens"].items()}
            # THE MODE TRAVELS WITH THE COLOURS, because the shell has to know
            # it at run time: the absolute desk is named "black" in a dark
            # theme and "white" in a light one, and a colour cannot be asked
            # which side of the ladder it is on (`ronoctua_tokens::theme::is_light`).
            # AND EVERY PAGE'S PALETTE BESIDE IT, in `PAGES` order: the runtime
            # table is `[theme][page][role]` (`ronoctua_tokens::theme::Ground`).
            grounds = {page: {k: t.token for k, t in results[theme_id]["pages"][page]["tokens"].items()} for page in PAGES[1:]}
            combined.append({"id": theme_id, "mode": results[theme_id]["mode"], "colors": flat, "pages": grounds})
            (args.out / f"{theme_id}.json").write_text(json.dumps(flat, indent=2, sort_keys=True) + "\n")
            for page, colours in grounds.items():
                (args.out / f"{theme_id}.{page}.json").write_text(json.dumps(colours, indent=2, sort_keys=True) + "\n")
            (args.out / f"{theme_id}.primitives.json").write_text(
                json.dumps({k: v.describe() for k, v in results[theme_id]["primitives"].items()}, indent=2) + "\n"
            )
            write_fragments(args.out / theme_id, theme_id, results[theme_id]["mode"], {page: r["tokens"] for page, r in results[theme_id]["pages"].items()})
        (args.out / "themes.json").write_text(json.dumps(combined, indent=1) + "\n")
        # THE PALETTES THE BINARIES READ, in the compiled role order.
        (args.out / PALETTES_FILE).write_bytes(
            palettes_bin(
                ids,
                args.default,
                {entry["id"]: entry["mode"] for entry in combined},
                {entry["id"]: {"chrome": entry["colors"], **entry["pages"]} for entry in combined},
                declared,
            )
        )
        (args.out / "report.md").write_text(report_markdown(results))
        (args.out / "report.json").write_text(
            json.dumps({tid: {"pages": {page: [f.row() for f in pr["findings"]] for page, pr in r["pages"].items()}, "cvd": r["cvd"]} for tid, r in results.items()}, indent=1) + "\n"
        )
        for theme_id in ids:
            for page, pr in results[theme_id]["pages"].items():
                fs = pr["findings"]
                print(f"OK: {theme_id} on {page}: {len(pr['tokens'])} tokens, Fail {sum(f.level == 'Fail' for f in fs)}, Warn {sum(f.level == 'Warn' for f in fs)}, Note {sum(f.level == 'Note' for f in fs)}")
        if failed and not args.report_only:
            print("ERROR: a gate failed; see the lines above and report.md", file=sys.stderr)
            return 1
        return 0
    except Refusal as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
