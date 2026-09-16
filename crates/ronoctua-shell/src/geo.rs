//! `LEADER s g` — the regions whose streets this machine has, and the ones it
//! could have.
//!
//! # Why the file manager does not own this
//!
//! Because a region's streets are data about the MACHINE, like a disk, a
//! network or a theme — not about a folder. The manager is the first thing
//! that benefits (a photograph's address gains the name of the road it was
//! taken on) and it will not be the last: the glance, the locale view and the
//! installer all hold coordinates today. A download manager inside a file
//! manager would have been the feature owning a thing that is not its.
//!
//! # Two doors, one view
//!
//! The shape the other nine already have: this file knows what the rows are
//! and what a key does to them, and nothing about a door. `s g g` draws it in
//! the footer's region, `s g w` in a window of its own.
//!
//! # Nothing here asks the network — not even "is there a newer one?"
//!
//! The catalogue is BAKED into `world.atlas` at build time, five hundred and
//! thirty regions of it, and this view reads that. A check for updates would
//! be a second network service and a second thing to be wrong about, and it is
//! exactly what `assets/world.nix` refused when it declined a geocoder. What a
//! person gets instead is a key: choosing a region they already have downloads
//! it again, and the DATED name the download redirects to is what says which
//! of the two is newer.
//!
//! # And the size is not known before the download starts
//!
//! It cannot be, offline: Geofabrik's index carries an id, a name and a URL
//! and no size, and the only honest offline answer would be an invented one.
//! What the screen does instead is say where the answer appears — the queue
//! (`LEADER t`) shows the total the moment the server answers it, and giving
//! up there costs one key. The two measured numbers are in `docs/PENDING.md`:
//! Andorra is 6.3 MB and Brazil's south-east is 2 131 MB.

use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit as FilesHit};

/// The keys this view answers to. The hint line is GENERATED from it
/// (`chrome::hint_of`), so a key that moves here moves on screen.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "geo-down"),
    ("<up>", "geo-up"),
    ("RET", "geo-enter"),
    // BARE LETTERS, for the reason the locale view gives: nothing is typed
    // INTO this screen — the filter is asked for in the minibuffer — so a
    // letter is not a query and is the shortest way to a row's second verb.
    ("f", "geo-find"),
    ("d", "geo-remove"),
];

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, skipping headings.
    Up,
    /// Down one row, skipping headings.
    Down,
    /// Act on the row: install, install again, answer a question.
    Enter,
    /// Ask for a filter.
    Find,
    /// Remove the region under the cursor, after a question.
    Remove,
    /// Close, or leave the question.
    Escape,
}

/// What `RET` on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a heading, or a sentence.
    Nothing,
    /// Install this catalogue region, or install it again. The index is into
    /// [`View::shown`], which is the catalogue as it is being FILTERED — never
    /// into the catalogue itself, because the cursor stands on what is drawn.
    Install(usize),
    /// Remove this installed region. The index is into [`View::installed`].
    Remove(usize),
    /// Ask for the filter.
    Find,
    /// A region Geofabrik publishes without a shapefile: drawn, and refusing.
    Whole(usize),
    /// One of the two answers to the question.
    Answer(bool),
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Redraw and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Put this region in the queue.
    Install {
        /// Geofabrik's id.
        region: String,
        /// What the catalogue calls it.
        label: String,
        /// Where to get it.
        url: String,
    },
    /// Ask for the filter in the minibuffer.
    Ask,
    /// Say this on the message line.
    Speak(&'static str),
}

/// One region of the catalogue, as this view carries it.
///
/// Its own type for the reason `ronoctua_atlas::baked::Region` is: three
/// strings in a tuple are three strings told apart by counting commas. The
/// LABEL is not the table's — it carries the parent's name when that is what
/// makes it readable — and the URL is EMPTY for a region Geofabrik publishes
/// without a shapefile, which is what the row draws refusing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Geofabrik's id.
    pub id: String,
    /// What the row says.
    pub label: String,
    /// Where to get it, or empty when there is nowhere.
    pub url: String,
}

