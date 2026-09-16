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
    Zone,
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

/// One of the two hosts the glance can be told to ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The weather host.
    Weather,
    /// The exchange-rate host.
    Rate,
}

impl Source {
    /// The host's name, as `assets/world.nix` declares it. It is DRAWN, so
    /// that turning it off is a decision about a named party rather than
    /// about an abstraction.
    #[must_use]
    pub const fn host(self) -> &'static str {
        match self {
            Self::Weather => ronoctua_world::WEATHER_HOST,
            Self::Rate => ronoctua_world::RATE_HOST,
        }
    }

    /// Is it being asked?
    #[must_use]
    pub const fn asked(self, prefs: &ronoctua_world::prefs::Preferences) -> bool {
        match self {
            Self::Weather => prefs.ask_weather,
            Self::Rate => prefs.ask_rate,
        }
    }

    /// Flips it.
    pub const fn flip(self, prefs: &mut ronoctua_world::prefs::Preferences) {
        match self {
            Self::Weather => prefs.ask_weather = !prefs.ask_weather,
            Self::Rate => prefs.ask_rate = !prefs.ask_rate,
        }
    }
}

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
/// It is only honest while the row COUNT and the row TARGETS do not depend on
/// the width, and they do not: the width is reached in `elide_end` and in a
/// field's column, both of which are about drawing. A test walks several
/// widths and compares the targets rather than trusting that.
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
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        self.cursor = self.cursor.min(rows.len() - 1);
        if rows[self.cursor].heading {
            // FORWARD FIRST, because a heading is followed by its section.
            if let Some(n) = rows.iter().skip(self.cursor).position(|r| !r.heading) {
                self.cursor += n;
            } else if let Some(n) = rows.iter().rposition(|r| !r.heading) {
                self.cursor = n;
            }
        }
    }

    /// Re-reads the files. Called when a watch rings.
    pub fn reread(&mut self) {
        let (speaking, from) = crate::language::resolve();
        self.speaking = speaking;
        self.from = Some(from);
        self.choice = prefs::read();
        self.glance = ronoctua_world::prefs::store().read();
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
            cursor: self.cursor,
            texts,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));

        // 1. THE LANGUAGE — one row, and its value is the language's own name.
        b.heading(key::LOCALE_HEADING_LANGUAGE);
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
        b.pair(
            texts.t(key::LOCALE_ZONE),
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

        // 4. THE GLANCE'S PLACES — `LEADER d`, and what each of them shows.
        b.heading(key::LOCALE_HEADING_PLACES);
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
            b.heading_of(if place.key.is_empty() {
                place.zone.clone()
            } else {
                texts.t(&place.key)
            });
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
            b.pair(
                texts.t(key::LOCALE_DROP_PLACE),
                place.zone.clone(),
                Target::DropPlace(n),
            );
        }
        // AND THE WAY TO ADD ONE, at the end of the list rather than the top:
        // it is the row somebody reaches after reading what is already there.
        //
        // WITH AIR IN FRONT, because without it the row sits flush under the
        // last place's own rows and reads as one more of them — `chrome::AIR`
        // is what says "this belongs to the section, not to the thing above".
        b.air();
        b.pair(
            texts.t(key::LOCALE_ADD_PLACE),
            texts.t(key::LOCALE_ADD_PLACE_HOW),
            Target::AddPlace,
        );

        // 5. AND WHO IS ASKED. Named, because turning a source off is a
        //    decision about a party rather than about an abstraction.
        b.heading(key::LOCALE_HEADING_SOURCES);
        for source in [Source::Weather, Source::Rate] {
            b.pair(
                source.host().to_owned(),
                texts.t(if source.asked(&self.glance) {
                    key::LOCALE_ASKED
                } else {
                    key::LOCALE_NOT_ASKED
                }),
                Target::Ask(source),
            );
        }
        b.note(key::LOCALE_SOURCES_ARE_ASKED);

        b.out
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
    fn act(&mut self, rows: &[Row], step: i32) -> Errand {
        let Some(target) = rows.get(self.cursor).map(|r| r.target.clone()) else {
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
            Target::Nothing => Errand::Nothing,
        }
    }

    /// Puts the cursor on row `n`, when it is a row one can stand on.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) {
        if rows.get(n).is_some_and(|r| !r.heading) {
            self.cursor = n;
        }
    }

    /// Where the cursor is.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// One step, skipping headings and stopping at the ends.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        loop {
            let next = if down {
                at.saturating_add(1)
            } else {
                match at.checked_sub(1) {
                    Some(n) => n,
                    None => return,
                }
            };
            match rows.get(next) {
                None => return,
                Some(r) if r.heading => at = next,
                Some(_) => {
                    self.cursor = next;
                    return;
                }
            }
        }
    }
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
            ink: Ink::Grid,
        }
    }

    /// A heading whose text is a VALUE and not a key.
    ///
    /// A place the person added has no translation and never will: what names
    /// it is its zone, which is text they typed. It is the rule the filter
    /// names already follow (`text.mine`).
    fn heading_of(&mut self, text: String) {
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
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
            focused: false,
            heading: true,
            target: Target::Nothing,
        });
    }

    fn heading(&mut self, key: &str) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
            });
        }
        let text = self.texts.t(key);
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
    fn sentence(&mut self, text: String) {
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                text,
                tokens::color::TEXT_DIM,
                self.room,
            )],
            focused: false,
            // A NOTE IS NOT A ROW ONE STANDS ON. It explains the rows around
            // it; a cursor that stopped there would be a cursor on a sentence,
            // with `RET` doing nothing and no way to tell why.
            heading: true,
            target: Target::Nothing,
        });
    }

    fn pair(&mut self, label: String, value: String, target: Target) {
        let focused = self.cursor == self.out.len();
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
                self.left,
                self.room,
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
            from: Some(crate::language::Source::Default),
        };
        me.settle();
        me
    }

    fn at(v: &View, want: &Target) -> usize {
        v.rows(DECIDING_WIDTH, &utc(), &texts())
            .iter()
            .position(|r| &r.target == want)
            .unwrap_or_else(|| panic!("no row for {want:?}"))
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
        assert!(!rows[v.cursor()].heading, "the cursor opened on a heading");
        assert_ne!(
            rows[v.cursor()].target,
            Target::Nothing,
            "the cursor opened on a row nothing happens on"
        );
    }

    /// The rows are the same THINGS at every width, so a key acting on the
    /// row it builds acts on the row the door drew.
    #[test]
    fn the_rows_are_the_same_things_at_every_width() {
        let (t, z) = (texts(), utc());
        let v = view();
        let want: Vec<Target> = v
            .rows(DECIDING_WIDTH, &z, &t)
            .into_iter()
            .map(|r| r.target)
            .collect();
        for columns in [20u16, 40, 60, 80, 120, 200] {
            let got: Vec<Target> = v
                .rows(columns, &z, &t)
                .into_iter()
                .map(|r| r.target)
                .collect();
            assert_eq!(
                got, want,
                "at {columns} columns the rows are different things"
            );
        }
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

    /// The two hosts are NAMED on screen, and each can be stopped.
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
        for host in [ronoctua_world::WEATHER_HOST, ronoctua_world::RATE_HOST] {
            assert!(drawn.contains(host), "`{host}` is not on the screen");
        }
        v.cursor = at(&v, &Target::Ask(Source::Weather));
        let Errand::Glance(after) = v.press(Press::Enter, &z, &t) else {
            panic!("no glance errand");
        };
        assert!(!after.ask_weather);
        assert!(after.ask_rate, "the other host went off too");
    }

    /// The cursor never lands on a heading or a note, and does not wrap.
    #[test]
    fn the_cursor_skips_what_cannot_be_acted_on() {
        let (t, z) = (texts(), utc());
        let mut v = view();
        let rows = v.rows(DECIDING_WIDTH, &z, &t);
        for _ in 0..rows.len() * 2 {
            v.press(Press::Down, &z, &t);
            assert!(!rows[v.cursor()].heading, "stopped on a heading");
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
