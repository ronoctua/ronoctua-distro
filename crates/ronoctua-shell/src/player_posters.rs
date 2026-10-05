//! THE CATALOGUE'S POSTERS, as a door draws them: the pictures of a
//! listing's items, in a grid of boxes each with its title under it — and
//! the compact columns of titles alone.
//!
//! # The daemon fetches, the door decodes
//!
//! A poster is a file the daemon keeps in the catalogue's `posters` folder
//! (`ronoctua_broadcast::catalog::poster_file`), fetched only for an item a
//! door SHOWS and only from a source whose posters switch is on. This door
//! names the ones it shows and has not got ([`Posters::look`]); it decodes
//! the ones on disk OFF ITS LOOP, one at a time, no larger than
//! `space.catalog.posterPx` times the screen's scale, and draws each as the
//! Metadata draws a cover (`player_rows::CoverRow`) — fitted in its box in
//! its own proportion.
//!
//! # One key a picture as it is drawn
//!
//! A texture goes up under a key made of what changes the drawing: the file,
//! the pixels it was decoded to (the scale among them) and the palette
//! (`theme::palette()`) — a picture cached under less is a texture of
//! another screen or another theme.
//!
//! # One grid arithmetic
//!
//! The boxes are placed by `ronoctua_design::Grid` — `Grid::fitted` for how
//! many a shelf holds, `Grid::at` for where each goes — and the click reads
//! the cells those placed, never a sum of its own
//! (`the-grid-is-one-arithmetic`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, mpsc};

use ronoctua_broadcast::catalog::{self, ItemId, ItemKind, Row as CatalogRow};
use ronoctua_design::grid::Grid;
use ronoctua_render::image::Rgba;
use ronoctua_tokens as tokens;

use crate::player::Target;
use crate::player_find::Picture;
use crate::player_rows::{Builder, Cell, CoverRow, Lit, Row};

/// What a decode left: the key, and the picture — `None` for a file that is
/// no picture.
type Decoded = (String, Option<Rgba>);

/// The posters this door holds.
#[derive(Debug, Default)]
pub struct Posters {
    /// The posters' folder; `None` with no catalogue.
    dir: Option<PathBuf>,
    /// The screen's scale the door draws at (`View::scale_is`): a poster is
    /// decoded to that many pixels a logical one.
    pub(crate) scale: u32,
    /// Every picture decoded, by its key — `None` for a file that is no
    /// picture. Only the ones of the page on screen are kept.
    decoded: BTreeMap<String, Option<Picture>>,
    /// The decodes asked and not back yet.
    coming: BTreeSet<String>,
    /// The items the daemon was asked for, by the posters' generation they
    /// were asked at: asked again once another generation says some came.
    asked: BTreeSet<ItemId>,
    /// The posters' generation last seen.
    seen: u64,
    /// The decoding thread's mailbox, once it runs.
    worker: Option<mpsc::Sender<(String, PathBuf, u32)>>,
    /// What it decoded, for the next look.
    back: Arc<Mutex<Vec<Decoded>>>,
}

impl Posters {
    /// The posters of the catalogue in `dir`.
    #[must_use]
    pub fn at(dir: Option<&PathBuf>) -> Self {
        Self {
            dir: dir.map(|d| d.join(catalog::POSTERS)),
            scale: 2,
            ..Self::default()
        }
    }