/// One row of the view.
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// A heading names the section and is not in it: the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub target: Target,
}

/// The width `key()` builds rows at. `power.rs`'s constant, for its reason:
/// the row COUNT and the row TARGETS do not depend on the width.
const DECIDING_WIDTH: u16 = 120;

/// The view.
pub struct View {
    cursor: usize,
    /// What is on disk, freshest at the top of the screen.
    installed: Vec<ronoctua_atlas_packs::Installed>,
    /// Every region the baked world carries, as `(id, label, url)`.
    catalogue: Vec<Region>,
    /// What was typed into the minibuffer, folded for comparing.
    filter: String,
    /// The region a question is open about, by its index in `installed`.
    asking: Option<usize>,
    /// Where the packs are. Held so every answer in one drawing comes from one
    /// place, and so a test can hand it a scratch directory.
    root: std::path::PathBuf,
}

impl View {
    /// Opens it on what this machine has.
    #[must_use]
    pub fn fresh() -> Self {
        let root = ronoctua_xdg::data_dir()
            .map(|d| ronoctua_atlas_packs::root_of(&d))
            .unwrap_or_default();
        Self::at(root)
    }

    /// The same, against a given root.
    #[must_use]
    pub fn at(root: std::path::PathBuf) -> Self {
        let catalogue = crate::globe::world().map_or_else(Vec::new, |atlas| {
            let raw = atlas.catalogue();
            // THE NAME CARRIES THE COUNTRY, when the country is not the name.
            //
            // Geofabrik's rows are `Sudeste`, `Sul`, `Norte` — five parts of
            // Brazil whose own names say nothing about Brazil, in a list five
            // hundred and fifty rows long. The owner went looking for the
            // country and found neither it nor its parts: *"não achei o Brasil
            // na lista"*. The parent's name is what makes the row readable AND
            // what makes the filter find it, because the filter reads the
            // label.
            //
            // ONLY ONE LEVEL UP, and never the continent: `Brazil,
            // South America` is noise on a row nobody was confused about. A
            // top-level region is one whose parent has no parent of its own.
            let top: std::collections::BTreeSet<&str> = raw
                .iter()
                .filter(|r| r.parent.is_empty())
                .map(|r| r.id.as_str())
                .collect();
            let names: std::collections::BTreeMap<&str, &str> = raw
                .iter()
                .map(|r| (r.id.as_str(), r.label.as_str()))
                .collect();
            let mut all: Vec<Region> = raw
                .iter()
                .map(|r| Region {
                    id: r.id.clone(),
                    label: match names.get(r.parent.as_str()) {
                        Some(parent) if !top.contains(r.parent.as_str()) => {
                            format!("{}, {parent}", r.label)
                        }
                        _ => r.label.clone(),
                    },
                    url: r.url.clone(),
                })
                .collect();
            // BY THE NAME AND NOT BY THE ID, which is the order the table
            // arrives in. Geofabrik sorts by its own identifier, so the first
            // row of a list somebody reads alphabetically was `Australian
            // Capital Territory` — the region whose id is `act`, above
            // `Afghanistan`. The photograph of this scene is what showed it.
            //
            // A PLAIN BYTE ORDER and not a collation: there is no collator in
            // this tree, the names are the world's regions as Geofabrik writes
            // them, and what this ordering has to be is PREDICTABLE — the
            // filter is how somebody actually reaches one.
            all.sort_by(|a, b| a.label.cmp(&b.label));
            all
        });
        let mut me = Self {
            cursor: 0,
            installed: ronoctua_atlas_packs::installed(&root),
            catalogue,
            filter: String::new(),
            asking: None,
            root,
        };
        me.settle();
        me
    }

