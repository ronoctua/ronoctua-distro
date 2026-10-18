//! `LEADER s l` — the language, the clock, the date and what the region owns.
//!
//! # Two doors, one view
//!
//! The shape `display.rs`, `power.rs`, `network.rs` and `bluetooth.rs` already
//! have: this file knows what the rows are and what a key does to them, and
//! knows nothing about a door. `s l l` draws it in the footer's region, `s l w`
//! in a window of its own, and both ask `View::rows` and paint `Row.fields`.
//!
//! # What it can change, and what it can only SAY
//!
//! The distinction is the whole design of this screen, and it is not a
//! limitation to be apologised for — it is invariant 6. Two of the four
//! sections are the person's and are written to a file; two are the MACHINE's
//! and are declared in Nix, where changing them is a rebuild.
//!
//!   - the LANGUAGE is ours: `language.json`, watched by every door, live.
//!   - the CLOCK and the DATE ORDER are ours: `locale.json`, read by
//!     `chrome::when_text`, which is the one place a date becomes a string.
//!   - the TIME ZONE is the machine's (`time.timeZone`), and the row says so
//!     and says where.
//!   - the UNITS, the PAPER and the decimal separator are the machine's or the
//!     language's, and each row says which.
//!
//! A screen that offered all four and silently honoured two would be worse
//! than one that offers two: the person would change a thing, see nothing
//! happen, and have no way of finding out why.
//!
//! # Why the language is not simply `LANG`
//!
//! Because `LANG` takes a rebuild, and choosing between two translations is
//! not a rebuild-sized decision. See `crate::language` for the three sources
//! and why the file beats the environment.

use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::when::{Choice, Clock, Decimal, Order, Setting, Units, prefs};

/// The keys this view answers to. The hint line is GENERATED from it
/// (`chrome::hint_of`), so a key that moves here moves on screen.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "locale-down"),
    ("<up>", "locale-up"),
    // `S-<arrows>` MOVE THE VALUE, which is this system's rule wherever a row
    // has one: the file manager's sections, the bluetooth volume, the display
    // view's modes. Plain arrows walk the list; shifted ones turn the ring.
    ("S-<left>", "locale-less"),
    ("S-<right>", "locale-more"),
    ("RET", "locale-enter"),
    // BARE LETTERS, and they are free here for the power view's reason:
    // nothing is typed INTO the list. What is typed is typed on a line of its
    // own — the minibuffer in the footer, the header's line in a window — and
    // while that line asks, every key is the line's. They are the three
    // questions, reachable without walking to their row, in both doors
    // (`press_of_key`); `c` asks where the place under the cursor is.
    // `z` IS THIS INTERFACE'S ZONE, and `locale-zone` says so in words now:
    // this list holds COMMAND IDS, not text keys, so the hint is renamed in
    // the `.ftl` rather than here. It read `Time zone`, which is what the
    // MACHINE's row is called a section below.
    ("z", "locale-zone"),
    ("a", "locale-place"),
    ("c", "locale-where"),
];

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, skipping headings.
    Up,
    /// Down one row, skipping headings.
    Down,
    /// The value under the cursor, one back.
    Less,
    /// The value under the cursor, one on.
    More,
    /// Act on the row: flip a switch, cycle a ring, open a question.
    Enter,
    /// Ask for the zone this interface shows times in — `z`.
    Zone,
    /// Ask for a place to add to the glance — `a`.
    Place,
    /// Ask where the place under the cursor is — `c`.
    Where,
    /// Close.
    Escape,
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call, from the keyboard or from a click on the hint.
///
/// # Why one reader
///
/// Because there were two, and neither knew `z`, `a` or `c`: the table taught
/// them on the hint line of both doors, and in both a key and a click on it
/// did nothing. Read out of [`OF_THE_VIEW`] by notation, a key the table
/// teaches is a key both doors answer, and a chord that is not in the table
/// (`C-a`) is not taken for the letter under it. `Esc` is every level's way
/// back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    Some(match id {
        "locale-down" => Press::Down,
        "locale-up" => Press::Up,
        "locale-less" => Press::Less,
        "locale-more" => Press::More,
        "locale-enter" => Press::Enter,
        "locale-zone" => Press::Zone,
        "locale-place" => Press::Place,
        "locale-where" => Press::Where,
        _ => return None,
    })
}

/// What `RET` on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a heading, or a fact this system cannot change from here.
    Nothing,
    /// The language, as ONE row whose value is a ring.
    ///
    /// It was a row per language, and the owner read that off the screen: two
    /// alternatives one under the other are not two settings, they are one
    /// setting and its values. The same is true of the clock and the date, and
    /// they were all three wrong in the same way.
    Language,
    /// The clock, as a ring: twenty-four hours or twelve.
    Clock,
    /// The date's order, as a ring.
    Order,
    /// The decimal mark, as a ring: the language's, or one of the two.
    Decimal,
    /// The units, as a ring.
    Units,
    /// The zone this interface shows times in. `RET` asks for one.
    ///
    /// THE INTERFACE'S, and not the machine's: it decides what this footer and
    /// these windows draw, and nothing else on the computer. [`Self::Machine`]
    /// is the other one, and the two rows say which is which.
    Zone,
    /// Whether the MACHINE's clock is kept right over the internet.
    Automatic,
    /// Whether the brain's clock watch is running. The same switch as on that
    /// task's own page — one truth, two doors.
    Watched,
    /// The MACHINE's date. `RET` asks for one, and only while the clock is
    /// not being kept automatically.
    MachineDate,
    /// The MACHINE's time, on the same terms.
    MachineTime,
    /// The MACHINE's time zone. `RET` asks for one, unless the flake pins it.
    MachineZone,
    /// Add a place to the glance. `RET` asks for a zone.
    AddPlace,
    /// Take a place off the glance.
    DropPlace(usize),
    /// Turn one of a place's four things on or off. `usize` is which place.
    Shows(usize, Shown),
    /// Choose a place's two currencies — its own, and the one it is worth
    /// in. `RET` asks for the pair.
    Currency(usize),
    /// Ask this host, or stop asking it.
    Ask(Source),
}

/// One of the four things a place can show.
///
/// An enum and not a `&str`: the row has to say WHICH switch it is to the
/// code that flips it, and a string would be a name matched in two places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// The time there.
    Clock,
    /// The date there.
    Date,
    /// The temperature there.
    Weather,
    /// One of its currency, in the other.
    Rate,
}

impl Shown {
    /// The id, which is also the tail of its text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Date => "date",
            Self::Weather => "weather",
            Self::Rate => "rate",
        }
    }

    /// The four, in the order they are drawn on the line.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::Clock, Self::Date, Self::Weather, Self::Rate].into_iter()
    }

    /// Whether this place shows it.
    #[must_use]
    pub const fn of(self, place: &ronoctua_world::prefs::Wanted) -> bool {
        match self {
            Self::Clock => place.clock,
            Self::Date => place.date,
            Self::Weather => place.weather,
            Self::Rate => place.rate,
        }
    }

    /// Turns it on, or off, on a place.
    pub const fn set(self, place: &mut ronoctua_world::prefs::Wanted, on: bool) {
        match self {
            Self::Clock => place.clock = on,
            Self::Date => place.date = on,
            Self::Weather => place.weather = on,
            Self::Rate => place.rate = on,
        }
    }
}

/// One of the two things the glance asks somebody else for.
///
/// # Why this is the world crate's type and not a second enum here
///
/// Because it WAS a second enum here, with the same two arms and a `host()`
/// that read the generated table — and the day a source grew a LIST of hosts,
/// this copy would have had to grow one too, or keep naming the first of them
/// as though it were the only one. What a source is belongs beside the table
/// that declares it.
pub use ronoctua_world::Source;

/// The width `press` builds rows at.
///
/// A key acts on the row under the cursor, so it has to know the rows — and it
/// does not know how wide the screen is: the two doors measure differently.
///
/// # Why it is honest, and what changed under it
///
/// It rests on the ROWS ONE CAN STAND ON being the same things at every width,
/// which is what the cursor now counts. The DRAWN rows are not: a note wraps,
/// so a narrow screen draws more of them, and a cursor into the drawn rows
/// would have named a different setting at a different width — the key acting
/// on one row while the door drew the accent on another, which is the defect
/// `the_band_and_the_footer_agree` records on the brain's page.
///
/// That is an invariant and not an observation, so it is checked rather than
/// trusted: `the_rows_are_the_same_things_at_every_width` walks several widths
/// and compares the acting rows' targets.
const DECIDING_WIDTH: u16 = 80;

/// One row the view draws. The shape the other views use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// Is the cursor on it?
    pub focused: bool,
    /// A heading names the section and is not in it: the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub target: Target,
}

/// What the view wants done, after a key or a click.
///
/// # A change, and never the whole record
///
/// `Keep` and `Glance` carry what CHANGED, and the door lays it over what the
/// file says at the moment of writing ([`prefs::with`], [`glance_change`]).
/// They used to carry the view's whole copy — and the file has two writers,
/// the footer's door and the window's: a door that wrote its copy back undid
/// whatever the other door had written since that copy was read. A zone
/// chosen in the footer went back to the old one at the next switch flipped
/// in a window opened before it. Laid over the file, the two doors' changes
/// add up instead of taking turns; the watch (`Notice::on_file`, in both
/// doors) is what then brings the other door's rows up to date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Speak this language everywhere: write `language.json`, which every
    /// door watches.
    Speak(&'static str),
    /// Write one of the date and number preferences.
    Keep(Setting),
    /// Write one change to the glance's places and switches.
    Glance(GlanceChange),
    /// Turn the MACHINE's automatic time-keeping on, or off.
    ///
    /// The DOOR does it, and not this type: what is on the other side is a
    /// system bus and a polkit rule, and a view that is drawn by two doors has
    /// no business holding a connection to either. It is the shape every other
    /// errand here has.
    Automatic(bool),
    /// Turn the brain's clock watch on, or off. Also the door's work — the
    /// brain's socket is on the other side of it.
    Watch(bool),
    /// Something has to be typed: the DOOR asks, on its own line — the
    /// footer's minibuffer, a window's header — under
    /// [`Asking::command`], and hands what was typed to [`took`].
    Ask(Asking),
    /// Say this on the message line — a key that cannot act where the cursor
    /// is says why, rather than doing nothing.
    Say(&'static str),
    /// Close the view.
    Close,
}

/// One change to the glance's file, named by what it changes.
///
/// A PLACE IS NAMED BY ITS ZONE, which is its identity — the cache is keyed
/// by it and two places cannot share one — and never by its row: the other
/// door may have added or removed a place since this one read the list, and
/// the third row of one door's copy is not the third row of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlanceChange {
    /// One of a place's four things, turned on or off.
    Show {
        /// The place.
        zone: String,
        /// Which of the four.
        shown: Shown,
        /// On, or off.
        on: bool,
    },
    /// A place taken off the glance.
    Drop {
        /// The place.
        zone: String,
    },
    /// A host asked, or no longer asked.
    Ask {
        /// Which one.
        source: Source,
        /// Asked, or not.
        on: bool,
    },
}

impl GlanceChange {
    /// This change, laid over what the file says. A place the file no longer
    /// has is a change with nothing to change — the other door took it off.
    pub fn onto(&self, prefs: &mut ronoctua_world::prefs::Preferences) {
        match self {
            Self::Show { zone, shown, on } => {
                let mut places = prefs.places();
                if let Some(place) = places.iter_mut().find(|p| &p.zone == zone) {
                    shown.set(place, *on);
                }
                prefs.places = places;
                prefs.chosen = true;
            }
            Self::Drop { zone } => {
                let mut places = prefs.places();
                places.retain(|p| &p.zone != zone);
                prefs.places = places;
                prefs.chosen = true;
            }
            Self::Ask { source, on } => {
                if source.asked(prefs) != *on {
                    source.flip(prefs);
                }
            }
        }
    }
}

/// Where the view stands and what it is showing.
#[derive(Debug, Clone, Default)]
pub struct View {
    cursor: usize,
    /// What the file says, as this door last read it.
    choice: Choice,
    /// The language in force, and whether it is the person's own word.
    speaking: String,
    /// The glance's places and switches, as this door last read them.
    glance: ronoctua_world::prefs::Preferences,
    /// What the MACHINE says about its own clock, as this door last asked.
    ///
    /// A SNAPSHOT, read when the view opens and when a watch rings — never
    /// from `rows`, which is called on every frame and must not open a bus.
    /// It is the rule the whole footer is built on: whoever draws gets
    /// microseconds or nothing.
    machine: ronoctua_clock::Standing,
    /// And the machine's zone, read once beside it: the machine's clock is
    /// drawn in the machine's zone, and reading a TZif file per frame is the
    /// same mistake by a different road.
    machine_zone: ronoctua_files::when::Zone,
    /// Whether the brain's clock watch is on, or `None` when the brain could
    /// not be asked. The row is then drawn REFUSING and says why — a switch
    /// that silently did nothing would be worse than no switch.
    ///
    /// HANDED IN, never asked for here: the footer reads it out of the mind
    /// it already streams, and the window asks off its loop
    /// ([`ask_the_brain`]). A view drawn by two doors holds no socket — and
    /// it held one: every open, every switch and every language change asked
    /// the brain twice on the loop's own thread, half a second each at worst.
    watched: Option<Watched>,
    /// And what the fetcher last found, which is where the sources' section
    /// gets "did anybody answer" from. READ and not parsed: a cache that will
    /// not parse costs a sentence, and the rows are still drawn.
    world: ronoctua_world::Cache,
    /// Whose answer the language is — it is what tells "the machine is in
    /// English" from "you chose English".
    from: Option<crate::language::Source>,
}

