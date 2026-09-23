//! What an effect does to a camera frame.
//!
//! # The shape
//!
//! A [`Frame`] is straight (not premultiplied) RGBA, row-major. An effect is a
//! [`Step`] — an id from the table and one integer per declared parameter — and
//! [`apply`] runs it IN PLACE. A [`Chain`] is several steps in order: the studio
//! keeps one per source, and a person reorders it.
//!
//! The table ([`DECLARED`]) comes from `assets/camera-effects.nix`; this crate
//! owns only the arithmetic. [`every_declared_effect_is_implemented`] is the
//! test that keeps the two halves one list: an id in the Nix file with no arm
//! here would be an effect the ring offers and that does nothing.
//!
//! # What an effect may need from outside
//!
//! Three background effects need a MASK — where the person is — and one needs
//! a BACKDROP. Neither is computed here: a [`Context`] carries them, already at
//! the frame's size, and an effect that needs one it was not given REFUSES
//! ([`Refusal`]) rather than drawing something plausible. A background blur
//! that blurred the whole frame because the mask was late would read as "the
//! effect is broken", not as "the model is still loading".
//!
//! [`every_declared_effect_is_implemented`]: tests

#![forbid(unsafe_code)]

mod ops;

/// One declared parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// Kebab-case; the text key is `camera-param-<name>`.
    pub name: &'static str,
    /// The text key naming it.
    pub key: &'static str,
    /// The smallest value.
    pub min: i32,
    /// The largest value.
    pub max: i32,
    /// Where it starts.
    pub default: i32,
    /// How far one key press moves it.
    pub step: i32,
}

/// One declared effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    /// Kebab-case, stable: it is what a scene file stores.
    pub id: &'static str,
    /// The text key naming it.
    pub key: &'static str,
    /// Which group of the ring it sits in.
    pub family: &'static str,
    /// Whether it needs to know where the person is.
    pub mask: bool,
    /// Its parameters, in order.
    pub params: &'static [Param],
}

include!(concat!(env!("OUT_DIR"), "/effects.rs"));

/// The declaration of an id.
pub fn declared(id: &str) -> Option<&'static Declared> {
    DECLARED.iter().find(|d| d.id == id)
}

/// An RGBA frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frame {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// `width * height * 4` bytes, straight alpha.
    pub rgba: Vec<u8>,
}

impl Frame {
    /// A frame of one colour.
    pub fn filled(width: usize, height: usize, rgba: [u8; 4]) -> Self {
        let mut bytes = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            bytes.extend_from_slice(&rgba);
        }
        Self {
            width,
            height,
            rgba: bytes,
        }
    }

    /// Whether the byte count is the one the size says.
    pub fn is_whole(&self) -> bool {
        self.rgba.len() == self.width * self.height * 4
    }
}

/// One effect with its values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The declared id.
    pub id: String,
    /// One per declared parameter, in order; missing ones take the default,
    /// values outside the range are pulled inside it.
    pub values: Vec<i32>,
}

impl Step {
    /// The effect at its declared defaults.
    pub fn new(id: &str) -> Option<Self> {
        let d = declared(id)?;
        Some(Self {
            id: d.id.to_owned(),
            values: d.params.iter().map(|p| p.default).collect(),
        })
    }

    /// The value of a parameter by name, clamped and defaulted.
    pub fn value(&self, name: &str) -> i32 {
        let Some(d) = declared(&self.id) else {
            return 0;
        };
        let Some((i, p)) = d.params.iter().enumerate().find(|(_, p)| p.name == name) else {
            return 0;
        };
        self.values
            .get(i)
            .copied()
            .unwrap_or(p.default)
            .clamp(p.min, p.max)
    }

    /// Moves a parameter by `steps` of its declared step, inside its range.
    pub fn turn(&mut self, name: &str, steps: i32) {
        let Some(d) = declared(&self.id) else { return };
        let Some((i, p)) = d.params.iter().enumerate().find(|(_, p)| p.name == name) else {
            return;
        };
        if self.values.len() < d.params.len() {
            self.values
                .extend(d.params[self.values.len()..].iter().map(|p| p.default));
        }
        self.values[i] = (self.values[i] + steps * p.step).clamp(p.min, p.max);
    }

    /// Whether this step needs a mask.
    pub fn needs_mask(&self) -> bool {
        declared(&self.id).is_some_and(|d| d.mask)
    }
}

/// Several steps, applied in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chain(pub Vec<Step>);

impl Chain {
    /// Whether any step needs a mask — the question that decides whether the
    /// studio runs the model at all.
    pub fn needs_mask(&self) -> bool {
        self.0.iter().any(Step::needs_mask)
    }

    /// The largest `steady` any masked step asks for, in `[0, 1)`.
    pub fn steadiness(&self) -> f32 {
        self.0
            .iter()
            .filter(|s| s.needs_mask())
            .map(|s| s.value("steady"))
            .max()
            .map_or(0.0, |v| v as f32 / 100.0)
    }
}

/// What an effect may need from outside the frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct Context<'a> {
    /// Person alpha per pixel, `255` person, at the frame's size.
    pub mask: Option<&'a [u8]>,
    /// The replacement background, at the frame's size.
    pub backdrop: Option<&'a Frame>,
}