    /// Where the poster of `url` is kept.
    fn file_of(&self, url: &str) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(catalog::poster_file(url)))
    }

    /// The longest side a poster is decoded to, in pixels.
    fn pixels(&self) -> u32 {
        u32::from(tokens::space::CATALOG_POSTERPX).saturating_mul(self.scale.max(1))
    }

    /// The texture's key of the poster of `url`: its file, its pixels and the
    /// palette.
    #[must_use]
    pub fn key_of(&self, url: &str) -> String {
        format!(
            "poster:{}:{}:{}",
            catalog::poster_file(url),
            self.pixels(),
            tokens::theme::palette()
        )
    }

    /// The picture held under `key`.
    #[must_use]
    pub fn picture(&self, key: &str) -> Option<&Picture> {
        self.decoded.get(key).and_then(Option::as_ref)
    }

    /// The decoded picture of `url`, when it is here.
    fn of(&self, url: &str) -> Option<(String, (u32, u32))> {
        let key = self.key_of(url);
        let (_, rgba) = self.decoded.get(&key)?.as_ref()?;
        Some((key, (rgba.width, rgba.height)))
    }

    /// WHAT A LOOK TAKES: the decodes that came back; the files of `rows`
    /// on disk and not decoded, asked to be (in place with no `wake`); and
    /// the items whose poster is not on disk and whose source's switch is
    /// `on`, returned — the daemon's to fetch, each asked once until
    /// `generation` says posters arrived. One whose switch is off is not
    /// counted as asked: turned on, it is asked at the next look.
    pub fn look(
        &mut self,
        rows: &[CatalogRow],
        generation: u64,
        wake: Option<&crate::player_subtitles::Wake>,
        on: &dyn Fn(&str) -> bool,
    ) -> Vec<ItemId> {
        let back: Vec<Decoded> =
            std::mem::take(&mut *self.back.lock().unwrap_or_else(PoisonError::into_inner));
        for (key, picture) in back {
            self.coming.remove(&key);
            let picture = picture.map(|rgba| (key.clone(), Arc::new(rgba)));
            self.decoded.insert(key, picture);
        }
        if generation != self.seen {
            self.seen = generation;
            self.asked.clear();
        }
        // ONLY THE PAGE ON SCREEN is kept decoded: a page left is textures
        // nobody draws.
        let keys: BTreeSet<String> = rows
            .iter()
            .filter_map(|r| r.poster_url.as_deref())
            .map(|u| self.key_of(u))
            .collect();
        self.decoded.retain(|k, _| keys.contains(k));
        let mut wanted = Vec::new();
        for r in rows {
            let Some(url) = r.poster_url.as_deref() else {
                continue;
            };
            let key = self.key_of(url);
            if self.decoded.contains_key(&key) || self.coming.contains(&key) {
                continue;
            }
            let Some(file) = self.file_of(url) else {
                continue;
            };
            if file.exists() {
                self.decode(key, file, wake);
            } else if on(&r.source) && self.asked.insert(r.id) {
                wanted.push(r.id);
            }
        }
        wanted
    }

    /// THE TEXTURE'S KEY OF A PICTURE ON DISK — a thumbnail the video
    /// catalogue keeps (`crate::player_ranking`): its file, its pixels and the
    /// palette, as [`Posters::key_of`] for a poster.
    #[must_use]
    pub fn key_of_file(&self, file: &Path) -> String {
        format!(
            "picture:{}:{}:{}",
            file.display(),
            self.pixels(),
            tokens::theme::palette()
        )
    }

    /// The decoded picture of `file`, when it is here.
    #[must_use]
    pub fn of_file(&self, file: &Path) -> Option<(String, (u32, u32))> {
        let key = self.key_of_file(file);
        let (_, rgba) = self.decoded.get(&key)?.as_ref()?;
        Some((key, (rgba.width, rgba.height)))
    }

    /// WHAT A LOOK AT PICTURES ON DISK TAKES — the thumbnails of a page of
    /// the ranking: the decodes that came back, and every one of `files` not
    /// decoded yet asked to be (in place with no `wake`). Only the page's are
    /// kept; nothing is asked of anyone, the files are there or they are not.
    /// `true` when a picture came back, and the rows draw it now.
    pub fn look_files(
        &mut self,
        files: &[PathBuf],
        wake: Option<&crate::player_subtitles::Wake>,
    ) -> bool {
        let back: Vec<Decoded> =
            std::mem::take(&mut *self.back.lock().unwrap_or_else(PoisonError::into_inner));
        let came = !back.is_empty();
        for (key, picture) in back {
            self.coming.remove(&key);
            let picture = picture.map(|rgba| (key.clone(), Arc::new(rgba)));
            self.decoded.insert(key, picture);
        }
        let keys: BTreeSet<String> = files.iter().map(|f| self.key_of_file(f)).collect();
        self.decoded.retain(|k, _| keys.contains(k));
        for file in files {
            let key = self.key_of_file(file);
            if self.decoded.contains_key(&key) || self.coming.contains(&key) {
                continue;
            }
            self.decode(key, file.clone(), wake);
        }
        came
    }

    /// `file` decoded under `key` — on the decoding thread, or in place.
    fn decode(&mut self, key: String, file: PathBuf, wake: Option<&crate::player_subtitles::Wake>) {
        let most = self.pixels();
        let Some(wake) = wake else {
            let picture = decoded(&file, most).map(|rgba| (key.clone(), Arc::new(rgba)));
            self.decoded.insert(key, picture);
            return;
        };
        if self.worker.is_none() {
            let (tx, rx) = mpsc::channel::<(String, PathBuf, u32)>();
            let back = self.back.clone();
            let wake = wake.clone();
            let born = std::thread::Builder::new()
                .name("player-posters-door".to_owned())
                .spawn(move || {
                    while let Ok((key, file, most)) = rx.recv() {
                        let picture = decoded(&file, most);
                        back.lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push((key, picture));
                        wake.call();
                    }
                });
            match born {
                Ok(_) => self.worker = Some(tx),
                Err(e) => {
                    tracing::warn!(error = %e, "could not start the thread that decodes posters");
                    return;
                }
            }
        }
        if let Some(w) = &self.worker
            && w.send((key.clone(), file, most)).is_ok()
        {
            self.coming.insert(key);
        }
    }
}

