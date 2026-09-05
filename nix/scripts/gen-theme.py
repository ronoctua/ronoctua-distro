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

`themes.json` — `[{"id", "colors": {flat key: "0xRRGGBBAA"}}, …]` in the
order of `assets/themes/themes.nix`, the first being the default; every theme
has the identical key set or this exits 1. Per theme: `<id>.json` (the flat
map), `<id>.primitives.json` (every primitive with L, C, h, its anchor and any
shortfall), the fragments consumers outside the shell read (`<id>/niri-*`,
`<id>/ghostty.conf`), and `report.md` + `report.json` with every gate's
margin and the regression against the frozen reference.

Any Fail exits 1 with the finding on stderr. Warn and Note are in the report.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import pathlib
import re
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
#: `element` is `surface.selection` (0.102, 0.39), `border` is
#: `border.inactive` (0.178, 0.46). `border-strong` was on no screen: it is the
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
NEUTRAL_PAINT = (
    ("window", 0.018),
    ("chrome-raised", 0.053),
    ("hover", 0.070),
    ("chrome-loud", 0.078),
    ("border", 0.103),
    ("border-strong", 0.178),
)
#: The one neutral step BELOW the chrome: the desk the compositor paints where
#: no window is, when the person asks for the theme's own instead of black
#: (`LEADER s t`). The window's step, mirrored: a window stands one step above
#: the chrome, the desk one step below it, so a window on the desk is two
#: steps up and the footer's chrome one. Its hue and relative chroma are the
#: seed's `backdrop` — the only neutral step with a hue of its own, because
#: this is the one surface that is MEANT to read as the theme's colour.
NEUTRAL_BACKDROP = ("backdrop", -0.018)
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
SEED_FIELDS = frozenset(("ground", "chrome", "neutral", "text", "backdrop", "chroma", "families", "overrides"))

#: Families the chrome spends, each of which gets a `context.*` group.
CONTEXT_FAMILIES = ("accent", "success", "warning", "error", "group", "subtle", "alternate", "brain")
#: Families that only content (the syntax palette) spends.
CONTENT_FAMILIES = ("slate", "sky", "azure", "cyan", "orange")
REQUIRED_FAMILIES = CONTEXT_FAMILIES + CONTENT_FAMILIES
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

