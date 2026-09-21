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

use crate::files::{Field, Hit as FilesHit, Ink};

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
    // nothing is typed into this screen, so a letter is not a query. They are
    // the three questions, reachable without walking to their row — and it is
    // also what makes them reachable at all, since a command the keymap
    // cannot get to is a command `M-x` alone knows about.
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
    /// Close.
    Escape,
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

    /// Flips it on a place.
    pub const fn flip(self, place: &mut ronoctua_world::prefs::Wanted) {
        match self {
            Self::Clock => place.clock = !place.clock,
            Self::Date => place.date = !place.date,
            Self::Weather => place.weather = !place.weather,
            Self::Rate => place.rate = !place.rate,
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

/// Twelve hours or twenty-four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clock {
    /// `14:32`. The default, and what both declared languages use.
    #[default]
    TwentyFour,
    /// `2:32 pm`.
    Twelve,
}

impl Clock {
    /// The id, as it is written in the file and in the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TwentyFour => "24h",
            Self::Twelve => "12h",
        }
    }

    /// The two, in the order they are offered.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::TwentyFour, Self::Twelve].into_iter()
    }

    /// The one this id names, if any.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::all().find(|c| c.id() == id)
    }
}

/// The order the parts of a date are written in.
///
/// # Why this is a preference and not the language's
///
/// Because it was the language's, and both languages spelt it the same way:
/// `files-when` is `{ $y }-{ $m }-{ $d }` in `en-GB.ftl` and in `pt-BR.ftl`.
/// A Brazilian reading `2026-09-08` is reading an ISO date, not a Portuguese
/// one, and the key had no way of saying which was meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    /// `2026-09-08`. Sorts as text, which is why it is the default.
    #[default]
    BigEndian,
    /// `08/09/2026`.
    DayFirst,
    /// `09/08/2026`.
    MonthFirst,
}

impl Order {
    /// The id, as written in the file and in the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BigEndian => "ymd",
            Self::DayFirst => "dmy",
            Self::MonthFirst => "mdy",
        }
    }

    /// The three, in the order they are offered.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::BigEndian, Self::DayFirst, Self::MonthFirst].into_iter()
    }

    /// The one this id names, if any.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::all().find(|o| o.id() == id)
    }
}

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
/// `PartialEq` and NOT `Eq`: a place carries a latitude, which is an `f64`.
/// Nothing here compares two glances for equality — the derive is for the
/// tests — and claiming a total order over a float would be a claim the type
/// cannot keep.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Speak this language everywhere: write `language.json`, which every
    /// door watches.
    Speak(&'static str),
    /// Write the pair of date preferences.
    Keep(Choice),
    /// Write the glance's places and switches.
    Glance(Box<ronoctua_world::prefs::Preferences>),
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
    /// Something has to be typed, and the footer is where.
    Ask(Asking),
    /// Close the view.
    Close,
}

/// What the person chose about dates and times.
///
/// One record and not two fields on the view, because it is what is WRITTEN:
/// a writer that carried one of the two would take the other back to its
/// default every time it saved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Choice {
    /// Twelve hours or twenty-four.
    pub clock: Clock,
    /// The order of the parts of a date.
    pub order: Order,
    /// What separates a whole number from its fraction.
    pub decimal: Decimal,
    /// Metric or the other one.
    pub units: Units,
    /// The zone this interface shows times in.
    ///
    /// `None` is the machine's own — `/etc/localtime`, which the system
    /// declares. It is a PREFERENCE over that and not a replacement for it:
    /// `time.timeZone` is what the machine's logs, its cron and every other
    /// program use, and changing that is a rebuild (invariant 6). What this
    /// changes is the zone THIS INTERFACE reads a timestamp in, which is the
    /// question somebody sitting in another country is actually asking.
    pub zone: Option<String>,
}

/// What separates a whole number from its fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Decimal {
    /// The language's own, which is what it always was.
    #[default]
    OfTheLanguage,
    /// A full stop, whatever the language.
    Point,
    /// A comma, whatever the language.
    Comma,
}

