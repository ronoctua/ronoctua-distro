//! THE METADATA of what plays — the last section of the player's home, and
//! its pages: the cover, the streams, the chapters and every tag
//! (plan P7).
//!
//! # The sheet is asked, never broadcast
//!
//! What a file says about itself — every field, cover, stream, chapter and
//! tag — is kilobytes, and the player's state carries only
//! `State::sheet_generation`, which moves when what is known of a file that
//! plays changes. A door that shows the section asks the daemon for the sheet
//! of the file it shows (`client::sheet`) when that generation moves or the
//! file does, OFF ITS LOOP: a file read for the first time is seconds of
//! ffprobe and exiftool. The thread leaves the answer where the view takes
//! it, and wakes the door ([`Meta::look`], the pattern of the folder beside a
//! film, `player_subtitles::Beside`); a door with no loop — a photograph, a
//! test — is handed its sheet ([`Meta::given`]) and asks nobody.
//!
//! # Every common field, always
//!
//! The owner's rule: a blank field is a row that can be filled, not an absent
//! one — the daemon's sheet lists the common fields of its kind blanks
//! included (`Field::common`), and a blank says `player-tag-empty` in the
//! answer's register and acts like any other. `RET` (or the second click)
//! asks a one-line field on the door's line HOLDING what it says now; nothing
//! typed is never mind; a number, a date or an ISRC is checked
//! (`Field::check`) and refused in a sentence before anything is sent; `d`
//! clears it after a question, "no" first. A several-line field — the
//! lyrics, a comment, a description — opens in the EDITOR: the daemon writes
//! it into a file it watches (`Ask::Draft`), the door opens that file
//! (`Told::Draft`), and once it is saved with something new the state says
//! so (`State::drafts`) and ANY door asks whether to write it, "no" first —
//! the door that opened the editor may be gone.
//!
//! A field the file cannot take — an address plays, Matroska's tools are not
//! installed, the file is read only — is drawn as a fact, and `RET` on it
//! says why; when none of them can change for one reason, the reason is said
//! once under the title.
//!
//! # Every tag, by its own name
//!
//! The Every-tag page shows ALL a file says, grouped by who read it, and
//! every row the file can take is a row that acts (the owner's rule: all of
//! it shown, all of it editable where the format writes it): `RET` asks its
//! value on the door's line — holding it when it is one line and the name's
//! only value, empty with a question that says it replaces all of it
//! otherwise — and `d` takes it out after a question, "no" first. "Add a
//! tag" asks a name and then a value, into the file's main tag
//! (`Sheet::adding`). The name is checked by the tag's own grammar
//! (`ronoctua_tags::tag_name`) before anything is sent, and refused in a
//! sentence that says the rule. A row nothing writes — read to be shown,
//! data and not text, a Matroska track's — is a fact, and `RET` on it says
//! why; a reason every row of a group (or of the page) shares is said once,
//! under its title.
//!
//! # The cover is a picture of the rows
//!
//! The daemon keeps each file's cover in its cache and names the file in the
//! sheet (`Sheet::cover_file`): the door decodes that file on the thread that
//! fetched the sheet, to at most `space.player.coverPx`, and draws it over
//! `space.player.coverLines` rows of its own ([`CoverRow`]) — the way the
//! globe sits in the file manager's rows. Its row opens the cover's page:
//! another picture from the files, saved beside the file, or taken out after
//! a question.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::player::{Errand, NAME_IN_A_SENTENCE, PageId, Purpose, Target, name_of};
use crate::player_question::Question;
use crate::player_rows::Builder;
use ronoctua_i18n::{Arg, key};
use ronoctua_player::queue::Name;
use ronoctua_player::wire::{Ask, DraftId, RetagStatus, State, Trouble};
use ronoctua_render::image::Rgba;
use ronoctua_tags::{
    Can, Chapter, CoverEdit, Edit, Fact, Field, Lines, Origin, Sheet, Source, StreamKind, Tag,
    TagFormat, TagValue, Target as MkvTarget, TechKey, Why,
};
use ronoctua_tokens as tokens;

/// What the door holds of the file the section shows: the sheet the daemon
/// answered, or why there is none.
#[derive(Debug, Clone, PartialEq)]
pub enum Got {
    /// The sheet.
    Sheet(Box<Sheet>),
    /// The daemon refused, and why.
    Refused(Trouble),
    /// No player answered.
    NotRunning,
    /// The player did not answer within `client::SHEET_PATIENCE` — a file
    /// on a slow mount is still being read: asked again.
    Slow,
}

/// The sheet of one file, as the door last heard it, and its cover decoded.
#[derive(Debug, Clone)]
pub struct Held {
    /// The file (or the address) it was asked for.
    pub location: String,
    /// `State::sheet_generation` when it was asked.
    pub generation: u64,
    /// What came back.
    pub got: Got,
    /// Its cover, decoded: the texture's key and its pixels.
    pub cover: Option<(String, Arc<Rgba>)>,
}

/// Where a fetch leaves what it got.
type Slot = Arc<Mutex<Option<Held>>>;

/// A cover as a door lays it out for its batch of pictures: the texture's
/// key, where it goes, the part of the picture drawn (fractions of its
/// height) and the picture.
pub type CoverDrawn = (String, ronoctua_render::image::Area, (f32, f32), Arc<Rgba>);

/// The METADATA the view shows: which file of what plays, the sheet held of
/// it, the one being fetched, and the drafts already asked about.
#[derive(Debug, Default)]
pub struct Meta {
    /// Which of what plays the section shows: its place among
    /// [`locations`] — the file ring.
    pub file: usize,
    /// The sheet held.
    held: Option<Held>,
    /// The fetch under way: for which file and generation, and its slot.
    coming: Option<(String, u64, Slot)>,
    /// The drafts a question was put for while they stayed saved: one saved
    /// again after a refusal is asked about again.
    asked: std::collections::BTreeSet<DraftId>,
    /// The asks of one file's sheet, at one generation, that came back with
    /// no sheet — no player, or none in time: asked again on the next
    /// settles, up to [`SHEET_TRIES`], an older sheet shown meanwhile.
    failed: Option<(String, u64, u8)>,
}

/// How many times a sheet that came back with no answer is asked for, at one
/// generation, before the section says so and waits for the next.
const SHEET_TRIES: u8 = 3;

/// Every file (or address) of what plays, each once, in the player's order —
/// what the file ring walks: the sources of a set's lanes, a file alone.
#[must_use]
pub fn locations(s: &State) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for source in &s.sources {
        if !out.contains(&source.location) {
            out.push(source.location.clone());
        }
    }
    if out.is_empty()
        && let Some(l) = s.location.as_ref()
    {
        out.push(l.clone());
    }
    out
}

impl Meta {
    /// The file the section shows in `s`: the ring's place, kept inside the
    /// ring as what plays changes.
    #[must_use]
    pub fn location(&self, s: &State) -> Option<String> {
        let all = locations(s);
        if all.is_empty() {
            return None;
        }
        all.get(self.file.min(all.len() - 1)).cloned()
    }

    /// The sheet held of the file the section shows — the last one heard, a
    /// newer being fetched or not: an older answer is better than none while
    /// the new one comes.
    #[must_use]
    pub fn shown(&self, s: &State) -> Option<&Held> {
        let here = self.location(s)?;
        self.held.as_ref().filter(|h| h.location == here)
    }

    /// The sheet held, when it is one, of the file the section shows.
    #[must_use]
    pub fn sheet(&self, s: &State) -> Option<&Sheet> {
        match &self.shown(s)?.got {
            Got::Sheet(sheet) => Some(sheet),
            Got::Refused(_) | Got::NotRunning | Got::Slow => None,
        }
    }

    /// The cover to draw, when the sheet shown has one decoded: its key and
    /// its pixels.
    #[must_use]
    pub fn cover(&self, s: &State) -> Option<&(String, Arc<Rgba>)> {
        self.shown(s)?.cover.as_ref()
    }

    /// What the view takes on each settle: a fetch that finished, and — the
    /// file shown or its generation having moved — a new one, on a thread
    /// that wakes the door. With no `wake` (a photograph, a test) nothing is
    /// asked of anybody: the sheet is the one it was handed.
    pub fn look(&mut self, state: Option<&State>, wake: Option<&crate::player_subtitles::Wake>) {
        if let Some(done) = self
            .coming
            .as_ref()
            .and_then(|(_, _, slot)| slot.lock().ok()?.take())
        {
            self.coming = None;
            if matches!(done.got, Got::NotRunning | Got::Slow) {
                // NO ANSWER is no sheet: counted, asked again, and an older
                // sheet of the same file stays shown meanwhile.
                let tries = match &self.failed {
                    Some((l, g, n)) if *l == done.location && *g == done.generation => n + 1,
                    _ => 1,
                };
                self.failed = Some((done.location.clone(), done.generation, tries));
                let older_here = self
                    .held
                    .as_ref()
                    .is_some_and(|h| h.location == done.location && matches!(h.got, Got::Sheet(_)));
                if !older_here {
                    self.held = Some(done);
                }
            } else {
                self.failed = None;
                self.held = Some(done);
            }
        }
        let Some(s) = state.filter(|s| crate::player::loaded(s)) else {
            return;
        };
        let Some(location) = self.location(s) else {
            return;
        };
        let generation = s.sheet_generation;
        let wanted = |l: &str, g: u64| l == location && g == generation;
        let answered = self.held.as_ref().is_some_and(|h| {
            wanted(&h.location, h.generation) && matches!(h.got, Got::Sheet(_) | Got::Refused(_))
        });
        let gave_up = self
            .failed
            .as_ref()
            .is_some_and(|(l, g, n)| wanted(l, *g) && *n >= SHEET_TRIES);
        if answered || gave_up || self.coming.as_ref().is_some_and(|(l, g, _)| wanted(l, *g)) {
            return;
        }
        let Some(wake) = wake else {
            return;
        };
        let slot: Slot = Arc::default();
        self.coming = Some((location.clone(), generation, slot.clone()));
        let wake = wake.clone();
        let born = std::thread::Builder::new()
            .name("player-sheet-door".to_owned())
            .spawn(move || {
                let got = fetched(&location);
                let cover = cover_of(&got);
                if let Ok(mut s) = slot.lock() {
                    *s = Some(Held {
                        location,
                        generation,
                        got,
                        cover,
                    });
                }
                wake.call();
            });
        if let Err(e) = born {
            tracing::warn!(error = %e, "could not start the thread that asks for a file's sheet");
            self.coming = None;
        }
    }

    /// THE PLAYER IS GONE: every sheet held of it, and the ask under way,
    /// forgotten — a player that comes back counts its generations from zero
    /// again, and one held at the same number would be another file's read.
    pub fn forget(&mut self) {
        self.held = None;
        self.coming = None;
        self.failed = None;
    }

    /// A sheet HANDED to the door — a photograph's, a test's — for the file
    /// it is of, its cover decoded here: nothing else of the door waits.
    pub fn given(&mut self, location: &str, generation: u64, got: Got) {
        let cover = cover_of(&got);
        self.held = Some(Held {
            location: location.to_owned(),
            generation,
            got,
            cover,
        });
        self.coming = None;
    }

    /// The saved draft to ask about now, when there is one not asked about
    /// yet — and every draft no longer saved forgotten, so one saved again
    /// after a refusal is asked about again.
    pub fn draft_to_ask(&mut self, s: &State) -> Option<ronoctua_player::wire::Drafted> {
        self.asked
            .retain(|id| s.drafts.iter().any(|d| d.id == *id && d.saved));
        let d = s
            .drafts
            .iter()
            .find(|d| d.saved && !self.asked.contains(&d.id))?;
        self.asked.insert(d.id);
        Some(d.clone())
    }
}

/// The sheet of `location`, asked of a player that RUNS — never started for
/// it. Blocks: a thread's.
fn fetched(location: &str) -> Got {
    match ronoctua_player::client::sheet(location) {
        Ok(ronoctua_player::wire::Say::Sheet(sheet)) => Got::Sheet(sheet),
        Ok(ronoctua_player::wire::Say::Refused(t)) => Got::Refused(t),
        Ok(other) => {
            tracing::warn!(?other, "the player answered a sheet with something else");
            Got::Refused(Trouble::NotUnderstood { why: String::new() })
        }
        // NONE IN TIME is not "not running": a file on a slow mount is
        // still being read, and the section says so.
        Err(e) if timed_out(&e) => {
            tracing::info!(error = %e, "no sheet in time: the player is still reading the file");
            Got::Slow
        }
        Err(e) => {
            tracing::debug!(error = %e, "no sheet: the player is not running");
            Got::NotRunning
        }
    }
}

/// Whether an ask of the player failed because no answer came in time.
fn timed_out(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|c| c.downcast_ref::<std::io::Error>())
        .any(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            )
        })
}