/// A poster's pixels, made no larger than `most` on its longer side; `None`
/// for a file that is no picture.
fn decoded(file: &Path, most: u32) -> Option<Rgba> {
    let picture = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let picture = if picture.width() > most || picture.height() > most {
        picture.thumbnail(most, most)
    } else {
        picture
    };
    let rgba = picture.to_rgba8();
    let (w, h) = rgba.dimensions();
    Rgba::new(w, h, rgba.into_raw())
}

/// The glyph a box with no poster yet stands with.
const fn kind_glyph(kind: ItemKind) -> char {
    match kind {
        ItemKind::Live => tokens::icon::CATALOG_LIVE,
        ItemKind::Video => tokens::icon::PLAYER_LANE_PICTURE,
        ItemKind::Audio => tokens::icon::PLAYER_LANE_SOUND,
    }
}

/// A row of the grid: its fields, its covers and its cells.
fn line(
    fields: Vec<crate::files::Field>,
    covers: Vec<CoverRow>,
    cells: Vec<Cell>,
    (owner, focused): (bool, bool),
    target: Target,
    spot: String,
) -> Row {
    Row {
        fields,
        focused,
        heading: false,
        target,
        spot,
        cells,
        slider: None,
        continues: !owner,
        chips: Vec::new(),
        deck: None,
        schematic: None,
        covers,
        globe: None,
        caret: None,
        lit: Lit::Row,
    }
}

/// ONE ITEM OF A GRID, OR OF A STRIP: its picture when it is decoded (the
/// texture's key and its size), the glyph its box stands with while it is
/// not, its title, the lines said under it — data, each one line — what a
/// click on it does, its spot, and the `⋯` CONTROL at its end that opens
/// more of it, with the command whose sentence says so.
#[derive(Debug, Clone)]
pub(crate) struct Tile {
    /// The decoded picture: its texture's key, and its size in pixels.
    pub(crate) picture: Option<(String, (u32, u32))>,
    /// What its box shows while it has no picture.
    pub(crate) glyph: char,
    /// Its title — data.
    pub(crate) title: String,
    /// What is said under the title, a line each — and each line in its
    /// PARTS (a channel, a score): joined on one line when they fit there,
    /// else a line each, so a number is never parted from its unit.
    pub(crate) under: Vec<Vec<String>>,
    /// What `RET` and the second click do.
    pub(crate) target: Target,
    /// Its spot — the cursor keeps it.
    pub(crate) spot: String,
    /// The control at its end, and the command it is.
    pub(crate) more: Option<(Target, &'static str)>,
}

/// A picture's box and its title's lines, in cells and lines of the rows'
/// grid — a token's, never a number of this file's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    /// The box's width, in cells.
    pub(crate) cells: u16,
    /// Its height, in lines.
    pub(crate) lines: u16,
    /// The most lines a title wraps to; the last one ends in `…` when cut.
    pub(crate) titles: u16,
}

