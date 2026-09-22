#!/usr/bin/env python3
"""Colour science for the theme generator — standard library only.

The theme generator (`gen-theme.py`) never writes a colour: it writes HUES,
RELATIVE CHROMAS and CONTRAST TARGETS, and every value on screen is solved
from them here. This module is the whole of that arithmetic, ported from the
owner's `noctua-colors` (`crates/noctua-core`), which carries the reasoning in
full; the short version of each decision is beside the function that makes it.

# What is in here, and why each piece exists

- sRGB <-> linear <-> Oklab <-> OKLCH. The Oklab matrices are the published
  ones, CONDITIONED so that white is `L = 1` exactly and every grey has zero
  chroma exactly. The published numbers carry a rounding residual (D65 gives
  `L = 0.9999988`), invisible to the eye and fatal to a generator that anchors
  lightness steps on "this IS white".
- `max_chroma(L, h)`: the sRGB boundary at a lightness and hue, SOLVED, not
  searched. At fixed `L` and `h` each cone response is affine in chroma, so
  each linear channel is an exact cubic in chroma; the boundary is the
  smallest positive root of six cubics. A bisection is wrong here — the gamut
  is not convex in Oklab, and near the blue primary the red channel crosses
  zero three times along one ray, so a search converges to whichever crossing
  it happens to bracket (a 19% jump between hue 264.0 and 264.1).
- `map_into_gamut`: keep `L` and `h`, clamp chroma to `max_chroma`. For every
  chroma at or below the boundary, and for every colour outside the gamut, this
  is what the CSS Color 4 bisection gives once its result is finished against
  the analytic boundary (the reference does that finish for a reason:
  reporting the bisection's candidate leaves coordinates out of gamut, and
  reporting the clipped colour moves hue by up to 15°). The one place the two
  differ is the non-convex islands near the blue primary — a ray that has left
  the gamut and come back in — where the bisection would keep the island and
  this clamps to the FIRST exit; relative chroma never asks for an island, and
  a ramp that jumped into one would show the kink the boundary exists to
  remove. Idempotent by construction.
- APCA-W3 0.1.9, signed: positive is dark-on-light, negative is
  light-on-dark. `Lc(a, b) != -Lc(b, a)` on purpose. Its luminance is a plain
  2.4 power and NOT the piecewise sRGB decode: the constants were fitted
  against that curve and "correcting" it shifts every Lc.
- ΔE_ok: plain Euclidean distance in Oklab. The just-noticeable difference
  used throughout is 0.02.
- `solve_lightness`: the lightness at which a colour of a family meets an
  anchor — a fixed `L`, a lightness separation from a reference, or an APCA
  target against a reference. The APCA case runs the WHOLE pipeline at every
  candidate (hue torsion, relative chroma, gamut, 8-bit quantisation),
  because chroma is a fraction of a boundary that depends on lightness and
  chroma moves luminance, which moves contrast: the search is over the thing
  that reaches the screen.
- Brettel colour-vision simulation (protan, deutan, tritan), for the REPORT.
  The one place in this file where a per-channel clamp is right: a projected
  saturated blue comes back with a negative red channel, and Oklab has no
  meaningful hue there to preserve.

Performance is irrelevant: a theme is a few hundred solves of forty bisection
steps each, well under a second.
"""

from __future__ import annotations

import math
import sys
from typing import Callable, Sequence

# ---------------------------------------------------------------------------
# Types. Plain tuples: nothing here needs a class, and tuples hash and compare.
# ---------------------------------------------------------------------------

#: Encoded sRGB, each channel in `[0, 1]`.
Rgb = tuple[float, float, float]
#: Linear-light sRGB, each channel in `[0, 1]` when inside the gamut.
Linear = tuple[float, float, float]
#: Oklab: lightness, then the two opponent axes.
Lab = tuple[float, float, float]
#: OKLCH: lightness, chroma, hue in degrees `[0, 360)`.
Lch = tuple[float, float, float]
Mat3 = tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]
Vec3 = tuple[float, float, float]