/// Why a step did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The id is not in the table.
    Unknown(String),
    /// It needs a mask and the context has none (the model is loading, or is
    /// absent).
    NoMask,
    /// It needs a backdrop and none was chosen.
    NoBackdrop,
    /// The frame's bytes do not match its size.
    Torn,
}

/// Buffers an effect borrows, kept between frames so a frame allocates nothing.
#[derive(Debug, Default)]
pub struct Scratch {
    pub(crate) copy: Vec<u8>,
    pub(crate) line: Vec<u32>,
    pub(crate) small: Vec<u8>,
}

/// Runs one step on a frame, in place.
pub fn apply(
    step: &Step,
    frame: &mut Frame,
    ctx: &Context<'_>,
    scratch: &mut Scratch,
) -> Result<(), Refusal> {
    if !frame.is_whole() {
        return Err(Refusal::Torn);
    }
    let d = declared(&step.id).ok_or_else(|| Refusal::Unknown(step.id.clone()))?;
    let mask = ctx.mask.filter(|m| m.len() == frame.width * frame.height);
    if d.mask && mask.is_none() {
        return Err(Refusal::NoMask);
    }
    let v = |name: &str| step.value(name);
    match d.id {
        "grey" => ops::grey(frame, v("amount")),
        "sepia" => ops::sepia(frame, v("amount")),
        "negative" => ops::negative(frame),
        "posterize" => ops::posterize(frame, v("levels")),
        "thermal" => ops::thermal(frame),
        "night" => ops::night(frame, v("gain")),
        "tone" => ops::tone(
            frame,
            [
                v("brightness"),
                v("contrast"),
                v("saturation"),
                v("temperature"),
            ],
        ),
        "pixelate" => ops::pixelate(frame, v("size")),
        "blur" => ops::blur(frame, v("radius"), scratch),
        "sketch" => ops::sketch(frame, v("strength"), scratch),
        "vignette" => ops::vignette(frame, v("strength")),
        "symmetry" => ops::symmetry(frame),
        "kaleidoscope" => ops::kaleidoscope(frame, v("segments"), scratch),
        "chroma-key" => ops::chroma_key(frame, v("hue"), v("tolerance")),
        "background-blur" => {
            ops::background_blur(frame, v("radius"), mask.unwrap_or_default(), scratch)
        }
        "background-replace" => {
            let back = ctx
                .backdrop
                .filter(|b| b.width == frame.width && b.height == frame.height && b.is_whole())
                .ok_or(Refusal::NoBackdrop)?;
            ops::background_replace(frame, back, mask.unwrap_or_default());
        }
        "background-remove" => ops::background_remove(frame, mask.unwrap_or_default()),
        other => return Err(Refusal::Unknown(other.to_owned())),
    }
    Ok(())
}

/// Runs a whole chain; a refused step is skipped and reported, and the rest
/// still run — one missing backdrop must not turn a grey camera colour again.
pub fn apply_chain(
    chain: &Chain,
    frame: &mut Frame,
    ctx: &Context<'_>,
    scratch: &mut Scratch,
) -> Vec<(usize, Refusal)> {
    let mut refused = Vec::new();
    for (i, step) in chain.0.iter().enumerate() {
        if let Err(r) = apply(step, frame, ctx, scratch) {
            refused.push((i, r));
        }
    }
    refused
}

/// The next effect in the ring after `id` (or the first, from nothing), and
/// `None` after the last — "no effect" is the ring's own stop.
pub fn next(id: Option<&str>) -> Option<&'static str> {
    match id.and_then(|id| DECLARED.iter().position(|d| d.id == id)) {
        None if id.is_none() => DECLARED.first().map(|d| d.id),
        None => None,
        Some(i) => DECLARED.get(i + 1).map(|d| d.id),
    }
}