impl Decimal {
    /// The id, as written in the file and in the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::OfTheLanguage => "language",
            Self::Point => "point",
            Self::Comma => "comma",
        }
    }

    /// The three, in the order the ring turns.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::OfTheLanguage, Self::Point, Self::Comma].into_iter()
    }

    /// The one this id names, if any.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::all().find(|d| d.id() == id)
    }

    /// The character, given what the language would have said.
    #[must_use]
    pub const fn mark(self, of_the_language: char) -> char {
        match self {
            Self::OfTheLanguage => of_the_language,
            Self::Point => '.',
            Self::Comma => ',',
        }
    }
}

/// Metric, or the other one.
///
/// # Why this is a choice at all
///
/// `LC_MEASUREMENT` is pinned to `en_GB` in `nix/modules/base.nix` and NOT as
/// a `mkDefault` — "choosing `en_US` in the installer chooses a language,
/// never inches". That decision stands and this does not touch it: it is what
/// the GLANCE draws a temperature in, which is the one number this interface
/// produces that has a unit at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Units {
    /// Degrees Celsius.
    #[default]
    Metric,
    /// Degrees Fahrenheit.
    Imperial,
}

impl Units {
    /// The id, as written in the file and in the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }

    /// The two, in the order the ring turns.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::Metric, Self::Imperial].into_iter()
    }

    /// The one this id names, if any.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::all().find(|u| u.id() == id)
    }

    /// Celsius, as this asks for it.
    #[must_use]
    pub fn of(self, celsius: f64) -> f64 {
        match self {
            Self::Metric => celsius,
            Self::Imperial => celsius.mul_add(9.0 / 5.0, 32.0),
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
    watched: Option<bool>,
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
            watched: watched_by_the_brain(),
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

    /// Re-reads the files. Called when a watch rings.
    pub fn reread(&mut self) {
        let (speaking, from) = crate::language::resolve();
        self.speaking = speaking;
        self.from = Some(from);
        self.choice = prefs::read();
        self.glance = ronoctua_world::prefs::store().read();
        self.world = ronoctua_world::store::store().read();
        self.machine = machine_clock();
        self.machine_zone = zone_named(&self.machine.zone);
        self.watched = watched_by_the_brain();
        // THE ROWS MAY HAVE CHANGED SHAPE — a place added or removed by the
        // other door — so the cursor is brought back inside them.
        self.settle();
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
        use ronoctua_i18n::key;
        let (left, room) = crate::power::extent(columns);
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

        // 5. THE GLANCE'S PLACES — `LEADER d`, and what each of them shows.
        b.heading(key::LOCALE_HEADING_GLANCE);
        b.note_with(
            key::LOCALE_GLANCE_DOES,
            &[(
                "keys",
                &format!("{} {}", crate::leader(), crate::leader::GLANCE_KEYS),
            )],
        );
        let places = self.glance.places();
        if places.is_empty() {
            b.note_with(
                key::LOCALE_NO_PLACES,
                &[(
                    "keys",
                    &format!("{} {}", crate::leader(), crate::leader::GLANCE_KEYS),
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
        use ronoctua_i18n::key;
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
                Some(true) => key::LOCALE_AUTOMATIC_ON,
                Some(false) => key::LOCALE_AUTOMATIC_OFF,
                None => key::LOCALE_ZONE_UNKNOWN,
            }),
            if self.watched.is_some() {
                Target::Watched
            } else {
                Target::Nothing
            },
        );
        b.note(if self.watched.is_some() {
            key::LOCALE_WATCHED_DOES
        } else {
            key::LOCALE_WATCHED_UNKNOWN
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
            Target::Clock => {
                self.choice.clock = turn_of(Clock::all(), self.choice.clock, step);
                Errand::Keep(self.choice.clone())
            }
            Target::Order => {
                self.choice.order = turn_of(Order::all(), self.choice.order, step);
                Errand::Keep(self.choice.clone())
            }
            Target::Decimal => {
                self.choice.decimal = turn_of(Decimal::all(), self.choice.decimal, step);
                Errand::Keep(self.choice.clone())
            }
            Target::Units => {
                self.choice.units = turn_of(Units::all(), self.choice.units, step);
                Errand::Keep(self.choice.clone())
            }
            Target::Zone => Errand::Ask(Asking::Zone),
            Target::AddPlace => Errand::Ask(Asking::Place),
            Target::DropPlace(n) => {
                let mut places = self.glance.places();
                if n < places.len() {
                    places.remove(n);
                }
                self.glance.places = places;
                self.glance.chosen = true;
                // THE CURSOR COMES BACK INSIDE THE LIST. Removing the last
                // place leaves it past the end, and a cursor past the end is
                // a screen where nothing is focused and every key does
                // nothing — which is the defect this whole rebuild started
                // from.
                self.cursor = self.cursor.saturating_sub(1);
                Errand::Glance(Box::new(self.glance.clone()))
            }
            Target::Shows(n, shown) => {
                let mut places = self.glance.places();
                // A COORDINATE IS WHAT THE WEATHER NEEDS, and a place somebody
                // typed the name of has none. Asking is better than a switch
                // that flips and does nothing.
                if shown == Shown::Weather
                    && places
                        .get(n)
                        .is_some_and(|p| !p.weather && !p.has_a_coordinate())
                {
                    return Errand::Ask(Asking::Where(n));
                }
                if let Some(place) = places.get_mut(n) {
                    shown.flip(place);
                }
                self.glance.places = places;
                self.glance.chosen = true;
                Errand::Glance(Box::new(self.glance.clone()))
            }
            Target::Ask(source) => {
                source.flip(&mut self.glance);
                Errand::Glance(Box::new(self.glance.clone()))
            }
            // THE MACHINE'S HALF. Every one of these is the DOOR's work: what
            // is on the other side is a system bus or the brain's socket, and
            // a view drawn by two doors holds neither.
            Target::Automatic => Errand::Automatic(!self.machine.automatic),
            Target::Watched => Errand::Watch(!self.watched.unwrap_or(true)),
            Target::MachineDate => Errand::Ask(Asking::MachineDate),
            Target::MachineTime => Errand::Ask(Asking::MachineTime),
            Target::MachineZone => Errand::Ask(Asking::MachineZone),
            Target::Nothing => Errand::Nothing,
        }
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

    /// Where the cursor is, counted over the rows one can stand on.
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
        return Err(ronoctua_i18n::key::LOCALE_NO_SUCH_ZONE);
    }
    ronoctua_clock::set_zone(said).map_err(|e| {
        tracing::warn!(error = %e, "the machine's zone could not be set");
        ronoctua_i18n::key::LOCALE_CLOCK_REFUSED
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
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_DATE);
    };
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<i64>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_DATE);
    };
    let now = machine_civil();
    set_the_clock(
        (year, month, day),
        (now.hour, now.minute, now.second),
        ronoctua_i18n::key::LOCALE_NOT_A_DATE,
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
        _ => return Err(ronoctua_i18n::key::LOCALE_NOT_A_TIME),
    };
    let (Ok(hour), Ok(minute), Ok(second)) =
        (hh.parse::<u32>(), mm.parse::<u32>(), ss.parse::<u32>())
    else {
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_TIME);
    };
    if hour > 23 || minute > 59 || second > 59 {
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_TIME);
    }
    let now = machine_civil();
    set_the_clock(
        (now.year, now.month, now.day),
        (hour, minute, second),
        ronoctua_i18n::key::LOCALE_NOT_A_TIME,
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
        ronoctua_i18n::key::LOCALE_CLOCK_REFUSED
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

/// Turns the brain's clock watch on, or off. Answers whether it was done.
///
/// # Why the WHOLE preferences go back
///
/// Because that is the wire this daemon has: `Ask::Tune` carries every
/// preference, so there is one shape to validate instead of one message per
/// field. The screen has the mind in front of it and sends back what it wants
/// — and the DAEMON is what writes the file, which is what keeps this switch
/// and the task's own page from being two writers of one truth.
#[must_use]
pub fn watch_the_clock(on: bool) -> bool {
    use ronoctua_brain::wire::{Ask, Say};
    let Ok(Say::Mind { mind }) = ronoctua_brain::client::ask(&Ask::Look) else {
        return false;
    };
    let mut prefs = mind.prefs.clone();
    let task = prefs
        .tasks
        .entry(ronoctua_brain::tasks::clock::ID.to_owned())
        .or_default();
    task.enabled = on;
    ronoctua_brain::client::tell(&Ask::Tune {
        prefs: Box::new(prefs),
    })
    .map_err(|e| tracing::warn!(error = %e, "the clock watch could not be switched"))
    .is_ok()
}

/// Is the brain's clock watch on? `None` when the brain could not be asked.
///
/// ONE TRUTH, TWO DOORS: the switch is the task's own `enabled`, which the
/// daemon owns and writes — so this asks it rather than keeping a copy, and
/// the task's page in the brain's buffer flips the very same field.
fn watched_by_the_brain() -> Option<bool> {
    let said = ronoctua_brain::client::ask(&ronoctua_brain::wire::Ask::Look).ok()?;
    let ronoctua_brain::wire::Say::Mind { mind } = said else {
        return None;
    };
    Some(
        mind.prefs
            .tasks
            .get(ronoctua_brain::tasks::clock::ID)
            .is_none_or(|t| t.enabled),
    )
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
        return Err(ronoctua_i18n::key::LOCALE_NO_SUCH_ZONE);
    }
    let mut choice = prefs::read();
    choice.zone = Some(said.to_owned());
    prefs::keep(&choice).map_err(|e| {
        tracing::warn!(error = %e, "the zone could not be written");
        ronoctua_i18n::key::LOCALE_NOT_WRITTEN
    })
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
        return Err(ronoctua_i18n::key::LOCALE_NO_SUCH_ZONE);
    }
    let store = ronoctua_world::prefs::store();
    let mut prefs = store.read();
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
        ronoctua_i18n::key::LOCALE_NOT_WRITTEN
    })
}

