//! The globe, on this side of the boundary.
//!
//! # What lives here and what lives in the crate
//!
//! `ronoctua-atlas` is pure: `no_std`, zero dependencies, and it gives back
//! geometry and an [`Ink`] — a ROLE — and never a colour. That is
//! `ronoctua-syntax`'s arrangement one subject along, and it is what lets the
//! installer (where there is no store yet) and a future app ask the same
//! question without linking a rasteriser.
//!
//! This file is the other half: it finds the bytes, turns an `Ink` into a
//! token, paints with tiny-skia, and runs the thread. And it holds the
//! component any VIEW can embed — the file manager is its first caller and
//! never its owner.

use std::sync::{Arc, OnceLock};

use smithay_client_toolkit::reexports::calloop;

use ronoctua_atlas::{Address, Atlas, Ink, Placed, Point, Pose, Tongue};
use ronoctua_render::image::Rgba;
use ronoctua_tokens as tokens;

/// How far a key moves a globe, in pixels of the box.
///
/// A KEY HAS NO DISTANCE, so it borrows one: thirty pixels of the picture,
/// which `Pose::panned` then divides by the zoom — so the step is a tenth of
/// what is on screen whether the globe shows the world or one city. Every
/// view that embeds a globe steps it by this.
pub const GLOBE_STEP: f32 = 30.0;

/// How much one notch of the wheel zooms a globe — and the file manager's
/// shape, which borrows it so the two figures of one panel answer the same
/// gesture the same way.
///
/// Smaller than a key's step, because a wheel sends several notches for one
/// flick of a finger and the same factor would cross the whole range in one
/// gesture.
pub const WHEEL_ZOOM: f32 = 1.12;

/// How much one key brings a globe nearer, or takes it further — `C-k` and
/// `C-j` in every view that embeds one, and the file manager's shape, which
/// borrows it: a quarter, a step the eye follows as one place coming closer.
pub const KEY_ZOOM: f32 = 1.25;

/// WHO IS SHOWING A GLOBE.
///
/// An enum and never a free string, and it is not tidiness: it goes into the
/// texture key, and a key that two views can collide on is two pictures
/// overwriting each other every frame with nothing in any log. A typo in a
/// string would do exactly that; a variant cannot be misspelt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// The file manager's info panel.
    Files,
    /// The world-data view.
    ///
    /// NOT REACHED YET, and the variant is here on purpose rather than added
    /// when it is: the texture key carries the owner, and a key that gains a
    /// part later is a key that was colliding until it did. Two views showing
    /// one coordinate at one size in one theme would resolve to a single
    /// texture and overwrite each other every frame, with nothing in any log —
    /// so the part goes in on the first day, when there is nothing to break.
    ///
    /// See `docs/PENDING.md` for what the view still needs.
    #[allow(dead_code)]
    Atlas,
    /// The player's world radio and TV pages: a globe of marks — countries,
    /// stations, cities — and no coordinate (`crate::player_globe`).
    Player,
}

impl Owner {
    /// How many there can be at once.
    pub const COUNT: usize = 3;

    const fn slot(self) -> usize {
        match self {
            Self::Files => 0,
            Self::Atlas => 1,
            Self::Player => 2,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Atlas => "atlas",
            Self::Player => "player",
        }
    }
}

/// THE MARKS A VIEW PUTS ON ITS GLOBE — stations, countries, cities — and the
/// number that names this set of them.
///
/// Shared (`Arc`) because a set is thousands of marks and it travels with
/// every order to the thread; named by `generation` because the texture key
/// has to say which set a picture shows without carrying it.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkSet {
    /// A number the view moves each time the set is a different one — in the
    /// texture key, and on every frame, so a picture of an older set is never
    /// grabbed by the ids of this one.
    pub generation: u64,
    /// The marks, each with the view's own id.
    pub all: Vec<ronoctua_atlas::Mark>,
}

impl MarkSet {
    /// Where the mark `id` is.
    fn at_of(&self, id: u32) -> Option<Point> {
        self.all.iter().find(|m| m.id == id).map(|m| m.at)
    }

    /// The set as the pure crate draws it: the sizes and the merge are the
    /// tokens' (`atlas.markTier1..4`, in tenths of a pixel, and
    /// `atlas.markMerge`), and `lit` the group drawn as chosen.
    fn marks(&self, lit: Option<u32>) -> ronoctua_atlas::Marks<'_> {
        let tenths = |t: u16| f32::from(t) / 10.0;
        ronoctua_atlas::Marks {
            all: &self.all,
            merge_px: f32::from(tokens::space::ATLAS_MARKMERGE),
            radii: [
                tenths(tokens::space::ATLAS_MARKTIER1),
                tenths(tokens::space::ATLAS_MARKTIER2),
                tenths(tokens::space::ATLAS_MARKTIER3),
                tenths(tokens::space::ATLAS_MARKTIER4),
            ],
            lit,
        }
    }
}

/// How many globes can be on screen at once. See [`Owner`].
pub const GLOBES_AT_ONCE: usize = Owner::COUNT;

/// The baked world, read once.
///
/// `std::env::var_os` and never `option_env!`: baked in, the store path would
/// be a reference this binary carries into every closure, and re-baking the
/// table would rebuild the workspace. See `nix/rust.nix`'s `atRunTime`.
///
/// ABSENCE IS NOT AN ERROR — the `RONOCTUA_COVERAGE_FONTS` rule. The installer
/// and the rescue console have no atlas; there the panel shows the coordinate
/// and no globe, and says so.
fn bytes() -> Option<&'static [u8]> {
    static READ: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    READ.get_or_init(|| {
        let path = std::env::var_os("RONOCTUA_ATLAS")?;
        match std::fs::read(&path) {
            Ok(b) => Some(b),
            Err(e) => {
                tracing::warn!(error = %e, "the world data could not be read");
                None
            }
        }
    })
    .as_deref()
}

/// The world, opened.
///
/// THE ONE PLACE IN THE SHELL that reads the variable, so a machine cannot
/// have the atlas for one view and not for another. The player's daemon reads
/// the same variable for its directories' marks
/// (`ronoctua_player_engine`'s `atlas_map`), from its unit's environment.
pub fn world() -> Option<Atlas<'static>> {
    let b = bytes()?;
    match Atlas::open(b) {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::warn!(?e, "the world data is not a table this build reads");
            None
        }
    }
}

/// Whether there is any world data on this machine.
#[must_use]
pub fn present() -> bool {
    world().is_some()
}

/// Which of the two declared languages a name is wanted in.
#[must_use]
pub fn tongue(texts: &ronoctua_i18n::Texts) -> Tongue {
    if texts.language().starts_with("pt") {
        Tongue::Portuguese
    } else {
        Tongue::English
    }
}

/// Where a coordinate is, in the language on screen.
///
/// TWO SOURCES, AND THE SECOND ONLY ADDS. The baked world answers the country,
/// the region and the nearest town — everywhere, with nothing installed. A
/// region PACK (`LEADER s g`) answers the street and the district, and only
/// for the region it covers: what it finds is written over the two fields it
/// owns and nothing else, so a pack can never make an address worse than the
/// one without it.
#[must_use]
pub fn address_of(at: (f32, f32), texts: &ronoctua_i18n::Texts) -> Option<Address> {
    let atlas = world()?;
    let mut said = ronoctua_atlas::address(
        &atlas,
        Point {
            lat: at.0,
            lon: at.1,
        },
        tongue(texts),
    );
    streets_of(&mut said, at);
    (!said.is_empty()).then_some(said)
}