/// The effect before `id` in the ring, and `None` before the first.
pub fn previous(id: Option<&str>) -> Option<&'static str> {
    match id.and_then(|id| DECLARED.iter().position(|d| d.id == id)) {
        None if id.is_none() => DECLARED.last().map(|d| d.id),
        None | Some(0) => None,
        Some(i) => DECLARED.get(i - 1).map(|d| d.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: usize, h: usize) -> Frame {
        let mut f = Frame {
            width: w,
            height: h,
            rgba: Vec::new(),
        };
        for y in 0..h {
            for x in 0..w {
                f.rgba.extend_from_slice(&[
                    (x * 255 / w.max(1)) as u8,
                    (y * 255 / h.max(1)) as u8,
                    128,
                    255,
                ]);
            }
        }
        f
    }

    #[test]
    fn every_declared_effect_is_implemented() {
        let mask = vec![255u8; 16 * 8];
        let back = Frame::filled(16, 8, [0, 0, 255, 255]);
        let ctx = Context {
            mask: Some(&mask),
            backdrop: Some(&back),
        };
        let mut scratch = Scratch::default();
        for d in DECLARED {
            let mut f = gradient(16, 8);
            let step = Step::new(d.id).expect("declared");
            assert_eq!(
                apply(&step, &mut f, &ctx, &mut scratch),
                Ok(()),
                "{} has no arm in `apply`",
                d.id
            );
            assert!(f.is_whole(), "{} changed the frame's size", d.id);
        }
    }

    #[test]
    fn every_effect_but_the_background_ones_changes_something() {
        let mut scratch = Scratch::default();
        for d in DECLARED.iter().filter(|d| !d.mask) {
            let before = gradient(24, 24);
            let mut f = before.clone();
            apply(
                &Step::new(d.id).expect("declared"),
                &mut f,
                &Context::default(),
                &mut scratch,
            )
            .expect("applies");
            // `tone` at its defaults is the identity, by design: it is a set of
            // four sliders that all start at zero.
            if d.id == "tone" {
                assert_eq!(f, before, "tone at its defaults must be the identity");
            } else {
                assert_ne!(f, before, "{} at its defaults does nothing", d.id);
            }
        }
    }

    #[test]
    fn a_masked_effect_without_a_mask_refuses() {
        let mut f = gradient(4, 4);
        let before = f.clone();
        let step = Step::new("background-blur").expect("declared");
        assert_eq!(
            apply(&step, &mut f, &Context::default(), &mut Scratch::default()),
            Err(Refusal::NoMask)
        );
        assert_eq!(f, before, "a refusal leaves the frame alone");
    }

    #[test]
    fn a_replacement_without_a_backdrop_refuses() {
        let mask = vec![0u8; 16];
        let mut f = gradient(4, 4);
        let step = Step::new("background-replace").expect("declared");
        let ctx = Context {
            mask: Some(&mask),
            backdrop: None,
        };
        assert_eq!(
            apply(&step, &mut f, &ctx, &mut Scratch::default()),
            Err(Refusal::NoBackdrop)
        );
    }

    #[test]
    fn the_background_is_what_the_mask_says_it_is() {
        // Left half person, right half background: removing the background
        // must make exactly the right half transparent.
        let mut mask = Vec::new();
        for _ in 0..4 {
            mask.extend_from_slice(&[255, 255, 0, 0]);
        }
        let mut f = gradient(4, 4);
        let ctx = Context {
            mask: Some(&mask),
            backdrop: None,
        };
        apply(
            &Step::new("background-remove").expect("declared"),
            &mut f,
            &ctx,
            &mut Scratch::default(),
        )
        .expect("applies");
        let alphas: Vec<u8> = f.rgba.chunks(4).map(|p| p[3]).collect();
        for row in alphas.chunks(4) {
            assert_eq!(row, [255, 255, 0, 0]);
        }
    }

    #[test]
    fn a_torn_frame_is_refused() {
        let mut f = Frame {
            width: 4,
            height: 4,
            rgba: vec![0; 10],
        };
        assert_eq!(
            apply(
                &Step::new("negative").expect("declared"),
                &mut f,
                &Context::default(),
                &mut Scratch::default()
            ),
            Err(Refusal::Torn)
        );
    }

    #[test]
    fn a_turn_stays_inside_the_range() {
        let mut s = Step::new("posterize").expect("declared");
        s.turn("levels", 100);
        assert_eq!(s.value("levels"), 16);
        s.turn("levels", -100);
        assert_eq!(s.value("levels"), 2);
        let s = Step {
            id: "posterize".into(),
            values: vec![],
        };
        assert_eq!(s.value("levels"), 4, "a missing value is the default");
    }

    #[test]
    fn the_ring_walks_both_ways_and_stops_at_none() {
        let first = DECLARED[0].id;
        let last = DECLARED[DECLARED.len() - 1].id;
        assert_eq!(next(None), Some(first));
        assert_eq!(next(Some(last)), None);
        assert_eq!(previous(None), Some(last));
        assert_eq!(previous(Some(first)), None);
        let mut walked = 0;
        let mut at = next(None);
        while let Some(id) = at {
            walked += 1;
            at = next(Some(id));
        }
        assert_eq!(walked, DECLARED.len());
    }

    #[test]
    fn a_chain_keeps_going_past_a_refusal() {
        let chain = Chain(vec![
            Step::new("background-replace").expect("declared"),
            Step::new("negative").expect("declared"),
        ]);
        let mut f = Frame::filled(2, 2, [10, 20, 30, 255]);
        let mask = vec![255u8; 4];
        let refused = apply_chain(
            &chain,
            &mut f,
            &Context {
                mask: Some(&mask),
                backdrop: None,
            },
            &mut Scratch::default(),
        );
        assert_eq!(refused, vec![(0, Refusal::NoBackdrop)]);
        assert_eq!(
            &f.rgba[..4],
            &[245, 235, 225, 255],
            "the negative still ran"
        );
    }

    #[test]
    fn the_param_names_list_is_every_name_once() {
        let mut names: Vec<&str> = DECLARED
            .iter()
            .flat_map(|d| d.params.iter().map(|p| p.name))
            .collect();
        names.sort_unstable();
        names.dedup();
        let mut listed = PARAM_NAMES.to_vec();
        listed.sort_unstable();
        assert_eq!(names, listed);
    }
}