/// Where a place is, so it can have weather.
///
/// # Errors
///
/// The text key of what is wrong with it.
pub fn took_a_coordinate(n: usize, said: &str) -> Result<(), &'static str> {
    let numbers: Vec<f64> = said
        .split([',', ' ', ';'])
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.trim().parse::<f64>().ok())
        .collect();
    let [latitude, longitude] = numbers[..] else {
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_COORDINATE);
    };
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_COORDINATE);
    }
    let store = ronoctua_world::prefs::store();
    let mut prefs = store.read();
    let mut places = prefs.places();
    let Some(place) = places.get_mut(n) else {
        return Err(ronoctua_i18n::key::LOCALE_NOT_A_COORDINATE);
    };
    place.latitude = latitude;
    place.longitude = longitude;
    // AND THE SWITCH GOES ON, because turning it on is what asked this.
    place.weather = true;
    prefs.places = places;
    prefs.chosen = true;
    store.write(&prefs).map_err(|e| {
        tracing::warn!(error = %e, "the coordinate could not be written");
        ronoctua_i18n::key::LOCALE_NOT_WRITTEN
    })
}

/// Writes the glance's places and switches.
///
/// # Errors
///
/// There is no configuration directory, or it cannot be written.
pub fn glance_keep(prefs: &ronoctua_world::prefs::Preferences) -> std::io::Result<()> {
    ronoctua_world::prefs::store().write(prefs)
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

/// What the view needs typed, and the footer is where typing happens.
///
/// A window has no minibuffer — "senha e caminho se digitam no RODAPÉ, e a
/// janela DIZ isso" — so a window that reaches one of these says where to go
/// rather than opening a prompt of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asking {
    /// A zone for this interface's clock.
    Zone,
    /// A zone for a new place on the glance.
    Place,
    /// A coordinate for a place that has none, so it can have weather.
    Where(usize),
    /// A date for the MACHINE's clock.
    MachineDate,
    /// A time for the MACHINE's clock.
    MachineTime,
    /// A zone for the MACHINE.
    MachineZone,
}