/// What the installed packs know about this coordinate, if any of them does.
///
/// EVERY PACK IS ASKED and the nearest answer wins, because regions overlap:
/// Geofabrik publishes Portugal and Europe, and somebody can have both. Asking
/// them all costs one `cell_of` per pack, which is arithmetic — the seeks only
/// happen where the coordinate actually falls.
fn streets_of(said: &mut Address, at: (f32, f32)) {
    let Some(root) = ronoctua_xdg::data_dir().map(|d| ronoctua_atlas_packs::root_of(&d)) else {
        return;
    };
    let (lat, lon) = (f64::from(at.0), f64::from(at.1));
    for pack in ronoctua_atlas_packs::installed(&root) {
        let path = ronoctua_atlas_packs::path_of(&root, &pack.id);
        let Ok(mut open) = ronoctua_atlas_packs::Pack::open(&path) else {
            continue;
        };
        if let Ok(Some((name, metres))) = open.street_near(lat, lon)
            && said.street.as_ref().is_none_or(|(_, had)| metres < *had)
        {
            said.street = Some((name, metres));
        }
        // AND THE DISTRICT, which is what a pack gives that `cities500`
        // cannot: the baked world stops at towns of five hundred people, and a
        // pack knows the neighbourhood. It replaces the town only when it is
        // NEARER, so a pack is never a worse answer than no pack.
        if let Some((name, metres)) = open.place_near(lat, lon)
            && said.place.as_ref().is_none_or(|(_, had)| metres < *had)
        {
            said.place = Some((name, metres));
        }
    }
}

/// An address as the sentence it is drawn as.
///
/// The SHAPE is a key with a selector and the ORDER of the words is the
/// language's; which branch is taken is arithmetic over what was found. A
/// separator written into a `.ftl` could not be counted and could not be given
/// a register, which is the rule `chrome::Piece::separator` already keeps.
#[must_use]
pub fn said(address: &Address, texts: &ronoctua_i18n::Texts) -> String {
    let nothing = String::new();
    let country = address
        .country
        .as_ref()
        .map_or(&nothing, |(n, _)| n)
        .clone();
    let region = address.region.clone().unwrap_or_default();
    let place = address.place.as_ref().map_or(&nothing, |(n, _)| n).clone();
    let street = address.street.as_ref().map_or(&nothing, |(n, _)| n).clone();
    texts.t_with(
        ronoctua_i18n::key::GLOBE_ADDRESS,
        &[
            ("has", address.shape()),
            ("country", &country),
            ("region", &region),
            ("place", &place),
            ("street", &street),
        ],
    )
}

/// An `Ink` as a colour — the ONE place that translates.
///
/// [`chrome::syntax_color`]'s twin, and it sits beside it for the same reason:
/// that file is already "the one place that translates", and a second one is a
/// second place to disagree. The `match` is exhaustive, so a role added to the
/// pure crate is a compile error in exactly one file.
#[must_use]
pub const fn ink_color(ink: Ink) -> tokens::Color {
    match ink {
        Ink::Ocean => tokens::color::SURFACE_ATLAS_OCEAN,
        Ink::Land => tokens::color::SURFACE_ATLAS_LAND,
        Ink::Here => tokens::color::SURFACE_ATLAS_HERE,
        Ink::Lake => tokens::color::SURFACE_ATLAS_LAKE,
        Ink::Border => tokens::color::RULE_ATLAS_BORDER,
        Ink::Region => tokens::color::RULE_ATLAS_REGION,
        Ink::Rim => tokens::color::RULE_ATLAS_RIM,
        Ink::Street => tokens::color::RULE_ATLAS_STREET,
        Ink::Dot => tokens::color::MARK_ATLAS_DOT,
        Ink::Halo => tokens::color::MARK_ATLAS_HALO,
        Ink::Chosen => tokens::color::MARK_ATLAS_CHOSEN,
    }
}

/// Paints a projected globe.
///
/// tiny-skia through `resvg`, which re-exports it whole — the precedent is
/// `pixels::from_svg`, and the same demultiply at the end: tiny-skia delivers
/// PREMULTIPLIED and everything downstream of this expects straight. Getting
/// that wrong looks like "the pictures are dirty", which nobody connects to
/// alpha.
///
/// The pixmap is left TRANSPARENT where nothing is drawn: what is behind the
/// disc is each door's own surface, which is the argument `surface.mesh`
/// already makes — one texture, two doors.
#[must_use]
/// `from` is how many pixels of the picture's TOP are cut away.
///
/// # Why a cut and not a smaller box
///
/// Because the globe's rows SCROLL, and a picture that answered a shorter box
/// by being reprojected into it changes what it SHOWS — the owner read that off
/// the screen as the map shrinking: *"conforme a scrollbar é arrastada para
/// baixo, o globo vai encolhendo, não quero que isso ocorra; quero que o globo
/// mantenha o tamanho"*.
///
/// So the drawing is projected at its FULL size, every time, and what moves is
/// where it is put down: `tiny_skia` clips to the pixmap, so translating the
/// path up by `from` keeps the scale and takes the top off. It is the crop this
/// renderer has nowhere else — nothing downstream clips, which is why an image
/// drawn half outside its panel is drawn over the listing rather than cut.
///
/// # In the palette the sight names
///
/// `palette` is [`Sight::theme`], and every ink is resolved IN it
/// (`Color::to_rgba8_in`), never in the palette the process wears at the
/// moment this runs: this runs on the worker's thread while the footer
/// paints its screens, each wearing its own page, and an ink read from the
/// process would be whichever page the loop had just put on — a texture
/// keyed for one palette carrying another's colours, and never drawn again,
/// since the sight did not change.
pub fn paint(
    drawing: &ronoctua_atlas::Drawing,
    width: u32,
    height: u32,
    from: u32,
    palette: usize,
) -> Option<Rgba> {
    use resvg::tiny_skia;
    let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))?;
    #[allow(clippy::cast_precision_loss)]
    let cut = tiny_skia::Transform::from_translate(0.0, -(from as f32));
    let stroke = tiny_skia::Stroke {
        width: f32::from(tokens::space::ATLAS_STROKE) / 10.0,
        ..tiny_skia::Stroke::default()
    };
    for shape in &drawing.shapes {
        let mut b = tiny_skia::PathBuilder::new();
        let mut points = shape.points.iter();
        let Some(first) = points.next() else { continue };
        b.move_to(first.0, first.1);
        for p in points {
            b.line_to(p.0, p.1);
        }
        if shape.closed {
            b.close();
        }
        let Some(path) = b.finish() else { continue };
        let rgba = ink_color(shape.ink).to_rgba8_in(palette);
        let mut paint = tiny_skia::Paint {
            anti_alias: true,
            ..tiny_skia::Paint::default()
        };
        paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
        if shape.closed {
            // EVEN-ODD, and it is the line that decides whether a lake is a
            // hole: nothing in the data tags an inner ring, so winding fill
            // would put land in the Caspian.
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::EvenOdd, cut, None);
        } else {
            pixmap.stroke_path(&path, &paint, &stroke, cut, None);
        }
    }
    // STRAIGHT AND NOT PREMULTIPLIED. See the header.
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let d = p.demultiply();
            [d.red(), d.green(), d.blue(), d.alpha()]
        })
        .collect();
    Rgba::new(width.max(1), height.max(1), pixels)
}