/// A sheet's cover, decoded from the file the daemon kept it in — to at
/// most `space.player.coverPx` on its longer side — and the key its texture
/// goes up under: the kept file's own name, which is the file's, its stamp's
/// and the picture's place. `None` with no cover kept, or one that is no
/// picture.
fn cover_of(got: &Got) -> Option<(String, Arc<Rgba>)> {
    let Got::Sheet(sheet) = got else {
        return None;
    };
    let file = sheet.cover_file.as_deref()?;
    let rgba = decoded(file)?;
    Some((format!("player:cover:{}", file.display()), Arc::new(rgba)))
}

/// A picture file, decoded and made no larger than `space.player.coverPx`.
fn decoded(file: &Path) -> Option<Rgba> {
    let picture = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .map_err(
            |e| tracing::info!(file = %file.display(), error = %e, "a cover that is no picture"),
        )
        .ok()?;
    let most = u32::from(tokens::space::PLAYER_COVERPX);
    let picture = if picture.width() > most || picture.height() > most {
        picture.thumbnail(most, most)
    } else {
        picture
    };
    let rgba = picture.to_rgba8();
    let (w, h) = rgba.dimensions();
    Rgba::new(w, h, rgba.into_raw())
}

/// A field's label — its own key.
#[must_use]
pub const fn field_key(field: Field) -> &'static str {
    match field {
        Field::Title => key::PLAYER_TAG_TITLE,
        Field::Artist => key::PLAYER_TAG_ARTIST,
        Field::Album => key::PLAYER_TAG_ALBUM,
        Field::AlbumArtist => key::PLAYER_TAG_ALBUM_ARTIST,
        Field::Track => key::PLAYER_TAG_TRACK,
        Field::Disc => key::PLAYER_TAG_DISC,
        Field::Date => key::PLAYER_TAG_DATE,
        Field::Genre => key::PLAYER_TAG_GENRE,
        Field::Composer => key::PLAYER_TAG_COMPOSER,
        Field::Lyricist => key::PLAYER_TAG_LYRICIST,
        Field::Comment => key::PLAYER_TAG_COMMENT,
        Field::Lyrics => key::PLAYER_TAG_LYRICS,
        Field::Publisher => key::PLAYER_TAG_PUBLISHER,
        Field::Isrc => key::PLAYER_TAG_ISRC,
        Field::Description => key::PLAYER_TAG_DESCRIPTION,
        Field::Show => key::PLAYER_TAG_SHOW,
        Field::Season => key::PLAYER_TAG_SEASON,
        Field::Episode => key::PLAYER_TAG_EPISODE,
        Field::Director => key::PLAYER_TAG_DIRECTOR,
    }
}

/// The spot of a field's row on the Metadata section — and of its page's,
/// for a several-line field, whose `Esc` lands back on it.
#[must_use]
pub const fn field_spot(field: Field) -> &'static str {
    match field {
        Field::Title => "meta-title",
        Field::Artist => "meta-artist",
        Field::Album => "meta-album",
        Field::AlbumArtist => "meta-album-artist",
        Field::Track => "meta-track",
        Field::Disc => "meta-disc",
        Field::Date => "meta-date",
        Field::Genre => "meta-genre",
        Field::Composer => "meta-composer",
        Field::Lyricist => "meta-lyricist",
        Field::Comment => "meta-comment",
        Field::Lyrics => "meta-lyrics",
        Field::Publisher => "meta-publisher",
        Field::Isrc => "meta-isrc",
        Field::Description => "meta-description",
        Field::Show => "meta-show",
        Field::Season => "meta-season",
        Field::Episode => "meta-episode",
        Field::Director => "meta-director",
    }
}

/// A SEVERAL-LINE FIELD'S PAGE — the description, the lyrics, a comment —
/// read WHOLE: every line, wrapped, never cut; then the row that opens it in
/// the editor, or why the file cannot take it, and whether a draft of it is
/// open already. The file's name when several play.
pub(crate) fn field_page(meta: &Meta, field: Field, s: &State, b: &mut Builder<'_>) {
    let Some(sheet) = meta.sheet(s) else {
        b.note(key::PLAYER_META_READING);
        return;
    };
    if locations(s).len() > 1 {
        b.pair(
            b.t(key::PLAYER_META_FILE),
            named(&sheet.location.to_string_lossy(), b.texts),
            Target::MetaFile,
            "meta-file",
        );
    }
    let row = sheet.field(field);
    let value = row
        .and_then(|r| r.value.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    match value {
        Some(text) => {
            for line in text.lines() {
                // A BLANK LINE between verses is kept: it is part of the text.
                let lines = crate::chrome::note_lines(line, b.room);
                for l in lines {
                    b.inert(vec![Builder::piece(
                        b.left,
                        l,
                        crate::chrome::Kind::Fact.tones(false).1,
                    )]);
                }
            }
        }
        None => b.note(key::PLAYER_FIELD_BLANK),
    }
    b.inert(crate::chrome::AIR);
    let open = s
        .drafts
        .iter()
        .any(|d| d.field == field && draft_is_of(d, meta, s));
    match row.map(|r| &r.can) {
        Some(Can::No(why)) => b.warning(&crate::player::tags_refusal(why, b.texts)),
        _ => b.pair(
            b.t(key::PLAYER_FIELD_EDIT),
            if open {
                b.t(key::PLAYER_DRAFT_OPEN)
            } else {
                String::new()
            },
            Target::Draft(field),
            "field-edit",
        ),
    }
}

/// The question a one-line field is asked with: its own, so the line says
/// which field it holds (`ask-player-tag-<field>`). A several-line field is
/// never asked on a line: it opens in the editor.
#[must_use]
pub const fn question_of(field: Field) -> Option<&'static str> {
    Some(match field {
        Field::Title => "player-tag-title",
        Field::Artist => "player-tag-artist",
        Field::Album => "player-tag-album",
        Field::AlbumArtist => "player-tag-album-artist",
        Field::Track => "player-tag-track",
        Field::Disc => "player-tag-disc",
        Field::Date => "player-tag-date",
        Field::Genre => "player-tag-genre",
        Field::Composer => "player-tag-composer",
        Field::Lyricist => "player-tag-lyricist",
        Field::Publisher => "player-tag-publisher",
        Field::Isrc => "player-tag-isrc",
        Field::Show => "player-tag-show",
        Field::Season => "player-tag-season",
        Field::Episode => "player-tag-episode",
        Field::Director => "player-tag-director",
        Field::Comment | Field::Lyrics | Field::Description => return None,
    })
}

/// The field a question is about, when it is one of a field's.
#[must_use]
pub fn field_asked(id: &str) -> Option<Field> {
    Field::ALL.into_iter().find(|f| question_of(*f) == Some(id))
}

/// Whether `id` is one of the questions the metadata asks on a door's line —
/// a field's, a tag's, or a picture typed for the cover: the footer hands
/// their answers to the view.
#[must_use]
pub fn asks(id: &str) -> bool {
    typed_here(id) || id == crate::player_question::COVER_FILE
}

/// Whether `id` is a question whose answer [`typed`] takes: a field's value,
/// a tag's value, a new tag's name.
#[must_use]
pub fn typed_here(id: &str) -> bool {
    use crate::player_question::{TAG_NAME, TAG_VALUE, TAG_VALUE_ALL};
    field_asked(id).is_some() || matches!(id, TAG_NAME | TAG_VALUE | TAG_VALUE_ALL)
}

/// A technical fact's label — its own key.
#[must_use]
pub const fn tech_key(k: TechKey) -> &'static str {
    match k {
        TechKey::Container => key::PLAYER_TECH_CONTAINER,
        TechKey::DurationMs => key::PLAYER_TECH_LENGTH,
        TechKey::BitRateBitsPerSecond => key::PLAYER_TECH_BIT_RATE,
        TechKey::VideoCodec => key::PLAYER_TECH_VIDEO_CODEC,
        TechKey::WidthPx => key::PLAYER_TECH_WIDTH,
        TechKey::HeightPx => key::PLAYER_TECH_HEIGHT,
        TechKey::FramesPerSecond => key::PLAYER_TECH_FPS,
        TechKey::AudioCodec => key::PLAYER_TECH_AUDIO_CODEC,
        TechKey::SampleRateHz => key::PLAYER_TECH_SAMPLE_RATE,
        TechKey::Channels => key::PLAYER_TECH_CHANNELS,
        TechKey::ChannelLayout => key::PLAYER_TECH_CHANNEL_LAYOUT,
        TechKey::BitsPerSample => key::PLAYER_TECH_BITS_PER_SAMPLE,
        TechKey::Programs => key::PLAYER_TECH_PROGRAMS,
    }
}

/// A technical fact's value as a person reads it: a length as a clock, a
/// rate with its unit, a number with the language's decimal mark — and a
/// name (a codec, a layout) as ffmpeg spells it.
#[must_use]
pub fn tech_value(f: &Fact, texts: &ronoctua_i18n::Texts) -> String {
    let number = || f.value.trim().parse::<f64>().ok().filter(|n| n.is_finite());
    #[allow(clippy::cast_possible_truncation)]
    let whole = |n: f64| Arg::Number(n.round() as i64);
    match (f.key, number()) {
        (TechKey::DurationMs, Some(ms)) => crate::player::clock(ms / 1000.0),
        (TechKey::BitRateBitsPerSecond, Some(b)) => {
            texts.t_args(key::PLAYER_TECH_VALUE_KBPS, &[("kbps", whole(b / 1000.0))])
        }
        (TechKey::WidthPx | TechKey::HeightPx, Some(px)) => {
            texts.t_args(key::PLAYER_TECH_VALUE_PX, &[("px", whole(px))])
        }
        (TechKey::SampleRateHz, Some(hz)) => {
            texts.t_args(key::PLAYER_TECH_VALUE_HZ, &[("hz", whole(hz))])
        }
        // UP TO THE THREE DECIMALS a rate has (`23.976`), none written that
        // it does not have (`25`), after the language's decimal mark.
        (TechKey::FramesPerSecond, Some(n)) => {
            let written = format!("{n:.3}");
            let trimmed = written.trim_end_matches('0').trim_end_matches('.');
            trimmed.replace('.', &texts.decimal_mark().to_string())
        }
        (TechKey::Channels | TechKey::BitsPerSample | TechKey::Programs, Some(n)) => {
            texts.t_args(key::PLAYER_META_COUNT, &[("count", whole(n))])
        }
        _ => f.value.clone(),
    }
}

/// A count, as the rows that open a page say it.
fn count(n: usize, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        key::PLAYER_META_COUNT,
        &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
    )
}

/// What a field says, on its row: its value — ONE line: the first, after how
/// many it has, so a cut row still says there are more — or
/// `player-tag-empty` for a blank.
#[must_use]
pub fn field_value(value: Option<&str>, texts: &ronoctua_i18n::Texts) -> String {
    let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return texts.t(key::PLAYER_TAG_EMPTY);
    };
    let mut lines = v.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or_default();
    let more = lines.count();
    if more == 0 {
        return first.to_owned();
    }
    texts.t_args(
        key::PLAYER_TAG_MORE_LINES,
        &[
            ("first", Arg::Text(first)),
            (
                "lines",
                Arg::Number(i64::try_from(more + 1).unwrap_or(i64::MAX)),
            ),
        ],
    )
}

/// The one reason NONE of `sheet`'s fields can change, when there is one: it
/// is said once, under the title, and not on every row.
fn fixed_for_one_reason(sheet: &Sheet) -> Option<&Why> {
    let mut whys = sheet.fields.iter().map(|r| match &r.can {
        Can::No(why) => Some(why),
        Can::Yes(_) => None,
    });
    let first = whys.next()??;
    whys.all(|w| w == Some(first)).then_some(first)
}

/// A file's name, as a sentence says it.
fn named(location: &str, texts: &ronoctua_i18n::Texts) -> String {
    name_of(
        &Name::Location(location.to_owned()),
        texts,
        NAME_IN_A_SENTENCE,
    )
}

/// How the last write of the file shown goes, when the state says so: its
/// progress, the writes waiting behind it, and how it ended.
pub(crate) fn retag_lines(s: &State, sheet: &Sheet, shown: &str, b: &mut Builder<'_>) {
    let Some(r) = s
        .retag
        .as_ref()
        .filter(|r| r.location == shown || Path::new(&r.location) == sheet.location.as_path())
    else {
        return;
    };
    match &r.status {
        RetagStatus::Writing { permille } => {
            let said = b.texts.t_args(
                key::PLAYER_RETAG_WRITING,
                &[("percent", Arg::Number(i64::from(*permille / 10)))],
            );
            note_said(&said, b);
        }
        RetagStatus::Written => note_said(&b.t(key::PLAYER_TAG_WRITTEN), b),
        RetagStatus::Saved { path } => {
            let said = b
                .texts
                .t_with(key::PLAYER_COVER_SAVED, &[("name", &named(path, b.texts))]);
            note_said(&said, b);
        }
        RetagStatus::Refused(t) => b.warning(&crate::player::trouble_sentence(t, b.texts)),
    }
    if r.waiting > 0 {
        let said = b.texts.t_args(
            key::PLAYER_RETAG_WAITING,
            &[(
                "count",
                Arg::Number(i64::try_from(r.waiting).unwrap_or(i64::MAX)),
            )],
        );
        note_said(&said, b);
    }
}