/// Where the choices live on disk.
pub mod prefs {
    use super::{Choice, Clock, Decimal, Order, Units};

    /// The file, under `$XDG_CONFIG_HOME/ronoctua`.
    pub const FILE: &str = "locale.json";

    /// What this process is currently writing dates and times with.
    ///
    /// # Why a process-wide value and not a parameter
    ///
    /// Because it is the shape the THEME already has, and for the same
    /// reason. A `Color` is a role that resolves through the current theme at
    /// the moment it is drawn; a date is a shape that resolves through the
    /// current preference at the moment it is written. Threading either one
    /// through every caller would put the same argument in five signatures —
    /// and, the day a sixth appears, in none.
    ///
    /// It is read on every date drawn, so it is a lock held for the length of
    /// a clone of five small fields, never a file.
    static CURRENT: std::sync::RwLock<Option<Choice>> = std::sync::RwLock::new(None);

    /// What dates are being written with right now.
    #[must_use]
    pub fn current() -> Choice {
        CURRENT.read().map_or_else(
            |poisoned| poisoned.into_inner().clone().unwrap_or_default(),
            |guard| guard.clone().unwrap_or_default(),
        )
    }

    /// Re-reads the file into [`current`]. Called at start and when the watch
    /// rings — the same two moments the theme is adopted.
    pub fn adopt() {
        let choice = read();
        if let Ok(mut guard) = CURRENT.write() {
            *guard = Some(choice);
        }
    }