/// What a caller wants on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Sight {
    /// Which view is asking.
    pub owner: Owner,
    /// What the picture is OF — the file's stamp, or whatever names the
    /// subject. It comes back on the frame, so a picture cannot land under
    /// another subject's name.
    pub key: String,
    /// The coordinate the globe is pointed at, when there is one.
    pub at: Option<(f32, f32)>,
    /// Where it is pointed and how near.
    pub pose: Pose,
    /// The box, in physical pixels — what is SEEN, not what is drawn.
    pub size: (u32, u32),
    /// How many pixels of the picture's top have scrolled away.
    ///
    /// The drawing is always projected at `size.1 + from` and then cut, so the
    /// map keeps its size as its rows leave. See [`paint`].
    pub from: u32,
    /// Which PALETTE it is painted in — the theme and the page, as
    /// `ronoctua_tokens::theme::palette` numbers them: part of the texture's
    /// identity, and what [`paint`] resolves every ink in.
    pub theme: usize,
    /// Which language the address is wanted in.
    pub portuguese: bool,
    /// The marks on it, when the view puts any.
    pub marks: Option<Arc<MarkSet>>,
    /// The mark whose group is lit — the one under the pointer, or the one
    /// the cursor's row names.
    pub lit: Option<u32>,
}

impl Sight {
    /// Whether two sights are about the same SUBJECT.
    ///
    /// The key and nothing else — the same rule `Turn::same_file` keeps. It is
    /// what lets a drag reuse an address that has already been looked up.
    #[must_use]
    pub fn same_place(&self, other: &Self) -> bool {
        self.owner == other.owner && self.key == other.key && self.at == other.at
    }

    /// The texture's name.
    ///
    /// FIVE PARTS: owner, subject, size, the cut and theme.
    ///
    /// NOT the pose, the marks' set or the lit mark, though all three are
    /// painted INTO the picture: a globe is a MOVING picture, and both doors
    /// upload it every frame under this key (`files_window::present`, the
    /// footer's paint), so what changed reaches the GPU by replacing the
    /// texture, as a film's frame does. A key per lit group would be a new
    /// full-size texture each time the pointer crosses one, and past the
    /// renderer's ceiling its `dump` drops every texture the frame does not
    /// hold — the thumbnails and icons off screen with them.
    ///
    /// Without the OWNER two views showing one coordinate at one size in one
    /// theme resolve to a single key — and both are redrawn under it every
    /// frame, so they would overwrite each other with nothing in any log. And
    /// without the CUT a globe scrolled by one row would be handed back the
    /// picture of the row before it, because everything else about the two is
    /// the same.
    #[must_use]
    pub fn texture_key(&self) -> String {
        format!(
            "globe:{}:{}:{}x{}+{}:{}",
            self.owner.name(),
            self.key,
            self.size.0,
            self.size.1,
            self.from,
            self.theme,
        )
    }
}

/// What the thread is asked to do.
#[derive(Debug)]
pub enum Ask {
    /// Draw this.
    Show(Box<Sight>),
    /// Stop.
    Stop,
}

/// One picture, and where the dot landed in it.
#[derive(Debug)]
pub struct Frame {
    /// The subject it is of.
    pub key: String,
    /// Which view asked.
    pub owner: Owner,
    /// The picture.
    pub rgba: Rgba,
    /// Where the coordinate was drawn, in the box's own pixels — `None` when
    /// it is round the back.
    ///
    /// It comes back rather than being worked out again because WHERE A THING
    /// IS DRAWN IS WHERE IT IS GRABBED: one projection, one answer.
    pub dot: Option<(f32, f32)>,
    /// The groups of marks, where they were painted — in the box's own
    /// pixels, the cut applied, as `dot` — and only the ones whose centre is
    /// in the picture: a mark that scrolled away cannot be grabbed.
    pub marks: Vec<Placed>,
    /// Which set of marks they are of.
    pub generation: Option<u64>,
    /// The lit group's centre, as bits — what a [`Beat::Placed`] that comes
    /// after the picture is matched against.
    pub lit_centre: Option<(u32, u32)>,
    /// Where the lit group is, when the thread already knew it; otherwise it
    /// follows the picture ([`Beat::Placed`]).
    pub lit_place: Option<Address>,
}

/// The lit group's place as the thread last looked it up: the set's
/// generation, the group's centre (as bits, compared exactly), whether it
/// was asked in Portuguese — a place's name is in the language asked — and
/// the place.
type LitPlace = (u64, (u32, u32), bool, Address);

/// A picture of a sight, and where its dot and its marks landed in THE
/// PAINTED PICTURE'S pixels — what the thread sends, out of one projection.
struct Painted {
    rgba: Rgba,
    dot: Option<(f32, f32)>,
    marks: Vec<Placed>,
}

/// Projects `sight` whole, paints it with its top cut, and moves the dot and
/// the marks up with everything else — the ONE function between a sight and
/// what is shown of it and grabbed in it.
///
/// THE WHOLE PICTURE IS PROJECTED, and only part of it is put down:
/// `sight.from` is how much of the top has scrolled away, and a globe that
/// answered the shorter box by being reprojected into it would change what it
/// SHOWS. See [`paint`].
fn painted(atlas: &Atlas<'_>, sight: &Sight) -> Option<Painted> {
    #[allow(clippy::cast_precision_loss)]
    let whole = (sight.size.1 + sight.from) as f32;
    let marks = sight
        .marks
        .as_ref()
        .map_or(ronoctua_atlas::Marks::NONE, |m| m.marks(sight.lit));
    #[allow(clippy::cast_precision_loss)]
    let drawing = ronoctua_atlas::project(
        atlas,
        sight.pose,
        sight.at.map(|(lat, lon)| Point { lat, lon }),
        (sight.size.0 as f32, whole),
        &[],
        f32::from(tokens::space::ATLAS_DOTRADIUS) / 10.0,
        &marks,
    );
    let rgba = paint(
        &drawing,
        sight.size.0,
        sight.size.1,
        sight.from,
        sight.theme,
    )?;
    // IN THE PAINTED PICTURE'S OWN PIXELS, which is not where the projection
    // put them: the drawing is made at full height and the top `from` of it
    // is cut away, so every mark moves up with everything else. A mark left
    // in the projection's coordinates is a mark the eye sees in one place and
    // the hand finds in another, which is the rule these fields exist to keep.
    #[allow(clippy::cast_precision_loss)]
    let (cut, tall) = (sight.from as f32, sight.size.1 as f32);
    let seen = |y: f32| (0.0..tall).contains(&(y - cut));
    Some(Painted {
        rgba,
        dot: drawing
            .dot
            .and_then(|(x, y)| seen(y).then_some((x, y - cut))),
        marks: drawing
            .marks
            .into_iter()
            .filter(|p| seen(p.at.1))
            .map(|p| Placed {
                at: (p.at.0, p.at.1 - cut),
                ..p
            })
            .collect(),
    })
}