    /// Reads the directory again — a download finished, or one was removed.
    pub fn reread(&mut self) {
        self.installed = ronoctua_atlas_packs::installed(&self.root);
        self.settle();
    }

    /// Where the cursor is, for the door that draws a band.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Puts the cursor where a click landed.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) {
        if rows.get(n).is_some_and(|r| !r.heading) {
            self.cursor = n;
        }
    }

    /// The hint line, generated.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, texts)
    }

    /// Takes what was typed in the minibuffer.
    pub fn took_a_filter(&mut self, said: &str) {
        self.filter = said.trim().to_owned();
        self.cursor = 0;
        self.settle();
    }

    /// The catalogue as it is being shown: everything, or what the filter keeps.
    ///
    /// # There is no ceiling, and there was one for a round
    ///
    /// It showed the first thirty and said so in a note — *"showing 30 of
    /// 530"* — with the filter as the way through the rest. The owner read
    /// that off the screen and it is plainly wrong: a list that stops has to
    /// offer a way to go on, and this one offered none. *"é mostrada uma lista
    /// incompleta, mas não há nada para continuar carregando a lista"*.
    ///
    /// Nothing was being saved by it either. The rows are five hundred and
    /// thirty lines of label and value; building them costs a `String` each,
    /// the region draws twelve of them, and the page scrolls like every other
    /// list in this system. A ceiling on what a LIST holds is a ceiling on
    /// memory somebody can watch grow — a catalogue baked at build time is
    /// neither.
    #[must_use]
    pub fn shown(&self) -> Vec<&Region> {
        let want = fold(&self.filter);
        self.catalogue
            .iter()
            .filter(|r| {
                want.is_empty() || fold(&r.label).contains(&want) || fold(&r.id).contains(&want)
            })
            .collect()
    }

    /// The rows to draw, at this width.
    #[must_use]
    pub fn rows(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        use ronoctua_i18n::key;
        let (left, room) = crate::power::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        if let Some(n) = self.asking {
            // THE QUESTION OPENS ON `no`, and the cursor is born there.
            //
            // The format view's rule, for its reason: a question that appears
            // over what somebody was reading must not turn the next key into
            // an answer they did not mean. Removing is an hour of somebody's
            // connection, and the only way back is to download it again.
            let label = self
                .installed
                .get(n)
                .map(|p| p.label.clone())
                .unwrap_or_default();
            b.heading_of(texts.t_with(key::GEO_SURE, &[("region", &label)]));
            b.pair(texts.t(key::GEO_NO), String::new(), Target::Answer(false));
            b.pair(texts.t(key::GEO_YES), String::new(), Target::Answer(true));
            return b.out;
        }

        b.heading(key::GEO_INSTALLED);
        if self.installed.is_empty() {
            b.note(key::GEO_NONE_INSTALLED);
        }
        for (n, pack) in self.installed.iter().enumerate() {
            b.pair(
                pack.label.clone(),
                texts.t_with(
                    key::GEO_PACK,
                    &[
                        (
                            "size",
                            &crate::files::size_text(pack.bytes, texts.decimal_mark()),
                        ),
                        ("ways", &pack.ways.to_string()),
                        ("version", &pack.dated),
                    ],
                ),
                Target::Remove(n),
            );
        }

        b.heading(key::GEO_AVAILABLE);
        if self.catalogue.is_empty() {
            // NO ATLAS, NO CATALOGUE — and the machine says so instead of
            // showing an empty list, which would read as "there are none".
            b.note(key::GEO_NO_WORLD);
            return b.out;
        }
        // THE FILTER IS A ROW AND NOT A SENTENCE, because `f` is a bare
        // letter: a key with nothing on screen to point at is a key only
        // somebody who read the hint line knows about, and a row is what the
        // mouse can reach. Its VALUE is what was typed, so the row says both
        // what it does and what it is currently doing.
        b.pair(
            texts.t(key::GEO_FILTER),
            if self.filter.is_empty() {
                texts.t(key::GEO_FILTER_NONE)
            } else {
                self.filter.clone()
            },
            Target::Find,
        );
        let shown = self.shown();
        for (n, region) in shown.iter().enumerate() {
            let have = self.installed.iter().any(|p| p.id == region.id);
            // A REGION WITH NOWHERE TO GET IT IS DRAWN, AND REFUSES.
            //
            // Twenty-nine of the five hundred and fifty-five are published as
            // `.osm.pbf` only — Brazil, Germany, France, Russia, the United
            // States, every continent — and a shapefile is what a pack is
            // baked from. They used to be left OUT, and the owner went looking
            // for the biggest of them: *"não achei o Brasil na lista"*. From
            // outside, a row that is missing and a key that does nothing are
            // the same thing; this one is the format view's XFS line, which is
            // drawn, refuses, and says what to do instead.
            let word = if region.url.is_empty() {
                key::GEO_IN_PARTS
            } else if have {
                key::GEO_HAVE
            } else {
                key::GEO_GET
            };
            b.pair(
                region.label.clone(),
                texts.t(word),
                if region.url.is_empty() {
                    Target::Whole(n)
                } else {
                    Target::Install(n)
                },
            );
        }
        b.out
    }

    /// What a key does.
    pub fn press(&mut self, p: Press, texts: &ronoctua_i18n::Texts) -> Errand {
        let rows = self.rows(DECIDING_WIDTH, texts);
        match p {
            Press::Up => {
                self.walk(&rows, false);
                Errand::Nothing
            }
            Press::Down => {
                self.walk(&rows, true);
                Errand::Nothing
            }
            Press::Find => {
                if self.asking.is_some() {
                    return Errand::Nothing;
                }
                Errand::Ask
            }
            Press::Remove => {
                if self.asking.is_some() {
                    return Errand::Nothing;
                }
                match rows.get(self.cursor).map(|r| &r.target) {
                    Some(Target::Remove(n)) => {
                        self.asking = Some(*n);
                        self.cursor = 1;
                        Errand::Nothing
                    }
                    _ => Errand::Speak(ronoctua_i18n::key::GEO_NOT_INSTALLED),
                }
            }
            Press::Escape => {
                if self.asking.take().is_some() {
                    self.cursor = 0;
                    self.settle();
                    return Errand::Nothing;
                }
                Errand::Close
            }
            Press::Enter => self.act(rows.get(self.cursor).map(|r| r.target.clone())),
        }
    }

    /// `RET`, and what a click becomes.
    fn act(&mut self, target: Option<Target>) -> Errand {
        match target {
            Some(Target::Find) => Errand::Ask,
            Some(Target::Answer(false)) | None | Some(Target::Nothing) => {
                if self.asking.take().is_some() {
                    self.cursor = 0;
                    self.settle();
                }
                Errand::Nothing
            }
            Some(Target::Answer(true)) => {
                let gone = self.asking.take().and_then(|n| self.installed.get(n));
                let Some(id) = gone.map(|p| p.id.clone()) else {
                    return Errand::Nothing;
                };
                let outcome = ronoctua_atlas_packs::remove(&self.root, &id);
                self.cursor = 0;
                self.reread();
                match outcome {
                    Ok(()) => Errand::Nothing,
                    Err(e) => {
                        tracing::warn!(?e, "the region could not be removed");
                        Errand::Speak(ronoctua_i18n::key::GEO_NOT_REMOVED)
                    }
                }
            }
            // `d` ON A ROW THAT INSTALLS IS THE ROW'S OTHER VERB, and `RET` on
            // an installed row is "install it again", which is how a pack is
            // brought up to date: the dated name the download redirects to is
            // the version, and asking for the same region again is asking for
            // whatever is current.
            Some(Target::Remove(n)) => {
                let Some(pack) = self.installed.get(n) else {
                    return Errand::Nothing;
                };
                let id = pack.id.clone();
                let Some(region) = self
                    .catalogue
                    .iter()
                    .find(|r| r.id == id && !r.url.is_empty())
                    .cloned()
                else {
                    return Errand::Speak(ronoctua_i18n::key::GEO_NOT_IN_CATALOGUE);
                };
                Errand::Install {
                    region: region.id,
                    label: region.label,
                    url: region.url,
                }
            }
            Some(Target::Install(n)) => {
                let Some(region) = self.shown().get(n).map(|r| (*r).clone()) else {
                    return Errand::Nothing;
                };
                Errand::Install {
                    region: region.id,
                    label: region.label,
                    url: region.url,
                }
            }
            // AND THE WHOLE-COUNTRY ROW SAYS WHERE ITS PARTS ARE. Pressing it
            // is not a mistake to be swallowed: somebody looking for Brazil
            // pressed the row called Brazil.
            Some(Target::Whole(_)) => Errand::Speak(ronoctua_i18n::key::GEO_ONLY_IN_PARTS),
        }
    }

    /// Moves the cursor to the next row it can stand on.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(|r| !r.heading) {
                self.cursor = at;
                return;
            }
        }
    }

    /// Puts the cursor on a row that acts, which is never row zero.
    ///
    /// Every section opens with a heading, so a cursor born at zero is a
    /// cursor on a title with nothing under it — a screen where no key does
    /// anything, which is what the locale view shipped once and the owner read
    /// off the screen.
    fn settle(&mut self) {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT);
        let Some(texts) = texts else { return };
        let rows = self.rows(DECIDING_WIDTH, &texts);
        if rows.get(self.cursor).is_some_and(|r| !r.heading) {
            return;
        }
        self.cursor = rows.iter().position(|r| !r.heading).unwrap_or(0);
    }
}