    /// Sets [`current`] directly. For tests, and for a door that has just
    /// written the file and should not race its own watch.
    pub fn set(choice: Choice) {
        if let Ok(mut guard) = CURRENT.write() {
            *guard = Some(choice);
        }
    }

    /// What the file says, or the defaults.
    ///
    /// It never fails: a file that will not parse costs the two preferences in
    /// it and says so at `warn`. It is the READER's half of the rule — a
    /// writer would have to refuse instead, and [`keep`] does not read.
    #[must_use]
    pub fn read() -> Choice {
        let Some(dir) = ronoctua_store::config_dir() else {
            return Choice::default();
        };
        read_at(&dir.join(FILE))
    }

    /// [`read`], with its path as a parameter.
    #[must_use]
    pub fn read_at(path: &std::path::Path) -> Choice {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Choice::default(),
            Err(e) => {
                tracing::warn!(error = %e, "the locale file could not be read");
                return Choice::default();
            }
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
            tracing::warn!("the locale file will not parse; the dates are back at their defaults");
            return Choice::default();
        };
        let field = |name: &str| {
            parsed
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Choice {
            clock: field("clock")
                .and_then(|s| Clock::by_id(&s))
                .unwrap_or_default(),
            order: field("date")
                .and_then(|s| Order::by_id(&s))
                .unwrap_or_default(),
            decimal: field("decimal")
                .and_then(|s| Decimal::by_id(&s))
                .unwrap_or_default(),
            units: field("units")
                .and_then(|s| Units::by_id(&s))
                .unwrap_or_default(),
            // A ZONE IS A NAME AND NOT A CHOICE OF FEW, so it is kept as it
            // was written and checked when it is READ — a zone that has gone
            // from the tzdata should say so on the screen, not vanish out of
            // the file.
            zone: field("zone").filter(|z| !z.is_empty()),
        }
    }

