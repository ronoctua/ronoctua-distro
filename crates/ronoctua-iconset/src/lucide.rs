//! Lucide's adjustments and its SVG.
//!
//! What a person changes on the icon page (size, stroke, colour, line cap,
//! absolute stroke width) lives in an [`Adjust`], kept while a set is being
//! assembled rather than per icon. [`svg`] writes the figure as Lucide does,
//! with those adjustments; the page rasterises exactly that text (resvg,
//! `Fit::Natural`) and the copy puts exactly that text on the clipboard, so
//! what is copied is what was seen.

use crate::Lucide;

/// How a stroke ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Linecap {
    /// Rounded — Lucide's own.
    #[default]
    Round,
    /// Cut flat at the end.
    Butt,
    /// Squared past the end.
    Square,
}

impl Linecap {
    /// The three, in the ring's order.
    pub const ALL: [Self; 3] = [Self::Round, Self::Butt, Self::Square];

    /// Its SVG value — also the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Round => "round",
            Self::Butt => "butt",
            Self::Square => "square",
        }
    }
}

/// The adjustments of a copied figure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjust {
    /// The `width` and `height`, in pixels.
    pub size_px: u16,
    /// The stroke width, in tenths (`20` is `2`).
    pub stroke_tenths: u16,
    /// The `stroke` value: `currentColor` or a colour the person typed.
    pub colour: String,
    /// The line cap.
    pub linecap: Linecap,
    /// Whether the stroke keeps its width when the size changes
    /// (`absoluteStrokeWidth`).
    pub absolute: bool,
}

impl Adjust {
    /// What lucide.dev copies unadjusted: 24 px, stroke 2, `currentColor`,
    /// round caps. These are the FORMAT's defaults — the SVG a page expects —
    /// not a value of this interface's design.
    #[must_use]
    pub fn lucide() -> Self {
        Self {
            size_px: 24,
            stroke_tenths: 20,
            colour: "currentColor".to_owned(),
            linecap: Linecap::Round,
            absolute: false,
        }
    }
}

impl Default for Adjust {
    fn default() -> Self {
        Self::lucide()
    }
}

/// The side of Lucide's `viewBox`: every figure is drawn on a 24 × 24 grid.
pub const VIEW_BOX: u16 = 24;
/// The smallest size a copy takes, in pixels.
pub const SIZE_MIN_PX: u16 = 1;
/// The largest, in pixels.
pub const SIZE_MAX_PX: u16 = 512;
/// The thinnest stroke, in tenths.
pub const STROKE_MIN_TENTHS: u16 = 1;
/// The thickest, in tenths.
pub const STROKE_MAX_TENTHS: u16 = 80;

impl Linecap {
    /// The line JOIN that goes with it, as lucide.dev pairs them: round with
    /// round, butt with miter, square with bevel.
    #[must_use]
    pub const fn linejoin(self) -> &'static str {
        match self {
            Self::Round => "round",
            Self::Butt => "miter",
            Self::Square => "bevel",
        }
    }
}

impl Adjust {
    /// The size a copy writes: never below [`SIZE_MIN_PX`] nor above
    /// [`SIZE_MAX_PX`].
    #[must_use]
    pub fn size(&self) -> u16 {
        self.size_px.clamp(SIZE_MIN_PX, SIZE_MAX_PX)
    }

    /// The `stroke-width` a copy writes.
    ///
    /// # What "absolute" means
    ///
    /// The SVG `stroke-width` is relative to the `viewBox`, so a 48 px figure
    /// with stroke 2 is twice as thick in pixels as a 24 px one. Whoever mixes
    /// sizes wants the opposite, and lucide.dev divides: `stroke × 24 / size`.
    /// Computed in thousandths with integers, so the text is the same on every
    /// machine: `2 × 24 / 48` is `1`, `2 × 24 / 36` is `1.333`.
    #[must_use]
    pub fn stroke(&self) -> String {
        let tenths = u64::from(
            self.stroke_tenths
                .clamp(STROKE_MIN_TENTHS, STROKE_MAX_TENTHS),
        );
        if !self.absolute {
            return decimal(tenths * 100);
        }
        let size = u64::from(self.size());
        // Thousandths, rounded half up: tenths × 100 × 24 / size.
        let thousandths = (tenths * 100 * u64::from(VIEW_BOX) * 2 + size) / (2 * size);
        decimal(thousandths)
    }

    /// The colour a copy writes: what the person typed, without what would
    /// close the attribute it goes into (`"`, `<`, `>`, `&`) — the one place
    /// where input becomes markup. Nothing left is `currentColor`.
    #[must_use]
    pub fn colour(&self) -> String {
        let cleaned: String = self
            .colour
            .chars()
            .filter(|c| !matches!(c, '"' | '<' | '>' | '&'))
            .collect();
        if cleaned.trim().is_empty() {
            Self::lucide().colour
        } else {
            cleaned
        }
    }
}

