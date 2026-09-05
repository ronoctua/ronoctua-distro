"""The colour science, proved. `python3 -m unittest nix/scripts/test_colorspace.py`
— and the `colorspace-tests` check runs the same file inside the build.

Every assertion here is a property the generator LEANS ON: if one of them
went, a theme would still build and the defect would be a colour nobody can
trace. That is why they are exact where exactness is by construction (white,
grey, the fixed point) and bounded where the reference numbers are published
with a tolerance (the APCA anchors)."""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import colorspace as cs  # noqa: E402


class Transfer(unittest.TestCase):
    def test_srgb_round_trips_every_byte(self):
        for v in range(256):
            back = cs.linear_to_srgb(cs.srgb_to_linear(v / 255.0)) * 255.0
            self.assertEqual(round(back), v, f"byte {v} came back as {back}")

    def test_transfer_is_sign_preserving(self):
        self.assertLess(cs.srgb_to_linear(-0.5), 0.0)
        self.assertLess(cs.linear_to_srgb(-0.5), 0.0)


class Oklab(unittest.TestCase):
    def test_white_is_lightness_one_exactly(self):
        l, a, b = cs.linear_to_oklab((1.0, 1.0, 1.0))
        self.assertAlmostEqual(l, 1.0, delta=1e-12)
        self.assertAlmostEqual(a, 0.0, delta=1e-12)
        self.assertAlmostEqual(b, 0.0, delta=1e-12)

    def test_every_grey_has_no_chroma(self):
        for k in range(1, 100):
            v = k / 100.0
            _, c, _ = cs.oklab_to_oklch(cs.linear_to_oklab((v, v, v)))
            self.assertLess(c, 1e-12, f"grey {v} has chroma {c}")

    def test_oklab_round_trips_on_a_grid(self):
        for i in range(9):
            for j in range(9):
                for k in range(9):
                    rgb = (i / 8.0, j / 8.0, k / 8.0)
                    back = cs.oklab_to_linear(cs.linear_to_oklab(rgb))
                    for got, want in zip(back, rgb):
                        self.assertAlmostEqual(got, want, delta=1e-9)

    def test_oklch_round_trips(self):
        for lch in [(0.5, 0.1, 30.0), (0.7, 0.05, 264.0), (0.2, 0.0, 0.0)]:
            l, c, h = cs.oklab_to_oklch(cs.oklch_to_oklab(lch))
            self.assertAlmostEqual(l, lch[0], delta=1e-12)
            self.assertAlmostEqual(c, lch[1], delta=1e-12)
            if lch[1] > 0.0:
                self.assertAlmostEqual(h, lch[2], delta=1e-9)

    def test_delta_e_between_black_and_white_is_one(self):
        self.assertAlmostEqual(cs.delta_e_ok((0.0, 0.0, 0.0), (1.0, 0.0, 0.0)), 1.0, delta=1e-12)

    def test_hue_difference_takes_the_short_arc(self):
        self.assertAlmostEqual(cs.hue_difference(350.0, 10.0), 20.0)
        self.assertAlmostEqual(cs.hue_difference(10.0, 350.0), 20.0)
        self.assertAlmostEqual(cs.hue_difference(0.0, 180.0), 180.0)


class Bytes(unittest.TestCase):
    def test_tokens_parse_and_format_both_ways(self):
        rgb8, a = cs.parse_token("0x7AA2F7FF")
        self.assertEqual((rgb8, a), ((0x7A, 0xA2, 0xF7), 0xFF))
        self.assertEqual(cs.format_token(rgb8, a), "0x7AA2F7FF")
        self.assertEqual(cs.parse_token("#121212"), ((0x12, 0x12, 0x12), 0xFF))
        self.assertEqual(cs.parse_token("0x121212CC")[1], 0xCC)
        self.assertEqual(cs.format_hex((1, 2, 255)), "#0102FF")
        with self.assertRaises(ValueError):
            cs.parse_token("7AA2F7")

    def test_quantize_rounds_half_up_and_clamps(self):
        self.assertEqual(cs.quantize((0.5, 1.2, -0.1)), (128, 255, 0))
        self.assertEqual(cs.quantize(cs.dequantize((7, 200, 33))), (7, 200, 33))