impl View {
    /// A view over the current answers.
    #[must_use]
    pub fn fresh() -> Self {
        let (speaking, from) = crate::language::resolve();
        let mut me = Self {
            cursor: 0,
            choice: prefs::read(),
            speaking,
            glance: ronoctua_world::prefs::store().read(),
            world: ronoctua_world::store::store().read(),
            machine: machine_clock(),
            machine_zone: zone_named(&machine_clock().zone),
            // NOT KNOWN YET, and said as such — not as a brain that is not
            // running — until the door hands it in.
            watched: Some(Watched::Asking),
            from: Some(from),
        };
        // THE CURSOR STARTS ON A ROW ONE CAN ACT ON, and it did not.
        //
        // Row zero is the first HEADING — every section opens with one — so a
        // fresh view had its cursor on a row that is skipped, nothing drawn as
        // focused, and `RET` landing on `Target::Nothing`. From the outside
        // that is a screen where no key does anything, which is exactly what
        // the owner reported. `walk` from a heading finds the first real row.
        me.settle();
        me
    }

    /// Puts the cursor on the first row one can stand on, and never past the
    /// end. Called when the view opens and whenever the rows are rebuilt.
    fn settle(&mut self) {
        let rows = self.rows(
            DECIDING_WIDTH,
            &ronoctua_files::when::Zone::utc(),
            &ronoctua_i18n::Texts::fresh(&self.speaking)
                .unwrap_or_else(ronoctua_i18n::Texts::from_env),
        );
        // THE CURSOR COUNTS ACTING ROWS, so it cannot land on a heading at
        // all — which is what the whole of this used to be for. What is left
        // is keeping it inside them: a place removed, or a section that lost
        // its rows because a daemon went away, leaves it past the end, and a
        // cursor past the end is a screen where nothing is focused and every
        // key does nothing. That was the defect this rebuild started from.
        self.cursor = self.cursor.min(acting(&rows).count().saturating_sub(1));
    }

    /// Re-reads the files and the machine's clock. Called after this door
    /// changed something. The brain's watch is not read here: it is handed in
    /// ([`View::set_watched`]).
    pub fn reread(&mut self) {
        self.machine = machine_clock();
        self.machine_zone = zone_named(&self.machine.zone);
        self.reread_files();
    }

    /// What the brain's clock watch is, as the door learned it — and whether
    /// that changed what the view draws.
    pub fn set_watched(&mut self, watched: Option<Watched>) -> bool {
        let moved = self.watched != watched;
        self.watched = watched;
        moved
    }

    /// Re-reads the FILES alone — the language, the dates' shape and the
    /// glance's places. What a watch ringing calls, in both doors.
    ///
    /// The files and nothing else, because a watch rings on a write this door
    /// did not make, and the bus and the brain's socket are two round trips
    /// that answer the same as a moment ago.
    pub fn reread_files(&mut self) {
        let (speaking, from) = crate::language::resolve();
        self.speaking = speaking;
        self.from = Some(from);
        self.choice = prefs::read();
        self.glance = ronoctua_world::prefs::store().read();
        self.world = ronoctua_world::store::store().read();
        // THE ROWS MAY HAVE CHANGED SHAPE — a place added or removed by the
        // other door — so the cursor is brought back inside them.
        self.settle();
    }

    /// The place whose rows the cursor is on — by its zone, the place's
    /// identity — or `None` when it is on no place's rows.
    #[must_use]
    pub fn place_here(&self) -> Option<String> {
        let rows = self.rows(
            DECIDING_WIDTH,
            &ronoctua_files::when::Zone::utc(),
            &ronoctua_i18n::Texts::fresh(&self.speaking)
                .unwrap_or_else(ronoctua_i18n::Texts::from_env),
        );
        self.place_on(&rows)
    }

    /// The same, against rows already built.
    fn place_on(&self, rows: &[Row]) -> Option<String> {
        let n = match acting(rows).nth(self.cursor).map(|r| &r.target)? {
            Target::Shows(n, _) | Target::DropPlace(n) | Target::Currency(n) => *n,
            _ => return None,
        };
        self.glance.places().get(n).map(|p| p.zone.clone())
    }