    /// Writes the pair, atomically.
    ///
    /// # Errors
    ///
    /// There is no configuration directory, or it cannot be written.
    pub fn keep(choice: &Choice) -> std::io::Result<()> {
        let dir = ronoctua_store::config_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no configuration directory")
        })?;
        keep_in(&dir, choice)
    }

    /// [`keep`], with its root as a parameter.
    ///
    /// # Errors
    ///
    /// The directory cannot be written.
    pub fn keep_in(dir: &std::path::Path, choice: &Choice) -> std::io::Result<()> {
        // THE WHOLE RECORD, both fields. Writing one would take the other back
        // to its default, which is the defect `display.json`'s merge exists
        // because of — here it is cheap to avoid, because there are two.
        let text = serde_json::json!({
            "clock": choice.clock.id(),
            "date": choice.order.id(),
            "decimal": choice.decimal.id(),
            "units": choice.units.id(),
            "zone": choice.zone.clone().unwrap_or_default(),
        })
        .to_string();
        ronoctua_xdg::write_atomically(dir, FILE, text.as_bytes())
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
                tokens::color::TEXT_GROUP,
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
        for line in layout::wrap(&text, room, SENTENCE_LINES) {
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
                if focused {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_LABEL_ACTS
                },
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
        let (label_colour, value_colour) = if focused {
            // AND UNDER THE CURSOR IT STILL SAYS SO — the label carries "the
            // keyboard is here" and the value keeps saying whether the answer
            // can be changed. See `colors.nix`, `text.here`.
            (
                tokens::color::STATE_ACCENT,
                if target == Target::Nothing {
                    tokens::color::TEXT_HERE_READ
                } else {
                    tokens::color::TEXT_HERE_ACTS
                },
            )
        } else {
            (
                if target == Target::Nothing {
                    tokens::color::TEXT_LABEL_FACT
                } else {
                    tokens::color::TEXT_LABEL_ACTS
                },
                // AND THE VALUE SAYS THE SAME FROM THE OTHER SIDE, which this
                // view alone was not doing: it drew every value in the body
                // text's tone, so a fact and a door looked alike on the half
                // of the line the eye goes to. Every other view of this shape
                // already asked the question; this one now asks it too.
                if target == Target::Nothing {
                    tokens::color::TEXT_READ_STRONG
                } else {
                    tokens::color::TEXT_ANSWER
                },
            )
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left + self.nest,
                self.room.saturating_sub(self.nest),
                &label,
                &value,
                label_colour,
                value_colour,
            ),
            focused,
            heading: false,
            target,
        });
    }
}