class Boundary(unittest.TestCase):
    def test_the_gamut_is_not_convex_near_the_blue_primary(self):
        # At L 0.4525 the ray at hue 264.1 leaves sRGB near chroma 0.270,
        # comes back in near 0.310 and leaves again near 0.313. A bisection
        # converges to whichever crossing it brackets; the boundary has to be
        # the FIRST one. This is the case the analytic solver exists for.
        l, h = 0.4525, 264.1
        crossings, inside = [], True
        c = 0.0
        while c <= 0.35:
            now = cs.in_gamut(cs.oklch_to_oklab((l, c, h)), 1e-9)
            if now != inside:
                crossings.append(round(c, 3))
                inside = now
            c += 0.0005
        self.assertGreaterEqual(len(crossings), 3, f"the island is gone: crossings at {crossings}")
        first = cs.max_chroma(l, h)
        self.assertTrue(0.26 < first < 0.29, f"max_chroma returned {first}, not the first exit {crossings[0]}")
        self.assertAlmostEqual(first, crossings[0], delta=0.001)
        # And no kink: the neighbouring hue's boundary is within a few percent.
        beside = cs.max_chroma(l, 264.0)
        self.assertLess(abs(first - beside) / beside, 0.05, f"{beside} against {first}")

    def test_zero_at_the_lightness_extremes(self):
        self.assertEqual(cs.max_chroma(0.0, 120.0), 0.0)
        self.assertEqual(cs.max_chroma(1.0, 120.0), 0.0)

    def test_everything_below_the_boundary_is_in_gamut(self):
        for li in range(1, 20):
            for hi in range(0, 360, 5):
                l, h = li / 20.0, float(hi)
                limit = cs.max_chroma(l, h)
                self.assertGreater(limit, 0.0)
                self.assertTrue(cs.in_gamut(cs.oklch_to_oklab((l, limit * 0.999, h))), f"L {l} h {h}")
                # LARGEST: a hair above the boundary is already outside.
                self.assertFalse(cs.in_gamut(cs.oklch_to_oklab((l, limit * 1.01 + 1e-4, h)), 1e-9), f"L {l} h {h}: {limit} is not the edge")
                # And the first exit of a linear scan is not below it.
                c, step = 0.0, 0.001
                while c < limit and cs.in_gamut(cs.oklch_to_oklab((l, c, h)), 1e-6):
                    c += step
                self.assertGreaterEqual(c, limit, f"scan left the gamut at {c} before {limit} (L {l} h {h})")

    def test_mapping_is_a_fixed_point_that_keeps_lightness_and_hue(self):
        for lch in [(0.62, 0.9, 264.0), (0.3, 0.5, 30.0), (0.85, 0.4, 110.0), (0.5, 0.01, 200.0)]:
            once = cs.map_into_gamut(lch)
            twice = cs.map_into_gamut(once)
            self.assertEqual(once, twice)
            self.assertAlmostEqual(once[0], lch[0], delta=1e-12)
            self.assertAlmostEqual(once[2], lch[2], delta=1e-12)
            self.assertLessEqual(once[1], lch[1])
            self.assertTrue(cs.in_gamut(cs.oklch_to_oklab(once)))
            self.assertEqual(cs.quantize(cs.oklch_to_rgb(once)), cs.quantize(cs.oklch_to_rgb(twice)))

    def test_the_extremes_map_to_black_and_white(self):
        self.assertEqual(cs.map_into_gamut((1.2, 0.3, 40.0)), (1.0, 0.0, 40.0))
        self.assertEqual(cs.map_into_gamut((-0.1, 0.3, 40.0)), (0.0, 0.0, 40.0))

    def test_mapping_normalises_the_hue(self):
        self.assertAlmostEqual(cs.map_into_gamut((0.5, 0.01, 400.0))[2], 40.0, delta=1e-9)

    def test_the_cusp_is_inside_the_range(self):
        l, c = cs.cusp(264.0, steps=200)
        self.assertGreater(c, 0.2)
        self.assertTrue(0.0 < l < 1.0)