/// A sentence already in words, wrapped in the note's register.
fn note_said(said: &str, b: &mut Builder<'_>) {
    for line in crate::chrome::note_lines(said, b.room) {
        b.inert(vec![Builder::piece(b.left, line, tokens::color::TEXT_DIM)]);
    }
}

/// THE METADATA SECTION, the home's last: which file, the suggestions, the
/// cover, every common field, the technical facts and the rows that open
/// the streams, the chapters and every tag — while something plays.
pub(crate) fn section(v: &crate::player::View, s: &State, b: &mut Builder<'_>) {
    let meta = &v.meta;
    if !crate::player::loaded(s) {
        return;
    }
    let Some(shown) = meta.location(s) else {
        return;
    };
    b.heading(key::PLAYER_HEADING_METADATA);
    // THE FILE RING, while several play.
    let all = locations(s);
    if all.len() > 1 {
        b.pair(
            b.t(key::PLAYER_META_FILE),
            named(&shown, b.texts),
            Target::MetaFile,
            "meta-file",
        );
    }
    let Some(held) = meta.shown(s) else {
        b.note(key::PLAYER_META_READING);
        return;
    };
    let sheet = match &held.got {
        Got::Sheet(sheet) => sheet,
        Got::NotRunning => {
            b.note(key::PLAYER_NOT_RUNNING);
            return;
        }
        Got::Slow => {
            b.note(key::PLAYER_META_STILL_READING);
            refresh_row(b);
            return;
        }
        Got::Refused(t) => {
            b.warning(&crate::player::trouble_sentence(t, b.texts));
            refresh_row(b);
            return;
        }
    };
    retag_lines(s, sheet, &shown, b);
    // SUGGESTIONS FROM THE NETWORK (plan P8): their page, the row saying
    // what the daemon is doing for this file, or how many it found.
    let suggestions = PageId::Suggestions;
    b.pair(
        b.t(suggestions.key()),
        crate::player_suggest::summary(v, s, b.texts),
        Target::Go(suggestions),
        suggestions.spot(),
    );
    // THE COVER: its picture over rows of its own, and the row of its page.
    if let Some((cover_key, rgba)) = &held.cover {
        b.cover(cover_key, (rgba.width, rgba.height));
    }
    let front = sheet.front_cover();
    b.pair(
        b.t(PageId::Cover.key()),
        front.map_or_else(
            || b.t(key::PLAYER_META_COVER_NONE),
            |c| match (c.width, c.height) {
                (Some(w), Some(h)) => b.texts.t_args(
                    key::PLAYER_META_COVER_SIZE,
                    &[
                        ("width", Arg::Number(i64::from(w))),
                        ("height", Arg::Number(i64::from(h))),
                    ],
                ),
                _ => b.t(key::PLAYER_META_COVER_SOME),
            },
        ),
        Target::Go(PageId::Cover),
        PageId::Cover.spot(),
    );
    // WHY NONE OF THEM CHANGES, once, when one reason holds for all.
    if let Some(why) = fixed_for_one_reason(sheet) {
        let said = b.texts.t_with(
            key::PLAYER_META_FIXED,
            &[("why", &crate::player::tags_refusal(why, b.texts))],
        );
        note_said(&said, b);
    }
    for row in &sheet.fields {
        let open = s
            .drafts
            .iter()
            .any(|d| d.field == row.field && d.location == shown && !d.saved);
        let value = if open {
            b.t(key::PLAYER_DRAFT_OPEN)
        } else {
            field_value(row.value.as_deref(), b.texts)
        };
        let target = if row.can.can() {
            Target::Tag(row.field)
        } else {
            Target::TagFixed(row.field)
        };
        // THE VALUE WHOLE, wrapped in its column: "see all the metadata" —
        // a title cut to "The Lighthous…" in a narrow door said less than
        // the file does.
        b.pair_whole(
            b.t(field_key(row.field)),
            value,
            target,
            field_spot(row.field),
        );
    }
    if !sheet.technical.is_empty() {
        b.heading_within(b.t(key::PLAYER_HEADING_TECHNICAL));
        for f in &sheet.technical {
            b.pair_whole(
                b.t(tech_key(f.key)),
                tech_value(f, b.texts),
                Target::Nothing,
                &format!("meta-tech-{}", tech_key(f.key)),
            );
        }
    }
    b.inert(crate::chrome::AIR);
    for (page, n) in [
        (PageId::Streams, sheet.streams.len()),
        (PageId::Chapters, sheet.chapters.len()),
        (PageId::EveryTag, sheet.other.len()),
    ] {
        b.pair(
            b.t(page.key()),
            count(n, b.texts),
            Target::Go(page),
            page.spot(),
        );
    }
    refresh_row(b);
    // WHAT COULD NOT BE READ, last, after air: a sheet with fewer rows says
    // why it has fewer.
    if !sheet.unread.is_empty() {
        b.inert(crate::chrome::AIR);
        unread(sheet, b);
    }
}

/// The row that reads the file again — after another program changed it, or
/// a change refused because it had.
fn refresh_row(b: &mut Builder<'_>) {
    b.pair(
        b.t(key::PLAYER_META_REFRESH),
        String::new(),
        Target::MetaRefresh,
        "meta-refresh",
    );
}

/// What could not be read, and why — a sheet with fewer rows says so.
fn unread(sheet: &Sheet, b: &mut Builder<'_>) {
    for u in &sheet.unread {
        let source = b.t(match u.source {
            Source::Lofty => key::PLAYER_SHEET_SOURCE_LOFTY,
            Source::Ffprobe => key::PLAYER_SHEET_SOURCE_FFPROBE,
            Source::Exiftool => key::PLAYER_SHEET_SOURCE_EXIFTOOL,
            Source::Matroska => key::PLAYER_SHEET_SOURCE_MATROSKA,
        });
        let said = b.texts.t_with(
            key::PLAYER_SHEET_UNREAD,
            &[
                ("source", &source),
                ("why", &crate::player::tags_refusal(&u.why, b.texts)),
            ],
        );
        b.warning(&said);
    }
}

/// THE COVER'S PAGE: the picture, and what can be done with it — another one
/// from the files, saved beside the file, taken out after a question — or
/// why none of that can be done here.
pub(crate) fn cover_page(meta: &Meta, s: &State, b: &mut Builder<'_>) {
    let Some(sheet) = meta.sheet(s) else {
        b.note(key::PLAYER_META_READING);
        return;
    };
    if let Some((cover_key, rgba)) = meta.cover(s) {
        b.cover(cover_key, (rgba.width, rgba.height));
    }
    let front = sheet.front_cover();
    if front.is_none() {
        b.note(key::PLAYER_META_COVER_NOTHING);
    }
    match &sheet.cover_can {
        Can::Yes(_) => {
            let pick = PageId::Files(Purpose::Cover);
            b.pair(
                b.t(pick.key()),
                String::new(),
                Target::Go(pick),
                pick.spot(),
            );
        }
        Can::No(why) => b.warning(&crate::player::tags_refusal(why, b.texts)),
    }
    if let Some(c) = front {
        // SAVED BESIDE: reading the picture out is no change to the file.
        if sheet.stamp != ronoctua_tags::Stamp::NONE {
            b.pair(
                b.t(key::PLAYER_COVER_SAVE),
                String::new(),
                Target::CoverSave,
                "cover-save",
            );
        }
        if c.can.can() {
            b.pair(
                b.t(key::PLAYER_COVER_REMOVE),
                String::new(),
                Target::CoverRemove,
                "cover-remove",
            );
        }
    }
}

/// THE STREAMS' PAGE: every stream of the container, what it is, its codec,
/// language and title, and its marks.
pub(crate) fn streams_page(meta: &Meta, s: &State, b: &mut Builder<'_>) {
    let Some(sheet) = meta.sheet(s) else {
        b.note(key::PLAYER_META_READING);
        return;
    };
    if sheet.streams.is_empty() {
        b.note(key::PLAYER_STREAMS_NONE);
    }
    for st in &sheet.streams {
        let kind = b.t(match st.kind {
            StreamKind::Video if st.attached => key::PLAYER_STREAM_KIND_COVER,
            StreamKind::Video => key::PLAYER_STREAM_KIND_VIDEO,
            StreamKind::Audio => key::PLAYER_STREAM_KIND_AUDIO,
            StreamKind::Subtitle => key::PLAYER_STREAM_KIND_SUBTITLE,
            StreamKind::Data => key::PLAYER_STREAM_KIND_DATA,
            StreamKind::Attachment => key::PLAYER_STREAM_KIND_ATTACHMENT,
        });
        let label = b.texts.t_args(
            key::PLAYER_STREAM_LABEL,
            &[
                ("index", Arg::Number(i64::from(st.index))),
                ("kind", Arg::Text(&kind)),
            ],
        );
        // THE LANGUAGE BY ITS NAME where the table knows its code (`por` is
        // Portuguese), as the service's own word where it does not.
        let lang = st
            .lang
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| crate::player_find::tongue_name(l, b.texts));
        let mut said: Vec<String> = [st.codec.clone(), lang, st.title.clone()]
            .into_iter()
            .flatten()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
            .collect();
        for (on, k) in [
            (st.default, key::PLAYER_STREAM_DEFAULT),
            (st.forced, key::PLAYER_STREAM_FORCED),
            (st.hearing, key::PLAYER_STREAM_HEARING),
        ] {
            if on {
                said.push(b.t(k));
            }
        }
        b.pair_whole(
            label,
            said.join(&b.t(key::PLAYER_META_JOINER)),
            Target::Nothing,
            &format!("stream-{}", st.index),
        );
    }
}

/// THE CHAPTERS' PAGE: each chapter where it starts, and its title.
pub(crate) fn chapters_page(meta: &Meta, s: &State, b: &mut Builder<'_>) {
    let Some(sheet) = meta.sheet(s) else {
        b.note(key::PLAYER_META_READING);
        return;
    };
    if sheet.chapters.is_empty() {
        b.note(key::PLAYER_CHAPTERS_NONE);
    }
    for (
        i,
        Chapter {
            start_ms, title, ..
        },
    ) in sheet.chapters.iter().enumerate()
    {
        #[allow(clippy::cast_precision_loss)]
        let at = crate::player::clock(*start_ms as f64 / 1000.0);
        b.pair(
            at,
            title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map_or_else(|| b.t(key::PLAYER_CHAPTER_UNTITLED), ToOwned::to_owned),
            Target::Nothing,
            &format!("chapter-{i}"),
        );
    }
}

/// The title of the group a tag is in: who read it and from where.
fn origin_title(origin: &Origin, texts: &ronoctua_i18n::Texts) -> String {
    match origin {
        Origin::Lofty { format } => texts.t(match format {
            TagFormat::Id3v2 => key::PLAYER_ORIGIN_ID3V2,
            TagFormat::Id3v1 => key::PLAYER_ORIGIN_ID3V1,
            TagFormat::Ape => key::PLAYER_ORIGIN_APE,
            TagFormat::Vorbis => key::PLAYER_ORIGIN_VORBIS,
            TagFormat::Mp4 => key::PLAYER_ORIGIN_MP4,
            TagFormat::RiffInfo => key::PLAYER_ORIGIN_RIFF_INFO,
            TagFormat::AiffText => key::PLAYER_ORIGIN_AIFF_TEXT,
        }),
        Origin::Matroska { target } => texts.t(match target {
            MkvTarget::Global => key::PLAYER_ORIGIN_MATROSKA_GLOBAL,
            MkvTarget::Track => key::PLAYER_ORIGIN_MATROSKA_TRACK,
            MkvTarget::Other => key::PLAYER_ORIGIN_MATROSKA_OTHER,
        }),
        Origin::Ffprobe { stream: None } => texts.t(key::PLAYER_ORIGIN_CONTAINER),
        Origin::Ffprobe {
            stream: Some(index),
        } => texts.t_args(
            key::PLAYER_ORIGIN_STREAM_N,
            &[("index", Arg::Number(i64::from(*index)))],
        ),
        Origin::Exiftool { group } => {
            texts.t_with(key::PLAYER_ORIGIN_EXIFTOOL, &[("group", group.as_str())])
        }
        Origin::Stream => texts.t(key::PLAYER_ORIGIN_STREAM),
    }
}

/// A tag's value on its row: its text on one line, the beginning of a long
/// one and how long it really is, or how many bytes of data.
fn tag_value(v: &TagValue, texts: &ronoctua_i18n::Texts) -> String {
    let size = |bytes: u64| crate::files::size_text(bytes, texts.decimal_mark());
    match v {
        TagValue::Text(text) => field_value(Some(text), texts),
        TagValue::Cut { head, bytes } => texts.t_with(
            key::PLAYER_TAG_VALUE_CUT,
            &[
                ("head", head.lines().next().unwrap_or_default()),
                ("size", &size(*bytes)),
            ],
        ),
        TagValue::Binary { bytes } => {
            texts.t_with(key::PLAYER_TAG_VALUE_BINARY, &[("size", &size(*bytes))])
        }
    }
}