#: The colours `nix/shots.nix` asserts NEVER appear on screen; no token may
#: come near them. Distance is Euclidean in encoded RGB, divided by √3; the
#: shots match with an 18% fuzz, so a quarter keeps a margin over it, and the
#: content floor sits just above the fuzz itself.
FILM_COLOURS = ("#FF00FF", "#00FFFF", "#FFFF00", "#FF6000")
#: The desk when the person wants none of the theme's colour under the
#: windows: the pixel switched off, by name.
BLACK = "#000000"
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
        self.ground = _number(seed["ground"], f"{source}: ground", 0.0, 1.0)
        self.chrome = _number(seed["chrome"], f"{source}: chrome", 0.0, 1.0)
        if not 0.0 < self.chrome < 1.0:
            raise Refusal(f"{source}: chrome must be strictly inside (0, 1)")
        if self.chrome <= self.ground:
            raise Refusal(f"{source}: the chrome ({self.chrome}) has to be lighter than the ground ({self.ground}) — the ladder climbs from it")
        for group, fields in (("neutral", ("hue", "tint")), ("text", ("hue", "chroma")), ("backdrop", ("hue", "chroma"))):
            if not isinstance(seed[group], dict) or set(seed[group]) != set(fields):
                raise Refusal(f"{source}: {group} must be an attribute set with exactly {list(fields)}")
        self.neutral_hue = _number(seed["neutral"]["hue"], f"{source}: neutral.hue", 0.0, 360.0) % 360.0
        self.tint = _number(seed["neutral"]["tint"], f"{source}: neutral.tint", 0.0, 1.0)
        self.text_hue = _number(seed["text"]["hue"], f"{source}: text.hue", 0.0, 360.0) % 360.0
        self.text_chroma = _number(seed["text"]["chroma"], f"{source}: text.chroma", 0.0, 1.0)
        self.backdrop_hue = _number(seed["backdrop"]["hue"], f"{source}: backdrop.hue", 0.0, 360.0) % 360.0
        self.backdrop_chroma = _number(seed["backdrop"]["chroma"], f"{source}: backdrop.chroma", 0.0, 1.0)
        self.multiplier = _number(seed["chroma"], f"{source}: chroma", 0.0, 2.0)
        if not isinstance(seed["overrides"], dict):
            raise Refusal(f"{source}: overrides must be an attribute set")
        self.overrides: dict = seed["overrides"]

        families = seed["families"]
        if not isinstance(families, dict):
            raise Refusal(f"{source}: families must be an attribute set")
        self.families: dict[str, dict[str, float]] = {}
        for name, fields in families.items():
            if not VALID_SEGMENT.match(name):
                raise Refusal(f"{source}: the family name {name!r} is not a WIT identifier")
            if not isinstance(fields, dict) or "hue" not in fields:
                raise Refusal(f"{source}: family {name!r} needs at least a hue")
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

    # -- the chrome, the reference of every solve --------------------------

    def chrome_swatch(self) -> Swatch:
        return self.primitive("neutral", "chrome")

    def _rel(self, base: float, scale: float) -> float:
        return min(1.0, max(0.0, base * self.multiplier * scale))

    def _solve_text(self, hue: float, rel: float, lc: float, anchor: str) -> Swatch:
        chrome = self.chrome_swatch()
        l, shortfall = cs.solve_lightness(
            ("apca", lc), lambda _: hue, rel, reference_lightness=chrome.lch[0], reference_rgb=chrome.rgb
        )
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        return Swatch(lch, rgb8, anchor, shortfall)

    def _paint(self, hue: float, rel: float, delta: float, anchor: str) -> Swatch:
        l, shortfall = cs.solve_lightness(("delta-l", delta), lambda _: hue, rel, reference_lightness=self.chrome)
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        return Swatch(lch, rgb8, anchor, shortfall)

    # -- primitives --------------------------------------------------------

    def primitive(self, family: str, step: str, scale: float = 1.0) -> Swatch:
        """The solved colour of `family.step`, with the step's relative chroma
        multiplied by `scale` (a recipe's `chroma`)."""
        key = (family, step, scale)
        if key in self._memo:
            return self._memo[key]
        swatch = self._neutral(step, scale) if family == "neutral" else self._chromatic(family, step, scale)
        self._memo[key] = swatch
        return swatch

    def _neutral(self, step: str, scale: float) -> Swatch:
        hue, tint = self.neutral_hue, self._rel(self.tint, scale)
        if step == "ground":
            lch, rgb8 = cs.screen_colour(self.ground, hue, tint)
            return Swatch(lch, rgb8, f"fixed L {self.ground}")
        if step == "chrome":
            lch, rgb8 = cs.screen_colour(self.chrome, hue, tint)
            return Swatch(lch, rgb8, f"fixed L {self.chrome}")
        for name, delta in NEUTRAL_PAINT:
            if name == step:
                return self._paint(hue, tint, delta, f"chrome + {delta}")
        if step == NEUTRAL_BACKDROP[0]:
            delta = NEUTRAL_BACKDROP[1]
            return self._paint(self.backdrop_hue, self._rel(self.backdrop_chroma, scale), delta, f"chrome {delta:+}, h {self.backdrop_hue:.0f}")
        for name, lc in TEXT_STEPS:
            if name == step:
                return self._solve_text(self.text_hue, self._rel(self.text_chroma, scale), lc, f"Lc {lc} on chrome")
        raise Refusal(f"{self.source}: the neutral family has no step {step!r}")

    def _chromatic(self, family: str, step: str, scale: float) -> Swatch:
        if family not in self.families:
            raise Refusal(f"{self.source}: no family {family!r} in the seed")
        fam = self.families[family]
        hue = fam["hue"]
        for name, delta, rel in PAINT_STEPS:
            if name == step:
                # The torsion turns the PAINT steps only: a ground is not a solid.
                turned = (hue + fam["torsion"]) % 360.0
                return self._paint(turned, self._rel(rel * fam["paint-chroma"], scale), delta, f"chrome + {delta}, h {turned:.0f}")
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
        for step in ("ground", "chrome", NEUTRAL_BACKDROP[0], *(n for n, _ in NEUTRAL_PAINT), *(n for n, _ in TEXT_STEPS)):
            out[f"neutral.{step}"] = self.primitive("neutral", step)
        for family in self.families:
            for step in (*(n for n, _, _ in PAINT_STEPS), *(n for n, _, _ in SOLID_STEPS), *(n for n, _ in TEXT_STEPS), "on-solid"):
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