/// THE POSTER GRID: each item a box of `space.catalog.posterCells` by
/// `posterLines` — `bigCells` by `bigLines` in the big grid — with its title
/// in `titleLines` lines under it, as many a shelf as the room holds. The
/// first line of a shelf is its row: the cursor stops there and `←`/`→` go
/// across it; every other line of the shelf continues it, so a click on a
/// picture or a title is a click on its item — the cell the grid placed.
pub(crate) fn grid(
    posters: &Posters,
    rows: &[CatalogRow],
    big: bool,
    item: &dyn Fn(&CatalogRow) -> Target,
    b: &mut Builder<'_>,
) {
    let shape = if big {
        Shape {
            cells: tokens::space::CATALOG_BIGCELLS,
            lines: tokens::space::CATALOG_BIGLINES,
            titles: tokens::space::CATALOG_TITLELINES,
        }
    } else {
        Shape {
            cells: tokens::space::CATALOG_POSTERCELLS,
            lines: tokens::space::CATALOG_POSTERLINES,
            titles: tokens::space::CATALOG_TITLELINES,
        }
    };
    let tiles: Vec<Tile> = rows
        .iter()
        .map(|r| Tile {
            picture: r.poster_url.as_deref().and_then(|u| posters.of(u)),
            glyph: kind_glyph(r.kind),
            title: r.title.clone(),
            under: Vec::new(),
            target: item(r),
            spot: format!("item-{}", r.id.0),
            more: None,
        })
        .collect();
    tiled(&tiles, shape, b);
}

/// The lines `text` takes in `cells`, at most `most` of them — wrapped, and
/// the last one ending in `…` when the text went on.
fn wrapped(text: &str, cells: u16, most: u16) -> Vec<String> {
    let mut lines = crate::chrome::note_lines(text, cells);
    let cut = lines.len() > usize::from(most);
    lines.truncate(usize::from(most));
    if cut && let Some(last) = lines.last_mut() {
        *last = ronoctua_render::layout::elide_end(&format!("{last} …"), cells);
    }
    lines
}