class Cubics(unittest.TestCase):
    def test_three_real_roots(self):
        # (x - 1)(x - 2)(x - 3) = x^3 - 6x^2 + 11x - 6
        self.assertAlmostEqual(cs.smallest_root_above((1.0, -6.0, 11.0, -6.0), 0.0), 1.0, delta=1e-9)
        self.assertAlmostEqual(cs.smallest_root_above((1.0, -6.0, 11.0, -6.0), 1.5), 2.0, delta=1e-9)
        self.assertAlmostEqual(cs.smallest_root_above((1.0, -6.0, 11.0, -6.0), 2.5), 3.0, delta=1e-9)
        self.assertIsNone(cs.smallest_root_above((1.0, -6.0, 11.0, -6.0), 3.5))

    def test_one_real_root_and_a_triple_root(self):
        self.assertAlmostEqual(cs.smallest_root_above((1.0, 0.0, 0.0, -8.0), 0.0), 2.0, delta=1e-9)
        self.assertAlmostEqual(cs.smallest_root_above((1.0, -3.0, 3.0, -1.0), 0.0), 1.0, delta=1e-6)

    def test_degenerate_polynomials(self):
        self.assertAlmostEqual(cs.smallest_root_above((0.0, 1.0, -3.0, 2.0), 0.0), 1.0, delta=1e-9)  # (x-1)(x-2)
        self.assertAlmostEqual(cs.smallest_root_above((0.0, 0.0, 2.0, -5.0), 0.0), 2.5, delta=1e-12)
        self.assertIsNone(cs.smallest_root_above((0.0, 0.0, 0.0, 1.0), 0.0))
        self.assertIsNone(cs.smallest_root_above((0.0, 1.0, 0.0, 1.0), 0.0))  # x^2 + 1

    def test_the_lower_bound_is_strict(self):
        self.assertIsNone(cs.smallest_root_above((1.0, 0.0, 0.0, 0.0), 0.0))


class Apca(unittest.TestCase):
    def test_the_published_anchors(self):
        self.assertAlmostEqual(cs.apca((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)), 106.04, delta=0.01)
        self.assertAlmostEqual(cs.apca((1.0, 1.0, 1.0), (0.0, 0.0, 0.0)), -107.88, delta=0.01)

    def test_sign_follows_polarity_and_is_not_symmetric(self):
        grey, dark = (0.6, 0.6, 0.6), (0.1, 0.1, 0.1)
        self.assertLess(cs.apca(grey, dark), 0.0)
        self.assertGreater(cs.apca(dark, grey), 0.0)
        self.assertNotAlmostEqual(cs.apca(grey, dark), -cs.apca(dark, grey), delta=0.5)

    def test_noise_reads_as_zero(self):
        self.assertEqual(cs.apca((0.5, 0.5, 0.5), (0.5005, 0.5, 0.5)), 0.0)

    def test_wcag_is_symmetric_and_bounded(self):
        self.assertAlmostEqual(cs.wcag_ratio((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)), 21.0, delta=1e-9)
        self.assertEqual(cs.wcag_ratio((0.3, 0.2, 0.1), (0.9, 0.8, 0.7)), cs.wcag_ratio((0.9, 0.8, 0.7), (0.3, 0.2, 0.1)))