def shaped_tokens(theme: Theme) -> dict[str, Resolved]:
    out: dict[str, Resolved] = {}

    def put(key: str, swatch: Swatch, source: str) -> None:
        out[key] = Resolved(swatch, 255, source)

    for family in CONTEXT_FAMILIES:
        for name, step in CONTEXT_SHAPE:
            put(f"context.{family}.{name}", theme.primitive(family, step), f"{family}.{step}")
    for name, step in NEUTRAL_SHAPE:
        put(f"context.neutral.{name}", theme.primitive("neutral", step), f"neutral.{step}")
    for name, family, step in TERMINAL:
        put(f"terminal.{name}", theme.primitive(family, step), f"{family}.{step}")

    accent = theme.families["accent"]
    rel = min(1.0, CHART_CHROMA * theme.multiplier)
    hues = cs.even_delta_e_hues(CHART_COUNT, accent["hue"], CHART_MEASURE_L, rel)
    for i, hue in enumerate(hues):
        l = 0.50 + 0.40 * i / (CHART_COUNT - 1)
        lch, rgb8 = cs.screen_colour(l, hue, rel)
        put(f"chart.{WORDS[i + 1]}", Swatch(lch, rgb8, f"h {hue:.0f}, L {l:.2f}, equal ΔE arcs from the accent"), "computed")

    bottom = theme.primitive("accent", "ground").lch[0]
    for k in range(11):
        t = k / 10.0
        l = bottom + (LEVEL_TOP_L - bottom) * t
        rel_k = min(1.0, (0.3 + 0.6 * math.sin(math.pi * t)) * theme.multiplier)
        hue = (accent["hue"] + accent["torsion"] * (1.0 - t)) % 360.0
        lch, rgb8 = cs.screen_colour(l, hue, rel_k)
        put(f"level.{WORDS[k]}", Swatch(lch, rgb8, f"accent hue, L {l:.2f}, rel C {rel_k:.2f}"), "computed")
    return out


# ---------------------------------------------------------------------------
# Gates. Data first; the code only measures.
# ---------------------------------------------------------------------------

SIX_GROUNDS = ("surface.chrome", "modeline.background", "notification.background", "surface.hover", "surface.buffer", "surface.base")

#: (text key, ground keys, Fail below, Warn below). Lc = |APCA|.
CONTRAST_GATES = (
    ("text.primary", (*SIX_GROUNDS, "surface.selection"), 75, 82),
    ("text.secondary", SIX_GROUNDS, 43, 45),
    ("text.secondary", ("surface.selection",), 40, 45),
    ("text.dim", SIX_GROUNDS[:5], 15, 20),
    # ON THE BAND TOO: the file manager lifts its dim fields to secondary there
    # (`lift_onto_a_band`), but the notification manager's summary column and
    # the update window's step glyphs are dim on a banded row and lift nothing.
    ("text.dim", ("surface.selection",), 15, 20),
    ("state.accent", ("surface.selection",), 45, 48),
    ("text.group", ("surface.selection",), 45, 50),
    ("text.subtle.strong", ("surface.chrome",), 40, 45),
    ("text.subtle.default", ("surface.chrome",), 18, 20),
    ("text.subtle.faint", ("surface.chrome",), 7, 8),
    ("text.subtle.alternate", ("surface.chrome",), 38, 42),
    # THE WINDOW'S HINT LINE draws the same register on the buffer's ground,
    # which is a hair lighter than the chrome by construction; the floors hold
    # and there is no warn band, because the hair is the ladder's own.
    ("text.subtle.strong", ("surface.buffer",), 40, 40),
    ("text.subtle.default", ("surface.buffer",), 18, 18),
    ("text.subtle.faint", ("surface.buffer",), 7, 7),
    ("text.subtle.alternate", ("surface.buffer",), 38, 38),
    ("text.group", ("surface.chrome",), 45, 55),
    ("brain.text", ("brain.background",), 90, 95),
    ("brain.dim", ("brain.background",), 44, 46),
    ("brain.countdown", ("brain.background",), 60, 70),
    ("modeline.badgeText", ("modeline.badge",), 45, 50),
    ("modeline.badge", ("modeline.background",), 40, 45),
    ("modeline.state.normal", ("modeline.background",), 15, 20),
    ("modeline.state.alternate", ("modeline.background",), 45, 60),
    ("notification.quiet", ("notification.background",), 43, 45),
    ("notification.loud", ("notification.background",), 45, 60),
    ("notification.countdown", ("notification.background",), 15, 20),
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
)
SYNTAX_LOUD_FLOOR = (45, 48)
CONTEXT_TEXT_FLOOR, CONTEXT_ON_FLOOR = (75, 80), 45
TERMINAL_FLOOR, TERMINAL_DIM_FLOOR = (40, 45), (15, 20)

