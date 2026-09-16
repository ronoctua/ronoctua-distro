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

use std::sync::OnceLock;

use smithay_client_toolkit::reexports::calloop;

use ronoctua_atlas::{Address, Atlas, Ink, Point, Pose, Tongue};
use ronoctua_render::image::Rgba;
use ronoctua_tokens as tokens;

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
}

impl Owner {
    /// How many there can be at once.
    pub const COUNT: usize = 2;

    const fn slot(self) -> usize {
        match self {
            Self::Files => 0,
            Self::Atlas => 1,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Atlas => "atlas",
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
/// THE ONE PLACE that reads the variable, so a machine cannot have the atlas
/// for one view and not for another.
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
pub fn paint(drawing: &ronoctua_atlas::Drawing, width: u32, height: u32) -> Option<Rgba> {
    use resvg::tiny_skia;
    let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))?;
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
        let rgba = ink_color(shape.ink).to_rgba8();
        let mut paint = tiny_skia::Paint {
            anti_alias: true,
            ..tiny_skia::Paint::default()
        };
        paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
        if shape.closed {
            // EVEN-ODD, and it is the line that decides whether a lake is a
            // hole: nothing in the data tags an inner ring, so winding fill
            // would put land in the Caspian.
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::EvenOdd,
                tiny_skia::Transform::identity(),
                None,
            );
        } else {
            pixmap.stroke_path(
                &path,
                &paint,
                &stroke,
                tiny_skia::Transform::identity(),
                None,
            );
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
    /// The box, in physical pixels.
    pub size: (u32, u32),
    /// Which theme it is painted in — part of the texture's identity.
    pub theme: usize,
    /// Which language the address is wanted in.
    pub portuguese: bool,
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
    /// FOUR PARTS, and the fourth is new: owner, subject, size and theme.
    /// Without the OWNER two views showing one coordinate at one size in one
    /// theme resolve to a single key — and both are redrawn under it every
    /// frame, so they would overwrite each other with nothing in any log.
    #[must_use]
    pub fn texture_key(&self) -> String {
        format!(
            "globe:{}:{}:{}x{}:{}",
            self.owner.name(),
            self.key,
            self.size.0,
            self.size.1,
            self.theme
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

                let drawing = ronoctua_atlas::project(
                    &atlas,
                    sight.pose,
                    sight.at.map(|(lat, lon)| Point { lat, lon }),
                    (sight.size.0 as f32, sight.size.1 as f32),
                    &[],
                    f32::from(tokens::space::ATLAS_DOTRADIUS) / 10.0,
                );
                // AND AFTER THE DRAW TOO: putting a picture on screen under a
                // name the cursor has already left is the one failure a preview
                // must not have.
                if mine[slot].load(std::sync::atomic::Ordering::SeqCst) != now {
                    continue;
                }
                let Some(rgba) = paint(&drawing, sight.size.0, sight.size.1) else {
                    continue;
                };
                if out
                    .send(Beat::Drawn(Box::new(Frame {
                        key: sight.key.clone(),
                        owner: sight.owner,
                        rgba,
                        dot: drawing.dot,
                    })))
                    .is_err()
                {
                    return;
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
    theme: usize,
    portuguese: bool,
    frame: Option<Shown>,
    address: Option<Address>,
    absent: bool,
    told: Option<Sight>,
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
            theme: 0,
            portuguese: false,
            frame: None,
            address: None,
            absent: false,
            told: None,
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
        self.theme = theme;
        self.portuguese = portuguese;
    }

    /// What should be drawn, when it is not what was last asked for.
    ///
    /// The value diff `wants_turn` already makes, and the same shape.
    pub fn wants(&mut self) -> Option<Ask> {
        let now = (self.at.is_some() && self.size.0 > 0 && self.size.1 > 0).then(|| Sight {
            owner: self.owner,
            key: self.key.clone(),
            at: self.at,
            pose: self.pose,
            size: self.size,
            theme: self.theme,
            portuguese: self.portuguese,
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
                });
                true
            }
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
        self.frame
            .as_ref()
            .filter(|s| s.key == self.key)
            .and_then(|s| s.dot)
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

#[cfg(test)]
mod tests {
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
}