/// How many lines one note may wrap to.
///
/// A bound on WORK and not on prose: `layout::wrap` walks the whole string
/// whatever happens, and this is what stops a sentence somebody pastes into a
/// `.ftl` from becoming a page. Eight is more than any of these needs at the
/// narrowest width the footer's region is ever drawn at.
const SENTENCE_LINES: usize = 8;

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
        let mut me = View {
            cursor: 0,
            choice: Choice::default(),
            speaking: ronoctua_i18n::DEFAULT.to_owned(),
            glance: ronoctua_world::prefs::Preferences::default(),
            world: ronoctua_world::Cache::default(),
            // A PLANTED MACHINE, and not this one's: a test that read the
            // developer's own timedated would draw a different number of rows
            // on a machine whose clock is kept by hand, and its assertions
            // would pass or fail by where it ran.
            machine: ronoctua_clock::Standing {
                reachable: true,
                zone: "Europe/London".to_owned(),
                automatic: true,
                can_be_automatic: true,
                synchronised: true,
                server: "3.pool.ntp.org".to_owned(),
                now: 1_788_000_000,
            },
            machine_zone: ronoctua_files::when::Zone::utc(),
            watched: Some(true),
            from: Some(crate::language::Source::Default),
        };
        me.settle();
        me
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

        let Errand::Keep(one) = v.press(Press::More, &z, &t) else {
            panic!("turning the clock did not ask for it to be kept");
        };
        assert_ne!(one.clock, start, "the ring did not turn");

        // ROUND, for a ring of two: one more press is back where it began.
        let Errand::Keep(two) = v.press(Press::More, &z, &t) else {
            panic!("no errand");
        };
        assert_eq!(two.clock, start, "the ring stopped at the end");

        // AND THE OTHER WAY.
        let Errand::Keep(back) = v.press(Press::Less, &z, &t) else {
            panic!("no errand");
        };
        assert_eq!(back.clock, one.clock);
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
        let Errand::Keep(by_enter) = a.press(Press::Enter, &z, &t) else {
            panic!("no errand");
        };
        let Errand::Keep(by_more) = b.press(Press::More, &z, &t) else {
            panic!("no errand");
        };
        assert_eq!(by_enter.order, by_more.order);
    }

    /// Turning one ring leaves every other value alone — the `display.json`
    /// merge defect, in the small.
    #[test]
    fn turning_one_ring_keeps_the_rest() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        v.cursor = at(&v, &Target::Decimal);
        let before = v.choice.clone();
        let Errand::Keep(after) = v.press(Press::More, &z, &t) else {
            panic!("no errand");
        };
        assert_ne!(after.decimal, before.decimal);
        assert_eq!(after.clock, before.clock);
        assert_eq!(after.order, before.order);
        assert_eq!(after.units, before.units);
        assert_eq!(after.zone, before.zone);
    }

    /// The four switches of a place are INDEPENDENT, and each flips only
    /// itself — the owner's "choose exactly which information appears".
    #[test]
    fn each_of_the_four_switches_flips_only_itself() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let before = v.glance.places();
        assert!(!before.is_empty());
        v.cursor = at(&v, &Target::Shows(0, Shown::Date));
        let Errand::Glance(after) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
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
        let before = v.glance.places().len();
        assert!(before >= 2, "the distro declares more than one place");
        v.cursor = at(&v, &Target::DropPlace(1));
        let Errand::Glance(after) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
        assert_eq!(after.places().len(), before - 1);
        assert!(after.chosen, "removing did not make the list one's own");
        // AND THE CURSOR IS STILL ON A ROW, which is what stops the screen
        // from going dead after the last place goes.
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        assert!(v.cursor() < rows.len());
    }

    /// AND ONE CAN BE ADDED — the other half. The row asks; the answer is
    /// typed in the footer, which is where typing happens.
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
        assert_eq!(v.press(Press::Enter, &z, &t), Errand::Ask(Asking::Where(n)));
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
        let Errand::Glance(after) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
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
        let long = t.t(ronoctua_i18n::key::LOCALE_MACHINE_DOES);
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
        let said = t.t(ronoctua_i18n::key::LOCALE_MACHINE_DATE);
        assert!(
            kept.iter().any(|r| r.fields.iter().any(|f| f.text == said)),
            "the machine's date is not on the screen at all"
        );
        assert!(
            kept.iter().any(|r| r.fields.iter().any(|f| {
                f.text == t.t(ronoctua_i18n::key::LOCALE_MACHINE_KEPT)
                    || t.t(ronoctua_i18n::key::LOCALE_MACHINE_KEPT)
                        .contains(&f.text)
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
        let Errand::Keep(after) = v.press(Press::Enter, &z, &t) else {
            panic!("the second click did not act");
        };
        assert_ne!(after.clock, before);
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

    /// A coordinate is two numbers in range, however they are separated.
    #[test]
    fn a_coordinate_is_two_numbers_in_range() {
        // The parser is exercised through the refusals, which is the half
        // that does not write a file.
        for bad in ["", "x", "51.5", "51.5 0.1 3", "91,0", "0,181", "a,b"] {
            assert_eq!(
                took_a_coordinate(0, bad),
                Err(ronoctua_i18n::key::LOCALE_NOT_A_COORDINATE),
                "`{bad}` was accepted as a coordinate"
            );
        }
    }
}