#: The just-noticeable difference in ΔE_ok. Two colours closer than this are
#: one colour to a viewer; the generator's distance gates are expressed in it.
JND = 0.02

# ---------------------------------------------------------------------------
# Small matrix algebra, so the inverses are DERIVED rather than transcribed.
# ---------------------------------------------------------------------------


def _mul_vec(m: Mat3, v: Vec3) -> Vec3:
    return (
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    )


def _mul(a: Mat3, b: Mat3) -> Mat3:
    return tuple(  # type: ignore[return-value]
        tuple(sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)) for i in range(3)
    )


def _inverse(m: Mat3) -> Mat3:
    (a, b, c), (d, e, f), (g, h, i) = m
    det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
    if det == 0.0:
        raise ZeroDivisionError("singular matrix")
    return (
        ((e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det),
        ((f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det),
        ((d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det),
    )


def _signed_cube_root(x: float) -> float:
    return math.copysign(abs(x) ** (1.0 / 3.0), x)


#: A real cube root that keeps the sign, so an out-of-gamut cone response
#: (negative) still has a defined Oklab image. `math.cbrt` arrived in 3.11;
#: the fallback is for an older interpreter and is resolved once, at import.
_cbrt = math.cbrt if hasattr(math, "cbrt") else _signed_cube_root


# ---------------------------------------------------------------------------
# sRGB. The primaries are the eight numbers everything else is derived from.
# ---------------------------------------------------------------------------

_D65_XY = (0.3127, 0.3290)
_SRGB_PRIMARIES = ((0.64, 0.33), (0.30, 0.60), (0.15, 0.06))


def _xyz_of_chromaticity(xy: tuple[float, float]) -> Vec3:
    x, y = xy
    return (x / y, 1.0, (1.0 - x - y) / y)


def _rgb_to_xyz_matrix(
    primaries: tuple[tuple[float, float], ...], white_xy: tuple[float, float]
) -> Mat3:
    """The standard construction: columns are the primaries, scaled so that
    RGB white lands on the reference white."""
    columns = [_xyz_of_chromaticity(p) for p in primaries]
    p: Mat3 = tuple(tuple(columns[j][i] for j in range(3)) for i in range(3))  # type: ignore[assignment]
    scale = _mul_vec(_inverse(p), _xyz_of_chromaticity(white_xy))
    return tuple(  # type: ignore[return-value]
        tuple(p[i][j] * scale[j] for j in range(3)) for i in range(3)
    )


_D65_XYZ: Vec3 = _xyz_of_chromaticity(_D65_XY)
_SRGB_TO_XYZ: Mat3 = _rgb_to_xyz_matrix(_SRGB_PRIMARIES, _D65_XY)
_XYZ_TO_SRGB: Mat3 = _inverse(_SRGB_TO_XYZ)


def srgb_to_linear(c: float) -> float:
    """The sRGB electro-optical transfer, sign-preserving."""
    a = abs(c)
    lin = a / 12.92 if a <= 0.04045 else ((a + 0.055) / 1.055) ** 2.4
    return math.copysign(lin, c)


def linear_to_srgb(c: float) -> float:
    """The inverse of [`srgb_to_linear`], sign-preserving."""
    a = abs(c)
    enc = a * 12.92 if a <= 0.0031308 else 1.055 * a ** (1.0 / 2.4) - 0.055
    return math.copysign(enc, c)


# ---------------------------------------------------------------------------
# Oklab. Published matrices, then conditioned.
# ---------------------------------------------------------------------------

_XYZ_TO_LMS_PUBLISHED: Mat3 = (
    (0.8189330101, 0.3618667424, -0.1288597137),
    (0.0329845436, 0.9293118715, 0.0361456387),
    (0.0482003018, 0.2643662691, 0.6338517070),
)
_LMS_TO_OKLAB_PUBLISHED: Mat3 = (
    (0.2104542553, 0.7936177850, -0.0040720468),
    (1.9779984951, -2.4285922050, 0.4505937099),
    (0.0259040371, 0.7827717662, -0.8086757660),
)


def _condition_white(m: Mat3, white: Vec3) -> Mat3:
    """Rescales each row so the reference white maps to unit cone response."""
    w = _mul_vec(m, white)
    return tuple(  # type: ignore[return-value]
        tuple(m[i][j] / w[i] for j in range(3)) for i in range(3)
    )


def _condition_achromatic_axis(m: Mat3) -> Mat3:
    """Forces row 0 to sum to exactly 1 and rows 1 and 2 to sum to exactly 0.

    Given unit cone response, lightness is the sum of row 0 and the opponent
    axes are the sums of rows 1 and 2; with these sums exact, "white is L = 1"
    and "grey has no chroma" hold by construction. The corrections are of the
    order of 1e-8.
    """
    scale = 1.0 / (m[0][0] + m[0][1] + m[0][2])
    shift_a = (m[1][0] + m[1][1] + m[1][2]) / 3.0
    shift_b = (m[2][0] + m[2][1] + m[2][2]) / 3.0
    return (
        (m[0][0] * scale, m[0][1] * scale, m[0][2] * scale),
        (m[1][0] - shift_a, m[1][1] - shift_a, m[1][2] - shift_a),
        (m[2][0] - shift_b, m[2][1] - shift_b, m[2][2] - shift_b),
    )


_XYZ_TO_LMS: Mat3 = _condition_white(_XYZ_TO_LMS_PUBLISHED, _D65_XYZ)
_LMS_TO_OKLAB: Mat3 = _condition_achromatic_axis(_LMS_TO_OKLAB_PUBLISHED)
_LMS_TO_XYZ: Mat3 = _inverse(_XYZ_TO_LMS)
_OKLAB_TO_LMS: Mat3 = _inverse(_LMS_TO_OKLAB)

#: Linear sRGB straight to cone response, skipping the XYZ hop.
_LINEAR_TO_LMS: Mat3 = _mul(_XYZ_TO_LMS, _SRGB_TO_XYZ)
_LMS_TO_LINEAR: Mat3 = _mul(_XYZ_TO_SRGB, _LMS_TO_XYZ)


def linear_to_oklab(rgb: Linear) -> Lab:
    """Linear sRGB to Oklab."""
    lms = _mul_vec(_LINEAR_TO_LMS, rgb)
    return _mul_vec(_LMS_TO_OKLAB, (_cbrt(lms[0]), _cbrt(lms[1]), _cbrt(lms[2])))


def oklab_to_linear(lab: Lab) -> Linear:
    """Oklab to linear sRGB. Out-of-gamut colours come back with channels
    outside `[0, 1]`; nothing here clips."""
    root = _mul_vec(_OKLAB_TO_LMS, lab)
    return _mul_vec(_LMS_TO_LINEAR, (root[0] ** 3, root[1] ** 3, root[2] ** 3))


def rgb_to_oklab(rgb: Rgb) -> Lab:
    """Encoded sRGB to Oklab."""
    return linear_to_oklab((srgb_to_linear(rgb[0]), srgb_to_linear(rgb[1]), srgb_to_linear(rgb[2])))


def oklab_to_rgb(lab: Lab) -> Rgb:
    """Oklab to encoded sRGB, each channel clipped into `[0, 1]`.

    Clipping here is a formatting step, not a mapping step: callers that care
    about staying in gamut go through [`map_into_gamut`] first, after which
    the clip only removes rounding dust."""
    lin = oklab_to_linear(lab)
    return tuple(min(1.0, max(0.0, linear_to_srgb(c))) for c in lin)  # type: ignore[return-value]


def oklab_to_oklch(lab: Lab) -> Lch:
    """Oklab to OKLCH, hue in `[0, 360)`."""
    l, a, b = lab
    c = math.hypot(a, b)
    h = math.degrees(math.atan2(b, a)) % 360.0 if c > 1e-12 else 0.0
    return (l, c, h)


def oklch_to_oklab(lch: Lch) -> Lab:
    """OKLCH to Oklab."""
    l, c, h = lch
    r = math.radians(h)
    return (l, c * math.cos(r), c * math.sin(r))


def oklch_to_rgb(lch: Lch) -> Rgb:
    """OKLCH to encoded sRGB, clipped. See [`oklab_to_rgb`]."""
    return oklab_to_rgb(oklch_to_oklab(lch))


def delta_e_ok(a: Lab, b: Lab) -> float:
    """Euclidean distance in Oklab, deliberately unweighted."""
    return math.sqrt((a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2)


def hue_difference(a: float, b: float) -> float:
    """The shorter arc between two hues, in degrees `[0, 180]`."""
    d = abs(a - b) % 360.0
    return 360.0 - d if d > 180.0 else d


# ---------------------------------------------------------------------------
# Bytes. One rounding, in one place.
# ---------------------------------------------------------------------------


def quantize(rgb: Rgb) -> tuple[int, int, int]:
    """Encoded sRGB to 8-bit channels, round half up, clamped."""
    return tuple(min(255, max(0, int(math.floor(c * 255.0 + 0.5)))) for c in rgb)  # type: ignore[return-value]


def dequantize(rgb8: Sequence[int]) -> Rgb:
    """8-bit channels back to encoded sRGB in `[0, 1]`."""
    return (rgb8[0] / 255.0, rgb8[1] / 255.0, rgb8[2] / 255.0)


def parse_token(value: str) -> tuple[tuple[int, int, int], int]:
    """`0xRRGGBBAA` (the tokens' format) or `#RRGGBB[AA]` to `((r, g, b), a)`."""
    v = value.strip()
    if v.startswith(("0x", "0X")):
        digits = v[2:]
    elif v.startswith("#"):
        digits = v[1:]
    else:
        raise ValueError(f"not a colour literal: {value!r}")
    if len(digits) == 6:
        digits += "FF"
    if len(digits) != 8 or any(ch not in "0123456789abcdefABCDEF" for ch in digits):
        raise ValueError(f"not a colour literal: {value!r}")
    n = int(digits, 16)
    return ((n >> 24) & 0xFF, (n >> 16) & 0xFF, (n >> 8) & 0xFF), n & 0xFF


def format_token(rgb8: Sequence[int], alpha8: int = 255) -> str:
    """`((r, g, b), a)` to the tokens' `0xRRGGBBAA`."""
    r, g, b = rgb8
    return f"0x{r:02X}{g:02X}{b:02X}{alpha8:02X}"


def format_hex(rgb8: Sequence[int]) -> str:
    """`(r, g, b)` to `#RRGGBB`, what the compositor, Ghostty and CSS read."""
    r, g, b = rgb8
    return f"#{r:02X}{g:02X}{b:02X}"


# ---------------------------------------------------------------------------
# The gamut boundary, solved.
# ---------------------------------------------------------------------------

#: Nothing in sRGB reaches this chroma; it bounds the search from above.
_MAX_REPRESENTABLE_CHROMA = 0.5
#: Below this magnitude a leading coefficient is treated as absent.
_DEGENERATE = 1e-12

Cubic = tuple[float, float, float, float]


def _evaluate(p: Cubic, x: float) -> float:
    return ((p[0] * x + p[1]) * x + p[2]) * x + p[3]


def _polish(p: Cubic, x: float) -> float:
    """Two Newton steps: Cardano and the trigonometric form both lose precision
    on nearly degenerate cubics, and the caller cares about the fourth decimal."""
    for _ in range(2):
        slope = (3.0 * p[0] * x + 2.0 * p[1]) * x + p[2]
        if abs(slope) < sys.float_info.min:
            break
        x -= _evaluate(p, x) / slope
    return x


def _real_roots(p: Cubic) -> list[float]:
    a, b, c, d = p
    if abs(a) < _DEGENERATE:
        if abs(b) < _DEGENERATE:
            return [] if abs(c) < _DEGENERATE else [-d / c]
        disc = c * c - 4.0 * b * d
        if disc < 0.0:
            return []
        root = math.sqrt(disc)
        return [(-c + root) / (2.0 * b), (-c - root) / (2.0 * b)]

    p2, q, r = b / a, c / a, d / a
    shift = p2 / 3.0
    big_p = q - p2 * p2 / 3.0
    big_q = 2.0 * p2 * p2 * p2 / 27.0 - p2 * q / 3.0 + r
    half_q = big_q / 2.0
    third_p = big_p / 3.0
    discriminant = half_q * half_q + third_p * third_p * third_p

    if discriminant > 0.0:
        root = math.sqrt(discriminant)
        t = _cbrt(-half_q + root) + _cbrt(-half_q - root)
        return [t - shift]
    if abs(big_p) < _DEGENERATE:
        return [-shift]
    m = 2.0 * math.sqrt(-third_p)
    inner = (3.0 * big_q) / (big_p * m)
    phi = math.acos(min(1.0, max(-1.0, inner))) / 3.0
    return [m * math.cos(phi - k * 2.0 * math.pi / 3.0) - shift for k in range(3)]


def smallest_root_above(p: Cubic, lower: float) -> float | None:
    """The smallest real root strictly greater than `lower`, polished first so
    a root a rounding error below `lower` is not promoted past it."""
    best: float | None = None
    for x in _real_roots(p):
        x = _polish(p, x)
        if math.isfinite(x) and x > lower and (best is None or x < best):
            best = x
    return best


def max_chroma(lightness: float, hue: float) -> float:
    """The largest chroma such that EVERY chroma in `[0, C]` is inside sRGB at
    this lightness and hue. Zero at and beyond the lightness extremes."""
    if lightness <= 0.0 or lightness >= 1.0:
        return 0.0
    rad = math.radians(hue)
    sin, cos = math.sin(rad), math.cos(rad)
    # Each cone response is affine in chroma: lms'[i] = base[i] + rate[i] * C.
    base = [_OKLAB_TO_LMS[i][0] * lightness for i in range(3)]
    rate = [_OKLAB_TO_LMS[i][1] * cos + _OKLAB_TO_LMS[i][2] * sin for i in range(3)]
    limit = _MAX_REPRESENTABLE_CHROMA
    for row in _LMS_TO_LINEAR:
        # channel(C) = sum_i row[i] * (base[i] + rate[i] * C)^3, a cubic in C.
        p3 = p2 = p1 = p0 = 0.0
        for i in range(3):
            a, b = base[i], rate[i]
            p3 += row[i] * b * b * b
            p2 += row[i] * 3.0 * a * b * b
            p1 += row[i] * 3.0 * a * a * b
            p0 += row[i] * a * a * a
        for target in (0.0, 1.0):
            root = smallest_root_above((p3, p2, p1, p0 - target), 0.0)
            if root is not None and root < limit:
                limit = root
    return max(limit, 0.0)


def in_gamut(lab: Lab, tolerance: float = 1e-9) -> bool:
    """Whether every linear channel of `lab` is within `[0, 1]`."""
    return all(-tolerance <= c <= 1.0 + tolerance for c in oklab_to_linear(lab))


def map_into_gamut(lch: Lch) -> Lch:
    """Fits a colour into sRGB, keeping lightness and hue EXACTLY: the chroma
    is clamped to [`max_chroma`], the first exit along the ray. A colour at or
    below that boundary passes through untouched, so the function is a fixed
    point: mapping twice gives the same coordinates and the same bytes. The
    hue comes back normalised to `[0, 360)`. See the module head for the one
    case where this is not what a CSS Color 4 bisection would keep."""
    l, c, h = lch
    h %= 360.0
    if l >= 1.0:
        return (1.0, 0.0, h)
    if l <= 0.0:
        return (0.0, 0.0, h)
    return (l, min(c, max_chroma(l, h)), h)


def cusp(hue: float, steps: int = 1000) -> tuple[float, float]:
    """The lightness at which this hue reaches its largest chroma in sRGB, and
    that chroma. The report prints it beside each family: it is where a colour
    would sit if "vivid" meant the gamut's edge, which is not where the ladder
    puts any solid — solids are solved to a contrast, not to the edge."""
    best = (0.0, 0.0)
    for k in range(1, steps):
        l = k / steps
        c = max_chroma(l, hue)
        if c > best[1]:
            best = (l, c)
    return best


# ---------------------------------------------------------------------------
# APCA-W3 0.1.9.
# ---------------------------------------------------------------------------

_R_CO, _G_CO, _B_CO = 0.2126729, 0.7151522, 0.0721750
_MAIN_TRC = 2.4
_BLACK_THRESHOLD, _BLACK_CLAMP = 0.022, 1.414
_NORM_BG, _NORM_TEXT = 0.56, 0.57
_REV_TEXT, _REV_BG = 0.62, 0.65
_SCALE_BOW = _SCALE_WOB = 1.14
_LO_BOW_OFFSET = _LO_WOB_OFFSET = 0.027
_DELTA_Y_MIN = 0.0005
_LO_CLIP = 0.1


def _screen_luminance(rgb: Rgb) -> float:
    ch = lambda v: min(1.0, max(0.0, v)) ** _MAIN_TRC  # noqa: E731 — a plain power, see the module head
    return _R_CO * ch(rgb[0]) + _G_CO * ch(rgb[1]) + _B_CO * ch(rgb[2])


def _soft_clamp_black(y: float) -> float:
    return y if y > _BLACK_THRESHOLD else y + (_BLACK_THRESHOLD - y) ** _BLACK_CLAMP


def apca(text: Rgb, background: Rgb) -> float:
    """Signed APCA lightness contrast: dark-on-light positive, light-on-dark
    negative. Black on white is +106.04; white on black is -107.88."""
    text_y = _soft_clamp_black(_screen_luminance(text))
    bg_y = _soft_clamp_black(_screen_luminance(background))
    if abs(bg_y - text_y) < _DELTA_Y_MIN:
        return 0.0
    if bg_y > text_y:
        sapc = (bg_y**_NORM_BG - text_y**_NORM_TEXT) * _SCALE_BOW
        raw = 0.0 if sapc < _LO_CLIP else sapc - _LO_BOW_OFFSET
    else:
        sapc = (bg_y**_REV_BG - text_y**_REV_TEXT) * _SCALE_WOB
        raw = 0.0 if sapc > -_LO_CLIP else sapc + _LO_WOB_OFFSET
    return raw * 100.0


def apca_magnitude(text: Rgb, background: Rgb) -> float:
    """`|apca|`, for thresholds that genuinely do not care about polarity."""
    return abs(apca(text, background))


def wcag_ratio(a: Rgb, b: Rgb) -> float:
    """The WCAG 2.x contrast ratio, symmetric, 1 to 21. REPORTING ONLY: nothing
    is solved against it, because it does not model light-on-dark text."""

    def luminance(rgb: Rgb) -> float:
        r, g, b_ = (srgb_to_linear(c) for c in rgb)
        return 0.2126 * r + 0.7152 * g + 0.0722 * b_

    la, lb = luminance(a), luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


# ---------------------------------------------------------------------------
# The solver: where a colour's lightness comes from.
# ---------------------------------------------------------------------------

#: Halving `[0, 1]` forty times resolves lightness far below the fourth
#: decimal at which values are quantized.
_STEPS = 40


def screen_colour(lightness: float, hue: float, relative_chroma: float) -> tuple[Lch, tuple[int, int, int]]:
    """The whole pipeline from a lightness to the bytes that reach the screen:
    relative chroma against the boundary at this exact `L` and `h`, gamut
    mapping, encoding and quantisation. Returns the mapped OKLCH and the bytes."""
    l = min(1.0, max(0.0, lightness))
    rel = min(1.0, max(0.0, relative_chroma))
    mapped = map_into_gamut((l, rel * max_chroma(l, hue), hue))
    return mapped, quantize(oklch_to_rgb(mapped))


def solve_lightness(
    anchor: tuple[str, float],
    hue_of: Callable[[float], float],
    relative_chroma: float,
    reference_lightness: float | None = None,
    reference_rgb: Rgb | None = None,
    direction: float = 1.0,
) -> tuple[float, float | None]:
    """The lightness at which `anchor` is met, and the shortfall when it is not.

    `anchor` is `("fixed", L)`, `("delta-l", amount)` — a lightness separation
    from the reference, in the mode's direction — or `("apca", lc)`, a contrast
    magnitude against `reference_rgb`. `hue_of(L)` lets the hue drift with
    lightness (torsion); `direction` is `+1` in a dark interface (search
    toward white) and `-1` in a light one.

    The APCA search runs [`screen_colour`] at every candidate — quantisation
    included — so the answer is the contrast of the bytes the screen shows,
    not of an idealised colour. Contrast rises monotonically away from the
    reference, so the crossing is unique."""
    kind, value = anchor
    if kind == "fixed":
        return min(1.0, max(0.0, value)), None

    if reference_lightness is None:
        raise ValueError(f"anchor {kind!r} needs a reference lightness")

    if kind == "delta-l":
        target = reference_lightness + direction * value
        if 0.0 <= target <= 1.0:
            return target, None
        clamped = min(1.0, max(0.0, target))
        return clamped, abs(clamped - reference_lightness)

    if kind != "apca":
        raise ValueError(f"unknown anchor kind {kind!r}")
    if reference_rgb is None:
        raise ValueError("an apca anchor needs a reference colour")

    def measure(l: float) -> float:
        _, rgb8 = screen_colour(l, hue_of(l), relative_chroma)
        return apca_magnitude(dequantize(rgb8), reference_rgb)

    limit = 1.0 if direction > 0.0 else 0.0
    reachable = measure(limit)
    if reachable < value:
        return limit, reachable
    near, far = reference_lightness, limit
    for _ in range(_STEPS):
        middle = (near + far) / 2.0
        if measure(middle) < value:
            near = middle
        else:
            far = middle
    return far, None


# ---------------------------------------------------------------------------
# Categorical hues at equal perceptual intervals.
# ---------------------------------------------------------------------------

#: Half a degree: finer than any hue difference a categorical set cares about.
_SAMPLES = 720


def even_delta_e_hues(count: int, start: float, lightness: float, relative_chroma: float) -> list[float]:
    """`count` hues starting at `start`, spaced at equal ΔE_ok arc length
    around the wheel measured at this lightness.

    Equal ANGLES are lumpy: the stretch between yellow and green is
    perceptually cramped and the blues are spacious, so eight hues 45° apart
    give two near-twins in the greens and a lonely blue.

    `relative_chroma` scales the whole wheel about the grey axis, so any
    positive value gives the same hues; it is kept for parity with the
    reference and because zero has to mean "achromatic, equal angles"."""
    if count <= 0:
        return []

    def colour_at(hue: float) -> Lab:
        return oklch_to_oklab((lightness, relative_chroma * max_chroma(lightness, hue), hue))

    cumulative = [0.0]
    total = 0.0
    previous = colour_at(start)
    for i in range(1, _SAMPLES + 1):
        current = colour_at(start + 360.0 * i / _SAMPLES)
        total += delta_e_ok(previous, current)
        cumulative.append(total)
        previous = current
    if total <= 0.0:
        return [start + 360.0 * i / count for i in range(count)]

    hues = []
    for i in range(count):
        target = total * i / count
        upper = 1
        while upper < len(cumulative) and cumulative[upper] < target:
            upper += 1
        upper = min(upper, len(cumulative) - 1)
        low, high = cumulative[upper - 1], cumulative[upper]
        within = 0.0 if abs(high - low) < 1e-15 else (target - low) / (high - low)
        hues.append((start + 360.0 * (upper - 1 + within) / _SAMPLES) % 360.0)
    return hues


# ---------------------------------------------------------------------------
# Colour-vision deficiency (Brettel, Viénot & Mollon 1997, via libDaltonLens).
# ---------------------------------------------------------------------------

_BRETTEL: dict[str, tuple[Mat3, Mat3, Vec3]] = {
    "protan": (
        ((0.14980, 1.19548, -0.34528), (0.10764, 0.84864, 0.04372), (0.00384, -0.00540, 1.00156)),
        ((0.14570, 1.16172, -0.30742), (0.10816, 0.85291, 0.03892), (0.00386, -0.00524, 1.00139)),
        (0.00048, 0.00393, -0.00441),
    ),
    "deutan": (
        ((0.36477, 0.86381, -0.22858), (0.26294, 0.64245, 0.09462), (-0.02006, 0.02728, 0.99278)),
        ((0.37298, 0.88166, -0.25464), (0.25954, 0.63506, 0.10540), (-0.01980, 0.02784, 0.99196)),
        (-0.00281, -0.00611, 0.00892),
    ),
    "tritan": (
        ((1.01277, 0.13548, -0.14826), (-0.01243, 0.86812, 0.14431), (0.07589, 0.80500, 0.11911)),
        ((0.93678, 0.18979, -0.12657), (0.06154, 0.81526, 0.12320), (-0.37562, 1.12767, 0.24796)),
        (0.03901, -0.02788, -0.01113),
    ),
}

#: The three deficiencies, in the order the report lists them.
DEFICIENCIES = ("protan", "deutan", "tritan")


def simulate_cvd(rgb: Rgb, deficiency: str, severity: float = 1.0) -> Rgb:
    """How `rgb` looks to someone with `deficiency`, at `severity` in `[0, 1]`.

    Projected in linear RGB onto the dichromat's surface, blended with the
    original by severity, and CLAMPED per channel — the one place a clamp is
    the right tool, see the module head."""
    plane_1, plane_2, normal = _BRETTEL[deficiency]
    s = min(1.0, max(0.0, severity))
    v = tuple(srgb_to_linear(c) for c in rgb)
    side = v[0] * normal[0] + v[1] * normal[1] + v[2] * normal[2]
    projected = _mul_vec(plane_1 if side >= 0.0 else plane_2, v)  # type: ignore[arg-type]
    blended = tuple(min(1.0, max(0.0, p * s + o * (1.0 - s))) for p, o in zip(projected, v))
    return tuple(linear_to_srgb(c) for c in blended)  # type: ignore[return-value]


def cvd_separation(a: Rgb, b: Rgb, deficiency: str) -> float:
    """ΔE_ok between two colours once both are seen through `deficiency`."""
    return delta_e_ok(rgb_to_oklab(simulate_cvd(a, deficiency)), rgb_to_oklab(simulate_cvd(b, deficiency)))


def worst_separation(a: Rgb, b: Rgb) -> tuple[str, float]:
    """The deficiency under which two colours come closest, and how close."""
    return min(((d, cvd_separation(a, b, d)) for d in DEFICIENCIES), key=lambda pair: pair[1])