    /// The hint line, generated from the key table.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, texts)
    }

    /// The rows to draw, at this width.
    /// The rows to draw, at this width.
    ///
    /// # One row per SETTING, and its value is a ring
    ///
    /// It was one row per VALUE — `24-hour clock` above `12-hour clock`, three
    /// rows for the date's order, one per language — and the owner read that
    /// off the screen and said what it is: those are not several settings,
    /// they are one setting and its alternatives. A list of alternatives also
    /// has no way to show which is in force except by repeating it in the
    /// value column, which is what it was doing.
    #[must_use]
    pub fn rows(
        &self,
        columns: u16,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        use crate::key;
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            nest: 0,
            cursor: self.cursor,
            nth: 0,
            texts,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));

        // 1. THE LANGUAGE — one row, and its value is the language's own name.
        //
        // EVERY SECTION OPENS WITH A SENTENCE saying what it changes and WHERE
        // that shows. Without them this screen named six settings and left the
        // person to work out which half of the machine each one reached — and
        // half of them reach only this interface, which is the distinction the
        // whole screen is about.
        b.heading(key::LOCALE_HEADING_LANGUAGE);
        b.note(key::LOCALE_INTERFACE_DOES);
        b.pair(
            texts.t(key::LOCALE_LANGUAGE),
            texts.t(&format!("language-{}", self.speaking.to_lowercase())),
            Target::Language,
        );
        if let Some(from) = self.from {
            b.note(match from {
                crate::language::Source::File => key::LOCALE_FROM_FILE,
                crate::language::Source::Env => key::LOCALE_FROM_SYSTEM,
                crate::language::Source::Default => key::LOCALE_FROM_DEFAULT,
            });
        }

        // 2. THE CLOCK AND THE DATE — one row each, and the value is a LIVE
        //    sample. A person choosing a shape is asking "what will this look
        //    like", and the name of the shape does not answer that.
        b.heading(key::LOCALE_HEADING_TIME);
        b.note(key::LOCALE_TIME_DOES);
        b.pair(
            texts.t(key::LOCALE_CLOCK),
            crate::chrome::clock_text(now, zone, self.choice.clock, texts),
            Target::Clock,
        );
        b.pair(
            texts.t(key::LOCALE_DATE),
            crate::chrome::date_text(now, zone, self.choice.order, texts),
            Target::Order,
        );
        // AND WHICH ZONE THIS INTERFACE DRAWS IN, which is not the machine's
        // zone and now says so in its own name. It used to be called `Time
        // zone`, one row above a note explaining that it was not the one
        // anybody means by that.
        b.pair(
            texts.t(key::LOCALE_SHOWS_TIMES_IN),
            self.choice
                .zone
                .clone()
                .or_else(crate::chrome::zone_name)
                .unwrap_or_else(|| texts.t(key::LOCALE_ZONE_UNKNOWN)),
            Target::Zone,
        );
        b.note(if self.choice.zone.is_some() {
            key::LOCALE_ZONE_IS_YOURS
        } else {
            key::LOCALE_ZONE_IS_THE_MACHINES
        });

        // 3. NUMBERS AND UNITS — the two that used to be facts.
        b.heading(key::LOCALE_HEADING_NUMBERS);
        b.note(key::LOCALE_NUMBERS_DOES);
        b.pair(
            texts.t(key::LOCALE_DECIMAL),
            texts.t_with(
                &format!("locale-decimal-{}", self.choice.decimal.id()),
                &[(
                    "mark",
                    &self.choice.decimal.mark(texts.decimal_mark()).to_string(),
                )],
            ),
            Target::Decimal,
        );
        b.pair(
            texts.t(key::LOCALE_MEASUREMENT),
            texts.t(&format!("locale-units-{}", self.choice.units.id())),
            Target::Units,
        );

        // 4. THE MACHINE'S OWN CLOCK, which this screen could not touch at all.
        //
        //    That was the owner's complaint about it in one sentence: the
        //    settings changed the clock, the date and the zone of the
        //    INTERFACE, and nothing about the computer. Everything here is the
        //    machine's, and what cannot be changed says why rather than being
        //    left out — a screen that offered four things and honoured two
        //    would be worse than one that offers two.
        b.heading(key::LOCALE_HEADING_MACHINE_CLOCK);
        b.note(key::LOCALE_MACHINE_DOES);
        self.machine_rows(&mut b, now, texts);

        // 5. THE GLANCE'S PLACES — `LEADER g`, and what each of them shows.
        b.heading(key::LOCALE_HEADING_GLANCE);
        b.note_with(
            key::LOCALE_GLANCE_DOES,
            &[(
                "keys",
                &crate::bindings::taught("world-glance", None).unwrap_or_default(),
            )],
        );
        let places = self.glance.places();
        if places.is_empty() {
            b.note_with(
                key::LOCALE_NO_PLACES,
                &[(
                    "keys",
                    &crate::bindings::taught("world-glance", None).unwrap_or_default(),
                )],
            );
        }
        for (n, place) in places.iter().enumerate() {
            // THE ZONE BELONGS TO THE NAME, discreetly and in brackets — it
            // was the VALUE of the row below, which said that removing a place
            // was somehow about `Europe/London`. The owner asked for exactly
            // this instead.
            let (name, tail) = if place.key.is_empty() {
                (place.zone.clone(), "")
            } else {
                (texts.t(&place.key), place.zone.as_str())
            };
            b.under(name, tail);
            for shown in Shown::all() {
                b.pair(
                    texts.t(&format!("locale-shows-{}", shown.id())),
                    texts.t(if shown.of(place) {
                        key::LOCALE_ON
                    } else {
                        key::LOCALE_OFF
                    }),
                    Target::Shows(n, shown),
                );
            }
            // ITS CURRENCIES, which the rate is between: the place's own and
            // the one it is worth in, chosen here — `RET` asks for the pair.
            b.pair(
                texts.t(key::LOCALE_CURRENCY),
                if has_a_pair(place) {
                    texts.t_with(
                        key::LOCALE_CURRENCY_PAIR,
                        &[("from", &place.currency), ("into", &place.rate_in)],
                    )
                } else {
                    texts.t(key::LOCALE_CURRENCY_NONE)
                },
                Target::Currency(n),
            );
            // AN ACTION, with no value. See `Builder::act`.
            b.act(texts.t(key::LOCALE_DROP_PLACE), Target::DropPlace(n));
        }
        // AND THE WAY TO ADD ONE, at the end of the list rather than the top:
        // it is the row somebody reaches after reading what is already there.
        //
        // WITH AIR IN FRONT, because without it the row sits flush under the
        // last place's own rows and reads as one more of them — `chrome::AIR`
        // is what says "this belongs to the section, not to the thing above".
        b.close();
        b.air();
        b.act(texts.t(key::LOCALE_ADD_PLACE), Target::AddPlace);
        b.note(key::LOCALE_ADD_PLACE_WHERE);

        // 6. AND WHO IS ASKED, with the hosts NAMED: turning a source off is
        //    a decision about a party rather than about an abstraction.
        //
        //    AND WHETHER THEY ANSWERED. It said nothing about that, and the
        //    miss was a `debug!` line — so a host that was down cost the
        //    temperature and the bar simply did not have it, which from the
        //    outside is a feature that does not work. This is where the glance
        //    says what happened; the bar itself is read in a glance and is no
        //    place for an error.
        b.heading(key::LOCALE_HEADING_SOURCES);
        b.note(key::LOCALE_SOURCES_ARE_ASKED);
        for source in Source::ALL {
            b.pair(
                texts.t(&format!("locale-source-{}", source.id())),
                texts.t(if source.asked(&self.glance) {
                    key::LOCALE_ASKED
                } else {
                    key::LOCALE_NOT_ASKED
                }),
                Target::Ask(source),
            );
            let hosts: Vec<&str> = source.from().iter().map(|p| p.host).collect();
            b.note_with(key::LOCALE_SOURCE_HOSTS, &[("hosts", &hosts.join(", "))]);
            let said = self.world.about(source);
            if !said.trouble.is_empty() {
                b.note_with(key::LOCALE_SOURCE_TROUBLE, &[("said", &said.trouble)]);
            }
            match said.at {
                Some(at) => b.note_with(
                    key::LOCALE_SOURCE_ANSWERED,
                    &[
                        ("when", &crate::chrome::when_text(at, zone, false, texts)),
                        ("host", &said.host),
                    ],
                ),
                None => b.note(key::LOCALE_SOURCE_NEVER),
            }
        }

        b.out
    }

    /// The machine's own clock, as rows — the section that did not exist.
    ///
    /// # Why the two halves refuse rather than disappear
    ///
    /// Because a row that is not drawn and a row that cannot be used are the
    /// same thing from a chair: nothing on the screen. With the clock kept
    /// automatically the date and the time cannot be set by hand — the daemon
    /// refuses, because the next check would put them back — and the honest
    /// screen is the one that shows them, says so, and lets the person turn
    /// the keeping off if that is what they meant.
    fn machine_rows(&self, b: &mut Builder<'_>, now: i64, texts: &ronoctua_i18n::Texts) {
        use crate::key;
        if !self.machine.reachable {
            b.note(key::LOCALE_MACHINE_UNREACHABLE);
            return;
        }
        b.pair(
            texts.t(key::LOCALE_AUTOMATIC),
            texts.t(if self.machine.automatic {
                key::LOCALE_AUTOMATIC_ON
            } else {
                key::LOCALE_AUTOMATIC_OFF
            }),
            // A MACHINE WITH NO TIME CLIENT CANNOT BE ASKED TO USE ONE, and a
            // switch that flipped to nothing is worse than a row that says why.
            if self.machine.can_be_automatic {
                Target::Automatic
            } else {
                Target::Nothing
            },
        );
        b.note(key::LOCALE_AUTOMATIC_DOES);
        if self.machine.can_be_automatic {
            if !self.machine.server.is_empty() {
                b.note_with(
                    key::LOCALE_AUTOMATIC_SERVER,
                    &[("server", &self.machine.server)],
                );
            }
            if self.machine.automatic {
                b.note(if self.machine.synchronised {
                    key::LOCALE_AUTOMATIC_SYNCHRONISED
                } else {
                    key::LOCALE_AUTOMATIC_ADRIFT
                });
            }
        } else {
            b.note(key::LOCALE_AUTOMATIC_CANNOT);
        }

        // AND THE WATCH THAT KEEPS IT THAT WAY. The same switch as on the
        // brain task's own page — one truth behind the daemon, two doors onto
        // it, which is what the owner asked for in those words.
        b.pair(
            texts.t(key::LOCALE_WATCHED),
            texts.t(match self.watched {
                Some(Watched::Set(true)) => key::LOCALE_AUTOMATIC_ON,
                Some(Watched::Set(false) | Watched::BySystem) => key::LOCALE_AUTOMATIC_OFF,
                Some(Watched::Asking) | None => key::LOCALE_ZONE_UNKNOWN,
            }),
            if matches!(self.watched, Some(Watched::Set(_))) {
                Target::Watched
            } else {
                Target::Nothing
            },
        );
        // THE SYSTEM'S "OFF" IS SAID AND NOT OFFERED: the brain refuses the
        // switch, so the row says why in the words the module's own row uses.
        b.note(match self.watched {
            Some(Watched::Set(_) | Watched::Asking) => key::LOCALE_WATCHED_DOES,
            Some(Watched::BySystem) => key::LOCALE_WATCHED_SYSTEM_OFF,
            None => key::LOCALE_WATCHED_UNKNOWN,
        });

        // THE DATE AND THE TIME, in the MACHINE's own zone: this section is
        // about the computer, and drawing its clock through this interface's
        // zone override would be the very confusion the section exists to end.
        let by_hand = self.machine.can_set_by_hand();
        b.pair(
            texts.t(key::LOCALE_MACHINE_DATE),
            crate::chrome::date_text(now, &self.machine_zone, self.choice.order, texts),
            if by_hand {
                Target::MachineDate
            } else {
                Target::Nothing
            },
        );
        b.pair(
            texts.t(key::LOCALE_MACHINE_TIME),
            crate::chrome::clock_text(now, &self.machine_zone, self.choice.clock, texts),
            if by_hand {
                Target::MachineTime
            } else {
                Target::Nothing
            },
        );
        if !by_hand {
            b.note(key::LOCALE_MACHINE_KEPT);
        }

        let declared = zone_is_declared();
        b.pair(
            texts.t(key::LOCALE_MACHINE_ZONE),
            if self.machine.zone.is_empty() {
                texts.t(key::LOCALE_ZONE_UNKNOWN)
            } else {
                self.machine.zone.clone()
            },
            if declared {
                Target::Nothing
            } else {
                Target::MachineZone
            },
        );
        if declared {
            b.note(key::LOCALE_ZONE_IS_DECLARED);
        }
    }

    /// A key. The rows are rebuilt at a fixed width — see `DECIDING_WIDTH`
    /// for why that is honest.
    pub fn press(
        &mut self,
        p: Press,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(DECIDING_WIDTH, zone, texts);
        match p {
            Press::Up => {
                self.walk(&rows, false);
                Errand::Nothing
            }
            Press::Down => {
                self.walk(&rows, true);
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            // THE THREE QUESTIONS, by their letters — the same ones the
            // rows ask, without walking to the row.
            Press::Zone => Errand::Ask(Asking::Zone),
            Press::Place => Errand::Ask(Asking::Place),
            // WHERE THE PLACE UNDER THE CURSOR IS. Not "place zero": a key
            // that answered for a place the person is not looking at would
            // write a coordinate onto somewhere else.
            Press::Where => self
                .place_on(&rows)
                .map_or(Errand::Say(crate::key::LOCALE_NO_PLACE_HERE), |zone| {
                    Errand::Ask(Asking::Where(zone))
                }),
            // THE RING TURNS EITHER WAY, and `RET` turns it forward.
            //
            // `RET` is what a CLICK becomes — "o clique é o `RET` da linha em
            // que cai" — so a row whose only gesture was a shifted arrow
            // would be a row the mouse could not use at all.
            Press::Less => self.act(&rows, -1),
            Press::More | Press::Enter => self.act(&rows, 1),
        }
    }

    /// Acts on the row under the cursor, turning any ring by `step`.
    ///
    /// THE CURSOR COUNTS ROWS ONE CAN STAND ON, not drawn ones — see
    /// [`Builder::cursor`]. A note wraps, so the drawn index of a setting
    /// moves with the width, and a key built at one width would act on a
    /// different setting from the one the door drew the accent on.
    fn act(&mut self, rows: &[Row], step: i32) -> Errand {
        let Some(target) = acting(rows).nth(self.cursor).map(|r| r.target.clone()) else {
            return Errand::Nothing;
        };
        match target {
            Target::Language => {
                let next = turn(&ronoctua_i18n::LANGUAGES, &self.speaking, step);
                Errand::Speak(next)
            }
            // THE VIEW'S COPY TURNS AT ONCE, so the row says the new value
            // before any file has been written; what goes to the file is the
            // one setting (`Setting`), laid over what the file says then.
            Target::Clock => self.keep(Setting::Clock(turn_of(
                Clock::all(),
                self.choice.clock,
                step,
            ))),
            Target::Order => self.keep(Setting::Order(turn_of(
                Order::all(),
                self.choice.order,
                step,
            ))),
            Target::Decimal => self.keep(Setting::Decimal(turn_of(
                Decimal::all(),
                self.choice.decimal,
                step,
            ))),
            Target::Units => self.keep(Setting::Units(turn_of(
                Units::all(),
                self.choice.units,
                step,
            ))),
            Target::Zone => Errand::Ask(Asking::Zone),
            Target::AddPlace => Errand::Ask(Asking::Place),
            Target::DropPlace(n) => {
                let Some(zone) = self.glance.places().get(n).map(|p| p.zone.clone()) else {
                    return Errand::Nothing;
                };
                // THE CURSOR COMES BACK INSIDE THE LIST. Removing the last
                // place leaves it past the end, and a cursor past the end is
                // a screen where nothing is focused and every key does
                // nothing — which is the defect this whole rebuild started
                // from.
                self.cursor = self.cursor.saturating_sub(1);
                self.glance_changes(GlanceChange::Drop { zone })
            }
            Target::Shows(n, shown) => {
                let Some(place) = self.glance.places().get(n).cloned() else {
                    return Errand::Nothing;
                };
                // A COORDINATE IS WHAT THE WEATHER NEEDS, and a place somebody
                // typed the name of has none. Asking is better than a switch
                // that flips and does nothing.
                if shown == Shown::Weather && !place.weather && !place.has_a_coordinate() {
                    return Errand::Ask(Asking::Weather(place.zone));
                }
                // AND A PAIR OF CURRENCIES IS WHAT THE RATE NEEDS — a place
                // somebody added has none, and the switch asks rather than
                // flipping to a line that never draws.
                if shown == Shown::Rate && !place.rate && !has_a_pair(&place) {
                    return Errand::Ask(Asking::Rate(place.zone));
                }
                self.glance_changes(GlanceChange::Show {
                    on: !shown.of(&place),
                    zone: place.zone,
                    shown,
                })
            }
            Target::Currency(n) => self.glance.places().get(n).map_or(Errand::Nothing, |p| {
                Errand::Ask(Asking::Currency(p.zone.clone()))
            }),
            Target::Ask(source) => self.glance_changes(GlanceChange::Ask {
                source,
                on: !source.asked(&self.glance),
            }),
            // THE MACHINE'S HALF. Every one of these is the DOOR's work: what
            // is on the other side is a system bus or the brain's socket, and
            // a view drawn by two doors holds neither.
            Target::Automatic => Errand::Automatic(!self.machine.automatic),
            Target::Watched => Errand::Watch(self.watched != Some(Watched::Set(true))),
            Target::MachineDate => Errand::Ask(Asking::MachineDate),
            Target::MachineTime => Errand::Ask(Asking::MachineTime),
            Target::MachineZone => Errand::Ask(Asking::MachineZone),
            Target::Nothing => Errand::Nothing,
        }
    }

    /// One setting, turned in the view's copy and handed to the door to write.
    fn keep(&mut self, setting: Setting) -> Errand {
        setting.onto(&mut self.choice);
        Errand::Keep(setting)
    }

    /// One change to the glance, made in the view's copy and handed to the
    /// door to write.
    fn glance_changes(&mut self, change: GlanceChange) -> Errand {
        change.onto(&mut self.glance);
        Errand::Glance(change)
    }

    /// Puts the cursor on the DRAWN row `n`, when it is one to stand on.
    ///
    /// Answers whether the cursor was ALREADY there, which is what makes the
    /// second click act: invariant 19 — *"Clique SELECIONA e o segundo faz o
    /// que o `RET` faz"*. The two doors disagreed about this and neither kept
    /// the rule: the footer ran `RET` on the FIRST click, and the window never
    /// ran it at all.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let Some(nth) = acting_index(rows, n) else {
            return false;
        };
        let again = self.cursor == nth;
        self.cursor = nth;
        again
    }

    /// Where the cursor is, counted over the rows one can stand on — for the
    /// tests. A door asks [`View::drawn_row`], which is the row it scrolls to.
    #[cfg(test)]
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// And which DRAWN row that is, which is what a door scrolls to.
    ///
    /// The two are not the same number and cannot be: between them sit the
    /// section titles, the blank rows and the notes, and a note is as many
    /// rows as the width allows.
    #[must_use]
    pub fn drawn_row(&self, rows: &[Row]) -> usize {
        rows.iter()
            .enumerate()
            .filter(|(_, r)| !r.heading)
            .nth(self.cursor)
            .map_or(0, |(n, _)| n)
    }

    /// One step over the rows one can stand on, stopping at the ends.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let last = acting(rows).count().saturating_sub(1);
        self.cursor = if down {
            self.cursor.saturating_add(1).min(last)
        } else {
            self.cursor.saturating_sub(1)
        };
    }
}

impl View {
    /// A view over a PLANTED machine and the distro's own places, and not
    /// this one's: a test that read the developer's own timedated, brain and
    /// files would draw a different number of rows on a machine whose clock
    /// is kept by hand, and its assertions would pass or fail by where it
    /// ran. `automatic` is whether the planted clock is kept automatically.
    pub(crate) fn planted(automatic: bool) -> Self {
        let mut me = Self {
            cursor: 0,
            choice: Choice::default(),
            speaking: ronoctua_i18n::DEFAULT.to_owned(),
            glance: ronoctua_world::prefs::Preferences::default(),
            world: ronoctua_world::Cache::default(),
            machine: ronoctua_clock::Standing {
                reachable: true,
                zone: "Europe/London".to_owned(),
                automatic,
                can_be_automatic: true,
                synchronised: true,
                server: "3.pool.ntp.org".to_owned(),
                now: 1_788_000_000,
            },
            machine_zone: ronoctua_files::when::Zone::utc(),
            watched: Some(Watched::Set(true)),
            from: Some(crate::language::Source::Default),
        };
        me.settle();
        me
    }

    /// The planted view with the glance's places given — what the
    /// photograph draws a place's currencies over.
    pub(crate) fn planted_with(places: Vec<ronoctua_world::prefs::Wanted>) -> Self {
        let mut me = Self::planted(false);
        me.glance.places = places;
        me.glance.chosen = true;
        me.settle();
        me
    }
}

/// The rows one can stand on, in the order they are drawn.
fn acting(rows: &[Row]) -> impl Iterator<Item = &Row> {
    rows.iter().filter(|r| !r.heading)
}

/// Which acting row the DRAWN row `n` is, when it is one at all.
fn acting_index(rows: &[Row], n: usize) -> Option<usize> {
    if rows.get(n)?.heading {
        return None;
    }
    Some(rows.iter().take(n).filter(|r| !r.heading).count())
}

/// The machine's zone got a new name.
///
/// # Errors
///
/// The text key of what is wrong, for the message line: no zone by that name,
/// or the daemon refused — which is what a flake-pinned zone looks like from
/// here, and why the row is drawn refusing when the option is set.
pub fn took_a_system_zone(said: &str) -> Result<(), &'static str> {
    if !a_real_zone(said) {
        return Err(crate::key::LOCALE_NO_SUCH_ZONE);
    }
    ronoctua_clock::set_zone(said).map_err(|e| {
        tracing::warn!(error = %e, "the machine's zone could not be set");
        crate::key::LOCALE_CLOCK_REFUSED
    })
}

/// The machine's DATE, keeping the time of day it already had.
///
/// # Errors
///
/// The text key of what is wrong: not a date, or the daemon refused.
pub fn took_a_system_date(said: &str) -> Result<(), &'static str> {
    let parts: Vec<&str> = said.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return Err(crate::key::LOCALE_NOT_A_DATE);
    };
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<i64>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return Err(crate::key::LOCALE_NOT_A_DATE);
    };
    let now = machine_civil();
    set_the_clock(
        (year, month, day),
        (now.hour, now.minute, now.second),
        crate::key::LOCALE_NOT_A_DATE,
    )
}

/// The machine's TIME, keeping the date it already had.
///
/// # Errors
///
/// The text key of what is wrong: not a time, or the daemon refused.
pub fn took_a_system_time(said: &str) -> Result<(), &'static str> {
    let parts: Vec<&str> = said.split(':').collect();
    let (hh, mm, ss) = match parts.as_slice() {
        [hh, mm] => (*hh, *mm, "0"),
        [hh, mm, ss] => (*hh, *mm, *ss),
        _ => return Err(crate::key::LOCALE_NOT_A_TIME),
    };
    let (Ok(hour), Ok(minute), Ok(second)) =
        (hh.parse::<u32>(), mm.parse::<u32>(), ss.parse::<u32>())
    else {
        return Err(crate::key::LOCALE_NOT_A_TIME);
    };
    if hour > 23 || minute > 59 || second > 59 {
        return Err(crate::key::LOCALE_NOT_A_TIME);
    }
    let now = machine_civil();
    set_the_clock(
        (now.year, now.month, now.day),
        (hour, minute, second),
        crate::key::LOCALE_NOT_A_TIME,
    )
}