#: Pairs that must be TOLD APART: (a, b, Fail below, Warn below), in ΔE_ok.
DISTANCE_GATES = (
    ("surface.base", "surface.chrome", 0.10, 0.15),
    # The desk is the ALTERNATIVE to black: one the eye cannot tell from black
    # is a choice that does nothing. And the footer and a window both stand on
    # it, so each needs an edge of its own against it.
    ("surface.backdrop", "surface.base", 0.10, 0.12),
    ("surface.backdrop", "surface.chrome", 0.02, 0.03),
    ("surface.backdrop", "surface.buffer", 0.03, 0.04),
    ("surface.chrome", "modeline.background", 0.03, 0.04),
    ("modeline.background", "notification.background", 0.02, 0.024),
    ("surface.chrome", "surface.hover", 0.05, 0.06),
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
    ("text.subtle.strong", "text.subtle.default", 0.10, 0.12),
    ("text.subtle.default", "text.subtle.faint", 0.10, 0.11),
    ("text.subtle.strong", "text.secondary", 0.03, 0.04),
    ("text.subtle.default", "text.dim", 0.03, 0.04),
    ("text.subtle.strong", "brain.dim", 0.03, 0.04),
    ("text.subtle.alternate", "text.subtle.strong", 0.03, 0.04),
    ("state.error", "state.critical", 0.04, 0.05),
)
#: Pairs reported but never gated: they are never drawn side by side.
DISTANCE_NOTES = (
    ("surface.chrome", "surface.buffer"),
    ("surface.hover", "notification.background"),
    ("brain.background", "modeline.background"),
)
LADDER_MIN_STEP, LADDER_MAX_STEP = 0.012, 0.26
HUE_GAP = (14.0, 20.0)
CVD_WARN = 0.05
CHART_GAP, LEVEL_GAP = (cs.JND, 0.05), 0.012


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
    findings: list[Finding] = []

    def lc(text: str, ground: str) -> float:
        return cs.apca_magnitude(tokens[text].rgb, tokens[ground].rgb)

    def de(a: str, b: str) -> float:
        return cs.delta_e_ok(tokens[a].lab, tokens[b].lab)

    for text, grounds, fail, warn in CONTRAST_GATES:
        for ground in grounds:
            v = lc(text, ground)
            findings.append(Finding(_level(v, fail, warn), "contrast", f"{text} on {ground}", v, f"Lc ≥ {fail} (warn < {warn})"))
    for key in tokens:
        if key.startswith("syntax.") and key != "syntax.comment":
            v = lc(key, "surface.buffer")
            findings.append(Finding(_level(v, *SYNTAX_LOUD_FLOOR), "contrast", f"{key} on surface.buffer", v, f"Lc ≥ {SYNTAX_LOUD_FLOOR[0]}"))
    for family in CONTEXT_FAMILIES:
        v = lc(f"context.{family}.text", f"context.{family}.ground")
        findings.append(Finding(_level(v, *CONTEXT_TEXT_FLOOR), "contrast", f"context.{family}.text on .ground", v, f"Lc ≥ {CONTEXT_TEXT_FLOOR[0]}"))
        v = lc(f"context.{family}.on", f"context.{family}.solid")
        findings.append(Finding(_level(v, CONTEXT_ON_FLOOR, CONTEXT_ON_FLOOR), "contrast", f"context.{family}.on on .solid", v, f"Lc ≥ {CONTEXT_ON_FLOOR}"))
    for name, _, _ in TERMINAL:
        if name.startswith("ansi.") and name != "ansi.black":
            floor = TERMINAL_DIM_FLOOR if name == "ansi.bright-black" else TERMINAL_FLOOR
            v = lc(f"terminal.{name}", "terminal.background")
            findings.append(Finding(_level(v, *floor), "contrast", f"terminal.{name} on terminal.background", v, f"Lc ≥ {floor[0]}"))

    for a, b, fail, warn in DISTANCE_GATES:
        v = de(a, b)
        findings.append(Finding(_level(v, fail, warn), "distance", f"{a} vs {b}", v, f"ΔE ≥ {fail} (warn < {warn})"))
    for a, b in DISTANCE_NOTES:
        findings.append(Finding("Note", "distance", f"{a} vs {b}", de(a, b), "reported only: never adjacent"))

    ladders = [("neutral", ("ground", "chrome", "window", "chrome-raised", "chrome-loud")), ("neutral", ("ground", "backdrop", "chrome")), ("neutral", ("chrome", "hover", "border", "border-strong")), ("neutral", tuple(n for n, _ in TEXT_STEPS))]
    for family in theme.families:
        ladders.append((family, tuple(n for n, _, _ in PAINT_STEPS)))
        ladders.append((family, tuple(n for n, _ in TEXT_STEPS)))
    for family, steps in ladders:
        for lower, upper in zip(steps, steps[1:]):
            a, b = primitives[f"{family}.{lower}"], primitives[f"{family}.{upper}"]
            gap = cs.delta_e_ok(a.lab, b.lab)
            level = "Fail" if gap < LADDER_MIN_STEP else "Warn" if gap > LADDER_MAX_STEP else "OK"
            findings.append(Finding(level, "ladder", f"{family}.{lower} → {family}.{upper}", gap, f"{LADDER_MIN_STEP} ≤ ΔE ≤ {LADDER_MAX_STEP}"))
            if b.lch[0] < a.lch[0] - 1e-9:
                findings.append(Finding("Fail", "ladder", f"{family}.{upper} is darker than {family}.{lower}", b.lch[0] - a.lch[0], "L monotone"))
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
    levels = [f"level.{WORDS[k]}" for k in range(11)]
    for a, b in zip(levels, levels[1:]):
        v = de(a, b)
        findings.append(Finding("Fail" if v < LEVEL_GAP else "OK", "ordinal", f"{a} → {b}", v, f"ΔE ≥ {LEVEL_GAP}"))

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