/// EVERY TAG'S PAGE: everything the tools read of the file, by its own name,
/// grouped by who read it — each row the file can take acting, each it cannot
/// saying why — the row that adds one, and what could not be read.
pub(crate) fn every_tag_page(meta: &Meta, s: &State, b: &mut Builder<'_>) {
    let Some(sheet) = meta.sheet(s) else {
        b.note(key::PLAYER_META_READING);
        return;
    };
    unread(sheet, b);
    // ADD A TAG, first — reached without scrolling past every tag — into
    // the file's main tag, named on the row; where the file takes none, the
    // row says why instead.
    if let Some(adding) = &sheet.adding {
        match (&adding.origin, &adding.can) {
            (Some(origin), Can::Yes(_)) => b.pair_whole(
                b.t(key::PLAYER_TAG_ADD),
                origin_title(origin, b.texts),
                Target::AddTag,
                "tag-add",
            ),
            (_, Can::No(why)) => b.pair_whole(
                b.t(key::PLAYER_TAG_ADD),
                crate::player::tags_refusal(why, b.texts),
                Target::Nothing,
                "tag-add",
            ),
            (None, Can::Yes(_)) => {}
        }
    }
    if sheet.other.is_empty() {
        b.note(key::PLAYER_TAGS_NONE);
    }
    // ONE REASON FOR THE WHOLE PAGE is said once; one for a whole group,
    // once under its title.
    // (Not twice: the "Add a tag" row may have said it already.)
    let page_why = one_reason(sheet.other.iter());
    let add_said = sheet.adding.as_ref().and_then(|a| match &a.can {
        Can::No(why) => Some(why),
        Can::Yes(_) => None,
    });
    if let Some(why) = page_why
        && add_said != Some(why)
    {
        note_said(&crate::player::tags_refusal(why, b.texts), b);
    }
    let mut groups: Vec<(&Origin, Vec<&Tag>)> = Vec::new();
    for tag in &sheet.other {
        match groups.iter_mut().find(|(o, _)| *o == &tag.origin) {
            Some((_, tags)) => tags.push(tag),
            None => groups.push((&tag.origin, vec![tag])),
        }
    }
    for (g, (origin, tags)) in groups.into_iter().enumerate() {
        b.heading_within(origin_title(origin, b.texts));
        if page_why.is_none()
            && let Some(why) = one_reason(tags.iter().copied())
        {
            note_said(&crate::player::tags_refusal(why, b.texts), b);
        }
        for (i, tag) in tags.into_iter().enumerate() {
            let target = if tag.can.can() {
                Target::OtherTag(tag.origin.clone(), tag.name.clone())
            } else {
                Target::OtherTagFixed(tag.origin.clone(), tag.name.clone())
            };
            b.pair_whole(
                tag.name.clone(),
                tag_value(&tag.value, b.texts),
                target,
                &format!("tag-{g}-{i}"),
            );
        }
    }
}

/// The one reason none of `tags` can be written, when one holds for them
/// all — said once, and not left for `RET` on each.
fn one_reason<'a>(mut tags: impl Iterator<Item = &'a Tag>) -> Option<&'a Why> {
    let why = |t: &'a Tag| match &t.can {
        Can::No(why) => Some(why),
        Can::Yes(_) => None,
    };
    let first = why(tags.next()?)?;
    tags.all(|t| why(t) == Some(first)).then_some(first)
}

/// The rows of tag `name` read from `origin` — several when the tag holds
/// several values (a Vorbis comment may say ARTIST twice).
fn rows_of<'a>(sheet: &'a Sheet, origin: &Origin, name: &str) -> Vec<&'a Tag> {
    sheet
        .other
        .iter()
        .filter(|t| &t.origin == origin && t.name == name)
        .collect()
}

/// The question a tag's new value is asked with: holding what it says when
/// the line can hold it all — one line, whole, the name's only value — else
/// empty, with a question that says what is typed replaces all of it.
fn value_question(rows: &[&Tag]) -> Question {
    use crate::player_question::{TAG_VALUE, TAG_VALUE_ALL};
    match rows {
        [one] => match &one.value {
            TagValue::Text(text) if text.trim().is_empty() => Question::plain(TAG_VALUE, false),
            TagValue::Text(text) if !text.contains('\n') => {
                Question::holding(TAG_VALUE, text.clone())
            }
            _ => Question::plain(TAG_VALUE_ALL, false),
        },
        _ => Question::plain(TAG_VALUE_ALL, false),
    }
}

/// Why a name typed for a new tag does not fit where it goes: the sentence
/// quotes it, names the tag and says the tag's rule.
fn bad_name(typed: &str, origin: &Origin, texts: &ronoctua_i18n::Texts) -> String {
    let rule = match origin {
        Origin::Lofty {
            format: TagFormat::Vorbis,
        } => key::PLAYER_TAG_NAME_RULE_VORBIS,
        Origin::Lofty {
            format: TagFormat::Ape,
        } => key::PLAYER_TAG_NAME_RULE_APE,
        Origin::Lofty {
            format: TagFormat::RiffInfo,
        } => key::PLAYER_TAG_NAME_RULE_RIFF,
        Origin::Lofty {
            format: TagFormat::AiffText,
        } => key::PLAYER_TAG_NAME_RULE_AIFF,
        Origin::Lofty {
            format: TagFormat::Mp4,
        } => key::PLAYER_TAG_NAME_RULE_MP4,
        Origin::Ffprobe { .. } => key::PLAYER_TAG_NAME_RULE_FFMPEG,
        _ => key::PLAYER_TAG_NAME_RULE_PLAIN,
    };
    texts.t_with(
        key::PLAYER_TAG_BAD_NAME,
        &[
            ("typed", typed.trim()),
            ("where", &origin_title(origin, texts)),
            ("rule", &texts.t(rule)),
        ],
    )
}

/// What a press on a row of the metadata does — `None` for a target that is
/// none of its. The view keeps the question it opened ([`Typed`]).
#[must_use]
pub fn turn(
    meta: &mut Meta,
    target: &Target,
    press: crate::player::Press,
    s: &State,
    texts: &ronoctua_i18n::Texts,
) -> Option<Turned> {
    use crate::player::Press;
    let forward = !matches!(press, Press::Less);
    match (target, press) {
        (Target::MetaFile, Press::Enter | Press::Less | Press::More) => {
            let n = locations(s).len().max(1);
            let now = meta.file.min(n - 1);
            meta.file = if forward {
                (now + 1) % n
            } else {
                (now + n - 1) % n
            };
            Some(Turned::Errand(Errand::Nothing))
        }
        (Target::MetaRefresh, Press::Enter) => {
            let location = meta.location(s)?;
            Some(Turned::Errand(Errand::TellSaying(
                Ask::Refresh { location },
                texts.t(key::PLAYER_META_REFRESHING),
            )))
        }
        // A SEVERAL-LINE FIELD is read whole on a page of its own, which
        // opens it in the editor — the file's to change or not.
        (Target::Tag(field) | Target::TagFixed(field), Press::Enter)
            if field.lines() == Lines::Many =>
        {
            Some(Turned::Page(PageId::Field(*field)))
        }
        (Target::Draft(field), Press::Enter) => {
            let sheet = meta.sheet(s)?;
            if let Some(Can::No(why)) = sheet.field(*field).map(|r| &r.can) {
                return Some(Turned::Errand(Errand::Said(crate::player::tags_refusal(
                    why, texts,
                ))));
            }
            Some(Turned::Errand(Errand::Tell(Ask::Draft {
                location: sheet.location.to_string_lossy().into_owned(),
                field: *field,
            })))
        }
        (Target::TagFixed(field), Press::Enter | Press::Remove) => {
            let why = meta
                .sheet(s)
                .and_then(|sh| sh.field(*field))
                .and_then(|r| match &r.can {
                    Can::No(why) => Some(why.clone()),
                    Can::Yes(_) => None,
                })?;
            Some(Turned::Errand(Errand::Said(crate::player::tags_refusal(
                &why, texts,
            ))))
        }
        (Target::Tag(field), Press::Enter) => {
            let sheet = meta.sheet(s)?;
            let location = sheet.location.to_string_lossy().into_owned();
            let was = sheet.value(*field).unwrap_or_default().to_owned();
            let id = question_of(*field)?;
            let question = if was.is_empty() {
                Question::plain(id, false)
            } else {
                Question::holding(id, was.clone())
            };
            Some(Turned::Asked(
                crate::player::Typing::Meta(Typed::Tag {
                    location,
                    stamp: sheet.stamp,
                    field: *field,
                    was,
                }),
                question,
            ))
        }
        // A DRAFT OPEN IN THE EDITOR for the field: `d` throws it away,
        // after a question — "no" first — and the file stays as it is.
        (Target::Tag(field), Press::Remove)
            if let Some(d) = s
                .drafts
                .iter()
                .find(|d| d.field == *field && draft_is_of(d, meta, s)) =>
        {
            let field_word = texts.t(field_key(*field));
            Some(Turned::Question(
                texts.t_with(key::PLAYER_ASK_DROP_DRAFT, &[("field", &field_word)]),
                Errand::TellSaying(
                    Ask::DropDraft { id: d.id },
                    texts.t_with(key::PLAYER_DRAFT_DROPPED, &[("field", &field_word)]),
                ),
            ))
        }
        // CLEARED, after a question — "no" first. A blank field is empty
        // already, and says so.
        (Target::Tag(field), Press::Remove) => {
            let sheet = meta.sheet(s)?;
            if sheet.value(*field).is_none_or(|v| v.trim().is_empty()) {
                return Some(Turned::Errand(Errand::Said(texts.t_with(
                    key::PLAYER_TAG_ALREADY_EMPTY,
                    &[("field", &texts.t(field_key(*field)))],
                ))));
            }
            let location = sheet.location.to_string_lossy().into_owned();
            let name = named(&location, texts);
            let field_word = texts.t(field_key(*field));
            Some(Turned::Question(
                texts.t_with(
                    key::PLAYER_ASK_CLEAR_TAG,
                    &[("field", &field_word), ("name", &name)],
                ),
                Errand::TellSaying(
                    Ask::Retag {
                        location,
                        stamp: sheet.stamp,
                        edits: vec![Edit::Clear { field: *field }],
                    },
                    texts.t_with(
                        key::PLAYER_TAG_CLEARING,
                        &[("field", &field_word), ("name", &name)],
                    ),
                ),
            ))
        }
        // A TAG BY ITS OWN NAME the file cannot take: why.
        (Target::OtherTagFixed(origin, name), Press::Enter | Press::Remove) => {
            let sheet = meta.sheet(s)?;
            let why = rows_of(sheet, origin, name)
                .into_iter()
                .find_map(|t| match &t.can {
                    Can::No(why) => Some(why.clone()),
                    Can::Yes(_) => None,
                })?;
            Some(Turned::Errand(Errand::Said(crate::player::tags_refusal(
                &why, texts,
            ))))
        }
        // ONE IT CAN: its value asked on the door's line.
        (Target::OtherTag(origin, name), Press::Enter) => {
            let sheet = meta.sheet(s)?;
            let rows = rows_of(sheet, origin, name);
            let question = value_question(&rows);
            let was = question.initial.clone().unwrap_or_default();
            Some(Turned::Asked(
                crate::player::Typing::Meta(Typed::NamedTag {
                    location: sheet.location.to_string_lossy().into_owned(),
                    stamp: sheet.stamp,
                    origin: origin.clone(),
                    name: name.clone(),
                    was,
                }),
                question,
            ))
        }
        // TAKEN OUT — every value it has — after a question, "no" first.
        (Target::OtherTag(origin, name), Press::Remove) => {
            let sheet = meta.sheet(s)?;
            let values = rows_of(sheet, origin, name).len().max(1);
            let location = sheet.location.to_string_lossy().into_owned();
            let file = named(&location, texts);
            Some(Turned::Question(
                texts.t_args(
                    key::PLAYER_ASK_REMOVE_TAG,
                    &[
                        ("tag", Arg::Text(name)),
                        ("name", Arg::Text(&file)),
                        (
                            "values",
                            Arg::Number(i64::try_from(values).unwrap_or(i64::MAX)),
                        ),
                    ],
                ),
                Errand::TellSaying(
                    Ask::Retag {
                        location,
                        stamp: sheet.stamp,
                        edits: vec![Edit::RemoveTag {
                            origin: origin.clone(),
                            name: name.clone(),
                        }],
                    },
                    texts.t_with(
                        key::PLAYER_TAG_NAMED_REMOVING,
                        &[("tag", name.as_str()), ("name", &file)],
                    ),
                ),
            ))
        }
        // ADD A TAG: its name first, into the file's main tag — or why the
        // file takes none.
        (Target::AddTag, Press::Enter) => {
            let sheet = meta.sheet(s)?;
            let adding = sheet.adding.as_ref()?;
            match (&adding.origin, &adding.can) {
                (_, Can::No(why)) => Some(Turned::Errand(Errand::Said(
                    crate::player::tags_refusal(why, texts),
                ))),
                (Some(origin), Can::Yes(_)) => Some(Turned::Asked(
                    crate::player::Typing::Meta(Typed::NewTagName {
                        location: sheet.location.to_string_lossy().into_owned(),
                        stamp: sheet.stamp,
                        origin: origin.clone(),
                        taken: sheet
                            .other
                            .iter()
                            .filter(|t| &t.origin == origin)
                            .map(|t| t.name.clone())
                            .collect(),
                    }),
                    Question::plain(crate::player_question::TAG_NAME, false),
                )),
                (None, Can::Yes(_)) => None,
            }
        }
        (Target::CoverSave, Press::Enter) => {
            let sheet = meta.sheet(s)?;
            let location = sheet.location.to_string_lossy().into_owned();
            let name = named(&location, texts);
            Some(Turned::Errand(Errand::TellSaying(
                Ask::SaveCover { location, at: None },
                texts.t_with(key::PLAYER_COVER_SAVING, &[("name", &name)]),
            )))
        }
        (Target::CoverRemove, Press::Enter) => {
            let sheet = meta.sheet(s)?;
            let location = sheet.location.to_string_lossy().into_owned();
            let name = named(&location, texts);
            Some(Turned::Question(
                texts.t_with(key::PLAYER_ASK_REMOVE_COVER, &[("name", &name)]),
                Errand::TellSaying(
                    Ask::Retag {
                        location,
                        stamp: sheet.stamp,
                        edits: vec![Edit::Cover(CoverEdit::Remove)],
                    },
                    texts.t_with(key::PLAYER_COVER_REMOVING, &[("name", &name)]),
                ),
            ))
        }
        _ => None,
    }
}