/// What comes back.
#[derive(Debug)]
pub enum Beat {
    /// A view of the globe.
    Drawn(Box<Frame>),
    /// Where the coordinate is — ONCE per subject, never per pose.
    ///
    /// It is what makes panning free: the address does not change when the hand
    /// moves, so it is looked up on the first order for a subject and never
    /// again.
    Said {
        /// The subject.
        key: String,
        /// Which view asked.
        owner: Owner,
        /// What was found.
        address: Box<Address>,
    },
    /// Where the lit group of a picture already sent is — looked up AFTER
    /// the picture went, so a hover onto a new group is drawn at once and not
    /// after a street lookup on a cold pack.
    Placed {
        /// The subject.
        key: String,
        /// Which view asked.
        owner: Owner,
        /// Which set of marks the group is of.
        generation: Option<u64>,
        /// The group's centre, as bits.
        centre: (u32, u32),
        /// What was found.
        place: Box<Address>,
    },
    /// There is no world data on this machine.
    Absent {
        /// The subject that could not be answered.
        key: String,
        /// Which view asked.
        owner: Owner,
    },
}

/// The thread that draws, and the era that keeps it honest.
///
/// `stage.rs`'s shape, with one difference written out below: the era is PER
/// OWNER. With a single counter, a drag in one view would bump it and cancel
/// the other view's frame — for ever, because the other view has nothing to
/// re-ask until its own value changes.
pub struct Stage {
    asks: std::sync::mpsc::Sender<Ask>,
    era: std::sync::Arc<[std::sync::atomic::AtomicU64; GLOBES_AT_ONCE]>,
    latest: [u64; GLOBES_AT_ONCE],
}

impl Stage {
    /// Hands an order over, and abandons whatever that owner asked for before.
    pub fn tell(&mut self, ask: Ask) {
        if let Ask::Show(sight) = &ask {
            let slot = sight.owner.slot();
            self.latest[slot] += 1;
            self.era[slot].store(self.latest[slot], std::sync::atomic::Ordering::SeqCst);
        }
        let _ = self.asks.send(ask);
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = self.asks.send(Ask::Stop);
    }
}