/// What the machine's clock says right now, in the machine's own zone.
fn machine_civil() -> ronoctua_files::when::Civil {
    let said = machine_clock();
    let now = if said.now > 0 {
        said.now
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
    };
    ronoctua_files::when::civil(now, &zone_named(&said.zone))
}

/// A local date and time in the MACHINE's zone, set on the machine's clock.
///
/// # Why the offset is applied twice
///
/// Because a zone's offset is a function of the INSTANT, and the instant is
/// what is being worked out. The first pass reads the offset at the naive
/// reading and the second reads it at the answer — one refinement, which
/// settles everything but the hour inside a transition, where two civil
/// readings name the same wall clock and no arithmetic can choose between
/// them. It is the same shape `ronoctua_files::trash` uses to read a
/// `DeletionDate` back.
fn set_the_clock(
    date: (i64, u32, u32),
    time: (u32, u32, u32),
    wrong: &'static str,
) -> Result<(), &'static str> {
    let (year, month, day) = date;
    let Some(days) = ronoctua_files::when::epoch_of(year, month, day) else {
        return Err(wrong);
    };
    let seconds = i64::from(time.0) * 3600 + i64::from(time.1) * 60 + i64::from(time.2);
    let naive = days.saturating_mul(86_400).saturating_add(seconds);
    let zone = zone_named(&machine_clock().zone);
    let once = naive - i64::from(zone.offset_at(naive));
    let then = naive - i64::from(zone.offset_at(once));
    ronoctua_clock::set_now(then).map_err(|e| {
        tracing::warn!(error = %e, "the machine's clock could not be set");
        crate::key::LOCALE_CLOCK_REFUSED
    })
}

/// What the machine says about its own clock, or an empty answer.
///
/// NEVER FAILS: a machine with no `timedated` is a machine whose clock this
/// screen can only describe, and `Standing::reachable` is what says so. A
/// refusal here would take the whole view down over a daemon that is allowed
/// to be absent.
fn machine_clock() -> ronoctua_clock::Standing {
    ronoctua_clock::read().unwrap_or_else(|e| {
        tracing::debug!(error = %e, "the machine's clock could not be asked about");
        ronoctua_clock::Standing::unknown()
    })
}

/// A zone by name, or UTC when there is no file for it.
///
/// UTC AND SAYING SO (`Zone::is_utc`) is `when.rs`'s rule: an offset that is
/// wrong by an hour looks exactly like one that is right, in every row, for
/// ever.
fn zone_named(name: &str) -> ronoctua_files::when::Zone {
    if !a_real_zone(name) {
        return ronoctua_files::when::Zone::utc();
    }
    std::env::var_os("RONOCTUA_ZONEINFO").map_or_else(ronoctua_files::when::Zone::utc, |root| {
        ronoctua_files::when::Zone::read_from(&std::path::Path::new(&root).join(name))
    })
}

/// Does the flake pin the machine's time zone?
///
/// # Why an environment variable and not a guess
///
/// Because only the module knows. `ronoctua.time.zone` decides whether NixOS
/// puts `/etc/localtime` in the store and starts `timedated` with
/// `NIXOS_STATIC_TIMEZONE=1` — and a screen that offered the row anyway would
/// be offering a key whose only outcome is a refusal from the bus, arriving
/// after the fact and without the one thing worth saying: where the value
/// comes from.
///
/// The module puts it in the footer's environment, which is the way
/// `RONOCTUA_EXIFTOOL` and its five siblings already travel.
#[must_use]
pub fn zone_is_declared() -> bool {
    std::env::var_os("RONOCTUA_TIME_ZONE").is_some_and(|v| !v.is_empty())
}

/// The brain's clock watch, as its row in the mind says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watched {
    /// On or off, and the person's to flip — here or on the module's page.
    Set(bool),
    /// Switched off by the SYSTEM (`ronoctua.brain.modules.<id>.enable =
    /// false`): the brain refuses the switch, so there is none here, and the
    /// row says why.
    BySystem,
    /// Not known yet: the door has asked, and the answer is on its way. Drawn
    /// as not known, with no switch — and not as a brain that is not running,
    /// which would be a sentence that is false for a moment.
    Asking,
}

/// What a watch's row says, from its two fields: whether it is on, and the
/// word it gives instead of a moment.
#[must_use]
pub fn watched_of(enabled: bool, why_not: &str) -> Watched {
    if why_not == ronoctua_brain::registry::SYSTEM_OFF {
        Watched::BySystem
    } else {
        Watched::Set(enabled)
    }
}

/// The ask that turns the brain's clock watch on, or off.
///
/// ONE KNOB OF ONE MODULE — `Ask::Set` with `Knob::Enabled` — and never the
/// whole preferences: the old `Ask::Tune` sent every preference back, built
/// from the shared default where the module had no entry yet, and switching
/// the watch off and on from here turned a six-hour watch into a weekly run
/// with a minute of countdown. The DAEMON writes the file, resolved over what
/// the module declared, so this switch and the module's own page are one
/// truth with two doors.
#[must_use]
pub fn the_clocks_switch(on: bool) -> ronoctua_brain::wire::Ask {
    ronoctua_brain::wire::Ask::Set {
        task: ronoctua_brain::compiled::clock_sync::ID.to_owned(),
        knob: ronoctua_brain::wire::Knob::Enabled(on),
    }
}

/// What the brain's clock watch is, read out of a mind the door already has:
/// `None` for no mind — the brain is not running, or has not spoken — or a
/// mind with no such row.
#[must_use]
pub fn watched_in(mind: Option<&ronoctua_brain::Mind>) -> Option<Watched> {
    let row = mind?.task(ronoctua_brain::compiled::clock_sync::ID)?;
    Some(watched_of(row.enabled, &row.why_not))
}

/// What a door heard back from the brain about the clock's watch.
#[derive(Debug)]
pub struct Heard {
    /// Its answer to the switch — `None` when nothing was switched.
    pub answer: Option<crate::mind_wire::Answer>,
    /// And the watch as it stands after it: `None` when the brain could not
    /// be asked.
    pub watched: Option<Watched>,
}

/// Switches the brain's clock watch — or, with `None`, only reads it — OFF
/// THE LOOP, and hands what was heard to `back` on that thread. `false` when
/// no thread could start, which the door says at once: nothing would ever come
/// back to say anything else.
///
/// ONE THREAD, TWO ASKS: the switch, and then a look at the mind, so the door
/// draws the watch as the brain now has it — a refusal leaves it as it was.
/// Each ask can wait half a second on the socket, which is why neither is
/// ever made on a door's loop.
pub fn ask_the_brain(switch: Option<bool>, back: impl FnOnce(Heard) + Send + 'static) -> bool {
    std::thread::Builder::new()
        .name("clock-watch".into())
        .spawn(move || {
            let answer = switch.map(|on| {
                crate::mind_wire::answer_from(
                    ronoctua_brain::client::ask(&the_clocks_switch(on)),
                    None,
                )
            });
            let watched = match ronoctua_brain::client::ask(&ronoctua_brain::wire::Ask::Look) {
                Ok(ronoctua_brain::wire::Say::Mind { mind }) => watched_in(Some(&*mind)),
                _ => None,
            };
            back(Heard { answer, watched });
        })
        .map_err(|e| tracing::error!(error = %e, "could not start the clock watch's ask"))
        .is_ok()
}

/// Is there a readable zone file by this name?
///
/// The FILE and not a table: a zone this system can read is a zone it can
/// show, and that is the only question worth asking. It is also what the
/// glance already relies on — `world::zones` reads the same directory.
#[must_use]
pub fn a_real_zone(name: &str) -> bool {
    // NO CLIMBING OUT. A name is a path segment pair like `Europe/London`, and
    // one with `..` in it would read a file outside the zone directory — the
    // rule the glTF reader in `ronoctua-inspect` already states for the one
    // other place a name becomes a path here.
    if name.is_empty() || name.contains("..") || name.starts_with('/') {
        return false;
    }
    std::env::var_os("RONOCTUA_ZONEINFO")
        .is_some_and(|root| std::path::Path::new(&root).join(name).is_file())
}

/// The zone this interface shows times in, as the person typed it.
///
/// # Errors
///
/// The text key of what is wrong with it, for the message line.
pub fn took_a_zone(said: &str) -> Result<(), &'static str> {
    if !a_real_zone(said) {
        return Err(crate::key::LOCALE_NO_SUCH_ZONE);
    }
    let mut choice = prefs::read();
    choice.zone = Some(said.to_owned());
    prefs::keep(&choice).map_err(|e| {
        tracing::warn!(error = %e, "the zone could not be written");
        crate::key::LOCALE_NOT_WRITTEN
    })
}

/// What was typed for a question of this view, carried out — the one call
/// both doors make with an answer, whichever line it was typed on.
///
/// `Ok(false)` is nothing typed, which is never mind: no file is written and
/// nothing is said. `Ok(true)` is written; the door re-reads.
///
/// # Errors
///
/// The text key of what is wrong with the answer, for the message line.
pub fn took(asking: &Asking, said: &str) -> Result<bool, &'static str> {
    let said = said.trim();
    if said.is_empty() {
        return Ok(false);
    }
    match asking {
        Asking::Zone => took_a_zone(said),
        Asking::Place => took_a_place(said),
        Asking::Where(zone) => took_a_coordinate(zone, said, false),
        Asking::Weather(zone) => took_a_coordinate(zone, said, true),
        Asking::Currency(zone) => took_a_currency(zone, said, false),
        Asking::Rate(zone) => took_a_currency(zone, said, true),
        Asking::MachineDate => took_a_system_date(said),
        Asking::MachineTime => took_a_system_time(said),
        Asking::MachineZone => took_a_system_zone(said),
    }
    .map(|()| true)
}

/// A new place on the glance, named by its zone.
///
/// It starts with the CLOCK and nothing else — which is what a place is for
/// before anybody says otherwise, and what the owner asked the complementary
/// one to be. Weather and a rate need a coordinate and a currency, and those
/// are asked for when they are turned on rather than up front.
///
/// # Errors
///
/// The text key of what is wrong with it.
pub fn took_a_place(said: &str) -> Result<(), &'static str> {
    if !a_real_zone(said) {
        return Err(crate::key::LOCALE_NO_SUCH_ZONE);
    }
    let store = ronoctua_world::prefs::store();
    let mut prefs = parsed_glance(&store)?;
    let mut places = prefs.places();
    // ALREADY THERE IS NOT AN ERROR, it is a no-op: the cache is keyed by
    // zone, and two rows for one zone would share a temperature and fight
    // over it.
    if places.iter().any(|p| p.zone == said) {
        return Ok(());
    }
    places.push(ronoctua_world::prefs::Wanted {
        zone: said.to_owned(),
        ..ronoctua_world::prefs::Wanted::default()
    });
    prefs.places = places;
    prefs.chosen = true;
    store.write(&prefs).map_err(|e| {
        tracing::warn!(error = %e, "the place could not be written");
        crate::key::LOCALE_NOT_WRITTEN
    })
}

/// Where a place is, so it can have weather.
///
/// The place is named by its ZONE — asked about when the question opened —
/// and not by its row, which the other door may have moved since. With
/// `and_the_weather` the weather switch goes on too: that is the switch that
/// asked (`Asking::Weather`); `c` asks for the coordinate alone.
///
/// # Errors
///
/// The text key of what is wrong with it.
pub fn took_a_coordinate(
    zone: &str,
    said: &str,
    and_the_weather: bool,
) -> Result<(), &'static str> {
    let numbers: Vec<f64> = said
        .split([',', ' ', ';'])
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.trim().parse::<f64>().ok())
        .collect();
    let [latitude, longitude] = numbers[..] else {
        return Err(crate::key::LOCALE_NOT_A_COORDINATE);
    };
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(crate::key::LOCALE_NOT_A_COORDINATE);
    }
    let store = ronoctua_world::prefs::store();
    let mut prefs = parsed_glance(&store)?;
    let mut places = prefs.places();
    // A PLACE THE OTHER DOOR TOOK OFF while the question was open has
    // nowhere to put the answer, and says so.
    let Some(place) = places.iter_mut().find(|p| p.zone == zone) else {
        return Err(crate::key::LOCALE_PLACE_GONE);
    };
    place.latitude = latitude;
    place.longitude = longitude;
    // AND THE SWITCH GOES ON when turning it on is what asked this.
    if and_the_weather {
        place.weather = true;
    }
    prefs.places = places;
    prefs.chosen = true;
    store.write(&prefs).map_err(|e| {
        tracing::warn!(error = %e, "the coordinate could not be written");
        crate::key::LOCALE_NOT_WRITTEN
    })
}

/// Whether a place has the two currencies a rate is between.
#[must_use]
pub fn has_a_pair(place: &ronoctua_world::prefs::Wanted) -> bool {
    !place.currency.is_empty() && !place.rate_in.is_empty()
}

/// The two currencies typed — `EUR BRL`, `eur → brl`, `EUR/BRL` — as the
/// codes the rate's source knows, or `None`.
///
/// THE CODES ARE THE ECB'S (`ronoctua_calc::units::CURRENCY_CODES`, the list
/// the calculator's rates read): the glance's first rate source is the ECB's
/// own mirror, and a pair it does not quote is a line that never draws. Three
/// letters each side, in any case, anything between them.
#[must_use]
pub fn currency_pair(said: &str) -> Option<(String, String)> {
    let words: Vec<String> = said
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_uppercase)
        .collect();
    let [from, into] = words.as_slice() else {
        return None;
    };
    let known = |c: &str| ronoctua_calc::units::CURRENCY_CODES.contains(&c);
    (from != into && known(from) && known(into)).then(|| (from.clone(), into.clone()))
}