/// What a press on the metadata turns into.
#[derive(Debug, Clone, PartialEq)]
pub enum Turned {
    /// An errand, for the door.
    Errand(Errand),
    /// A question on the door's line, and what its answer is about.
    Asked(crate::player::Typing, Question),
    /// A yes-or-no question on the page — its sentence and what yes does;
    /// no does nothing.
    Question(String, Errand),
    /// A page, over this one.
    Page(PageId),
}

/// A question of the metadata's being typed, and what its answer is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    /// A new value for `field` of the file at `location`, whose sheet was
    /// read with `stamp`; `was` is what it said when the question opened.
    Tag {
        /// The file.
        location: String,
        /// The file as its sheet was read: a file changed since is refused.
        stamp: ronoctua_tags::Stamp,
        /// The field.
        field: Field,
        /// Its value when asked.
        was: String,
    },
    /// A new value for the tag `name` read from `origin`; `was` is what the
    /// line held when it opened (nothing, when it could not hold it all).
    NamedTag {
        /// The file.
        location: String,
        /// The file as its sheet was read.
        stamp: ronoctua_tags::Stamp,
        /// Who read it: where it is written back.
        origin: Origin,
        /// Its own name.
        name: String,
        /// What the line held.
        was: String,
    },
    /// The name of a tag to add into `origin`, the file's main tag; `taken`
    /// are the names already there — typing one of them replaces it, and
    /// the value's question says so.
    NewTagName {
        /// The file.
        location: String,
        /// The file as its sheet was read.
        stamp: ronoctua_tags::Stamp,
        /// Where it goes.
        origin: Origin,
        /// The names that tag has now.
        taken: Vec<String>,
    },
    /// The value of the tag `name` being added, the name typed first.
    NewTagValue {
        /// The file.
        location: String,
        /// The file as its sheet was read.
        stamp: ronoctua_tags::Stamp,
        /// Where it goes.
        origin: Origin,
        /// Its name, the format's own (`tag_name`).
        name: String,
    },
}

/// What was typed on one of the metadata's questions.
///
/// For a field: nothing or what it already said is never mind; a value its
/// field refuses — a number, a date, an ISRC — is refused in a sentence
/// here, before anything is sent; anything else is written. For a tag by its
/// own name the same, its value one line (`tag_text`). A new tag's NAME is
/// checked by its tag's grammar (`tag_name`) — refused in a sentence that
/// says the rule — and asks its value next.
#[must_use]
pub fn typed(asked: Typed, said: &str, texts: &ronoctua_i18n::Texts) -> Turned {
    let said = said.trim();
    let writing = |location: String, stamp, edit: Edit, what: &str| {
        let name = named(&location, texts);
        let sentence = match &edit {
            Edit::Set { .. } => {
                texts.t_with(key::PLAYER_TAG_WRITING, &[("field", what), ("name", &name)])
            }
            _ => texts.t_with(
                key::PLAYER_TAG_NAMED_WRITING,
                &[("tag", what), ("name", &name)],
            ),
        };
        Turned::Errand(Errand::TellSaying(
            Ask::Retag {
                location,
                stamp,
                edits: vec![edit],
            },
            sentence,
        ))
    };
    let refused = |why: Why| Turned::Errand(Errand::Said(crate::player::tags_refusal(&why, texts)));
    match asked {
        Typed::Tag {
            location,
            stamp,
            field,
            was,
        } => {
            if said.is_empty() || said == was.trim() {
                return Turned::Errand(Errand::Nothing);
            }
            match field.check(said) {
                Ok(value) => writing(
                    location,
                    stamp,
                    Edit::Set { field, value },
                    &texts.t(field_key(field)),
                ),
                Err(why) => refused(why),
            }
        }
        Typed::NamedTag {
            location,
            stamp,
            origin,
            name,
            was,
        } => {
            if said.is_empty() || said == was.trim() {
                return Turned::Errand(Errand::Nothing);
            }
            match ronoctua_tags::tag_text(said) {
                Ok(value) => {
                    let what = name.clone();
                    writing(
                        location,
                        stamp,
                        Edit::SetTag {
                            origin,
                            name,
                            value,
                        },
                        &what,
                    )
                }
                Err(why) => refused(why),
            }
        }
        Typed::NewTagName {
            location,
            stamp,
            origin,
            taken,
        } => {
            if said.is_empty() {
                return Turned::Errand(Errand::Nothing);
            }
            match ronoctua_tags::tag_name(&origin, said) {
                Ok(name) => {
                    // A NAME ALREADY THERE is written whole: the question
                    // says what is typed replaces all of it.
                    let there = taken.iter().any(|t| t.eq_ignore_ascii_case(&name));
                    let id = if there {
                        crate::player_question::TAG_VALUE_ALL
                    } else {
                        crate::player_question::TAG_VALUE
                    };
                    Turned::Asked(
                        crate::player::Typing::Meta(Typed::NewTagValue {
                            location,
                            stamp,
                            origin,
                            name,
                        }),
                        Question::plain(id, false),
                    )
                }
                Err(Why::BadName) => Turned::Errand(Errand::Said(bad_name(said, &origin, texts))),
                Err(why) => refused(why),
            }
        }
        Typed::NewTagValue {
            location,
            stamp,
            origin,
            name,
        } => {
            if said.is_empty() {
                return Turned::Errand(Errand::Nothing);
            }
            match ronoctua_tags::tag_text(said) {
                Ok(value) => {
                    let what = name.clone();
                    writing(
                        location,
                        stamp,
                        Edit::SetTag {
                            origin,
                            name,
                            value,
                        },
                        &what,
                    )
                }
                Err(why) => refused(why),
            }
        }
    }
}

/// A picture picked (or typed) for the cover of the file shown: put in, in
/// place of its front cover. `None` with no sheet to put it in.
#[must_use]
pub fn cover_picked(
    meta: &Meta,
    s: &State,
    picture: &Path,
    texts: &ronoctua_i18n::Texts,
) -> Option<Errand> {
    let sheet = meta.sheet(s)?;
    if let Can::No(why) = &sheet.cover_can {
        return Some(Errand::Said(crate::player::tags_refusal(why, texts)));
    }
    let location = sheet.location.to_string_lossy().into_owned();
    Some(Errand::TellSaying(
        Ask::Retag {
            location: location.clone(),
            stamp: sheet.stamp,
            edits: vec![Edit::Cover(CoverEdit::Replace {
                from: PathBuf::from(picture),
            })],
        },
        texts.t_with(
            key::PLAYER_COVER_PUTTING,
            &[
                ("picture", &named(&picture.to_string_lossy(), texts)),
                ("name", &named(&location, texts)),
            ],
        ),
    ))
}

/// The question a saved draft puts: its sentence, what yes and no ask, the
/// draft it is about and what the editor saved.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftQuestion {
    /// The draft.
    pub id: DraftId,
    /// What is asked.
    pub sentence: String,
    /// Written into the file.
    pub yes: Errand,
    /// NOT NOW — and `Esc`: the draft stays open, and is asked about again
    /// when it is saved again (`Ask::KeepDraft`). Saving part of a text while
    /// still writing it is how an editor is used; throwing the draft away is
    /// `d` on its field, after a question of its own.
    pub no: Errand,
    /// What it would write, as the editor saved it — its lines, empty when
    /// the draft cannot be read here.
    pub quoted: Vec<String>,
}

/// The question a saved draft puts, in ANY door — "no" drawn first.
///
/// WHAT IT WOULD WRITE is shown with it: read from the draft, a file in the
/// session's runtime directory (memory) at most `ronoctua_tags`'
/// `VALUE_MOST_BYTES` long — a person says yes to words they read.
#[must_use]
pub fn draft_question(
    d: &ronoctua_player::wire::Drafted,
    texts: &ronoctua_i18n::Texts,
) -> DraftQuestion {
    let field = texts.t(field_key(d.field));
    let name = named(&d.location, texts);
    DraftQuestion {
        id: d.id,
        sentence: texts.t_with(
            key::PLAYER_ASK_WRITE_DRAFT,
            &[("field", &field), ("name", &name)],
        ),
        yes: Errand::TellSaying(
            Ask::WriteDraft { id: d.id },
            texts.t_with(
                key::PLAYER_TAG_WRITING,
                &[("field", &field), ("name", &name)],
            ),
        ),
        no: Errand::TellSaying(
            Ask::KeepDraft { id: d.id },
            texts.t_with(key::PLAYER_DRAFT_KEPT, &[("field", &field)]),
        ),
        quoted: draft_lines(Path::new(&d.path)),
    }
}

/// Whether draft `d` is of the file the section shows — named as the file's
/// sheet names it (the daemon's own path), or as the ring does.
fn draft_is_of(d: &ronoctua_player::wire::Drafted, meta: &Meta, s: &State) -> bool {
    let by_sheet = meta
        .sheet(s)
        .is_some_and(|sh| Path::new(&d.location) == sh.location.as_path());
    by_sheet
        || meta
            .location(s)
            .is_some_and(|here| Path::new(&d.location) == Path::new(&here))
}