/// Thousandths as a person reads them: `2000` → `2`, `1500` → `1.5`,
/// `1333` → `1.333`.
fn decimal(thousandths: u64) -> String {
    let whole = thousandths / 1000;
    let part = thousandths % 1000;
    if part == 0 {
        return whole.to_string();
    }
    let digits = format!("{part:03}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

/// The figure's drawing, decoded: `[(element, [(attribute, value)])]`.
#[must_use]
pub fn nodes(figure: &Lucide) -> Vec<(&str, Vec<(&str, &str)>)> {
    figure
        .nodes
        .split(';')
        .filter(|n| !n.is_empty())
        .map(|node| {
            let mut parts = node.split('|');
            let element = parts.next().unwrap_or_default();
            (element, parts.filter_map(|a| a.split_once('=')).collect())
        })
        .collect()
}

/// The figure's component name in the frameworks: `arrow-up` → `ArrowUp`.
#[must_use]
pub fn component(figure: &Lucide) -> String {
    figure
        .name
        .split('-')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + c.as_str()
            })
        })
        .collect()
}

/// The figure's SVG with `adjust`; with `jsx`, the attribute names JSX takes
/// (`strokeWidth`, `className`).
///
/// THE text the page rasterises and the text [`crate::copy::lucide`] puts on
/// the clipboard: one function, so what is copied is what was seen.
#[must_use]
pub fn svg(figure: &Lucide, adjust: &Adjust, jsx: bool) -> String {
    let (class, width, cap, join) = if jsx {
        (
            "className",
            "strokeWidth",
            "strokeLinecap",
            "strokeLinejoin",
        )
    } else {
        ("class", "stroke-width", "stroke-linecap", "stroke-linejoin")
    };
    let size = adjust.size();
    let mut out = format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" "#,
            r#"viewBox="0 0 {view} {view}" fill="none" stroke="{colour}" {width}="{stroke}" "#,
            r#"{cap}="{linecap}" {join}="{linejoin}" {class}="lucide lucide-{name}">"#,
            "\n"
        ),
        size = size,
        view = VIEW_BOX,
        colour = adjust.colour(),
        width = width,
        stroke = adjust.stroke(),
        cap = cap,
        linecap = adjust.linecap.id(),
        join = join,
        linejoin = adjust.linecap.linejoin(),
        class = class,
        name = figure.name,
    );
    for (element, attributes) in nodes(figure) {
        out.push_str("  <");
        out.push_str(element);
        for (k, v) in attributes {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            out.push_str(v);
            out.push('"');
        }
        out.push_str(" />\n");
    }
    out.push_str("</svg>");
    out
}

/// The SVG on one line — what fits inside a `url(…)`.
#[must_use]
pub fn svg_one_line(figure: &Lucide, adjust: &Adjust) -> String {
    svg(figure, adjust, false)
        .replace('\n', "")
        .replace("  <", "<")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn house() -> Lucide {
        Lucide {
            name: "house".to_owned(),
            nodes: "path|d=M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8;path|d=M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z".to_owned(),
            ..Lucide::default()
        }
    }

    #[test]
    fn the_svg_is_lucides_own_and_the_adjustments_reach_it() {
        let svg = svg(&house(), &Adjust::lucide(), false);
        assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-house">"#), "{svg}");
        assert!(svg.contains(r#"  <path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" />"#));
        assert!(svg.ends_with("</svg>"));
        let jsx = super::svg(&house(), &Adjust::lucide(), true);
        assert!(jsx.contains("strokeWidth=") && jsx.contains("className="));
        assert!(!jsx.contains("stroke-width="));

        let absolute = Adjust {
            size_px: 48,
            absolute: true,
            ..Adjust::lucide()
        };
        let svg = super::svg(&house(), &absolute, false);
        assert!(
            svg.contains(r#"width="48""#) && svg.contains(r#"stroke-width="1""#),
            "2 × 24 / 48 = 1: {svg}"
        );
        let thirds = Adjust {
            size_px: 36,
            absolute: true,
            ..Adjust::lucide()
        };
        assert_eq!(thirds.stroke(), "1.333");
        let odd = Adjust {
            stroke_tenths: 15,
            linecap: Linecap::Square,
            colour: "#f00\"><script".to_owned(),
            ..Adjust::lucide()
        };
        let svg = super::svg(&house(), &odd, false);
        assert!(svg.contains(r#"stroke-width="1.5""#));
        assert!(svg.contains(r#"stroke-linecap="square" stroke-linejoin="bevel""#));
        assert!(
            svg.contains(r##"stroke="#f00script""##),
            "the quote cannot close the attribute: {svg}"
        );
        assert_eq!(
            component(&Lucide {
                name: "arrow-up-right".to_owned(),
                ..Lucide::default()
            }),
            "ArrowUpRight"
        );
    }
}