class Solver(unittest.TestCase):
    CHROME = cs.dequantize((0x12, 0x12, 0x12))

    def test_fixed_and_delta_anchors_need_no_search(self):
        self.assertEqual(cs.solve_lightness(("fixed", 0.182), lambda _: 0.0, 0.0), (0.182, None))
        l, short = cs.solve_lightness(("delta-l", 0.1), lambda _: 0.0, 0.0, reference_lightness=0.2)
        self.assertAlmostEqual(l, 0.3, delta=1e-12)
        self.assertIsNone(short)
        l, short = cs.solve_lightness(("delta-l", 0.5), lambda _: 0.0, 0.0, reference_lightness=0.8)
        self.assertEqual(l, 1.0)
        self.assertAlmostEqual(short, 0.2, delta=1e-12)

    def test_the_accent_is_re_derived_from_its_anchor(self):
        # The measurement the seeds were written from: today's `state.accent`
        # (`7AA2F7`) is h 264.2, relative chroma 0.91, Lc 52.1 on the chrome.
        l, short = cs.solve_lightness(("apca", 52.1), lambda _: 264.2, 0.91, reference_lightness=0.182, reference_rgb=self.CHROME)
        self.assertIsNone(short)
        _, rgb8 = cs.screen_colour(l, 264.2, 0.91)
        self.assertEqual(rgb8, (0x7A, 0xA2, 0xF7))

    def test_the_solved_colour_meets_its_target(self):
        for lc in (23.0, 48.0, 87.0):
            l, short = cs.solve_lightness(("apca", lc), lambda _: 85.0, 0.26, reference_lightness=0.182, reference_rgb=self.CHROME)
            self.assertIsNone(short)
            _, rgb8 = cs.screen_colour(l, 85.0, 0.26)
            # The bisection ends on the far side of the crossing, so the BYTES
            # it returns meet the target — not "almost", which is what a solver
            # measuring an unquantised colour, or returning the near end, gives.
            self.assertGreaterEqual(cs.apca_magnitude(cs.dequantize(rgb8), self.CHROME), lc)

    def test_a_light_interface_searches_toward_black(self):
        paper = (0.99, 0.99, 0.99)
        paper_l = cs.rgb_to_oklab(paper)[0]
        l, short = cs.solve_lightness(("apca", 60.0), lambda _: 85.0, 0.26, reference_lightness=paper_l, reference_rgb=paper, direction=-1.0)
        self.assertIsNone(short)
        self.assertLess(l, 0.7)
        _, rgb8 = cs.screen_colour(l, 85.0, 0.26)
        self.assertGreater(cs.apca(cs.dequantize(rgb8), paper), 60.0)  # positive: dark on light
        l, short = cs.solve_lightness(("delta-l", 0.1), lambda _: 0.0, 0.0, reference_lightness=0.9, direction=-1.0)
        self.assertAlmostEqual(l, 0.8, delta=1e-12)
        self.assertIsNone(short)

    def test_the_hue_may_follow_the_lightness(self):
        # Torsion: the solver asks `hue_of` at every candidate, so the colour
        # it returns is at the hue of the lightness it settled on.
        hue_of = lambda l: 264.0 + 60.0 * l  # noqa: E731
        l, short = cs.solve_lightness(("apca", 60.0), hue_of, 0.6, reference_lightness=0.182, reference_rgb=self.CHROME)
        self.assertIsNone(short)

        # The crossing, measured WITH the torsion on both sides: the returned
        # lightness meets the target at its own hue, and a hair below it does
        # not. A solver that read `hue_of` at some fixed lightness lands on a
        # different crossing and fails one of the two.
        def measure(x):
            return cs.apca_magnitude(cs.dequantize(cs.screen_colour(x, hue_of(x), 0.6)[1]), self.CHROME)

        self.assertGreaterEqual(measure(l), 60.0)
        self.assertLess(measure(l - 1e-9), 60.0)
        # And it is not the constant-hue answer.
        fixed, _ = cs.solve_lightness(("apca", 60.0), lambda _: 264.0, 0.6, reference_lightness=0.182, reference_rgb=self.CHROME)
        self.assertNotAlmostEqual(l, fixed, delta=1e-4)

    def test_an_unreachable_target_reports_the_shortfall(self):
        l, short = cs.solve_lightness(("apca", 150.0), lambda _: 85.0, 0.26, reference_lightness=0.182, reference_rgb=self.CHROME)
        self.assertEqual(l, 1.0)
        self.assertIsNotNone(short)
        self.assertLess(short, 150.0)