/// A draft's words, a line each — none when it cannot be read.
fn draft_lines(path: &Path) -> Vec<String> {
    use std::io::Read as _;
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    if file
        .take(ronoctua_tags::VALUE_MOST_BYTES as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<String> = text.trim_end().lines().map(str::to_owned).collect();
    lines
}

/// Where a cover is drawn among `lines` rows of `cells` cells from its box's
/// top-left corner, in its own proportion, with `space.files.pictureAir` of
/// air above and below it inside the box — a picture never touches the row
/// over it or under it — and the part of it on screen when the rows are cut:
/// `visible` is the box's lines on screen, `(first, end)`. Answers the drawn
/// rectangle (logical pixels, from the box's corner, cut to what is visible)
/// and the picture's rows drawn there, as fractions of its height
/// `(top, bottom)`. `None` when nothing of it shows.
#[must_use]
pub fn cover_place(
    (cells, lines): (u16, u16),
    (cell_px, line_px): (f32, f32),
    (width, height): (u32, u32),
    (first, end): (u16, u16),
) -> Option<(ronoctua_render::image::Area, (f32, f32))> {
    #[allow(clippy::cast_precision_loss)]
    let aspect = width as f32 / (height.max(1) as f32);
    let air = f32::from(tokens::space::FILES_PICTUREAIR);
    let (box_w, box_h) = (
        f32::from(cells) * cell_px,
        (f32::from(lines) * line_px - air - air).max(0.0),
    );
    let (mut w, mut h) = (box_h * aspect, box_h);
    if w > box_w {
        (w, h) = (box_w, box_w / aspect.max(f32::MIN_POSITIVE));
    }
    let (top, bottom) = (
        f32::from(first) * line_px,
        f32::from(end.min(lines)) * line_px,
    );
    let (from, to) = (top.max(air), bottom.min(air + h));
    if to <= from || w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some((
        ronoctua_render::image::Area {
            x: 0.0,
            y: from,
            width: w,
            height: to - from,
        },
        ((from - air) / h, (to - air) / h),
    ))
}

/// `rgba`'s rows from `top` to `bottom` (fractions of its height) — the part
/// of a cover on screen when its rows are cut by the scroll.
#[must_use]
pub fn cropped(rgba: &Rgba, (top, bottom): (f32, f32)) -> Option<Rgba> {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let row = |f: f32| ((f.clamp(0.0, 1.0) * rgba.height as f32).round() as u32).min(rgba.height);
    let (from, to) = (row(top), row(bottom));
    if to <= from {
        return None;
    }
    let stride = rgba.width as usize * 4;
    let pixels = rgba.pixels[from as usize * stride..to as usize * stride].to_vec();
    Rgba::new(rgba.width, to - from, pixels)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::player::fixture::{alone, track};
    use crate::player::{Press, View};
    use ronoctua_player::wire::{Drafted, Retagging, TrackKind as Kind};
    use ronoctua_tags::{
        Container, Cover, CoverAt, CoverRole, FieldRow, Kind as SheetKind, Stamp, Stream, Writer,
    };

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect(language)
    }

    /// A SONG as the daemon says it: its common fields, blanks included, a
    /// cover, three technical facts, a stream, and exiftool's tags.
    pub(crate) fn song_sheet(location: &str) -> Sheet {
        let yes = Can::Yes(Writer::Lofty);
        let row = |field: Field, value: Option<&str>| FieldRow {
            field,
            value: value.map(ToOwned::to_owned),
            can: yes.clone(),
        };
        Sheet {
            location: PathBuf::from(location),
            stamp: Stamp {
                len: 100,
                mtime_ns: 2,
                ino: 3,
            },
            kind: SheetKind::Music,
            container: Container::Flac,
            fields: Field::common(SheetKind::Music)
                .iter()
                .map(|f| match f {
                    Field::Title => row(*f, Some("Harbour Lights")),
                    Field::Artist => row(*f, Some("Marta Quintela")),
                    Field::Date => row(*f, Some("2024-05")),
                    Field::Lyrics => row(*f, Some("First line\nSecond line\nThird")),
                    _ => row(*f, None),
                })
                .collect(),
            covers: vec![Cover {
                role: CoverRole::Front,
                mime: "image/png".into(),
                width: Some(600),
                height: Some(600),
                bytes: Some(12_000),
                at: CoverAt::Tag {
                    format: TagFormat::Vorbis,
                    index: 0,
                },
                can: yes.clone(),
            }],
            cover_can: yes,
            technical: vec![
                Fact {
                    key: TechKey::DurationMs,
                    value: "215000".into(),
                },
                Fact {
                    key: TechKey::SampleRateHz,
                    value: "44100".into(),
                },
                Fact {
                    key: TechKey::AudioCodec,
                    value: "flac".into(),
                },
            ],
            streams: vec![Stream {
                index: 0,
                kind: StreamKind::Audio,
                codec: Some("flac".into()),
                lang: None,
                title: None,
                default: true,
                forced: false,
                hearing: false,
                attached: false,
                can: Can::No(Why::NoWriter {
                    container: Container::Flac,
                }),
            }],
            chapters: Vec::new(),
            other: vec![Tag {
                origin: Origin::Exiftool {
                    group: "Vorbis".into(),
                },
                name: "Vendor".into(),
                value: TagValue::Text("reference libFLAC 1.4.3".into()),
                can: Can::No(Why::ReadOnlyView),
            }],
            // A FLAC IS NEVER READ BY MATROSKA'S TOOLS (`read::sheet` asks
            // them only for a Matroska container): its sheet says nothing of
            // them. An earlier fixture here — and the photographs' — said
            // "mkvextract is not installed" under a FLAC, which no daemon
            // answers.
            unread: Vec::new(),
            cover_file: None,
            adding: Some(ronoctua_tags::Adding {
                origin: Some(Origin::Lofty {
                    format: TagFormat::Vorbis,
                }),
                can: Can::Yes(Writer::Lofty),
            }),
        }
    }

    /// The song's sheet with its Every-tag page full: the Vorbis comments a
    /// file can take (one ARTIST said twice, one binary), a Matroska-free
    /// FLAC's exiftool rows that mirror them.
    pub(crate) fn tagged_sheet(location: &str) -> Sheet {
        let mut sheet = song_sheet(location);
        let vorbis = Origin::Lofty {
            format: TagFormat::Vorbis,
        };
        let row = |name: &str, value: TagValue, can: Can| Tag {
            origin: vorbis.clone(),
            name: name.into(),
            value,
            can,
        };
        let yes = || Can::Yes(Writer::Lofty);
        let text = |v: &str| TagValue::Text(v.into());
        sheet.other = vec![
            row("TITLE", text("Harbour Lights"), yes()),
            row("ARTIST", text("Marta Quintela"), yes()),
            row("ARTIST", text("Rui Faial"), yes()),
            row("LYRICS", text("First line\nSecond line"), yes()),
            row(
                "REPLAYGAIN_TRACK_GAIN",
                TagValue::Cut {
                    head: "-6.1 dB".into(),
                    bytes: 9000,
                },
                yes(),
            ),
            row(
                "FINGERPRINT",
                TagValue::Binary { bytes: 64 },
                Can::No(Why::NotText),
            ),
            Tag {
                origin: Origin::Exiftool {
                    group: "Vorbis".into(),
                },
                name: "Vendor".into(),
                value: text("reference libFLAC 1.4.3"),
                can: Can::No(Why::ReadOnlyView),
            },
            Tag {
                origin: Origin::Exiftool {
                    group: "Vorbis".into(),
                },
                name: "Title".into(),
                value: text("Harbour Lights"),
                can: Can::No(Why::ReadOnlyView),
            },
        ];
        sheet
    }

    fn a_song() -> State {
        alone(
            "/music/harbour.flac",
            vec![track(1, Kind::Audio, None, true)],
        )
    }

    fn held(v: &mut View, s: &State, sheet: Sheet) {
        let location = super::locations(s)[0].clone();
        v.meta
            .given(&location, s.sheet_generation, Got::Sheet(Box::new(sheet)));
    }

    fn row_of<'a>(
        rows: &'a [crate::player_rows::Row],
        target: &Target,
    ) -> Option<(usize, &'a crate::player_rows::Row)> {
        rows.iter().enumerate().find(|(_, r)| &r.target == target)
    }

    /// FAILS IF: the Metadata section stops being the home's LAST, stops
    /// drawing every common field — a blank one saying so in the answer's
    /// register and still acting — or a field's row stops asking its value
    /// typed, holding what it says, or a several-line field stops opening the
    /// editor through the daemon's draft.
    #[test]
    fn every_common_field_is_a_row_and_a_blank_one_acts() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let s = a_song();
            let mut v = View::blank(None);
            held(&mut v, &s, song_sheet("/music/harbour.flac"));
            let rows = v.rows(Some(&s), 80, &t);
            let title = |k: &str| {
                rows.iter()
                    .position(|r| r.heading && r.fields.iter().any(|f| f.text == t.t(k)))
                    .unwrap_or_else(|| panic!("{language}: no title {k}"))
            };
            assert!(
                title(key::PLAYER_HEADING_METADATA) > title(key::PLAYER_HEADING_FEATURES),
                "{language}: the Metadata is not the home's last section"
            );
            for f in Field::common(SheetKind::Music) {
                let (_, r) = row_of(&rows, &Target::Tag(*f))
                    .unwrap_or_else(|| panic!("{language}: no row for {f:?}"));
                assert_eq!(r.fields[0].text, t.t(field_key(*f)), "{language}: {f:?}");
            }
            let (at, blank) = row_of(&rows, &Target::Tag(Field::Composer)).expect("composer");
            assert_eq!(
                blank.fields.last().map(|f| f.text.clone()),
                Some(t.t(key::PLAYER_TAG_EMPTY))
            );
            assert_eq!(
                blank.fields.last().map(|f| f.color),
                Some(crate::chrome::Kind::Setting.tones(false).1),
                "{language}: a blank is not in the answer's register"
            );
            v.clicked(at, &rows);
            v.clicked(at, &rows);
            assert_eq!(
                v.key(Press::Enter, Some(&s), 80, &t),
                crate::player::Errand::Ask(Question::plain("player-tag-composer", false)),
                "{language}: a blank asks with nothing held"
            );
            v.let_go();
            let rows = v.rows(Some(&s), 80, &t);
            let (at, _) = row_of(&rows, &Target::Tag(Field::Title)).expect("title");
            v.grabbed(at);
            assert_eq!(
                v.key(Press::Enter, Some(&s), 80, &t),
                crate::player::Errand::Ask(Question::holding(
                    "player-tag-title",
                    "Harbour Lights".into()
                ))
            );
            v.let_go();
            let rows = v.rows(Some(&s), 80, &t);
            let (at, lyrics) = row_of(&rows, &Target::Tag(Field::Lyrics)).expect("lyrics");
            assert!(
                lyrics
                    .fields
                    .last()
                    .is_some_and(|f| f.text.contains("First line")),
                "{:?}",
                lyrics.fields
            );
            // SEVERAL LINES: read whole on a page of their own, which opens
            // them in the editor through the daemon's draft.
            v.grabbed(at);
            assert_eq!(
                v.key(Press::Enter, Some(&s), 80, &t),
                crate::player::Errand::Nothing
            );
            assert_eq!(v.page(), PageId::Field(Field::Lyrics), "{language}");
            let rows = v.rows(Some(&s), 30, &t);
            let words: Vec<String> = rows
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect();
            for line in ["First line", "Second line", "Third"] {
                assert!(
                    words.iter().any(|w| w == line),
                    "{language}: {line} is not read on the field's page: {words:?}"
                );
            }
            let (at, _) = row_of(&rows, &Target::Draft(Field::Lyrics)).expect("edit");
            v.grabbed(at);
            assert_eq!(
                v.key(Press::Enter, Some(&s), 30, &t),
                crate::player::Errand::Tell(Ask::Draft {
                    location: "/music/harbour.flac".into(),
                    field: Field::Lyrics
                })
            );
            // BACK on the field's row.
            v.key(Press::Escape, Some(&s), 80, &t);
            let rows = v.rows(Some(&s), 80, &t);
            assert_eq!(
                rows[v.cursor()].target,
                Target::Tag(Field::Lyrics),
                "{language}"
            );
        }
    }

    /// FAILS IF: a sheet that did not come in time — a file on a slow mount
    /// — replaces a good older one and says "not running", or is held as
    /// answered and never asked again; or with no older one the section does
    /// not say it is still reading; or the player gone leaves a sheet held.
    #[test]
    fn a_sheet_that_did_not_come_keeps_the_older_and_is_asked_again() {
        let t = texts("en-GB");
        let s = a_song();
        let location = "/music/harbour.flac";
        let mut m = Meta::default();
        m.given(location, 1, Got::Sheet(Box::new(song_sheet(location))));
        let late = |m: &mut Meta, got: Got| {
            let slot: Slot = Arc::default();
            m.coming = Some((location.into(), 2, slot.clone()));
            *slot.lock().expect("a slot") = Some(Held {
                location: location.into(),
                generation: 2,
                got,
                cover: None,
            });
            m.look(Some(&s), None);
        };
        late(&mut m, Got::Slow);
        assert!(
            matches!(m.held.as_ref().map(|h| &h.got), Some(Got::Sheet(_))),
            "the older sheet was dropped"
        );
        assert_eq!(m.failed, Some((location.into(), 2, 1)), "not counted");
        late(&mut m, Got::NotRunning);
        assert_eq!(m.failed, Some((location.into(), 2, 2)));
        // NONE OLDER: it says so, and the file can be read again.
        let mut v = View::blank(None);
        v.meta.given(location, 2, Got::Slow);
        let rows = v.rows(Some(&s), 80, &t);
        let text: Vec<String> = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        let still = t.t(key::PLAYER_META_STILL_READING);
        assert!(
            crate::chrome::note_lines(&still, 200)
                .iter()
                .all(|l| text.join(" ").contains(l.as_str())),
            "{text:?}"
        );
        assert!(row_of(&rows, &Target::MetaRefresh).is_some());
        // THE PLAYER GONE: nothing of it held.
        m.forget();
        assert!(m.held.is_none() && m.coming.is_none() && m.failed.is_none());
    }

    /// FAILS IF: a field's value is cut on the Metadata section — "see all
    /// the metadata" — in a door as narrow as the small column.
    #[test]
    fn a_value_is_read_whole_in_a_narrow_door() {
        let t = texts("pt-BR");
        let s = a_song();
        let mut v = View::blank(None);
        let mut sheet = song_sheet("/music/harbour.flac");
        let long = "The Lighthouse Keeper's Very Long Song Title";
        for r in &mut sheet.fields {
            if r.field == Field::Title {
                r.value = Some(long.into());
            }
        }
        held(&mut v, &s, sheet);
        let rows = v.rows(Some(&s), 30, &t);
        let (at, _) = row_of(&rows, &Target::Tag(Field::Title)).expect("title");
        let said: String = rows[at..]
            .iter()
            .take_while(|r| r.target == Target::Tag(Field::Title))
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!said.contains('…'), "the title is cut: {said}");
        for word in long.split(' ') {
            assert!(said.contains(word), "{word} is lost: {said}");
        }
    }

    /// FAILS IF: a value typed for a field is sent unchecked — a date that is
    /// none, a track that is no number — instead of refused in a sentence
    /// here; or nothing typed, or what it already said, asks the player
    /// anything; or `d` clears a field without a question, "no" first.
    #[test]
    fn a_value_is_checked_before_it_is_written_and_a_clear_is_asked() {
        let t = texts("en-GB");
        let s = a_song();
        let mut v = View::blank(None);
        let mut pairing = crate::player_machines::Pairing::default();
        held(&mut v, &s, song_sheet("/music/harbour.flac"));
        let mut answer = |v: &mut View, field: Field, said: &str| {
            let rows = v.rows(Some(&s), 80, &t);
            let (at, _) = row_of(&rows, &Target::Tag(field)).expect("the field");
            v.grabbed(at);
            let crate::player::Errand::Ask(q) = v.key(Press::Enter, Some(&s), 80, &t) else {
                panic!("{field:?} asked nothing");
            };
            v.answered(q.id, said, &mut pairing, Some(&s), 80, &t)
        };
        assert_eq!(
            answer(&mut v, Field::Date, "31 May"),
            crate::player::Errand::Said(t.t(key::PLAYER_TROUBLE_TAG_BAD_DATE))
        );
        assert_eq!(
            answer(&mut v, Field::Track, "three"),
            crate::player::Errand::Said(t.t(key::PLAYER_TROUBLE_TAG_BAD_NUMBER))
        );
        assert_eq!(
            answer(&mut v, Field::Title, "  "),
            crate::player::Errand::Nothing
        );
        assert_eq!(
            answer(&mut v, Field::Title, "Harbour Lights"),
            crate::player::Errand::Nothing
        );
        match answer(&mut v, Field::Track, "3 / 12") {
            crate::player::Errand::TellSaying(
                Ask::Retag {
                    location, edits, ..
                },
                _,
            ) => {
                assert_eq!(location, "/music/harbour.flac");
                assert_eq!(
                    edits,
                    vec![Edit::Set {
                        field: Field::Track,
                        value: "3/12".into()
                    }]
                );
            }
            other => panic!("a track typed asked {other:?}"),
        }
        // `d`: a question, born on its "no".
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Tag(Field::Artist)).expect("artist");
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            crate::player::Errand::Nothing
        );
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Target::Answer(false),
            "born on its no"
        );
        v.key(Press::Down, Some(&s), 80, &t);
        match v.key(Press::Enter, Some(&s), 80, &t) {
            crate::player::Errand::TellSaying(Ask::Retag { edits, .. }, _) => {
                assert_eq!(
                    edits,
                    vec![Edit::Clear {
                        field: Field::Artist
                    }]
                );
            }
            other => panic!("yes asked {other:?}"),
        }
        // A BLANK has nothing to clear, and says so — a refusal is a
        // sentence.
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Tag(Field::Genre)).expect("genre");
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            crate::player::Errand::Said(t.t_with(
                key::PLAYER_TAG_ALREADY_EMPTY,
                &[("field", &t.t(key::PLAYER_TAG_GENRE))]
            ))
        );
        let rows = v.rows(Some(&s), 80, &t);
        assert_ne!(rows[v.cursor()].target, Target::Answer(false));
    }

    /// FAILS IF: an ADDRESS's sheet — read only — draws its fields as
    /// settings, or says nothing of why they cannot change, or `RET` on one
    /// asks instead of saying why.
    #[test]
    fn a_stream_s_fields_are_facts_that_say_why() {
        let t = texts("en-GB");
        let mut s = alone(
            "https://radio.example/live",
            vec![track(1, Kind::Audio, None, true)],
        );
        s.location = Some("https://radio.example/live".into());
        let sheet = ronoctua_tags::stream::sheet(
            "https://radio.example/live",
            SheetKind::Music,
            &[("icy-title".into(), "Night Drive".into())],
            Vec::new(),
        );
        let mut v = View::blank(None);
        held(&mut v, &s, sheet);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            !rows.iter().any(|r| matches!(r.target, Target::Tag(_))),
            "a stream's field is a setting"
        );
        let said = t.t_with(
            key::PLAYER_META_FIXED,
            &[("why", &t.t(key::PLAYER_TROUBLE_TAG_NOT_A_FILE))],
        );
        let lines: Vec<String> = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            lines.join(" ").contains(&said),
            "why is not said once: {lines:?}"
        );
        let (at, title) = row_of(&rows, &Target::TagFixed(Field::Title)).expect("title");
        assert!(title.fields.iter().any(|f| f.text == "Night Drive"));
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            crate::player::Errand::Said(t.t(key::PLAYER_TROUBLE_TAG_NOT_A_FILE))
        );
    }

    /// FAILS IF: a draft saved in the editor is not asked about — in any
    /// door, "no" first, `Esc` keeping it for later and NEVER throwing it
    /// away — or is asked about twice, or a draft saved again is not asked
    /// about again; or the question does not show the words it would write,
    /// or stays on screen once another door answered it.
    #[test]
    fn a_saved_draft_is_asked_about_once_no_first() {
        let t = texts("pt-BR");
        let mut s = a_song();
        let mut v = View::blank(None);
        held(&mut v, &s, song_sheet("/music/harbour.flac"));
        let dir = std::env::temp_dir().join(format!("ronoctua-shell-draft-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let draft = dir.join("3-lyrics.txt");
        std::fs::write(&draft, "Sobre o mar\nnoite adentro\n").expect("written");
        s.drafts = vec![Drafted {
            id: DraftId(3),
            location: "/music/harbour.flac".into(),
            field: Field::Lyrics,
            path: draft.to_string_lossy().into_owned(),
            saved: false,
        }];
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let (_, lyrics) = row_of(&rows, &Target::Tag(Field::Lyrics)).expect("lyrics");
        assert_eq!(
            lyrics.fields.last().map(|f| f.text.clone()),
            Some(t.t(key::PLAYER_DRAFT_OPEN)),
            "an open draft says so on its row"
        );
        s.drafts[0].saved = true;
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Target::Answer(false),
            "born on its no"
        );
        let words: Vec<String> = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            words.iter().any(|w| w == "Sobre o mar") && words.iter().any(|w| w == "noite adentro"),
            "the words it would write are not shown: {words:?}"
        );
        assert!(
            matches!(
                v.key(Press::Escape, Some(&s), 80, &t),
                crate::player::Errand::TellSaying(Ask::KeepDraft { id: DraftId(3) }, _)
            ),
            "no is NOT NOW: the draft stays"
        );
        v.settle_in(Some(&s), 80, &t);
        assert_ne!(
            v.rows(Some(&s), 80, &t)[v.cursor()].target,
            Target::Answer(false),
            "asked twice"
        );
        // REFUSED, and saved again: asked again — yes writes it.
        s.drafts[0].saved = false;
        v.settle_in(Some(&s), 80, &t);
        s.drafts[0].saved = true;
        v.settle_in(Some(&s), 80, &t);
        v.key(Press::Down, Some(&s), 80, &t);
        match v.key(Press::Enter, Some(&s), 80, &t) {
            crate::player::Errand::TellSaying(Ask::WriteDraft { id }, _) => {
                assert_eq!(id, DraftId(3));
            }
            other => panic!("yes asked {other:?}"),
        }
        // SAVED AGAIN, and ANOTHER DOOR ANSWERED: the question here goes.
        s.drafts[0].saved = false;
        v.settle_in(Some(&s), 80, &t);
        s.drafts[0].saved = true;
        v.settle_in(Some(&s), 80, &t);
        assert_eq!(
            v.rows(Some(&s), 80, &t)[v.cursor()].target,
            Target::Answer(false)
        );
        s.drafts[0].saved = false;
        v.settle_in(Some(&s), 80, &t);
        assert!(
            !v.rows(Some(&s), 80, &t)
                .iter()
                .any(|r| r.target == Target::Answer(false)),
            "a question another door answered stays on screen"
        );
        // `d` ON THE FIELD while its draft is open throws the draft away,
        // after a question of its own — "no" first.
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Tag(Field::Lyrics)).expect("lyrics");
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            crate::player::Errand::Nothing
        );
        v.key(Press::Down, Some(&s), 80, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            crate::player::Errand::TellSaying(Ask::DropDraft { id: DraftId(3) }, _)
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: the section stops saying how a write of the file goes —
    /// its progress, the writes behind it, a refusal in its sentence — or
    /// the cover's page stops putting a picture, saving it beside and taking
    /// it out after a question.
    #[test]
    fn a_write_is_said_and_the_cover_s_page_acts() {
        let t = texts("en-GB");
        let mut s = a_song();
        let mut v = View::blank(None);
        held(&mut v, &s, song_sheet("/music/harbour.flac"));
        s.retag = Some(Retagging {
            location: "/music/harbour.flac".into(),
            status: RetagStatus::Writing { permille: 420 },
            waiting: 2,
        });
        let text = |v: &View, s: &State| -> String {
            v.rows(Some(s), 80, &t)
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let now = text(&v, &s);
        assert!(now.contains("42%"), "{now}");
        s.retag = Some(Retagging {
            location: "/music/harbour.flac".into(),
            status: RetagStatus::Refused(Trouble::Tags {
                path: "/music/harbour.flac".into(),
                why: Why::ReadOnlyFile,
            }),
            waiting: 0,
        });
        assert!(
            text(&v, &s).contains(&t.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_FILE)),
            "a refused write is not said"
        );
        v.key(Press::Go(PageId::Cover), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(row_of(&rows, &Target::Go(PageId::Files(Purpose::Cover))).is_some());
        let (at, _) = row_of(&rows, &Target::CoverSave).expect("save beside");
        v.grabbed(at);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            crate::player::Errand::TellSaying(Ask::SaveCover { at: None, .. }, _)
        ));
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::CoverRemove).expect("take it out");
        v.grabbed(at);
        v.key(Press::Enter, Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            crate::player::Errand::Nothing
        );
    }

    /// FAILS IF: `LEADER p d` stops being the metadata — the home, from any
    /// page, the cursor on the section's first row — or says nothing when
    /// nothing plays; or the file ring stops walking the files of a set.
    #[test]
    fn leader_p_d_is_the_metadata_and_the_ring_walks_the_files() {
        use ronoctua_player::own::{Lane, LaneId, LaneKind};
        let t = texts("en-GB");
        assert_eq!(
            crate::player::press_of("player-metadata"),
            Some(Press::Metadata)
        );
        let s = a_song();
        let mut v = View::blank(None);
        held(&mut v, &s, song_sheet("/music/harbour.flac"));
        v.key(Press::Go(PageId::Streams), Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Streams);
        assert_eq!(
            v.key(Press::Metadata, Some(&s), 80, &t),
            crate::player::Errand::Nothing
        );
        assert_eq!(v.page(), PageId::Home);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            rows[v.cursor()].spot.starts_with("meta-"),
            "the cursor is on {:?}",
            rows[v.cursor()].spot
        );
        assert!(matches!(
            v.key(
                Press::Metadata,
                Some(&crate::player::fixture::idle()),
                80,
                &t
            ),
            crate::player::Errand::Said(_)
        ));
        // A SET OF TWO FILES: the ring, and it walks them.
        let (set, _) = crate::player::fixture::set(
            vec![
                Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv"),
                Lane::new(LaneId(2), LaneKind::Audio, "/films/dub.flac"),
            ],
            &[Some(60.0), Some(60.0)],
            Some(60.0),
            0.0,
        );
        let mut set = set;
        set.sources = ["/films/one.mkv", "/films/dub.flac"]
            .iter()
            .map(|l| ronoctua_player::wire::Source {
                location: (*l).to_owned(),
                tracks: Vec::new(),
            })
            .collect();
        let mut v = View::blank(None);
        v.meta.given(
            "/films/one.mkv",
            0,
            Got::Sheet(Box::new(song_sheet("/films/one.mkv"))),
        );
        let rows = v.rows(Some(&set), 80, &t);
        let (at, ring) = row_of(&rows, &Target::MetaFile).expect("the file ring");
        assert!(ring.fields.iter().any(|f| f.text == "one.mkv"));
        v.grabbed(at);
        v.key(Press::More, Some(&set), 80, &t);
        let rows = v.rows(Some(&set), 80, &t);
        let (_, ring) = row_of(&rows, &Target::MetaFile).expect("the file ring");
        assert!(
            ring.fields.iter().any(|f| f.text == "dub.flac"),
            "{:?}",
            ring.fields
        );
        assert!(
            rows.iter().any(|r| r
                .fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_META_READING))),
            "the second file's sheet is not held yet, and that is said"
        );
    }

    /// FAILS IF: the picker of a cover stops opening beside the file, its
    /// cursor on a picture, listing pictures and nothing that plays — or a
    /// picture picked stops being put in the file shown, the page closing.
    #[test]
    fn a_cover_is_picked_beside_its_file() {
        let t = texts("en-GB");
        let root = std::env::temp_dir().join(format!("ronoctua-cover-{}", std::process::id()));
        let music = root.join("Music");
        std::fs::create_dir_all(&music).expect("a folder");
        for name in ["song.flac", "front.jpg", "notes.txt"] {
            std::fs::write(music.join(name), b"").expect("a file");
        }
        let song = music.join("song.flac").to_string_lossy().into_owned();
        let s = alone(&song, vec![track(1, Kind::Audio, None, true)]);
        let mut v = View::blank(Some(root.clone()));
        v.meta
            .given(&song, 0, Got::Sheet(Box::new(song_sheet(&song))));
        v.key(Press::Go(PageId::Cover), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) =
            row_of(&rows, &Target::Go(PageId::Files(Purpose::Cover))).expect("the picker's row");
        v.grabbed(at);
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Files(Purpose::Cover));
        let rows = v.rows(Some(&s), 80, &t);
        let picture = Target::File(music.join("front.jpg"));
        assert!(
            rows.iter()
                .any(|r| r.cells.iter().any(|c| c.target == picture)),
            "the picture beside the file is not listed"
        );
        assert!(
            !rows.iter().any(|r| r.cells.iter().any(|c| matches!(
                &c.target,
                Target::File(p) if p.ends_with("song.flac") || p.ends_with("notes.txt")
            ))),
            "a file that is no picture is listed"
        );
        assert_eq!(
            rows[v.cursor()].target,
            picture,
            "the cursor is not on the picture"
        );
        match v.key(Press::Enter, Some(&s), 80, &t) {
            crate::player::Errand::TellSaying(Ask::Retag { edits, .. }, _) => assert_eq!(
                edits,
                vec![Edit::Cover(CoverEdit::Replace {
                    from: music.join("front.jpg")
                })]
            ),
            other => panic!("a picture picked asked {other:?}"),
        }
        assert_eq!(v.page(), PageId::Cover, "the picker stayed open");
        std::fs::remove_dir_all(&root).expect("cleaned");
    }

    /// FAILS IF: a cover is drawn out of its proportion, past its box, or
    /// the part cut by the scroll is not cut from the picture too.
    #[test]
    fn a_cover_keeps_its_proportion_and_is_cut_with_its_rows() {
        let air = f32::from(tokens::space::FILES_PICTUREAIR);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        let (area, crop) = cover_place((40, 8), (10.0, 20.0), (600, 600), (0, 8)).expect("drawn");
        let side = 160.0 - 2.0 * air;
        assert!(
            near(area.width, side) && near(area.height, side),
            "{area:?}"
        );
        assert!(near(area.y, air), "no air above the picture: {area:?}");
        assert_eq!(crop, (0.0, 1.0));
        // A WIDE picture in a narrow box is fitted to the box's width.
        let (area, _) = cover_place((10, 8), (10.0, 20.0), (1000, 500), (0, 8)).expect("drawn");
        assert!(
            near(area.width, 100.0) && near(area.height, 50.0),
            "{area:?}"
        );
        // TWO LINES SCROLLED AWAY: the picture's rows under them go.
        let (area, crop) = cover_place((40, 8), (10.0, 20.0), (600, 600), (2, 8)).expect("drawn");
        assert!(
            near(area.y, 40.0) && near(area.height, air + side - 40.0),
            "{area:?}"
        );
        assert!((crop.0 - (40.0 - air) / side).abs() < 1e-5 && (crop.1 - 1.0).abs() < 1e-5);
        let rgba = Rgba::new(2, 4, (0..32).collect()).expect("pixels");
        let top = cropped(&rgba, (0.25, 1.0)).expect("cut");
        assert_eq!((top.width, top.height), (2, 3));
        assert_eq!(
            top.pixels[0], 8,
            "the first row kept is the picture's second"
        );
        assert!(cover_place((40, 8), (10.0, 20.0), (600, 600), (8, 8)).is_none());
    }

    /// The rows of the Every-tag page of `sheet`, the view on it.
    fn every_tag(
        v: &mut View,
        s: &State,
        t: &ronoctua_i18n::Texts,
    ) -> Vec<crate::player_rows::Row> {
        v.key(Press::Go(PageId::EveryTag), Some(s), 80, t);
        assert_eq!(v.page(), PageId::EveryTag);
        v.rows(Some(s), 80, t)
    }

    /// The words of `rows`, one string.
    fn words(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// FAILS IF: a tag the file can take stops being a row that acts — its
    /// value asked holding what it says, or empty with the question that
    /// says it replaces all of it when the line cannot hold it (several
    /// lines, cut, a name with two values) — or `d` stops asking first, or a
    /// row nothing writes stops saying why (once, for a group that shares
    /// the reason), or a typed value stops being sent as the tag by its own
    /// name.
    #[test]
    fn every_tag_a_file_can_take_acts_and_every_other_says_why() {
        use crate::player_question::{TAG_VALUE, TAG_VALUE_ALL};
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let s = a_song();
            let mut v = View::blank(None);
            let mut pairing = crate::player_machines::Pairing::default();
            held(&mut v, &s, tagged_sheet("/music/harbour.flac"));
            let rows = every_tag(&mut v, &s, &t);
            let vorbis = Origin::Lofty {
                format: TagFormat::Vorbis,
            };
            let named = |n: &str| Target::OtherTag(vorbis.clone(), n.into());
            // ADD A TAG, first, naming where it goes.
            let (add, row) = row_of(&rows, &Target::AddTag).expect("add a tag");
            assert!(words(std::slice::from_ref(row)).contains(&t.t(key::PLAYER_ORIGIN_VORBIS)));
            assert!(add < row_of(&rows, &named("TITLE")).expect("title").0);
            // THE EXIFTOOL GROUP: facts, its one reason said once.
            let exif = Origin::Exiftool {
                group: "Vorbis".into(),
            };
            let fixed = Target::OtherTagFixed(exif.clone(), "Vendor".into());
            let (vendor, _) = row_of(&rows, &fixed).expect("vendor");
            assert!(!rows[vendor].target.acts());
            let reason = t.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_VIEW);
            let all = words(&rows);
            assert_eq!(
                all.matches(reason.split_whitespace().next().unwrap_or_default())
                    .count(),
                1,
                "the group's reason is said once: {all}"
            );
            let binary = Target::OtherTagFixed(vorbis.clone(), "FINGERPRINT".into());
            assert!(row_of(&rows, &binary).is_some(), "data is a fact");
            let press = |v: &mut View, target: &Target, press: Press| {
                let rows = v.rows(Some(&s), 80, &t);
                let (at, _) = row_of(&rows, target).expect("the row");
                v.grabbed(at);
                v.key(press, Some(&s), 80, &t)
            };
            assert_eq!(
                press(&mut v, &fixed, Press::Enter),
                Errand::Said(reason.clone())
            );
            assert_eq!(
                press(&mut v, &binary, Press::Remove),
                Errand::Said(t.t(key::PLAYER_TROUBLE_TAG_NOT_TEXT))
            );
            // ONE LINE, ITS ONLY VALUE: asked holding it.
            assert_eq!(
                press(&mut v, &named("TITLE"), Press::Enter),
                Errand::Ask(Question::holding(TAG_VALUE, "Harbour Lights".into()))
            );
            assert_eq!(
                v.answered(TAG_VALUE, "Harbour Lights", &mut pairing, Some(&s), 80, &t),
                Errand::Nothing,
                "what it already says is never mind"
            );
            press(&mut v, &named("TITLE"), Press::Enter);
            match v.answered(
                TAG_VALUE,
                "  Harbour   Lights\tII ",
                &mut pairing,
                Some(&s),
                80,
                &t,
            ) {
                Errand::TellSaying(Ask::Retag { edits, .. }, said) => {
                    assert_eq!(
                        edits,
                        vec![Edit::SetTag {
                            origin: vorbis.clone(),
                            name: "TITLE".into(),
                            value: "Harbour Lights II".into(),
                        }]
                    );
                    assert!(said.contains("TITLE"), "{said}");
                }
                other => panic!("a tag typed asked {other:?}"),
            }
            // TWO VALUES, SEVERAL LINES, CUT: empty, replacing all of it.
            for name in ["ARTIST", "LYRICS", "REPLAYGAIN_TRACK_GAIN"] {
                assert_eq!(
                    press(&mut v, &named(name), Press::Enter),
                    Errand::Ask(Question::plain(TAG_VALUE_ALL, false)),
                    "{name}"
                );
                v.let_go();
            }
            // `d`: a question, "no" first, naming how many values go.
            assert_eq!(
                press(&mut v, &named("ARTIST"), Press::Remove),
                Errand::Nothing
            );
            let rows = v.rows(Some(&s), 80, &t);
            assert_eq!(rows[v.cursor()].target, Target::Answer(false));
            assert!(words(&rows).contains('2'), "{}", words(&rows));
            let (yes, _) = row_of(&rows, &Target::Answer(true)).expect("yes");
            v.grabbed(yes);
            match v.key(Press::Enter, Some(&s), 80, &t) {
                Errand::TellSaying(Ask::Retag { edits, .. }, _) => assert_eq!(
                    edits,
                    vec![Edit::RemoveTag {
                        origin: vorbis.clone(),
                        name: "ARTIST".into(),
                    }]
                ),
                other => panic!("yes asked {other:?}"),
            }
        }
    }

    /// FAILS IF: "Add a tag" stops asking a name and then a value, a name
    /// the tag's grammar refuses is sent instead of refused with its rule,
    /// a name already there stops being asked as the one replacing it all,
    /// or the tag stops going into the file's main tag by the name the
    /// format writes it with.
    #[test]
    fn a_tag_is_added_by_its_name_then_its_value() {
        use crate::player_question::{TAG_NAME, TAG_VALUE, TAG_VALUE_ALL};
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let s = a_song();
            let mut v = View::blank(None);
            let mut pairing = crate::player_machines::Pairing::default();
            held(&mut v, &s, tagged_sheet("/music/harbour.flac"));
            let rows = every_tag(&mut v, &s, &t);
            let (add, _) = row_of(&rows, &Target::AddTag).expect("add a tag");
            let ask = |v: &mut View| {
                v.grabbed(add);
                v.key(Press::Enter, Some(&s), 80, &t)
            };
            assert_eq!(ask(&mut v), Errand::Ask(Question::plain(TAG_NAME, false)));
            let refused = v.answered(TAG_NAME, "A=B", &mut pairing, Some(&s), 80, &t);
            let Errand::Said(said) = refused else {
                panic!("a bad name asked {refused:?}");
            };
            assert!(said.contains("A=B"), "{said}");
            assert!(
                said.contains(&t.t(key::PLAYER_TAG_NAME_RULE_VORBIS)),
                "{said}"
            );
            assert_eq!(ask(&mut v), Errand::Ask(Question::plain(TAG_NAME, false)));
            assert_eq!(
                v.answered(TAG_NAME, "title", &mut pairing, Some(&s), 80, &t),
                Errand::Ask(Question::plain(TAG_VALUE_ALL, false)),
                "a name already there replaces it"
            );
            v.let_go();
            assert_eq!(ask(&mut v), Errand::Ask(Question::plain(TAG_NAME, false)));
            assert_eq!(
                v.answered(TAG_NAME, " MOOD ", &mut pairing, Some(&s), 80, &t),
                Errand::Ask(Question::plain(TAG_VALUE, false))
            );
            match v.answered(TAG_VALUE, "calm", &mut pairing, Some(&s), 80, &t) {
                Errand::TellSaying(
                    Ask::Retag {
                        location, edits, ..
                    },
                    said,
                ) => {
                    assert_eq!(location, "/music/harbour.flac");
                    assert_eq!(
                        edits,
                        vec![Edit::SetTag {
                            origin: Origin::Lofty {
                                format: TagFormat::Vorbis
                            },
                            name: "MOOD".into(),
                            value: "calm".into(),
                        }]
                    );
                    assert!(said.contains("MOOD"), "{said}");
                }
                other => panic!("a value typed asked {other:?}"),
            }
            // NOTHING TYPED is never mind, at either question.
            ask(&mut v);
            assert_eq!(
                v.answered(TAG_NAME, "  ", &mut pairing, Some(&s), 80, &t),
                Errand::Nothing
            );
        }
    }

    /// FAILS IF: a file that takes no tag — nothing writes its container —
    /// stops saying why on the "Add a tag" row, says it again for the page
    /// right under it, or draws its rows as ones that act.
    #[test]
    fn a_file_that_takes_no_tag_says_why_once() {
        let t = texts("en-GB");
        let s = a_song();
        let mut v = View::blank(None);
        let mut sheet = song_sheet("/music/harbour.flac");
        let nothing = Why::NoWriter {
            container: Container::Avi,
        };
        sheet.container = Container::Avi;
        sheet.adding = Some(ronoctua_tags::Adding {
            origin: None,
            can: Can::No(nothing.clone()),
        });
        sheet.other = [None, Some(0), Some(1)]
            .into_iter()
            .map(|stream| Tag {
                origin: Origin::Ffprobe { stream },
                name: "title".into(),
                value: TagValue::Text("x".into()),
                can: Can::No(nothing.clone()),
            })
            .collect();
        held(&mut v, &s, sheet);
        let rows = every_tag(&mut v, &s, &t);
        assert!(row_of(&rows, &Target::AddTag).is_none());
        assert!(
            rows.iter()
                .all(|r| !matches!(r.target, Target::OtherTag(..)))
        );
        let reason = crate::player::tags_refusal(&nothing, &t);
        let all = words(&rows);
        assert_eq!(
            all.matches(&reason).count(),
            1,
            "on the add row, and not again for the page: {all}"
        );
    }
}