/// A place's two currencies, typed — and with `and_the_rate` the rate's
/// switch goes on too, which is what asked.
///
/// # Errors
///
/// The text key of what is wrong with it.
pub fn took_a_currency(zone: &str, said: &str, and_the_rate: bool) -> Result<(), &'static str> {
    let (from, into) = currency_pair(said).ok_or(crate::key::LOCALE_NOT_A_CURRENCY_PAIR)?;
    let store = ronoctua_world::prefs::store();
    let mut prefs = parsed_glance(&store)?;
    let mut places = prefs.places();
    let Some(place) = places.iter_mut().find(|p| p.zone == zone) else {
        return Err(crate::key::LOCALE_PLACE_GONE);
    };
    place.currency = from;
    place.rate_in = into;
    if and_the_rate {
        place.rate = true;
    }
    prefs.places = places;
    prefs.chosen = true;
    store.write(&prefs).map_err(|e| {
        tracing::warn!(error = %e, "the currencies could not be written");
        crate::key::LOCALE_NOT_WRITTEN
    })
}

/// The glance's file as it is NOW, for a writer — or the key of a refusal
/// when it will not parse.
///
/// `parsed`, and never `read`: whoever writes the whole list back after
/// reading the default that `read` gives for a broken file would erase the
/// places in it (`ronoctua_store::Store::parsed`).
fn parsed_glance(
    store: &ronoctua_store::Store<ronoctua_world::prefs::Preferences>,
) -> Result<ronoctua_world::prefs::Preferences, &'static str> {
    store.parsed().ok_or_else(|| {
        tracing::warn!("the glance's file will not parse; it is left as it is");
        crate::key::LOCALE_NOT_WRITTEN
    })
}

/// Writes one change to the glance, laid over what the file says NOW — see
/// [`Errand`] for why a door never writes its whole copy back.
///
/// # Errors
///
/// The text key of why nothing was written: the file will not parse, or it
/// cannot be written.
pub fn glance_change(change: &GlanceChange) -> Result<(), &'static str> {
    glance_change_in(&ronoctua_world::prefs::store(), change)
}

/// [`glance_change`], with its store as a parameter.
///
/// # Errors
///
/// As [`glance_change`].
pub fn glance_change_in(
    store: &ronoctua_store::Store<ronoctua_world::prefs::Preferences>,
    change: &GlanceChange,
) -> Result<(), &'static str> {
    let mut prefs = parsed_glance(store)?;
    change.onto(&mut prefs);
    store.write(&prefs).map_err(|e| {
        tracing::warn!(error = %e, "the glance's places could not be written");
        crate::key::LOCALE_NOT_WRITTEN
    })
}

/// One step around a ring of `&'static str`, wrapping at both ends.
///
/// A ring and not a list with ends: there are two languages, and a setting
/// with two values whose arrows stop at the edges takes two presses to do
/// what one should.
fn turn(all: &[&'static str], now: &str, step: i32) -> &'static str {
    let at = all.iter().position(|v| *v == now).unwrap_or(0);
    let n = all.len();
    let next = (at as i32 + step).rem_euclid(n as i32) as usize;
    all[next]
}

/// The same, for a ring of values that know their own order.
fn turn_of<T: PartialEq + Copy>(all: impl Iterator<Item = T>, now: T, step: i32) -> T {
    let all: Vec<T> = all.collect();
    let at = all.iter().position(|v| *v == now).unwrap_or(0);
    let n = all.len().max(1);
    let next = (at as i32 + step).rem_euclid(n as i32) as usize;
    all[next]
}

/// What the view needs typed.
///
/// BOTH DOORS TYPE IT, on a line of their own — the footer's minibuffer and
/// a window's header (`Ui::ask`) — under [`Asking::command`], and both hand
/// what was typed to [`took`]. A place is named by its ZONE, read when the
/// question opened, so an answer lands on the place that was asked about even
/// if the other door moved the list in between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// A zone for this interface's clock.
    Zone,
    /// A zone for a new place on the glance.
    Place,
    /// Where this place is — `c` on its rows.
    Where(String),
    /// Where this place is, asked by its weather switch: a place with no
    /// coordinate cannot have weather, and the answer turns the switch on.
    Weather(String),
    /// The two currencies of this place, asked by its currency row.
    Currency(String),
    /// The same, asked by the rate's switch: a place with no pair cannot
    /// have a rate, and the answer turns the switch on.
    Rate(String),
    /// A date for the MACHINE's clock.
    MachineDate,
    /// A time for the MACHINE's clock.
    MachineTime,
    /// A zone for the MACHINE.
    MachineZone,
}

impl Asking {
    /// The command that asks — and that answers: the question is its `ask-`
    /// key, and the answer comes back under it, in both doors.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        match self {
            Self::Zone => "locale-zone",
            Self::Place => "locale-place",
            Self::Where(_) | Self::Weather(_) => "locale-where",
            Self::Currency(_) | Self::Rate(_) => "locale-currency",
            Self::MachineDate => "locale-system-date",
            Self::MachineTime => "locale-system-time",
            Self::MachineZone => "locale-system-zone",
        }
    }

    /// The question a command asks when it names it alone — every one but
    /// `locale-where`, which is about a place and needs to know which.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Some(match id {
            "locale-zone" => Self::Zone,
            "locale-place" => Self::Place,
            "locale-system-date" => Self::MachineDate,
            "locale-system-time" => Self::MachineTime,
            "locale-system-zone" => Self::MachineZone,
            _ => return None,
        })
    }
}