def write_fragments(directory: pathlib.Path, theme_id: str, tokens: dict[str, Resolved]) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    hexes = {k: cs.format_hex(v.rgb8) for k, v in tokens.items()}
    # niri: three values for the static config, and TWO fragments per theme,
    # one per answer to "what is under the windows" — the footer copies the one
    # the person chose into the configuration directory when the theme or the
    # answer changes. Each carries the border as a `window-rule` (a repeatable
    # node; the last one wins over the static `layout { border { … } }`) and
    # the desk as a `layout { background-color }`: niri refuses a second
    # `layout` inside ONE file but merges the one an included file brings,
    # field by field, the later value winning (`niri-config`, `MergeWith`).
    # `black` is spelled as the colour it names, not as `surface.base`: the
    # option is "black", and a theme whose ground is not black keeps it so.
    (directory / "niri-border-active").write_text(hexes["state.accent"])
    (directory / "niri-border-inactive").write_text(hexes["border.inactive"])
    (directory / "niri-background").write_text(hexes["surface.base"])
    for name, desk in (("niri-black.kdl", BLACK), ("niri-backdrop.kdl", hexes["surface.backdrop"])):
        (directory / name).write_text(
            f"// GENERATED by nix/scripts/gen-theme.py for the theme `{theme_id}`. Do not edit.\n"
            "// Included by the system's niri config with `optional=true`; niri reloads it on change.\n"
            "window-rule {\n"
            "    border {\n"
            f'        active-color "{hexes["state.accent"]}"\n'
            f'        inactive-color "{hexes["border.inactive"]}"\n'
            "    }\n"
            "}\n"
            "layout {\n"
            f'    background-color "{desk}"\n'
            "}\n"
        )
    lines = [
        f"# GENERATED by nix/scripts/gen-theme.py for the theme `{theme_id}`. Do not edit.",
        f"background = {hexes['terminal.background']}",
        f"foreground = {hexes['terminal.foreground']}",
        f"cursor-color = {hexes['terminal.cursor']}",
        f"cursor-text = {hexes['terminal.cursor-text']}",
        f"selection-background = {hexes['terminal.selection-background']}",
        f"selection-foreground = {hexes['terminal.selection-foreground']}",
    ]
    for i, name in enumerate(ANSI_ORDER):
        lines.append(f"palette = {i}={hexes[f'terminal.ansi.{name}']}")
    for i, name in enumerate(ANSI_ORDER):
        lines.append(f"palette = {i + 8}={hexes[f'terminal.ansi.bright-{name}']}")
    (directory / "ghostty.conf").write_text("\n".join(lines) + "\n")