class Categorical(unittest.TestCase):
    def test_the_requested_hues_come_back_spaced(self):
        for count in (1, 3, 8):
            hues = cs.even_delta_e_hues(count, 264.0, 0.72, 0.75)
            self.assertEqual(len(hues), count)
            self.assertAlmostEqual(hues[0], 264.0)
            self.assertEqual(len({round(h, 3) for h in hues}), count)

    def test_consecutive_hues_are_equal_arcs_apart(self):
        l, rel = 0.72, 0.75
        hues = cs.even_delta_e_hues(8, 264.0, l, rel)

        def colour(h):
            return cs.oklch_to_oklab((l, rel * cs.max_chroma(l, h), h))

        def arc(a, b, n=400):  # re-integrated at a different resolution than the module's
            span = ((b - a) % 360.0) or 360.0
            total, previous = 0.0, colour(a)
            for i in range(1, n + 1):
                current = colour(a + span * i / n)
                total += cs.delta_e_ok(previous, current)
                previous = current
            return total

        arcs = [arc(a, b) for a, b in zip(hues, hues[1:] + hues[:1])]
        mean = sum(arcs) / len(arcs)
        for a in arcs:
            self.assertAlmostEqual(a, mean, delta=0.02 * mean, msg=f"arcs {arcs}")
        # Equal arcs are NOT equal angles: the wheel is cramped in the greens.
        angles = [cs.hue_difference(a, b) for a, b in zip(hues, hues[1:])]
        self.assertGreater(max(angles) - min(angles), 5.0)

    def test_hues_are_interpolated_between_samples(self):
        # The wheel is sampled every half degree; the answer must not snap to
        # the samples, or eight hues would share seven possible fractions.
        hues = cs.even_delta_e_hues(8, 264.0, 0.72, 0.75)
        self.assertTrue(any(abs((h * 2.0) - round(h * 2.0)) > 0.01 for h in hues[1:]), hues)

    def test_placement_depends_on_lightness_only(self):
        # Relative chroma scales the whole wheel about the grey axis, so the
        # cumulative measure scales uniformly and the fractions do not move.
        self.assertEqual(cs.even_delta_e_hues(8, 264.0, 0.72, 0.75), cs.even_delta_e_hues(8, 264.0, 0.72, 0.375))
        self.assertNotEqual(cs.even_delta_e_hues(8, 264.0, 0.72, 0.75), cs.even_delta_e_hues(8, 264.0, 0.40, 0.75))


class ColourVision(unittest.TestCase):
    def test_grey_is_untouched_by_every_deficiency(self):
        for d in cs.DEFICIENCIES:
            for v in (0.1, 0.5, 0.9):
                for c in cs.simulate_cvd((v, v, v), d):
                    self.assertAlmostEqual(c, v, delta=2e-3, msg=d)

    def test_severity_zero_is_the_identity(self):
        rgb = (0.2, 0.7, 0.4)
        for c, o in zip(cs.simulate_cvd(rgb, "deutan", 0.0), rgb):
            self.assertAlmostEqual(c, o, delta=1e-9)

    def test_red_and_green_collapse_under_deuteranopia(self):
        red, green = cs.dequantize((0xF7, 0x76, 0x8E)), cs.dequantize((0x9E, 0xCE, 0x6A))
        normal = cs.delta_e_ok(cs.rgb_to_oklab(red), cs.rgb_to_oklab(green))
        kind, worst = cs.worst_separation(red, green)
        self.assertEqual(kind, "deutan")
        self.assertLess(worst, normal / 2.0)

    def test_a_saturated_blue_takes_the_second_half_plane(self):
        # Its dot with the protan normal is negative, which selects `plane_2`;
        # the two planes give a visibly different green channel here.
        plane_1, plane_2, _ = cs._BRETTEL["protan"]
        blue = tuple(cs.srgb_to_linear(c) for c in (0.0, 0.0, 1.0))
        expected = tuple(cs.linear_to_srgb(min(1.0, max(0.0, c))) for c in cs._mul_vec(plane_2, blue))
        other = tuple(cs.linear_to_srgb(min(1.0, max(0.0, c))) for c in cs._mul_vec(plane_1, blue))
        got = cs.simulate_cvd((0.0, 0.0, 1.0), "protan")
        for g, e in zip(got, expected):
            self.assertAlmostEqual(g, e, delta=1e-9)
        self.assertNotAlmostEqual(got[1], other[1], delta=1e-3)


if __name__ == "__main__":
    unittest.main()