/// THE GRID OF [`Tile`]s, `shape` each: the boxes, the titles under them,
/// and — when a tile says something under its title, or has a control —
/// those lines under the titles. The picture's and the title's lines are
/// the tile's row (a click there is a click on it); the lines under them are
/// INERT, as a title's line is, and the control on them acts on the click —
/// so the `⋯` is taken where it is drawn and nowhere else of the box.
pub(crate) fn tiled(tiles: &[Tile], shape: Shape, b: &mut Builder<'_>) {
    if tiles.is_empty() {
        return;
    }
    let Shape {
        cells,
        lines,
        titles,
    } = shape;
    let cells = cells.min(b.room).max(1);
    let under = tiles.iter().map(|t| t.under.len()).max().unwrap_or(0);
    let more = tiles.iter().any(|t| t.more.is_some());
    let under = u16::try_from(under.max(usize::from(more))).unwrap_or(u16::MAX);
    let g = Grid::fitted(b.room, cells, 0, u16::MAX, lines + titles + under);
    // WHERE EACH GOES, by the grid: its shelf's first line and its first
    // cell — the one answer the boxes, the titles and the cells read.
    let placed: Vec<(u16, u16, &Tile)> = tiles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| g.at(i).map(|(at_line, at_cell)| (at_line, at_cell, t)))
        .collect();
    let shelves: BTreeSet<u16> = placed.iter().map(|(l, _, _)| *l).collect();
    for shelf in shelves {
        let on_shelf: Vec<&(u16, u16, &Tile)> =
            placed.iter().filter(|(l, _, _)| *l == shelf).collect();
        let first = b.out.len();
        let focused = b.cursor == first;
        let here = b.cell.min(on_shelf.len().saturating_sub(1));
        let row_target = if focused {
            on_shelf
                .get(here)
                .map_or(Target::Nothing, |(_, _, t)| t.target.clone())
        } else {
            on_shelf
                .first()
                .map_or(Target::Nothing, |(_, _, t)| t.target.clone())
        };
        let spot = on_shelf
            .first()
            .map_or_else(String::new, |(_, _, t)| format!("shelf-{}", t.spot));
        let shelf_cells: Vec<Cell> = on_shelf
            .iter()
            .map(|(_, at, t)| Cell {
                from: b.left + at,
                width: g.step,
                target: t.target.clone(),
                spot: t.spot.clone(),
                control: false,
                about: None,
            })
            .collect();
        // THE PICTURE'S LINES: each box that has its picture carries a line
        // of it; one that has not yet stands with its glyph, in the middle
        // of the box.
        for l in 0..lines {
            let mut covers = Vec::new();
            let mut fields = Vec::new();
            for (_, at, t) in &on_shelf {
                let left = b.left + at;
                match &t.picture {
                    Some((key, size)) => covers.push(CoverRow {
                        line: l,
                        lines,
                        left,
                        cells,
                        key: std::rc::Rc::from(key.as_str()),
                        size: *size,
                    }),
                    None if l == lines / 2 => fields.push(Builder::piece(
                        left + cells / 2,
                        t.glyph.to_string(),
                        tokens::color::TEXT_DIM,
                    )),
                    None => {}
                }
            }
            let owner = l == 0;
            b.out.push(line(
                fields,
                covers,
                if owner {
                    shelf_cells.clone()
                } else {
                    Vec::new()
                },
                (owner, owner && focused),
                row_target.clone(),
                spot.clone(),
            ));
        }
        // THE TITLES, wrapped in their box, the one the cursor is on in the
        // cursor's colour.
        let wrapped: Vec<Vec<String>> = on_shelf
            .iter()
            .map(|(_, _, t)| wrapped(&t.title, cells, titles))
            .collect();
        for k in 0..titles {
            let fields = on_shelf
                .iter()
                .zip(&wrapped)
                .enumerate()
                .filter_map(|(n, ((_, at, _), words))| {
                    let text = words.get(usize::from(k))?.clone();
                    let colour = if focused && n == here {
                        tokens::color::TEXT_HERE_ACTS
                    } else {
                        tokens::color::TEXT_PRIMARY
                    };
                    Some(Builder::piece(b.left + at, text, colour))
                })
                .collect();
            b.out.push(line(
                fields,
                Vec::new(),
                Vec::new(),
                (false, false),
                row_target.clone(),
                spot.clone(),
            ));
        }
        // WHAT IS SAID UNDER THE TITLES, in the quiet register — and the
        // `⋯` at the end of the first of those lines, a glyph's own cells
        // from the box's right edge, the said line cut before them.
        let glyph = tokens::space::PLAYER_QUEUEGLYPH;
        let joiner = b.texts.t(ronoctua_i18n::key::PLAYER_META_JOINER);
        for u in 0..under {
            let mut fields = Vec::new();
            let mut controls = Vec::new();
            for (_, at, t) in &on_shelf {
                let left = b.left + at;
                let control = t.more.as_ref().filter(|_| u == 0);
                let room = if control.is_some() {
                    cells.saturating_sub(glyph + tokens::space::PLAYER_QUEUEAIR)
                } else {
                    cells
                };
                if let Some(said) = t.under.get(usize::from(u)) {
                    fields.push(Builder::piece(
                        left,
                        ronoctua_render::layout::elide_end(&said.join(&joiner), room),
                        tokens::color::TEXT_DIM,
                    ));
                }
                if let Some((target, about)) = control {
                    let from = left + cells.saturating_sub(glyph);
                    fields.push(Builder::piece(
                        from,
                        tokens::icon::ELLIPSIS.to_string(),
                        tokens::color::TEXT_SECONDARY,
                    ));
                    controls.push(Cell {
                        from,
                        width: glyph,
                        target: target.clone(),
                        spot: format!("{}-more", t.spot),
                        control: true,
                        about: Some(*about),
                    });
                }
            }
            b.inert(fields);
            // AND IT LIGHTS ONLY WHERE A CLICK TAKES: the `⋯` under the
            // pointer, and not the tiles' lines beside it — a line of what
            // is said alone lights nothing.
            if let Some(row) = b.out.last_mut() {
                row.cells = controls;
                row.lit = Lit::Controls;
            }
        }
        // AND A LINE OF AIR under what is said, before the next shelf's
        // pictures: words against the picture below read as its caption.
        if under > 0 {
            b.inert(crate::chrome::AIR);
            if let Some(row) = b.out.last_mut() {
                row.lit = Lit::Controls;
            }
        }
    }
}