/// A string as it is compared: lower-cased, and its accents left alone.
///
/// LEFT ALONE on purpose. Folding accents needs a table of every language's
/// letters, and this list is the world's regions in English — `Ceará`, `Île-de-France`,
/// `Baden-Württemberg`. A person typing `cear` finds `Ceará` because the
/// comparison is a substring; one typing `ceara` does not, and the row above
/// says the filter so they can see why.
fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// Builds the rows. `locale.rs`'s builder, and the same reasons.
struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, colour: tokens::Color, room: u16) -> Field {
        Field {
            at,
            text: layout::elide_end(&text, room),
            color: colour,
            hit: FilesHit::Nothing,
            explain: None,
        }
    }

    fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.heading_of(text);
    }

    /// A heading whose text is already resolved.
    fn heading_of(&mut self, text: String) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                heading: true,
                target: Target::Nothing,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                text,
                tokens::color::TEXT_GROUP,
                self.room,
            )],
            heading: true,
            target: Target::Nothing,
        });
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    fn sentence(&mut self, text: String) {
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                text,
                tokens::color::TEXT_DIM,
                self.room,
            )],
            heading: true,
            target: Target::Nothing,
        });
    }

    fn pair(&mut self, label: String, value: String, target: Target) {
        let focused = self.cursor == self.out.len();
        let acts = target != Target::Nothing;
        let (label_colour, value_colour) = if focused {
            (
                tokens::color::STATE_ACCENT,
                if acts {
                    tokens::color::TEXT_HERE_ACTS
                } else {
                    tokens::color::TEXT_HERE_READ
                },
            )
        } else {
            (
                if acts {
                    tokens::color::TEXT_LABEL_ACTS
                } else {
                    tokens::color::TEXT_LABEL_FACT
                },
                if acts {
                    tokens::color::TEXT_ANSWER
                } else {
                    tokens::color::TEXT_READ_STRONG
                },
            )
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                label_colour,
                value_colour,
            ),
            heading: false,
            target,
        });
    }
}

#[cfg(test)]
mod tests;