/// Builds the rows, so that a heading, a note, a fact and a pair are each
/// written once. The same shape `power.rs` uses.
struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    /// How far the rows being written now are SET IN — `space.view.nestCells`
    /// while a sub-section is open, and nought outside one. See
    /// [`Builder::under`].
    nest: u16,
    /// WHICH ACTING ROW the cursor is on, counted over the rows one can stand
    /// on and NOT over the drawn ones.
    ///
    /// # Why it stopped being the drawn row
    ///
    /// Because a note WRAPS now, and a wrapped note is as many rows as the
    /// width allows. With the cursor an index into the drawn rows, the same
    /// cursor named a different setting at a different width — and `press`
    /// builds the rows itself, so the key would act on one row while the door
    /// drew the band on another. It is the defect `the_band_and_the_footer_agree`
    /// records on the brain's page, arriving here by a different road.
    cursor: usize,
    /// How many acting rows have been written, which is what `cursor` counts.
    nth: usize,
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
            ink: Ink::Grid,
        }
    }

    /// A SECTION's title: the loudest of the three levels, at the margin, and
    /// it closes whatever sub-section was open.
    fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.nest = 0;
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.air();
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                text,
                tokens::color::TEXT_HEADING,
                self.room,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
        });
    }

    /// A SUB-SECTION's title, and everything after it is SET IN until the next
    /// section.
    ///
    /// # Why a second level exists at all, and why it looks like this
    ///
    /// The glance's section is a list of PLACES, each with four switches and a
    /// way to remove it. Drawn flat — the section's title and every place's
    /// name in `text.group` at the same column — nothing on the screen said
    /// which rows belonged to which place. The owner read it and said what it
    /// is: the main title was identical to every sub-title under it, and the
    /// block could not be made out as a section.
    ///
    /// The device is AIR AND COLOUR, because those are the two this system
    /// has: a title is the size of the body text, and hierarchy is never size
    /// and never a rule. So a sub-title is set in by `space.view.nestCells`
    /// and drawn in `text.secondary`, one step quieter than the section above
    /// it and one louder than the rows under it.
    ///
    /// # And the TAIL is a fact about the thing, in the quietest tone
    ///
    /// A place's zone used to be the VALUE of its `Remove this place` row,
    /// which said that removing it was somehow about `Europe/London`. It
    /// belongs to the NAME, discreetly — which is what the owner asked for in
    /// those words.
    ///
    /// The text is a VALUE and not a key: a place somebody added has no
    /// translation and never will, and what names it is the zone they typed.
    /// It is the rule the filter names already follow (`text.mine`).
    fn under(&mut self, text: String, tail: &str) {
        self.nest = 0;
        self.air();
        let at = self.left + tokens::space::VIEW_NESTCELLS;
        let room = self.room.saturating_sub(tokens::space::VIEW_NESTCELLS);
        let mut fields = vec![Self::piece(at, text, tokens::color::TEXT_SECONDARY, room)];
        if !tail.is_empty() {
            let wide = layout::width(&fields[0].text);
            fields.push(Self::piece(
                at + wide + 1,
                format!("({tail})"),
                tokens::color::TEXT_DIM,
                room.saturating_sub(wide + 1),
            ));
        }
        self.out.push(Row {
            fields,
            focused: false,
            heading: true,
            target: Target::Nothing,
        });
        self.nest = tokens::space::VIEW_NESTCELLS;
    }

    /// Closes the sub-section that is open, if any, so that what follows
    /// belongs to the SECTION again.
    fn close(&mut self) {
        self.nest = 0;
    }

    /// A blank row, for where a group ends and something else begins.
    fn air(&mut self) {
        self.out.push(Row {
            fields: crate::chrome::AIR,
            focused: false,
            heading: true,
            target: Target::Nothing,
        });
    }

    fn note_with(&mut self, key: &str, args: &[(&str, &str)]) {
        let text = self.texts.t_with(key, args);
        self.sentence(text);
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    /// A note whose text is already resolved. One push, so a note with
    /// arguments and one without cannot be drawn two different ways.
    ///
    /// # IT WRAPS, and it used to be cut
    ///
    /// It was `elide_end` — one row, with an ellipsis where the room ran out —
    /// and these sentences are the part of the screen that says what a setting
    /// does and where it takes effect. Half of one is worse than none: the
    /// half that is missing is always the end, which is where the consequence
    /// is. The owner asked for them to break instead, in both doors.
    ///
    /// One `Row` per line, which is the shape the notification manager's body
    /// already has — so both doors draw it without knowing anything new.
    fn sentence(&mut self, text: String) {
        let at = self.left + self.nest;
        let room = self.room.saturating_sub(self.nest);
        for line in crate::chrome::note_lines(&text, room) {
            self.out.push(Row {
                fields: vec![Self::piece(at, line, tokens::color::TEXT_DIM, room)],
                focused: false,
                // A NOTE IS NOT A ROW ONE STANDS ON. It explains the rows
                // around it; a cursor that stopped there would be a cursor on
                // a sentence, with `RET` doing nothing and no way to tell why.
                heading: true,
                target: Target::Nothing,
            });
        }
    }

    /// A row that DOES something and has no value.
    ///
    /// # Why a value is not simply left empty
    ///
    /// Because it was not empty: `Remove this place` carried the place's zone
    /// and `Add a place` carried the sentence *"a zone, such as
    /// Europe/Lisbon"*. Both read as the row's ANSWER — the thing the column
    /// is for everywhere else on this screen — when one is a fact about
    /// something else and the other is an instruction. The owner named both.
    ///
    /// An action has no answer. What it does is its label, and what it needs
    /// explaining goes in a note under it, where every other explanation on
    /// this screen already is.
    fn act(&mut self, label: String, target: Target) {
        let focused = self.cursor == self.nth;
        self.nth += 1;
        let at = self.left + self.nest;
        let room = self.room.saturating_sub(self.nest);
        self.out.push(Row {
            fields: vec![Self::piece(
                at,
                label,
                // A DOOR IS A SETTING'S LABEL with no value beside it.
                crate::chrome::Kind::Setting.tones(focused).0,
                room,
            )],
            focused,
            heading: false,
            target,
        });
    }

    fn pair(&mut self, label: String, value: String, target: Target) {
        let focused = self.cursor == self.nth;
        self.nth += 1;
        // THE LABEL SAYS WHETHER THE ROW IS A DOOR, in a hue no value wears.
        let kind = crate::chrome::Kind::of(target != Target::Nothing);
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left + self.nest,
                self.room.saturating_sub(self.nest),
                &label,
                &value,
                kind,
                focused,
            ),
            focused,
            heading: false,
            target,
        });
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "locale",
    commands: &[
        // THE LANGUAGE AND THE REGION. Under `s` because what it changes
        // is what the MACHINE says back — the same group as the theme, the
        // screens and the clock's zone, and not something one does WITH the
        // machine.
        ronoctua_cmd::Command {
            id: "geo-open",
            label: crate::key::CMD_GEO_OPEN,
        },
        ronoctua_cmd::Command {
            id: "geo-window",
            label: crate::key::CMD_GEO_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "geo-up",
            label: crate::key::CMD_GEO_UP,
        },
        ronoctua_cmd::Command {
            id: "geo-down",
            label: crate::key::CMD_GEO_DOWN,
        },
        ronoctua_cmd::Command {
            id: "geo-enter",
            label: crate::key::CMD_GEO_ENTER,
        },
        // THE TWO THE LINE AND THE QUESTION ANSWER. `geo-find` runs twice in
        // the footer, like the language view's questions: once to ask, once
        // to be told — and a window's line answers under the same id.
        ronoctua_cmd::Command {
            id: "geo-find",
            label: crate::key::CMD_GEO_FIND,
        },
        ronoctua_cmd::Command {
            id: "geo-remove",
            label: crate::key::CMD_GEO_REMOVE,
        },
        ronoctua_cmd::Command {
            id: "locale-open",
            label: crate::key::CMD_LOCALE_OPEN,
        },
        ronoctua_cmd::Command {
            id: "locale-window",
            label: crate::key::CMD_LOCALE_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "locale-up",
            label: crate::key::CMD_LOCALE_UP,
        },
        ronoctua_cmd::Command {
            id: "locale-down",
            label: crate::key::CMD_LOCALE_DOWN,
        },
        ronoctua_cmd::Command {
            id: "locale-enter",
            label: crate::key::CMD_LOCALE_ENTER,
        },
        ronoctua_cmd::Command {
            id: "locale-less",
            label: crate::key::CMD_LOCALE_LESS,
        },
        ronoctua_cmd::Command {
            id: "locale-more",
            label: crate::key::CMD_LOCALE_MORE,
        },
        // THE QUESTIONS OF THE LANGUAGE VIEW, and the ids their answers come
        // back under — the footer's minibuffer runs them twice, once to ask
        // and once to be told, and a window's line answers under the same id
        // into the same `locale::took`. Reachable by name like every other
        // command.
        ronoctua_cmd::Command {
            id: "locale-zone",
            label: crate::key::CMD_LOCALE_ZONE,
        },
        ronoctua_cmd::Command {
            id: "locale-place",
            label: crate::key::CMD_LOCALE_PLACE,
        },
        ronoctua_cmd::Command {
            id: "locale-where",
            label: crate::key::CMD_LOCALE_WHERE,
        },
        // THE MACHINE'S THREE, asked by `RET` on their rows. They were asked
        // under ids no command had: the footer drew the raw id as the
        // question, and the answer ran nothing.
        ronoctua_cmd::Command {
            id: "locale-system-date",
            label: crate::key::CMD_LOCALE_SYSTEM_DATE,
        },
        ronoctua_cmd::Command {
            id: "locale-system-time",
            label: crate::key::CMD_LOCALE_SYSTEM_TIME,
        },
        ronoctua_cmd::Command {
            id: "locale-system-zone",
            label: crate::key::CMD_LOCALE_SYSTEM_ZONE,
        },
    ],
    bindings: &[
        // `s l` — LANGUAGE, and the letter is free under `s`.
        // `w` for WINDOW, the letter every view with a second door uses.
        ("s l l", "locale-open", None),
        ("s l w", "locale-window", None),
        // `s g` — GEOGRAPHY, and the letter was free under `s`: the taken ones
        // are `a b d f h k l m n p q s t u w`. What it holds is data about the
        // MACHINE — which regions' streets are on this disk — which is the
        // sentence the `s` group is defined by.
        ("s g g", "geo-open", None),
        ("s g w", "geo-window", None),
    ],
    groups: &[
        ("s l", crate::key::GROUP_LOCALE),
        ("s g", crate::key::GROUP_GEO),
    ],
    by_name: &[
        // THE MACHINE'S CLOCK, asked by `RET` on its rows in `LEADER s l`: each
        // opens its door's line asking for its words (`ask-<id>`), and the answer
        // comes back as the same command's argument. A key of their own would
        // ask for a date on a machine whose clock is kept automatically, which
        // the rows refuse on sight.
        "locale-system-date",
        "locale-system-time",
        "locale-system-zone",
    ],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = crate::locale::OF_THE_VIEW
        .iter()
        .map(|(_, id)| *id)
        .collect();
    out.extend(crate::geo::OF_THE_VIEW.iter().map(|(_, id)| *id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the default language")
    }

    fn utc() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    fn view() -> View {
        View::planted(true)
    }

    /// Where the cursor has to be to act on `want` — an index over the rows
    /// one can STAND on, which is what the cursor counts.
    fn at(v: &View, want: &Target) -> usize {
        let rows = v.rows(DECIDING_WIDTH, &utc(), &texts());
        acting(&rows)
            .position(|r| &r.target == want)
            .unwrap_or_else(|| panic!("no row for {want:?}"))
    }

    /// The row the cursor is on, as the door would draw it.
    fn on<'a>(v: &View, rows: &'a [Row]) -> &'a Row {
        acting(rows)
            .nth(v.cursor())
            .expect("the cursor is past the last row one can stand on")
    }

    /// What the screen says, printed — the check no assertion replaces.
    #[test]
    fn the_screen_reads_the_way_it_will_be_read() {
        let (t, z) = (texts(), utc());
        let v = view();
        for (n, row) in v.rows(72, &z, &t).into_iter().enumerate() {
            let text: Vec<String> = row.fields.iter().map(|f| f.text.clone()).collect();
            let mark = if n == v.cursor() {
                ">>"
            } else if row.heading {
                "  "
            } else {
                " ·"
            };
            println!("  {mark} {}", text.join("   "));
        }
    }

    /// THE CURSOR STARTS SOMEWHERE IT CAN ACT, and it did not.
    ///
    /// THE DEFECT THIS CATCHES, and it is the one the owner reported as "not
    /// interactive": row zero is the first HEADING, every section opens with
    /// one, so a fresh view had nothing focused and `RET` landing on
    /// `Target::Nothing`. From the outside that is a screen where no key does
    /// anything at all.
    #[test]
    fn a_fresh_view_has_its_cursor_on_a_row_that_acts() {
        let (t, z) = (texts(), utc());
        let v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        let row = on(&v, &rows);
        assert!(!row.heading, "the cursor opened on a heading");
        assert_ne!(
            row.target,
            Target::Nothing,
            "the cursor opened on a row nothing happens on"
        );
        // AND THE DRAWN ROW AGREES, which is what a door scrolls to.
        assert!(!rows[v.drawn_row(&rows)].heading);
    }

    /// THE ROWS ONE CAN STAND ON are the same things at every width, so a key
    /// acting on the row `press` builds acts on the row the door drew.
    ///
    /// # Why it is the ACTING rows and not all of them
    ///
    /// Because a note WRAPS now — that is what the owner asked for, in both
    /// doors — so a narrow screen draws more rows than a wide one. What must
    /// not move is the cursor's meaning, and the cursor counts these. The
    /// drawn count differing is the feature; the acting count differing would
    /// be the defect.
    #[test]
    fn the_rows_are_the_same_things_at_every_width() {
        let (t, z) = (texts(), utc());
        let v = view();
        let targets = |columns| -> Vec<Target> {
            let rows = v.rows(columns, &z, &t);
            acting(&rows).map(|r| r.target.clone()).collect()
        };
        let want = targets(DECIDING_WIDTH);
        assert!(want.len() > 10, "only {} acting rows", want.len());
        let mut widths = 0;
        for columns in [20u16, 40, 60, 80, 120, 200] {
            assert_eq!(
                targets(columns),
                want,
                "at {columns} columns the rows are different things"
            );
            widths += 1;
        }
        assert!(widths > 1);
        // AND THE DRAWN COUNT DOES DIFFER, which is what makes the assertion
        // above worth making: a note that never wrapped would prove nothing.
        assert!(
            v.rows(40, &z, &t).len() > v.rows(200, &z, &t).len(),
            "no note wrapped, so the acting-row promise is untested"
        );
    }

    /// ONE ROW PER SETTING, and its value is a ring.
    ///
    /// THE DEFECT THIS CATCHES: a row per VALUE — `24-hour clock` above
    /// `12-hour clock`, three rows for the date, one per language. The owner
    /// read that off the screen and said what it is: those are not several
    /// settings, they are one setting and its alternatives.
    #[test]
    fn the_clock_the_date_and_the_language_are_one_row_each() {
        let (t, z) = (texts(), utc());
        let v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        for want in [Target::Clock, Target::Order, Target::Language] {
            let n = rows.iter().filter(|r| r.target == want).count();
            assert_eq!(n, 1, "{want:?} has {n} rows and should have one");
        }
    }

    /// The ring turns both ways and WRAPS, so a setting of two values never
    /// takes two presses to do what one should.
    #[test]
    fn a_ring_turns_both_ways_and_comes_back_round() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let start = v.choice.clock;
        v.cursor = at(&v, &Target::Clock);

        let Errand::Keep(Setting::Clock(one)) = v.press(Press::More, &z, &t) else {
            panic!("turning the clock did not ask for it to be kept");
        };
        assert_ne!(one, start, "the ring did not turn");

        // ROUND, for a ring of two: one more press is back where it began.
        let Errand::Keep(Setting::Clock(two)) = v.press(Press::More, &z, &t) else {
            panic!("no errand");
        };
        assert_eq!(two, start, "the ring stopped at the end");

        // AND THE OTHER WAY.
        let Errand::Keep(Setting::Clock(back)) = v.press(Press::Less, &z, &t) else {
            panic!("no errand");
        };
        assert_eq!(back, one);
    }

    /// `RET` turns the ring forward, because `RET` is what a CLICK becomes —
    /// a row whose only gesture was a shifted arrow would be a row the mouse
    /// could not use at all.
    #[test]
    fn enter_turns_the_ring_the_way_more_does() {
        let (t, z) = (texts(), utc());
        let mut a = view();
        let mut b = view();
        a.cursor = at(&a, &Target::Order);
        b.cursor = a.cursor;
        let by_enter = a.press(Press::Enter, &z, &t);
        let by_more = b.press(Press::More, &z, &t);
        assert!(
            matches!(by_enter, Errand::Keep(Setting::Order(_))),
            "{by_enter:?}"
        );
        assert_eq!(by_enter, by_more);
    }

    /// Turning one ring leaves every other value alone — the `display.json`
    /// merge defect, in the small.
    #[test]
    fn turning_one_ring_keeps_the_rest() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        v.cursor = at(&v, &Target::Decimal);
        let before = v.choice.clone();
        let Errand::Keep(setting) = v.press(Press::More, &z, &t) else {
            panic!("no errand");
        };
        let mut after = before.clone();
        setting.onto(&mut after);
        assert_ne!(after.decimal, before.decimal);
        assert_eq!(after.clock, before.clock);
        assert_eq!(after.order, before.order);
        assert_eq!(after.units, before.units);
        assert_eq!(after.zone, before.zone);
        // AND THE VIEW'S OWN COPY TURNED, so the row says it at once.
        assert_eq!(v.choice, after);
    }

    /// A RING TURNED IN ONE DOOR KEEPS WHAT THE OTHER DOOR WROTE.
    ///
    /// THE DEFECT THIS CATCHES: each door wrote its WHOLE copy of the record,
    /// read when it opened. A zone chosen in the footer went back to the old
    /// one the moment a clock was turned in a window opened before it.
    #[test]
    fn a_ring_is_laid_over_what_the_file_says_now() {
        let dir = std::env::temp_dir().join(format!("ronoctua-locale-with-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // THE OTHER DOOR'S WRITE, which this view never read.
        let theirs = Choice {
            clock: Clock::Twelve,
            zone: Some("Europe/Lisbon".to_owned()),
            ..Choice::default()
        };
        prefs::keep_in(&dir, &theirs).unwrap();

        let (t, z) = (texts(), utc());
        let mut v = view();
        v.cursor = at(&v, &Target::Order);
        let Errand::Keep(setting) = v.press(Press::More, &z, &t) else {
            panic!("no errand");
        };
        let written = prefs::with_at(&dir.join(prefs::FILE), setting);
        assert_eq!(
            written.order,
            Order::DayFirst,
            "the ring's own change is lost"
        );
        assert_eq!(
            written.clock,
            Clock::Twelve,
            "the other door's clock was undone"
        );
        assert_eq!(
            written.zone.as_deref(),
            Some("Europe/Lisbon"),
            "and its zone"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The four switches of a place are INDEPENDENT, and each flips only
    /// itself — the owner's "choose exactly which information appears".
    #[test]
    fn each_of_the_four_switches_flips_only_itself() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let mut after = v.glance.clone();
        let before = v.glance.places();
        assert!(!before.is_empty());
        v.cursor = at(&v, &Target::Shows(0, Shown::Date));
        let Errand::Glance(change) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
        change.onto(&mut after);
        assert_eq!(after, v.glance, "the view's copy and the change disagree");
        let now = after.places();
        assert_eq!(now[0].date, !before[0].date, "the date did not flip");
        assert_eq!(now[0].clock, before[0].clock);
        assert_eq!(now[0].weather, before[0].weather);
        assert_eq!(now[0].rate, before[0].rate);
    }

    /// A PLACE CAN BE TAKEN OFF, which is half of what the owner asked for and
    /// was not possible at all: the two the distro declares were fixed.
    #[test]
    fn a_place_can_be_removed_and_the_cursor_stays_inside() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let mut after = v.glance.clone();
        let before = v.glance.places().len();
        assert!(before >= 2, "the distro declares more than one place");
        v.cursor = at(&v, &Target::DropPlace(1));
        let Errand::Glance(change) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
        change.onto(&mut after);
        assert_eq!(after.places().len(), before - 1);
        assert!(after.chosen, "removing did not make the list one's own");
        // AND THE CURSOR IS STILL ON A ROW, which is what stops the screen
        // from going dead after the last place goes.
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        assert!(v.cursor() < rows.len());
    }

    /// AND ONE CAN BE ADDED — the other half. The row asks; the answer is
    /// typed on the door's own line, in either door, and reaches `took`.
    #[test]
    fn adding_a_place_asks_for_a_zone() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        v.cursor = at(&v, &Target::AddPlace);
        assert_eq!(v.press(Press::Enter, &z, &t), Errand::Ask(Asking::Place));
    }

    /// The zone row asks too, and it is a NAME rather than a ring: there are
    /// some four hundred of them.
    #[test]
    fn the_zone_row_asks_rather_than_cycling() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        v.cursor = at(&v, &Target::Zone);
        assert_eq!(v.press(Press::Enter, &z, &t), Errand::Ask(Asking::Zone));
    }

    /// Turning the weather on for a place with no coordinate ASKS for one
    /// rather than flipping a switch that would do nothing: there is no
    /// geocoder here, so a place somebody typed the name of has no position.
    #[test]
    fn the_weather_asks_where_a_new_place_is() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let mut places = v.glance.places();
        places.push(ronoctua_world::prefs::Wanted {
            zone: "Europe/Lisbon".to_owned(),
            ..ronoctua_world::prefs::Wanted::default()
        });
        let n = places.len() - 1;
        v.glance.places = places;
        v.glance.chosen = true;
        v.cursor = at(&v, &Target::Shows(n, Shown::Weather));
        assert_eq!(
            v.press(Press::Enter, &z, &t),
            Errand::Ask(Asking::Weather("Europe/Lisbon".to_owned()))
        );
    }

    /// FAILS IF: a place with no currencies turns its rate on without asking
    /// which two, or its currency row stops asking for the pair.
    #[test]
    fn the_rate_and_the_currency_row_ask_for_two_currencies() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let mut places = v.glance.places();
        places.push(ronoctua_world::prefs::Wanted {
            zone: "Europe/Lisbon".to_owned(),
            ..ronoctua_world::prefs::Wanted::default()
        });
        let n = places.len() - 1;
        v.glance.places = places;
        v.glance.chosen = true;
        v.cursor = at(&v, &Target::Shows(n, Shown::Rate));
        assert_eq!(
            v.press(Press::Enter, &z, &t),
            Errand::Ask(Asking::Rate("Europe/Lisbon".to_owned()))
        );
        v.cursor = at(&v, &Target::Currency(n));
        assert_eq!(
            v.press(Press::Enter, &z, &t),
            Errand::Ask(Asking::Currency("Europe/Lisbon".to_owned()))
        );
        // BOTH ARE ONE QUESTION, answered under one command in both doors.
        assert_eq!(Asking::Rate(String::new()).command(), "locale-currency");
        assert_eq!(Asking::Currency(String::new()).command(), "locale-currency");
        // AND THE ROW SAYS NONE IS CHOSEN, in words.
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        let row = rows
            .iter()
            .find(|r| r.target == Target::Currency(n))
            .expect("the currency row");
        let drawn: String = row.fields.iter().map(|f| f.text.as_str()).collect();
        assert!(
            drawn.contains(&t.t(crate::key::LOCALE_CURRENCY_NONE)),
            "{drawn}"
        );
    }

    /// FAILS IF: a pair the rates do not quote is taken, or a pair written
    /// any of the ways a person writes one is refused.
    #[test]
    fn a_currency_pair_is_two_codes_the_rates_know() {
        for said in ["EUR BRL", "eur brl", "EUR→BRL", "eur/brl", " EUR - BRL "] {
            assert_eq!(
                currency_pair(said),
                Some(("EUR".to_owned(), "BRL".to_owned())),
                "{said}"
            );
        }
        for said in ["EUR", "EUR EUR", "EUR XYZ", "EURO REAL", "EUR BRL GBP", ""] {
            assert_eq!(currency_pair(said), None, "{said}");
        }
    }

    /// EVERY host is NAMED on screen — the fallbacks too — and each source can
    /// be stopped.
    ///
    /// THE DEFECT THIS CATCHES: a source grew a second host and the screen
    /// went on naming the first as though it were the only one. Turning a
    /// source off is a decision about who is told this machine's address, and
    /// it cannot be made about a party the screen does not mention.
    #[test]
    fn the_two_hosts_are_named_and_each_can_be_stopped() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        let drawn: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        let mut named = 0;
        for source in Source::ALL {
            for who in source.from() {
                assert!(
                    drawn.contains(who.host),
                    "`{}` is not on the screen",
                    who.host
                );
                named += 1;
            }
        }
        assert!(named >= 4, "only {named} hosts were checked");
        v.cursor = at(&v, &Target::Ask(Source::Weather));
        let mut after = v.glance.clone();
        let Errand::Glance(change) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
        change.onto(&mut after);
        assert!(!after.ask_weather);
        assert!(after.ask_rate, "the other host went off too");
    }

    /// AN EXPLANATION WRAPS, and it used to be cut.
    ///
    /// THE DEFECT THIS CATCHES: `elide_end` on a sentence that says what a
    /// setting does and WHERE it takes effect. The half an ellipsis takes is
    /// always the end, which is where the consequence is — and the owner asked
    /// for them to break instead, in both doors.
    #[test]
    fn an_explanation_wraps_instead_of_being_cut() {
        let (t, z) = (texts(), utc());
        let v = view();
        // THE LONGEST NOTE ON THE SCREEN, and it is long on purpose: it is the
        // one that says what the machine's clock section reaches.
        let long = t.t(crate::key::LOCALE_MACHINE_DOES);
        assert!(layout::width(&long) > 80, "the fixture sentence is short");
        let mut most = 0;
        for columns in [60u16, 100, 200] {
            let rows = v.rows(columns, &z, &t);
            let drawn: Vec<String> = rows
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect();
            // NO ELLIPSIS ANYWHERE IN THE PROSE. The rows one stands on may
            // still elide a value; a note may not.
            let notes: Vec<&String> = drawn
                .iter()
                .filter(|s| long.starts_with(s.as_str()) || long.contains(s.as_str()))
                .collect();
            assert!(
                !notes.is_empty(),
                "at {columns} the sentence is not on screen"
            );
            for one in &notes {
                assert!(
                    !one.contains('\u{2026}'),
                    "at {columns} a note was cut: {one:?}"
                );
                assert!(
                    layout::width(one) <= columns,
                    "at {columns} a note is wider than the screen: {one:?}"
                );
            }
            most = most.max(notes.len());
        }
        assert!(most > 1, "the sentence never took more than one row");
    }

    /// AN ACTION HAS NO VALUE, and both of these carried one.
    ///
    /// THE DEFECT THIS CATCHES, and the owner named both: `Remove this place`
    /// carried the place's zone — which said that removing it was somehow
    /// about `Europe/London` — and `Add a place` carried the sentence *"a
    /// zone, such as Europe/Lisbon"*. Both read as the row's ANSWER, which is
    /// what that column is for everywhere else on this screen.
    #[test]
    fn an_action_row_has_no_value() {
        let (t, z) = (texts(), utc());
        let v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        let mut seen = 0;
        for row in &rows {
            if !matches!(row.target, Target::DropPlace(_) | Target::AddPlace) {
                continue;
            }
            assert_eq!(
                row.fields.len(),
                1,
                "an action row draws {} fields: {:?}",
                row.fields.len(),
                row.fields.iter().map(|f| &f.text).collect::<Vec<_>>()
            );
            seen += 1;
        }
        assert!(seen >= 2, "only {seen} action rows were found");
    }

    /// A PLACE'S ZONE IS BESIDE ITS NAME, quietly, and is nobody's value.
    #[test]
    fn a_place_carries_its_zone_beside_its_name() {
        let (t, z) = (texts(), utc());
        let v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        let zone = &v.glance.places()[0].zone;
        let heading = rows
            .iter()
            .find(|r| r.heading && r.fields.iter().any(|f| f.text.contains(zone)))
            .expect("no place heading carries its zone");
        assert_eq!(
            heading.fields.len(),
            2,
            "the zone is not a piece of its own"
        );
        assert!(heading.fields[1].text.starts_with('('));
        assert_eq!(heading.fields[1].color, tokens::color::TEXT_DIM);
        // AND IT IS SET IN, which is the other half of the three levels: a
        // sub-title at the section's own column is a sub-title nothing marks.
        assert!(heading.fields[0].at > rows[0].fields[0].at);
        // AND NO ROW HAS IT AS A VALUE.
        for row in &rows {
            if matches!(row.target, Target::DropPlace(_)) {
                assert!(
                    !row.fields.iter().any(|f| f.text.contains(zone)),
                    "the zone is still the value of a row"
                );
            }
        }
    }

    /// WHAT THE MACHINE IS ALREADY KEEPING CANNOT BE TYPED INTO, and the
    /// screen says so instead of leaving the rows out.
    ///
    /// THE DEFECT THIS CATCHES: offering to set a clock that a time client
    /// corrects every half hour. The daemon refuses; a row that let the key
    /// through would turn that refusal into the explanation.
    #[test]
    fn the_machine_refuses_what_it_is_already_keeping() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let kept = v.rows(DECIDING_WIDTH, &z, &t);
        for want in [Target::MachineDate, Target::MachineTime] {
            assert!(
                !kept.iter().any(|r| r.target == want),
                "{want:?} acts while the clock is kept automatically"
            );
        }
        // BUT THE ROWS ARE DRAWN, with the machine's own date on them.
        let said = t.t(crate::key::LOCALE_MACHINE_DATE);
        assert!(
            kept.iter().any(|r| r.fields.iter().any(|f| f.text == said)),
            "the machine's date is not on the screen at all"
        );
        assert!(
            kept.iter().any(|r| r.fields.iter().any(|f| {
                f.text == t.t(crate::key::LOCALE_MACHINE_KEPT)
                    || t.t(crate::key::LOCALE_MACHINE_KEPT).contains(&f.text)
            })),
            "nothing says why they cannot be set"
        );

        // AND WITH THE KEEPING OFF THEY ACT.
        v.machine.automatic = false;
        v.settle();
        let by_hand = v.rows(DECIDING_WIDTH, &z, &t);
        for want in [Target::MachineDate, Target::MachineTime] {
            assert!(
                by_hand.iter().any(|r| r.target == want),
                "{want:?} cannot be set even with the keeping off"
            );
        }
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    ///
    /// THE DEFECT THIS CATCHES: the two doors disagreed and neither kept the
    /// rule. The footer ran `RET` on the FIRST click — so one click on a row
    /// changed the language — and the window never ran it at all, which made
    /// every setting on this screen unreachable with a mouse.
    #[test]
    fn a_second_click_does_what_return_does() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        let row = rows
            .iter()
            .position(|r| r.target == Target::Clock)
            .expect("no clock row");
        let before = v.choice.clock;
        assert!(
            !v.clicked(row, &rows),
            "the first click was taken as a second"
        );
        assert_eq!(v.choice.clock, before, "the first click changed the value");
        assert!(
            v.clicked(row, &rows),
            "the second click on the same row was not"
        );
        let Errand::Keep(Setting::Clock(after)) = v.press(Press::Enter, &z, &t) else {
            panic!("the second click did not act");
        };
        assert_ne!(after, before);
        // A HEADING IS NOT A ROW TO CLICK, and it never becomes one.
        let title = rows.iter().position(|r| r.heading).expect("no heading");
        assert!(!v.clicked(title, &rows));
    }

    /// The cursor never lands on a heading or a note, and does not wrap.
    #[test]
    fn the_cursor_skips_what_cannot_be_acted_on() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        for _ in 0..rows.len() * 2 {
            v.press(Press::Down, &z, &t);
            let row = on(&v, &rows);
            assert!(!row.heading, "stopped on a heading");
            // AND THE DRAWN ROW IS THE SAME ROW. The two readings of one
            // cursor have to agree or the accent is on one line and the key
            // acts on another.
            assert_eq!(
                rows[v.drawn_row(&rows)].target,
                row.target,
                "the drawn row and the acted row are different rows"
            );
        }
        let bottom = v.cursor();
        v.press(Press::Down, &z, &t);
        assert_eq!(v.cursor(), bottom, "the list wrapped");
    }

    /// Noon and midnight are the two hours a bare `hour % 12` gets wrong.
    #[test]
    fn noon_and_midnight_are_twelve_and_not_zero() {
        let (t, z) = (texts(), utc());
        let midnight = crate::chrome::clock_text(0, &z, Clock::Twelve, &t);
        let noon = crate::chrome::clock_text(12 * 3600, &z, Clock::Twelve, &t);
        assert!(midnight.starts_with("12:00"), "midnight was {midnight}");
        assert!(noon.starts_with("12:00"), "noon was {noon}");
        assert_ne!(midnight, noon);
        assert_eq!(
            crate::chrome::clock_text(0, &z, Clock::TwentyFour, &t),
            "00:00"
        );
    }

    /// The three orders write three different dates, each with its named part
    /// in front.
    #[test]
    fn the_three_orders_write_three_different_dates() {
        let (t, z) = (texts(), utc());
        let secs = 1_788_000_000;
        let ymd = crate::chrome::date_text(secs, &z, Order::BigEndian, &t);
        let dmy = crate::chrome::date_text(secs, &z, Order::DayFirst, &t);
        let mdy = crate::chrome::date_text(secs, &z, Order::MonthFirst, &t);
        assert_ne!(ymd, dmy);
        assert_ne!(dmy, mdy);
        let c = ronoctua_files::when::civil(secs, &z);
        assert!(ymd.starts_with(&format!("{:04}", c.year)));
        assert!(dmy.starts_with(&format!("{:02}", c.day)));
        assert!(mdy.starts_with(&format!("{:02}", c.month)));
    }

    /// The decimal mark can be the language's OR a choice, and Fahrenheit is
    /// really the other number.
    #[test]
    fn the_two_that_used_to_be_facts_are_now_choices() {
        assert_eq!(Decimal::OfTheLanguage.mark(','), ',');
        assert_eq!(
            Decimal::Point.mark(','),
            '.',
            "the choice lost to the language"
        );
        assert_eq!(Decimal::Comma.mark('.'), ',');
        assert!((Units::Metric.of(15.0) - 15.0).abs() < 0.01);
        assert!((Units::Imperial.of(0.0) - 32.0).abs() < 0.01);
        assert!((Units::Imperial.of(100.0) - 212.0).abs() < 0.01);
    }

    /// Every id written to the file is read back as the same value.
    #[test]
    fn every_id_is_its_own_value_again() {
        for clock in Clock::all() {
            assert_eq!(Clock::by_id(clock.id()), Some(clock));
        }
        for order in Order::all() {
            assert_eq!(Order::by_id(order.id()), Some(order));
        }
        for decimal in Decimal::all() {
            assert_eq!(Decimal::by_id(decimal.id()), Some(decimal));
        }
        for units in Units::all() {
            assert_eq!(Units::by_id(units.id()), Some(units));
        }
    }

    /// The whole record round-trips, and a file that will not parse costs the
    /// preferences and says so.
    #[test]
    fn the_choice_survives_the_file() {
        let dir = std::env::temp_dir().join(format!("ronoctua-locale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(prefs::FILE);

        assert_eq!(prefs::read_at(&path), Choice::default());

        let mine = Choice {
            clock: Clock::Twelve,
            order: Order::MonthFirst,
            decimal: Decimal::Comma,
            units: Units::Imperial,
            zone: Some("Europe/Lisbon".to_owned()),
        };
        prefs::keep_in(&dir, &mine).unwrap();
        assert_eq!(prefs::read_at(&path), mine);

        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(prefs::read_at(&path), Choice::default());
    }

    /// FAILS IF: an answer of a ring this build does not know — a newer
    /// machine's — costs the other rings, or is erased when the person turns
    /// a DIFFERENT ring (the store puts an untouched repair back).
    #[test]
    fn an_unknown_ring_answer_is_kept_through_a_turn_of_another() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-locale-newer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(prefs::FILE);
        std::fs::write(
            &path,
            br#"{"clock": "36h", "date": "mdy", "units": "imperial", "shade": "warm"}"#,
        )
        .unwrap();
        let read = prefs::read_at(&path);
        assert_eq!(
            read.clock,
            Clock::default(),
            "an unknown answer is its default"
        );
        assert_eq!(read.order, Order::MonthFirst, "the other rings were lost");
        assert_eq!(read.units, Units::Imperial);
        // THE PERSON TURNS THE DECIMAL RING.
        let turned = Choice {
            decimal: Decimal::Comma,
            ..read
        };
        prefs::keep_in(&dir, &turned).unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw["clock"], "36h",
            "the untouched ring's answer was written over"
        );
        assert_eq!(
            raw["shade"], "warm",
            "a field this build does not know was lost"
        );
        assert_eq!(prefs::read_at(&path).decimal, Decimal::Comma);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A zone name that climbs out of the directory is refused before it
    /// becomes a path — the one other place a name becomes a path here states
    /// the same rule.
    #[test]
    fn a_zone_name_cannot_climb_out_of_its_directory() {
        for bad in [
            "",
            "..",
            "../../etc/passwd",
            "/etc/passwd",
            "Europe/../../x",
        ] {
            assert!(!a_real_zone(bad), "`{bad}` was accepted");
        }
    }

    /// `z`, `a` AND `c` ASK THEIR QUESTIONS — the three the hint line teaches.
    ///
    /// THE DEFECT THIS CATCHES: both doors drew the three letters on the hint
    /// line and neither answered them — not the key, not a click on the hint,
    /// which presses the key. `c` asks about the place UNDER THE CURSOR, and
    /// elsewhere it says why it cannot, rather than asking about place zero.
    #[test]
    fn z_a_and_c_ask_their_questions() {
        let (t, z) = (texts(), utc());
        let read = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        let mut v = view();
        assert_eq!(read("z"), Some(Press::Zone));
        assert_eq!(v.press(Press::Zone, &z, &t), Errand::Ask(Asking::Zone));
        assert_eq!(read("a"), Some(Press::Place));
        assert_eq!(v.press(Press::Place, &z, &t), Errand::Ask(Asking::Place));

        assert_eq!(read("c"), Some(Press::Where));
        v.cursor = at(&v, &Target::Clock);
        assert_eq!(
            v.press(Press::Where, &z, &t),
            Errand::Say(crate::key::LOCALE_NO_PLACE_HERE),
            "`c` away from any place asked about one"
        );
        let second = v.glance.places()[1].zone.clone();
        for target in [Target::Shows(1, Shown::Rate), Target::DropPlace(1)] {
            v.cursor = at(&v, &target);
            assert_eq!(
                v.press(Press::Where, &z, &t),
                Errand::Ask(Asking::Where(second.clone())),
                "`c` on {target:?} did not ask about the place it is on"
            );
        }
    }

    /// EVERY KEY THE TABLE TEACHES IS READ BACK AS ITS OWN PRESS, in both
    /// doors (they call this one reader), and a chord is not the letter under
    /// it.
    #[test]
    fn every_taught_key_is_read_back_as_a_press() {
        let read = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        for (taught, id) in OF_THE_VIEW {
            assert!(
                read(taught).is_some(),
                "`{taught}` ({id}) is taught and reads as nothing"
            );
        }
        assert_eq!(read("S-<left>"), Some(Press::Less));
        assert_eq!(read("S-<right>"), Some(Press::More));
        assert_eq!(read("<left>"), None, "a plain arrow turned a ring");
        assert_eq!(read("ESC"), Some(Press::Escape));
        assert_eq!(read("C-a"), None, "a chord was taken for its letter");
        assert_eq!(read("x"), None);
    }

    /// EVERY QUESTION IS A COMMAND WITH ITS OWN WORDS — the id both doors ask
    /// under, a registered command, and an `ask-` key in every language, so
    /// the line says a question and never a command's label or its id.
    ///
    /// THE DEFECT THIS CATCHES: the machine's date, time and zone were asked
    /// under ids no command had. The footer drew the raw id as the question,
    /// and the answer ran a command that did not exist — "no action".
    #[test]
    fn every_question_is_a_command_with_its_own_words() {
        let all = [
            Asking::Zone,
            Asking::Place,
            Asking::Where("Europe/London".to_owned()),
            Asking::Weather("Europe/London".to_owned()),
            Asking::MachineDate,
            Asking::MachineTime,
            Asking::MachineZone,
        ];
        for asking in &all {
            let id = asking.command();
            assert!(
                KEYS.commands.iter().any(|c| c.id == id),
                "`{id}` is not a command"
            );
            for language in ronoctua_i18n::LANGUAGES {
                let texts = ronoctua_i18n::Texts::fresh(language).expect(language);
                let line = crate::line::Line::new(id, false, None);
                assert_eq!(
                    line.question(&texts),
                    texts.t(&format!("ask-{id}")),
                    "`{id}` has no question of its own in {language}"
                );
            }
            // AND A COMMAND THAT NAMES ITS QUESTION ALONE IS READ BACK AS IT.
            if !matches!(asking, Asking::Where(_) | Asking::Weather(_)) {
                assert_eq!(Asking::named(id).as_ref(), Some(asking));
            }
        }
        assert_eq!(Asking::named("locale-where"), None, "a place needs naming");
    }

    /// WHAT IS TYPED IS CHECKED BEFORE ANYTHING IS WRITTEN, and nothing typed
    /// is never mind — the same answer from either door's line.
    #[test]
    fn an_answer_is_checked_before_it_is_kept_and_nothing_is_never_mind() {
        use crate::key;
        for asking in [Asking::Zone, Asking::MachineDate, Asking::Place] {
            assert_eq!(took(&asking, "   "), Ok(false), "{asking:?}");
        }
        let place = || Asking::Where("Europe/London".to_owned());
        for (asking, said, refused) in [
            (
                Asking::Zone,
                "No/Such_Zone_At_All",
                key::LOCALE_NO_SUCH_ZONE,
            ),
            (Asking::Place, "../../etc/passwd", key::LOCALE_NO_SUCH_ZONE),
            (place(), "north", key::LOCALE_NOT_A_COORDINATE),
            (Asking::MachineDate, "yesterday", key::LOCALE_NOT_A_DATE),
            (Asking::MachineTime, "25:00", key::LOCALE_NOT_A_TIME),
            (
                Asking::MachineZone,
                "No/Such_Zone_At_All",
                key::LOCALE_NO_SUCH_ZONE,
            ),
        ] {
            assert_eq!(
                took(&asking, said),
                Err(refused),
                "{asking:?} took `{said}`"
            );
        }
    }

    /// A SWITCH FLIPPED IN ONE DOOR KEEPS WHAT THE OTHER DOOR WROTE, and a
    /// place is found by its zone, never by its row.
    ///
    /// THE DEFECT THIS CATCHES: each door wrote its whole copy of the places.
    /// A place added in the footer vanished the moment a switch was flipped
    /// in a window opened before it — and the switch, named by its row, would
    /// have flipped whichever place had moved into that row.
    #[test]
    fn a_glance_change_is_laid_over_what_the_file_says_now() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-locale-glance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = ronoctua_store::Store::<ronoctua_world::prefs::Preferences>::at(
            Some(dir.clone()),
            ronoctua_world::prefs::FILE,
        );
        // THIS VIEW READ THE DISTRO'S PLACES; THE OTHER DOOR THEN ADDED
        // LISBON IN FRONT OF THEM.
        let (t, z) = (texts(), utc());
        let mut v = view();
        let london = v.glance.places()[0].clone();
        let mut theirs = ronoctua_world::prefs::Preferences::default();
        let mut places = theirs.places();
        places.insert(
            0,
            ronoctua_world::prefs::Wanted {
                zone: "Europe/Lisbon".to_owned(),
                ..ronoctua_world::prefs::Wanted::default()
            },
        );
        theirs.places = places;
        theirs.chosen = true;
        store.write(&theirs).unwrap();

        v.cursor = at(&v, &Target::Shows(0, Shown::Date));
        let Errand::Glance(change) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
        glance_change_in(&store, &change).expect("written");
        let now = store.read().places();
        assert!(
            now.iter().any(|p| p.zone == "Europe/Lisbon"),
            "the other door's place was undone"
        );
        let there = now.iter().find(|p| p.zone == london.zone).expect("london");
        assert_eq!(
            there.date, !london.date,
            "the switch did not reach its own place"
        );
        let lisbon = now.iter().find(|p| p.zone == "Europe/Lisbon").unwrap();
        assert!(
            !lisbon.date,
            "the switch flipped the place that moved into its row"
        );

        // AND A FILE THAT WILL NOT PARSE IS REFUSED AND LEFT AS IT IS: whoever
        // wrote over it would erase every place in it.
        let path = store.path().unwrap();
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(
            glance_change_in(&store, &change),
            Err(crate::key::LOCALE_NOT_WRITTEN)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A coordinate is two numbers in range, however they are separated.
    #[test]
    fn a_coordinate_is_two_numbers_in_range() {
        // The parser is exercised through the refusals, which is the half
        // that does not write a file.
        for bad in ["", "x", "51.5", "51.5 0.1 3", "91,0", "0,181", "a,b"] {
            assert_eq!(
                took_a_coordinate("Europe/London", bad, false),
                Err(crate::key::LOCALE_NOT_A_COORDINATE),
                "`{bad}` was accepted as a coordinate"
            );
        }
    }

    /// THE CLOCK'S SWITCH IS ONE KNOB OF THE MODULE, and a system that
    /// switched the module off leaves no switch here.
    ///
    /// THE DEFECT THIS CATCHES: the whole preferences sent back (`Tune`),
    /// built from the shared default — a six-hour watch switched off and on
    /// from this page came back weekly with a minute of countdown — and a row
    /// that offered a switch the brain was going to refuse.
    #[test]
    fn the_clocks_switch_is_one_knob_and_the_systems_off_is_said() {
        use ronoctua_brain::wire::{Ask, Knob};
        match the_clocks_switch(false) {
            Ask::Set { task, knob } => {
                assert_eq!(task, ronoctua_brain::compiled::clock_sync::ID);
                assert_eq!(knob, Knob::Enabled(false));
            }
            other => panic!("the switch sends {other:?}"),
        }
        assert!(matches!(
            the_clocks_switch(true),
            Ask::Set {
                knob: Knob::Enabled(true),
                ..
            }
        ));
        assert_eq!(watched_of(true, ""), Watched::Set(true));
        assert_eq!(watched_of(false, "off"), Watched::Set(false));
        assert_eq!(
            watched_of(false, ronoctua_brain::registry::SYSTEM_OFF),
            Watched::BySystem
        );

        // AND THE ROW: on, the switch; switched off by the system, "off" with
        // the reason and nothing to press.
        let t = texts();
        let words = |row: &Row| -> String {
            row.fields
                .iter()
                .map(|f| f.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let label = t.t(crate::key::LOCALE_WATCHED);
        let mut v = view();
        let on = v.rows(DECIDING_WIDTH, &utc(), &t);
        let row = on
            .iter()
            .find(|r| words(r).contains(&label))
            .expect("the watch's row");
        assert_eq!(row.target, Target::Watched);
        v.watched = Some(Watched::BySystem);
        let off = v.rows(DECIDING_WIDTH, &utc(), &t);
        let row = off
            .iter()
            .find(|r| words(r).contains(&label))
            .expect("the watch's row");
        assert_eq!(row.target, Target::Nothing, "a switch the brain refuses");
        assert!(words(row).contains(&t.t(crate::key::LOCALE_AUTOMATIC_OFF)));
        // THE REASON, in a note that may wrap: its first words are enough.
        let said = t.t(crate::key::LOCALE_WATCHED_SYSTEM_OFF);
        let first: String = said
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            off.iter().any(|r| words(r).contains(&first)),
            "the system's reason is not on the page"
        );

        // NOT KNOWN YET — the window asked, off its loop — is not known, with
        // no switch, and not "the brain is not running", which it may be.
        assert!(v.set_watched(Some(Watched::Asking)));
        assert!(
            !v.set_watched(Some(Watched::Asking)),
            "the same answer moved the view"
        );
        let asking = v.rows(DECIDING_WIDTH, &utc(), &t);
        let row = asking
            .iter()
            .find(|r| words(r).contains(&label))
            .expect("the watch's row");
        assert_eq!(row.target, Target::Nothing, "a switch before the answer");
        let gone: String = t
            .t(crate::key::LOCALE_WATCHED_UNKNOWN)
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            !asking.iter().any(|r| words(r).contains(&gone)),
            "a brain on its way was said to be absent"
        );
        // AND NO MIND IS A BRAIN THAT COULD NOT BE ASKED.
        assert_eq!(watched_in(None), None);
        assert_eq!(watched_in(Some(&ronoctua_brain::Mind::default())), None);
    }
}