/// THE STRIP: a row each [`Tile`], its picture at the head — `shape.cells`
/// by `shape.lines`, when the row holds it beside a title as wide again —
/// and beside it its title, wrapped to `shape.titles` lines, the lines said
/// under it, wrapped whole, and the `⋯` control at the end of its first
/// line. Every line of a tile continues its row: a click anywhere on it is a
/// click on it — the `⋯` but taken on its own line, where it is drawn
/// (`player_rows::controls_under`). The pointer LIGHTS THE WORDS, on every
/// line of the tile, and never the picture's box ([`Lit::From`]), the same
/// whether the picture came or not. Too narrow for a picture, the rows are
/// its words alone, and light whole.
pub(crate) fn strip(tiles: &[Tile], shape: Shape, b: &mut Builder<'_>) {
    let glyph = tokens::space::PLAYER_QUEUEGLYPH;
    let air = ronoctua_design::GAP;
    let pictured = b.room >= shape.cells.saturating_mul(2) + air;
    let text_at = if pictured {
        b.left + shape.cells + air
    } else {
        b.left
    };
    let lit = if pictured {
        Lit::From(text_at)
    } else {
        Lit::Row
    };
    let end = b.left + b.room;
    let joiner = b.texts.t(ronoctua_i18n::key::PLAYER_META_JOINER);
    for t in tiles {
        let more_cells = if t.more.is_some() {
            glyph + tokens::space::PLAYER_QUEUEAIR
        } else {
            0
        };
        let room = end
            .saturating_sub(text_at)
            .saturating_sub(more_cells)
            .max(1);
        let title = wrapped(&t.title, room, shape.titles);
        let said = said_lines(&t.under, &joiner, room);
        let words = title.len() + said.len();
        let lines = if pictured {
            words.max(usize::from(shape.lines))
        } else {
            words
        };
        let first = b.out.len();
        let focused = b.cursor == first;
        let (label, _) = crate::chrome::Kind::of(true).tones(focused);
        for l in 0..lines {
            let mut fields = Vec::new();
            if let Some(text) = title.get(l) {
                fields.push(Builder::piece(text_at, text.clone(), label));
            } else if let Some(text) = said.get(l - title.len()) {
                fields.push(Builder::piece(
                    text_at,
                    text.clone(),
                    tokens::color::TEXT_DIM,
                ));
            }
            // THE PICTURE ON THE LINES OF ITS BOX, and on no other.
            let in_the_box = l < usize::from(shape.lines);
            let covers = match (&t.picture, pictured && in_the_box) {
                (Some((key, size)), true) => vec![CoverRow {
                    line: u16::try_from(l).unwrap_or(u16::MAX),
                    lines: shape.lines,
                    left: b.left,
                    cells: shape.cells,
                    key: std::rc::Rc::from(key.as_str()),
                    size: *size,
                }],
                (None, true) if l == usize::from(shape.lines / 2) => {
                    fields.push(Builder::piece(
                        b.left + shape.cells / 2,
                        t.glyph.to_string(),
                        tokens::color::TEXT_DIM,
                    ));
                    Vec::new()
                }
                _ => Vec::new(),
            };
            let mut cells = Vec::new();
            if l == 0
                && let Some((target, about)) = &t.more
            {
                let from = end.saturating_sub(glyph);
                fields.push(Builder::piece(
                    from,
                    tokens::icon::ELLIPSIS.to_string(),
                    if focused {
                        tokens::color::TEXT_HERE_ACTS
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                ));
                cells.push(Cell {
                    from,
                    width: glyph,
                    target: target.clone(),
                    spot: format!("{}-more", t.spot),
                    control: true,
                    about: Some(*about),
                });
            }
            let mut row = line(
                fields,
                covers,
                cells,
                (l == 0, l == 0 && focused),
                t.target.clone(),
                t.spot.clone(),
            );
            row.lit = lit;
            b.out.push(row);
        }
        // A LINE OF AIR under each, the tile's own: two pictures one over
        // the other read as one strip.
        let mut row = line(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            (false, false),
            t.target.clone(),
            t.spot.clone(),
        );
        row.lit = lit;
        b.out.push(row);
    }
}

/// The lines said under a strip's title in `room` cells: each line's parts
/// joined by `joiner` when the whole fits, else a line each, wrapped —
/// WRAPPED, never cut (a score cut to `70…` is no score), and a part kept
/// whole where it fits (a score parted from its unit, `7000` / `points`,
/// reads as two things).
fn said_lines(under: &[Vec<String>], joiner: &str, room: u16) -> Vec<String> {
    under
        .iter()
        .flat_map(|parts| {
            let whole = parts.join(joiner);
            if ronoctua_render::layout::width(&whole) <= room {
                vec![whole]
            } else {
                parts
                    .iter()
                    .flat_map(|p| crate::chrome::note_lines(p, room))
                    .collect()
            }
        })
        .collect()
}

/// THE COMPACT COLUMNS: titles alone, each after its kind's glyph, in the
/// columns the room holds — the files' grid, read down each column.
pub(crate) fn columns(entries: &[(char, String, Target, String)], b: &mut Builder<'_>) {
    b.grid(entries);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, title: &str, poster: Option<&str>) -> CatalogRow {
        CatalogRow {
            id: ItemId(id),
            source: "p1".into(),
            kind: ItemKind::Video,
            title: title.into(),
            letter: 'A',
            country: None,
            language: None,
            height: None,
            duration_s: None,
            year: None,
            popularity: 0,
            published_s: None,
            poster_url: poster.map(ToOwned::to_owned),
        }
    }

    /// FAILS IF: a click on a poster's picture or its title takes another
    /// item than the one drawn there — the cells the grid placed are the
    /// ones a click reads — or a shelf's lines stop continuing its row.
    #[test]
    fn a_poster_is_taken_where_it_is_drawn() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let rows: Vec<CatalogRow> = (0..7)
            .map(|n| row(n, &format!("Film number {n}"), None))
            .collect();
        let posters = Posters::default();
        let item = |r: &CatalogRow| Target::CatalogItem(catalog::Store::Main, r.id.0);
        let built = Builder::build(1, 60, 0, 0, &texts, |b| {
            grid(&posters, &rows, false, &item, b);
        });
        let step = tokens::space::CATALOG_POSTERCELLS + ronoctua_design::GAP;
        let across = usize::from((60 + ronoctua_design::GAP) / step);
        assert!(across >= 2, "the room holds {across}");
        let down =
            usize::from(tokens::space::CATALOG_POSTERLINES + tokens::space::CATALOG_TITLELINES);
        let shelves = rows.len().div_ceil(across);
        assert_eq!(built.len(), shelves * down);
        for (i, r) in rows.iter().enumerate() {
            let shelf = i / across;
            let owner = shelf * down;
            // EVERY LINE OF THE SHELF, the picture's and the title's, is
            // the item under the same column.
            for l in owner..owner + down {
                let at = Row::owner(&built, l);
                assert_eq!(at, owner);
                let column = 1 + u16::try_from(i % across).unwrap() * step + 1;
                let cell = built[at]
                    .cells
                    .iter()
                    .find(|c| crate::chrome::in_cell(column, c.from, c.width))
                    .expect("a cell under the column");
                assert_eq!(cell.target, item(r), "line {l}, item {i}");
            }
        }
        assert!(built.iter().filter(|r| r.stops()).count() == shelves);
    }

    fn tile(n: u32, picture: Option<(String, (u32, u32))>) -> Tile {
        Tile {
            picture,
            glyph: tokens::icon::PLAYER_LANE_PICTURE,
            title: format!("Tile number {n}"),
            under: vec![vec!["Slow Kitchen".into(), "7000 points".into()]],
            target: Target::Nothing,
            spot: format!("tile-{n}"),
            more: None,
        }
    }

    /// FAILS IF: a strip's row lights differently with its picture than
    /// without — every line of either from its words on, never the
    /// picture's box — or the picture is drawn on lines past its box.
    #[test]
    fn a_strip_s_rows_light_alike_from_their_words() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let shape = Shape {
            cells: 10,
            lines: 3,
            titles: 2,
        };
        let tiles = [tile(0, Some(("k".into(), (16, 9)))), tile(1, None)];
        let built = Builder::build(1, 80, 0, 0, &texts, |b| strip(&tiles, shape, b));
        let words = built[0].fields[0].at;
        assert!(words > shape.cells, "the words beside the box");
        for (n, r) in built.iter().enumerate() {
            assert_eq!(r.lit, Lit::From(words), "line {n}");
            assert!(
                r.covers.iter().all(|c| c.line < shape.lines),
                "a picture on line {n}, past its box"
            );
        }
        assert!(built.iter().any(|r| !r.covers.is_empty()));
    }

    /// FAILS IF: what is said under a strip's title is cut, or parted inside
    /// a part — a score from its unit — where its parts fit a line each.
    #[test]
    fn a_said_line_parts_between_its_parts() {
        let said = vec![vec!["Slow Kitchen".to_owned(), "7000 points".to_owned()]];
        assert_eq!(said_lines(&said, " · ", 40), ["Slow Kitchen · 7000 points"]);
        assert_eq!(
            said_lines(&said, " · ", 20),
            ["Slow Kitchen", "7000 points"]
        );
    }

    /// FAILS IF: a poster is cached under a key that does not change with
    /// the scale or the file.
    #[test]
    fn a_poster_s_key_carries_its_file_and_its_pixels() {
        let mut p = Posters::at(Some(&PathBuf::from("/nowhere")));
        let a = p.key_of("https://x.example/a.jpg");
        assert_ne!(a, p.key_of("https://x.example/b.jpg"));
        p.scale = 1;
        assert_ne!(a, p.key_of("https://x.example/a.jpg"), "the scale is in it");
    }

    /// FAILS IF: a poster not on disk is not asked of the daemon, or asked
    /// twice before posters arrived — or one whose source's switch was off is
    /// counted as asked, and never asked once the switch is turned on.
    #[test]
    fn a_missing_poster_is_asked_once_until_posters_arrive() {
        let dir = std::env::temp_dir().join(format!("ronoctua-posters-{}", std::process::id()));
        let mut p = Posters::at(Some(&dir));
        let rows = vec![
            row(1, "A", Some("https://x.example/a.jpg")),
            row(2, "B", None),
        ];
        let on = |_: &str| true;
        let off = |_: &str| false;
        assert!(p.look(&rows, 0, None, &off).is_empty(), "its switch is off");
        assert_eq!(p.look(&rows, 0, None, &on), [ItemId(1)], "turned on");
        assert!(p.look(&rows, 0, None, &on).is_empty(), "asked once");
        assert_eq!(p.look(&rows, 1, None, &on).len(), 1, "again once some came");
    }
}