/// Raises the drawing thread.
///
/// ITS OWN CHANNEL, never the door's `Report` one, for `stage()`'s own reason:
/// a drag emits an order per pointer event, and a picture per one.
#[must_use]
pub fn stage(out: calloop::channel::Sender<Beat>) -> Option<Stage> {
    let (asks, orders) = std::sync::mpsc::channel::<Ask>();
    let era = std::sync::Arc::new([const { std::sync::atomic::AtomicU64::new(0) }; GLOBES_AT_ONCE]);
    let mine = std::sync::Arc::clone(&era);
    let born = std::thread::Builder::new()
        .name("ronoctua-globe".to_owned())
        .spawn(move || {
            // WHAT IS HELD ACROSS ORDERS: one address per owner, so panning
            // globe A does not throw away globe B's lookup.
            let mut held: [Option<(Sight, Address)>; GLOBES_AT_ONCE] =
                [const { None }; GLOBES_AT_ONCE];
            // AND THE LIT GROUP'S PLACE, once per group: a drag redraws the
            // picture per pointer event, and the group under the hand is the
            // same one all the while.
            let mut lit_held: [Option<LitPlace>; GLOBES_AT_ONCE] = [const { None }; GLOBES_AT_ONCE];
            while let Ok(first) = orders.recv() {
                // ONLY THE NEWEST SURVIVES. A drag queues an order per pointer
                // event and every one behind the last is a view nobody is
                // looking at.
                let mut ask = first;
                while let Ok(next) = orders.try_recv() {
                    ask = next;
                }
                let Ask::Show(sight) = ask else { return };
                let sight = *sight;
                let slot = sight.owner.slot();
                let now = mine[slot].load(std::sync::atomic::Ordering::SeqCst);

                let Some(atlas) = world() else {
                    let _ = out.send(Beat::Absent {
                        key: sight.key.clone(),
                        owner: sight.owner,
                    });
                    continue;
                };

                // THE ADDRESS, ONCE PER SUBJECT. A street lookup on a cold pack
                // is tens of milliseconds, which is long enough for the cursor
                // to move — so the era is checked before it and after it.
                if mine[slot].load(std::sync::atomic::Ordering::SeqCst) != now {
                    continue;
                }
                let known = held[slot]
                    .as_ref()
                    .filter(|(was, _)| was.same_place(&sight))
                    .map(|(_, a)| a.clone());
                let address = match known {
                    Some(a) => a,
                    None => {
                        let found = sight.at.map_or_else(Address::default, |(lat, lon)| {
                            ronoctua_atlas::address(
                                &atlas,
                                Point { lat, lon },
                                if sight.portuguese {
                                    Tongue::Portuguese
                                } else {
                                    Tongue::English
                                },
                            )
                        });
                        if mine[slot].load(std::sync::atomic::Ordering::SeqCst) != now {
                            continue;
                        }
                        let _ = out.send(Beat::Said {
                            key: sight.key.clone(),
                            owner: sight.owner,
                            address: Box::new(found.clone()),
                        });
                        found
                    }
                };
                held[slot] = Some((sight.clone(), address));

                let Some(picture) = painted(&atlas, &sight) else {
                    continue;
                };
                // AND AFTER THE DRAW TOO: putting a picture on screen under a
                // name the cursor has already left is the one failure a preview
                // must not have.
                if mine[slot].load(std::sync::atomic::Ordering::SeqCst) != now {
                    continue;
                }
                let generation = sight.marks.as_ref().map(|m| m.generation);
                let lit = sight
                    .lit
                    .and_then(|id| picture.marks.iter().find(|p| p.members.contains(&id)))
                    .map(|p| (p.centre, (p.centre.lat.to_bits(), p.centre.lon.to_bits())));
                // THE LIT GROUP'S PLACE, when it is already known — for this
                // set, this group and this language.
                let known = lit.and_then(|(_, centre)| {
                    lit_held[slot]
                        .as_ref()
                        .filter(|(g, c, pt, _)| {
                            Some(*g) == generation && *c == centre && *pt == sight.portuguese
                        })
                        .map(|(_, _, _, a)| a.clone())
                });
                let owed = lit.filter(|_| known.is_none());
                if out
                    .send(Beat::Drawn(Box::new(Frame {
                        key: sight.key.clone(),
                        owner: sight.owner,
                        rgba: picture.rgba,
                        dot: picture.dot,
                        marks: picture.marks,
                        generation,
                        lit_centre: lit.map(|(_, c)| c),
                        lit_place: known,
                    })))
                    .is_err()
                {
                    return;
                }
                // AND THEN THE PLACE, when it was not known: the picture is not
                // held back for a street lookup, and the lookup is kept even
                // when the cursor has moved on — the next frame of that group
                // finds it. It is SENT only while this is still the newest.
                if let Some((point, centre)) = owed {
                    let found = ronoctua_atlas::address(
                        &atlas,
                        point,
                        if sight.portuguese {
                            Tongue::Portuguese
                        } else {
                            Tongue::English
                        },
                    );
                    lit_held[slot] = Some((
                        generation.unwrap_or(0),
                        centre,
                        sight.portuguese,
                        found.clone(),
                    ));
                    if mine[slot].load(std::sync::atomic::Ordering::SeqCst) != now {
                        continue;
                    }
                    if out
                        .send(Beat::Placed {
                            key: sight.key.clone(),
                            owner: sight.owner,
                            generation,
                            centre,
                            place: Box::new(found),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        });
    match born {
        Ok(_) => Some(Stage {
            asks,
            era,
            latest: [0; GLOBES_AT_ONCE],
        }),
        Err(e) => {
            tracing::error!(error = %e, "could not create the globe thread");
            None
        }
    }
}

/// A picture that has come back, and what it is of.
#[derive(Debug, Clone)]
struct Shown {
    /// The subject it is of, so a picture cannot outlive its name.
    key: String,
    /// The picture.
    rgba: Rgba,
    /// Where the coordinate was drawn in it.
    dot: Option<(f32, f32)>,
    /// Where its groups of marks were drawn.
    marks: Vec<Placed>,
    /// Which set of marks they are of.
    generation: Option<u64>,
    /// The lit group's centre, as bits.
    lit_centre: Option<(u32, u32)>,
    /// Where the lit group is.
    lit_place: Option<Address>,
}

/// A globe a VIEW embeds.
///
/// # Who owns what
///
/// The POSE is the view's, in one field, because a view lives in two doors and
/// a pose in the door would reset the globe when the manager moved from the
/// footer to a window. The THREAD is the door's, one per door, because a door
/// has one calloop and one channel. This type is the value in between.
#[derive(Debug, Clone)]
pub struct Globe {
    owner: Owner,
    pose: Pose,
    at: Option<(f32, f32)>,
    key: String,
    size: (u32, u32),
    from: u32,
    theme: usize,
    portuguese: bool,
    frame: Option<Shown>,
    address: Option<Address>,
    absent: bool,
    told: Option<Sight>,
    marks: Option<Arc<MarkSet>>,
    lit: Option<u32>,
}

impl Globe {
    /// A globe nobody has pointed anywhere yet.
    #[must_use]
    pub fn new(owner: Owner) -> Self {
        Self {
            owner,
            pose: Pose {
                centre: Point { lat: 0.0, lon: 0.0 },
                zoom: Pose::WORLD,
            },
            at: None,
            key: String::new(),
            size: (0, 0),
            from: 0,
            theme: 0,
            portuguese: false,
            frame: None,
            address: None,
            absent: false,
            told: None,
            marks: None,
            lit: None,
        }
    }

    /// What it is pointed at, and in what box.
    ///
    /// A NEW SUBJECT REFRAMES IT: somebody who has dragged a globe of one
    /// photograph across the Pacific and then moves to another photograph
    /// wants to see where THAT one was taken.
    pub fn looking_at(
        &mut self,
        at: Option<(f32, f32)>,
        key: &str,
        size: (u32, u32),
        from: u32,
        theme: usize,
        portuguese: bool,
    ) {
        if self.key != key {
            self.key = key.to_owned();
            self.address = None;
            self.absent = false;
            self.pose = match at {
                Some((lat, lon)) => Pose::on(Point { lat, lon }),
                None => Pose {
                    centre: Point { lat: 0.0, lon: 0.0 },
                    zoom: Pose::WORLD,
                },
            };
        }
        self.at = at;
        self.size = size;
        self.from = from;
        self.theme = theme;
        self.portuguese = portuguese;
    }

    /// What should be drawn, when it is not what was last asked for.
    ///
    /// The value diff `wants_turn` already makes, and the same shape.
    ///
    /// A globe of MARKS draws with no coordinate: the player's world pages
    /// point at nothing and show everything.
    pub fn wants(&mut self) -> Option<Ask> {
        let subject = self.at.is_some() || self.marks.is_some();
        let now = (subject && self.size.0 > 0 && self.size.1 > 0).then(|| Sight {
            owner: self.owner,
            key: self.key.clone(),
            at: self.at,
            pose: self.pose,
            size: self.size,
            from: self.from,
            theme: self.theme,
            portuguese: self.portuguese,
            marks: self.marks.clone(),
            lit: self.lit,
        });
        let ask = match (&self.told, &now) {
            (None, None) => None,
            (Some(_), None) => Some(Ask::Stop),
            (Some(was), Some(w)) if was == w => None,
            (_, Some(w)) => Some(Ask::Show(Box::new(w.clone()))),
        };
        self.told = now;
        ask
    }

    /// What came back.
    ///
    /// A beat about another subject, or about another view's globe, is dropped:
    /// one file's picture under another file's name is the one failure this
    /// must not have.
    pub fn saw(&mut self, beat: &Beat) -> bool {
        match beat {
            Beat::Drawn(f) if f.owner == self.owner && f.key == self.key => {
                self.frame = Some(Shown {
                    key: f.key.clone(),
                    rgba: f.rgba.clone(),
                    dot: f.dot,
                    marks: f.marks.clone(),
                    generation: f.generation,
                    lit_centre: f.lit_centre,
                    lit_place: f.lit_place.clone(),
                });
                true
            }
            // THE PLACE AFTER ITS PICTURE, taken only by the picture it is
            // of: the same set and the same group still lit in it.
            Beat::Placed {
                key,
                owner,
                generation,
                centre,
                place,
            } if *owner == self.owner && *key == self.key => match self.frame.as_mut() {
                Some(shown)
                    if shown.generation == *generation && shown.lit_centre == Some(*centre) =>
                {
                    shown.lit_place = Some((**place).clone());
                    true
                }
                _ => false,
            },
            Beat::Said {
                key,
                owner,
                address,
            } if *owner == self.owner && *key == self.key => {
                self.address = Some((**address).clone());
                true
            }
            Beat::Absent { key, owner } if *owner == self.owner && *key == self.key => {
                self.absent = true;
                true
            }
            _ => false,
        }
    }

    /// The texture's name, when there is a picture to name.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        self.told.as_ref().map(Sight::texture_key)
    }

    /// The picture, when one has come back for what is being shown.
    #[must_use]
    pub fn frame(&self) -> Option<&Rgba> {
        self.frame
            .as_ref()
            .filter(|s| s.key == self.key)
            .map(|s| &s.rgba)
    }

    /// Where the coordinate was drawn.
    #[must_use]
    pub fn dot(&self) -> Option<(f32, f32)> {
        self.current().and_then(|s| s.dot)
    }

    /// What was found, when anything was.
    #[must_use]
    pub const fn address(&self) -> Option<&Address> {
        self.address.as_ref()
    }

    /// Whether this machine has no world data at all.
    #[must_use]
    pub const fn absent(&self) -> bool {
        self.absent
    }

    /// A drag, in pixels. The sign convention lives HERE and nowhere else —
    /// `View::spin`'s doc records what a convention spelled at two call sites
    /// costs, and this one has more than two callers ahead of it.
    pub fn pan(&mut self, right: f32, down: f32) -> bool {
        let radius = (self.size.0.min(self.size.1) as f32) * 0.5;
        if radius <= 0.0 {
            return false;
        }
        self.pose = self.pose.panned(right, down, radius);
        true
    }

    /// Nearer or further.
    pub fn zoom(&mut self, by: f32) -> bool {
        let was = self.pose.zoom;
        self.pose = self.pose.scaled(by);
        (self.pose.zoom - was).abs() > f32::EPSILON
    }

    /// Framed on the coordinate, as near as the data can honestly go.
    pub fn frame_on_the_dot(&mut self) -> bool {
        let Some((lat, lon)) = self.at else {
            return false;
        };
        let near = (self.pose.zoom - Pose::LOCAL).abs() < 0.5;
        self.pose = Pose {
            centre: Point { lat, lon },
            // ONE KEY GIVES IT AND THE SAME KEY TAKES IT BACK, which is the
            // owner's own answer for `C-w`.
            zoom: if near {
                Pose::on(Point { lat, lon }).zoom
            } else {
                Pose::LOCAL
            },
        };
        true
    }

    /// Back to the country.
    pub fn reframe(&mut self) -> bool {
        let Some((lat, lon)) = self.at else {
            return false;
        };
        self.pose = Pose::on(Point { lat, lon });
        true
    }
}

/// THE MARKS, which the player's world pages put on a globe
/// (`crate::player_globe`).
impl Globe {
    /// The marks to put on the globe — `None` takes them off. A new set does
    /// not move the globe; [`Globe::frame_on`] does, when a view asks.
    pub fn showing(&mut self, marks: Option<Arc<MarkSet>>) {
        self.marks = marks;
    }

    /// Lights the group that holds the mark `id` — or none. `true` when it
    /// changed, and the picture is asked again: the lit group is painted in.
    pub fn light(&mut self, id: Option<u32>) -> bool {
        let moved = self.lit != id;
        self.lit = id;
        moved
    }

    /// The mark whose group is lit.
    #[must_use]
    pub const fn lit(&self) -> Option<u32> {
        self.lit
    }

    /// Where it is pointed and how near — what a test of a view asks.
    #[cfg(test)]
    #[must_use]
    pub const fn pose(&self) -> Pose {
        self.pose
    }

    /// The whole world, from where it is pointed now: what `C-r` gives back
    /// on a page of the whole world.
    pub fn whole_world(&mut self) -> bool {
        let was = self.pose;
        self.pose = Pose {
            centre: Point { lat: 0.0, lon: 0.0 },
            zoom: Pose::WORLD,
        };
        was != self.pose
    }

    /// DRAWN HERE, on the caller's own thread — a door with NO LOOP, a
    /// photograph's or a test's, which has no thread to hand the order to:
    /// what [`Globe::wants`] asks, painted by the ONE function the thread
    /// paints with ([`painted`]), the address and the lit group's place
    /// looked up as the thread looks them up. Nothing when nothing changed.
    pub fn drawn_here(&mut self) {
        let Some(Ask::Show(sight)) = self.wants() else {
            return;
        };
        let Some(atlas) = world() else {
            self.absent = true;
            return;
        };
        let tongue = if sight.portuguese {
            Tongue::Portuguese
        } else {
            Tongue::English
        };
        if self.address.is_none()
            && let Some((lat, lon)) = sight.at
        {
            self.address = Some(ronoctua_atlas::address(&atlas, Point { lat, lon }, tongue));
        }
        let Some(picture) = painted(&atlas, &sight) else {
            return;
        };
        let generation = sight.marks.as_ref().map(|m| m.generation);
        let lit = sight
            .lit
            .and_then(|id| picture.marks.iter().find(|p| p.members.contains(&id)))
            .map(|p| p.centre);
        self.frame = Some(Shown {
            key: sight.key.clone(),
            rgba: picture.rgba,
            dot: picture.dot,
            marks: picture.marks,
            generation,
            lit_centre: lit.map(|c| (c.lat.to_bits(), c.lon.to_bits())),
            lit_place: lit.map(|c| ronoctua_atlas::address(&atlas, c, tongue)),
        });
    }

    /// The picture last shown, while it is of the subject AND of the set of
    /// marks shown now — a picture of an older set is never grabbed by the
    /// ids of this one.
    fn current(&self) -> Option<&Shown> {
        let generation = self.marks.as_ref().map(|m| m.generation);
        self.frame
            .as_ref()
            .filter(|s| s.key == self.key && s.generation == generation)
    }

    /// The groups of marks where the last picture drew them, in its pixels.
    #[must_use]
    pub fn marks(&self) -> &[Placed] {
        self.current().map_or(&[], |s| &s.marks)
    }

    /// The group of marks at `px`, in the picture's own pixels (as
    /// [`Globe::dot`]): the nearest centre whose disc, and `atlas.markSlack`
    /// round it, holds the point — read off the numbers it was PAINTED from.
    #[must_use]
    pub fn mark_at(&self, px: (f32, f32)) -> Option<&Placed> {
        let slack = f32::from(tokens::space::ATLAS_MARKSLACK);
        let far = |p: &Placed| {
            let (dx, dy) = (p.at.0 - px.0, p.at.1 - px.1);
            (dx * dx + dy * dy).sqrt()
        };
        self.marks()
            .iter()
            .filter(|p| far(p) <= p.radius + slack)
            .min_by(|a, b| far(a).total_cmp(&far(b)))
    }

    /// Where the lit group is, once the thread has looked it up.
    #[must_use]
    pub fn lit_place(&self) -> Option<&Address> {
        self.current()?.lit_place.as_ref()
    }

    /// Framed on the marks `members` — a group, a country's stations — whole,
    /// with the disc's own air round them. `false` when the set knows none of
    /// them, and the pose stays.
    pub fn frame_on(&mut self, members: &[u32]) -> bool {
        let Some(set) = self.marks.as_ref() else {
            return false;
        };
        let points: Vec<Point> = members.iter().filter_map(|id| set.at_of(*id)).collect();
        if points.is_empty() {
            return false;
        }
        #[allow(clippy::cast_precision_loss)]
        let size = (self.size.0 as f32, self.size.1 as f32);
        self.pose = ronoctua_atlas::framing(&points, size, f32::from(tokens::space::ATLAS_AIR));
        true
    }
}

#[cfg(test)]
mod tests {
    /// FAILS IF: the rows that scrolled away are SQUEEZED out of the picture
    /// instead of being CUT off its top.
    ///
    /// THE OWNER READ THE OTHER ANSWER OFF THE SCREEN: *"conforme a scrollbar é
    /// arrastada para baixo, o globo vai encolhendo, não quero que isso ocorra;
    /// quero que o globo mantenha o tamanho"*. A picture reprojected into a
    /// shorter box shows LESS of the world at a smaller scale; a picture cut
    /// shows less of the world at the SAME scale, which is what scrolling
    /// something off the top of a panel looks like everywhere else.
    ///
    /// The subject is a band across the BOTTOM half of a tall box. Cut by half
    /// the height it fills the short box completely; drawn into the short box
    /// with no cut it is not there at all. Those two answers cannot both come
    /// from one arithmetic, which is what makes this test worth having.
    #[test]
    fn what_scrolled_away_is_cut_from_the_top_and_never_squeezed() {
        let band = ronoctua_atlas::Drawing {
            shapes: vec![ronoctua_atlas::Shape {
                ink: ronoctua_atlas::Ink::Land,
                closed: true,
                points: vec![(0.0, 10.0), (20.0, 10.0), (20.0, 20.0), (0.0, 20.0)],
            }],
            dot: None,
            rim: 10.0,
            marks: Vec::new(),
        };
        // HOW MUCH OF THE BOX THE BAND COVERS, as a fraction of its pixels.
        let covered = |height: u32, from: u32| {
            let rgba = super::paint(&band, 20, height, from, ronoctua_tokens::theme::palette())
                .expect("a pixmap");
            let lit = rgba.pixels.chunks_exact(4).filter(|p| p[3] > 128).count();
            lit as f32 / (20.0 * height as f32)
        };
        // THE WHOLE BOX: the band is its bottom half.
        let whole = covered(20, 0);
        assert!(
            (whole - 0.5).abs() < 0.05,
            "the fixture is not half a box of ink: {whole}"
        );
        // CUT BY TEN, the band is everything there is.
        let cut = covered(10, 10);
        assert!(
            cut > 0.95,
            "the picture was not cut at its top: {cut} of the short box is ink"
        );
        // AND WITH NO CUT the short box is above the band entirely — which is
        // what a SQUEEZE could never answer, because a squeeze would put half
        // the band in it whatever the box.
        let none = covered(10, 0);
        assert!(
            none < 0.05,
            "the drawing was scaled into the short box instead of being cut: \
             {none} of it is ink"
        );
    }

    /// FAILS IF: the reader cannot open the table this build bakes.
    ///
    /// It is skipped when `RONOCTUA_ATLAS` is not set, which is the same shape
    /// the ffmpeg tests have — and the same risk, so the DERIVATION sets it
    /// (`nix/rust.nix`'s `tests`) and `atlas-is-at-run-time` proves the
    /// variable is there to be set. Run bare from a shell with nothing
    /// exported, it proves nothing and says so by passing.
    #[test]
    fn the_baked_world_opens_and_answers() {
        let Some(atlas) = super::world() else {
            return;
        };
        // LONDON, which is one of the two places `assets/world.nix` declares —
        // so the coordinate is already public in this repository and nothing
        // personal is added by pinning it (invariant 13).
        let said = ronoctua_atlas::address(
            &atlas,
            ronoctua_atlas::Point {
                lat: 51.51,
                lon: -0.13,
            },
            ronoctua_atlas::Tongue::English,
        );
        assert_eq!(
            said.country.as_ref().map(|(n, iso)| (n.as_str(), *iso)),
            Some(("United Kingdom", *b"GB")),
            "London is not in the United Kingdom"
        );
        assert!(said.region.is_some(), "London has no region");
        let (place, metres) = said.place.expect("London has no nearest place");
        assert!(
            metres < 20_000,
            "the nearest place to London is {place}, {metres} m away"
        );
        assert!(atlas.credits().contains("GeoNames"));
    }

    /// Lisbon and Porto, a station each, and a country's worth at Madrid —
    /// three public capitals and a city (invariant 13).
    fn iberia(generation: u64) -> std::sync::Arc<super::MarkSet> {
        let mark = |id, lat, lon, weight| ronoctua_atlas::Mark {
            at: ronoctua_atlas::Point { lat, lon },
            id,
            weight,
        };
        std::sync::Arc::new(super::MarkSet {
            generation,
            all: vec![
                mark(1, 38.72, -9.14, 1),
                mark(2, 41.15, -8.61, 1),
                mark(3, 40.42, -3.70, 120),
            ],
        })
    }

    /// A sight of `marks` over Iberia, 320 × 240 with its top 40 scrolled away.
    fn sight_of(marks: std::sync::Arc<super::MarkSet>, lit: Option<u32>) -> super::Sight {
        super::Sight {
            owner: super::Owner::Player,
            key: "radio-world".into(),
            at: None,
            pose: ronoctua_atlas::Pose {
                centre: ronoctua_atlas::Point {
                    lat: 40.0,
                    lon: -6.0,
                },
                zoom: 12.0,
            },
            size: (320, 240),
            from: 40,
            theme: ronoctua_tokens::theme::palette(),
            portuguese: false,
            marks: Some(marks),
            lit,
        }
    }

    /// FAILS IF: the lit mark or the set of marks becomes part of the
    /// texture's name — a new full-size texture each time the pointer
    /// crosses a group, while the picture already replaces its texture every
    /// frame under one key, as the pose does.
    #[test]
    fn the_marks_are_painted_in_and_never_name_a_texture() {
        let plain = sight_of(iberia(1), None);
        let lit = sight_of(iberia(1), Some(2));
        let other = sight_of(iberia(1), Some(3));
        let newer = sight_of(iberia(2), Some(2));
        let keys = [&plain, &lit, &other, &newer].map(super::Sight::texture_key);
        for k in &keys[1..] {
            assert_eq!(k, &keys[0], "one moving picture, one texture");
        }
        let mut moved = sight_of(iberia(1), Some(2));
        moved.pose.zoom *= 2.0;
        assert_eq!(moved.texture_key(), keys[0], "nor does the pose");
        let mut cut = sight_of(iberia(1), None);
        cut.from += 1;
        assert_ne!(cut.texture_key(), keys[0], "the cut still does");
        assert!(
            keys[0].contains("player"),
            "the owner is not in {}",
            keys[0]
        );
        // AND A GLOBE ASKS AGAIN when its lit mark moves, with no coordinate.
        let mut globe = super::Globe::new(super::Owner::Player);
        globe.looking_at(None, "radio-world", (320, 240), 0, 0, false);
        assert!(globe.wants().is_none(), "nothing to draw yet");
        globe.showing(Some(iberia(1)));
        assert!(
            matches!(globe.wants(), Some(super::Ask::Show(_))),
            "marks and no coordinate"
        );
        assert!(globe.wants().is_none());
        assert!(globe.light(Some(1)));
        assert!(!globe.light(Some(1)));
        assert!(matches!(globe.wants(), Some(super::Ask::Show(s)) if s.lit == Some(1)));
    }

    /// FAILS IF: a group of marks is grabbed anywhere but where it was painted
    /// — its disc and the slack round it — or a picture of an older set of
    /// marks is grabbed by the ids of the new one.
    #[test]
    fn a_mark_is_grabbed_where_it_is_drawn() {
        let set = iberia(1);
        let sight = sight_of(std::sync::Arc::clone(&set), Some(3));
        // THE GROUPS AS THE PURE CRATE PLACES THEM on the whole picture, cut
        // as the thread cuts them: what a frame carries without the atlas.
        #[allow(clippy::cast_precision_loss)]
        let whole = (sight.size.0 as f32, (sight.size.1 + sight.from) as f32);
        let placed: Vec<_> = ronoctua_atlas::placed(&set.marks(Some(3)), sight.pose, whole)
            .into_iter()
            .map(|p| ronoctua_atlas::Placed {
                at: (p.at.0, p.at.1 - 40.0),
                ..p
            })
            .filter(|p| (0.0..240.0).contains(&p.at.1))
            .collect();
        assert_eq!(placed.len(), 3, "{placed:?}");
        let mut globe = super::Globe::new(super::Owner::Player);
        globe.looking_at(
            None,
            "radio-world",
            sight.size,
            sight.from,
            sight.theme,
            false,
        );
        globe.showing(Some(std::sync::Arc::clone(&set)));
        let frame = |marks: Vec<ronoctua_atlas::Placed>, generation| {
            super::Beat::Drawn(Box::new(super::Frame {
                key: "radio-world".into(),
                owner: super::Owner::Player,
                rgba: ronoctua_render::image::Rgba::new(1, 1, vec![0; 4]).expect("a pixel"),
                dot: None,
                marks,
                generation: Some(generation),
                lit_centre: None,
                lit_place: None,
            }))
        };
        assert!(globe.saw(&frame(placed.clone(), 1)));
        let slack = f32::from(ronoctua_tokens::space::ATLAS_MARKSLACK);
        for p in &placed {
            assert_eq!(globe.mark_at(p.at).map(|g| &g.members), Some(&p.members));
            let edge = (p.at.0 + p.radius + slack - 0.25, p.at.1);
            assert_eq!(
                globe.mark_at(edge).map(|g| &g.members),
                Some(&p.members),
                "the slack"
            );
        }
        let madrid = placed
            .iter()
            .find(|p| p.members.contains(&3))
            .expect("Madrid");
        let past = (madrid.at.0, madrid.at.1 + madrid.radius + slack + 1.0);
        assert!(
            globe
                .mark_at(past)
                .is_none_or(|g| g.members != madrid.members),
            "grabbed past its disc"
        );
        // A PICTURE OF ANOTHER SET is never grabbed.
        assert!(globe.saw(&frame(placed.clone(), 7)));
        assert!(globe.mark_at(madrid.at).is_none());
        assert!(globe.marks().is_empty());

        // AND ON THE BAKED WORLD, where it exists: each group's centre is
        // painted in its mark's ink, the lit one as chosen.
        let Some(atlas) = super::world() else {
            return;
        };
        let picture = super::painted(&atlas, &sight).expect("a picture");
        assert_eq!(picture.marks, placed, "the thread placed them elsewhere");
        let palette = sight.theme;
        for p in &picture.marks {
            // THE PIXEL THE CENTRE IS IN, whose farthest corner is well inside
            // the smallest disc.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let (x, y) = (p.at.0.floor() as usize, p.at.1.floor() as usize);
            let at = (y * sight.size.0 as usize + x) * 4;
            let ink = if p.members.contains(&3) {
                ronoctua_atlas::Ink::Chosen
            } else {
                ronoctua_atlas::Ink::Dot
            };
            let want = super::ink_color(ink).to_rgba8_in(palette);
            assert_eq!(
                &picture.rgba.pixels[at..at + 3],
                &want[..3],
                "the group {:?} is not painted where it is grabbed",
                p.members
            );
        }
    }

    /// FAILS IF: framing a group does not bring the globe onto it, a group the
    /// set does not know moves it anyway, or the lit group's place outlives
    /// the set it was looked up for.
    #[test]
    fn a_group_is_framed_whole_and_its_place_belongs_to_its_set() {
        let mut globe = super::Globe::new(super::Owner::Player);
        globe.looking_at(None, "radio-world", (320, 240), 0, 0, false);
        assert!(!globe.frame_on(&[1, 2]), "no marks, nothing to frame");
        globe.showing(Some(iberia(4)));
        let before = globe.pose;
        assert!(!globe.frame_on(&[99]), "an id the set does not have");
        assert_eq!(globe.pose, before);
        assert!(globe.frame_on(&[1, 2]));
        let c = globe.pose.centre;
        assert!(
            (38.7..41.2).contains(&c.lat) && (-9.2..-8.6).contains(&c.lon),
            "{c:?}"
        );
        assert!(globe.pose.zoom > ronoctua_atlas::Pose::WORLD);
        // THE PLACE comes with the picture, and goes with its set.
        let place = ronoctua_atlas::Address {
            region: Some("Lisboa".into()),
            ..ronoctua_atlas::Address::default()
        };
        let frame = |generation| {
            super::Beat::Drawn(Box::new(super::Frame {
                key: "radio-world".into(),
                owner: super::Owner::Player,
                rgba: ronoctua_render::image::Rgba::new(1, 1, vec![0; 4]).expect("a pixel"),
                dot: None,
                marks: Vec::new(),
                generation: Some(generation),
                lit_centre: Some((1, 2)),
                lit_place: Some(place.clone()),
            }))
        };
        assert!(globe.saw(&frame(4)));
        assert_eq!(globe.lit_place(), Some(&place));
        assert!(globe.saw(&frame(3)));
        assert_eq!(globe.lit_place(), None);
        // OR IT COMES AFTER THE PICTURE, and is taken only by the picture of
        // its own set whose lit group it is.
        let porto = ronoctua_atlas::Address {
            region: Some("Porto".into()),
            ..ronoctua_atlas::Address::default()
        };
        let placed = |generation, centre| super::Beat::Placed {
            key: "radio-world".into(),
            owner: super::Owner::Player,
            generation: Some(generation),
            centre,
            place: Box::new(porto.clone()),
        };
        let bare = super::Beat::Drawn(Box::new(super::Frame {
            key: "radio-world".into(),
            owner: super::Owner::Player,
            rgba: ronoctua_render::image::Rgba::new(1, 1, vec![0; 4]).expect("a pixel"),
            dot: None,
            marks: Vec::new(),
            generation: Some(4),
            lit_centre: Some((1, 2)),
            lit_place: None,
        }));
        assert!(globe.saw(&bare));
        assert_eq!(globe.lit_place(), None, "not known yet");
        assert!(!globe.saw(&placed(3, (1, 2))), "another set");
        assert!(!globe.saw(&placed(4, (9, 9))), "another group");
        assert_eq!(globe.lit_place(), None);
        assert!(globe.saw(&placed(4, (1, 2))));
        assert_eq!(globe.lit_place(), Some(&porto));
    }

    /// FAILS IF: a country's anchor — where its mark goes — is not on the
    /// land it names, as the reader answers it (`country_at`).
    ///
    /// It failed for ten countries on the first run: Natural Earth's label
    /// point is where a map writes the NAME, and New Zealand's is in the sea
    /// off Taranaki. The generator moves such a point onto the land
    /// (`gen-atlas.py`, `on_land`), and this is what says it did.
    ///
    /// Skipped without the baked world, like the test above it.
    #[test]
    fn every_anchor_is_inside_its_own_country() {
        let Some(atlas) = super::world() else {
            return;
        };
        let mut anchors = 0;
        let mut outside = Vec::new();
        for a in b'A'..=b'Z' {
            for b in b'A'..=b'Z' {
                let iso = [a, b];
                let Some(at) = ronoctua_atlas::anchor_of(&atlas, iso) else {
                    continue;
                };
                anchors += 1;
                let found = ronoctua_atlas::country_at(&atlas, at, ronoctua_atlas::Tongue::English)
                    .map(|(_, code, _)| code);
                if found != Some(iso) {
                    outside.push((String::from_utf8_lossy(&iso).into_owned(), found));
                }
            }
        }
        assert!(anchors >= 100, "only {anchors} anchors baked");
        assert!(
            outside.is_empty(),
            "anchors off their own land: {outside:?}"
        );
    }
}