# ---------------------------------------------------------------------------
# The report.
# ---------------------------------------------------------------------------


def report_markdown(results: dict[str, dict]) -> str:
    out = ["# Themes — the gates, measured", ""]
    for theme_id, r in results.items():
        counts = {lvl: sum(1 for f in r["findings"] if f.level == lvl) for lvl in ("Fail", "Warn", "Note", "OK")}
        out += [f"## {theme_id}", "", f"Fail {counts['Fail']} · Warn {counts['Warn']} · Note {counts['Note']} · OK {counts['OK']}", ""]
        worth = [f for f in r["findings"] if f.level != "OK"]
        if worth:
            out += ["| level | gate | subject | measured | threshold | note |", "|---|---|---|---|---|---|"]
            out += [f"| {f.level} | {f.gate} | {f.subject} | {f.measured:.4f} | {f.threshold} | {f.note} |" for f in worth]
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
            tokens = {key: resolve_recipe(theme, key, recipe, source) for key, recipe in merged.items()}
            for key, tok in shaped_tokens(theme).items():
                if key in tokens:
                    raise Refusal(f"the shape `{key}` collides with a recipe")
                check_key(key, "gen-theme.py")
                tokens[key] = tok
            primitives = theme.every_primitive()
            findings = run_gates(theme, tokens, primitives)
            regress = regression(theme, tokens, json.loads(references[theme_id].read_text())) if theme_id in references else []
            cvd = {}
            for i, a in enumerate(STATE_SOLIDS):
                for b in STATE_SOLIDS[i + 1 :]:
                    cvd[f"{a} vs {b}"] = {d: cs.cvd_separation(tokens[f"state.{a}"].rgb, tokens[f"state.{b}"].rgb, d) for d in cs.DEFICIENCIES}
            families = [(name, fam["hue"], theme.primitive(name, "solid")) for name, fam in theme.families.items()]
            results[theme_id] = {"tokens": tokens, "primitives": primitives, "findings": findings + regress, "regression": regress, "cvd": cvd, "families": families}
            key_sets[theme_id] = sorted(tokens)
            for f in findings + regress:
                if f.level == "Fail":
                    failed = True
                    print(f"FAIL [{theme_id}] {f.gate}: {f.subject} = {f.measured:.4f}, wanted {f.threshold} {f.note}", file=sys.stderr)

        first = key_sets[ids[0]]
        for theme_id, keys in key_sets.items():
            if keys != first:
                raise Refusal(f"the theme {theme_id} does not have the same keys as {ids[0]}: {sorted(set(keys) ^ set(first))}")

        args.out.mkdir(parents=True, exist_ok=True)
        combined = []
        for theme_id in ids:
            flat = {k: t.token for k, t in results[theme_id]["tokens"].items()}
            combined.append({"id": theme_id, "colors": flat})
            (args.out / f"{theme_id}.json").write_text(json.dumps(flat, indent=2, sort_keys=True) + "\n")
            (args.out / f"{theme_id}.primitives.json").write_text(
                json.dumps({k: v.describe() for k, v in results[theme_id]["primitives"].items()}, indent=2) + "\n"
            )
            write_fragments(args.out / theme_id, theme_id, results[theme_id]["tokens"])
        (args.out / "themes.json").write_text(json.dumps(combined, indent=1) + "\n")
        (args.out / "report.md").write_text(report_markdown(results))
        (args.out / "report.json").write_text(
            json.dumps({tid: {"findings": [f.row() for f in r["findings"]], "cvd": r["cvd"]} for tid, r in results.items()}, indent=1) + "\n"
        )
        for theme_id in ids:
            fs = results[theme_id]["findings"]
            print(f"OK: {theme_id}: {len(results[theme_id]['tokens'])} tokens, Fail {sum(f.level == 'Fail' for f in fs)}, Warn {sum(f.level == 'Warn' for f in fs)}, Note {sum(f.level == 'Note' for f in fs)}")
        if failed and not args.report_only:
            print("ERROR: a gate failed; see the lines above and report.md", file=sys.stderr)
            return 1
        return 0
    except Refusal as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
