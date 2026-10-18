//! WHEN, AS THE PERSON READS IT — the clock, the date's order, the decimal
//! mark, the units and the zone the interface shows times in
//! (`locale.json`), the zones a question can complete, and the second it is
//! now.
//!
//! # Why a module of its own
//!
//! Every door draws a date, a time or a number, and each read the person's
//! choice through the language-and-region VIEW that writes it — so the
//! chrome, the file manager, the player and a dozen windows named a view to
//! ask what a date looks like, and recompiled with it. The choice is a value
//! every door shares; the screen that turns its rings is one more reader of
//! it (`ronoctua_shell_locale::locale`).

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

/// One of the four rings of this screen, turned to a value.
///
/// What [`Errand::Keep`] carries: the one setting that changed, laid over
/// what `locale.json` says when it is written ([`prefs::with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// Twelve hours or twenty-four.
    Clock(Clock),
    /// The order of the parts of a date.
    Order(Order),
    /// What separates a whole number from its fraction.
    Decimal(Decimal),
    /// Metric or the other one.
    Units(Units),
}

impl Setting {
    /// This setting, laid over a record — and nothing else of it touched.
    pub const fn onto(self, choice: &mut Choice) {
        match self {
            Self::Clock(c) => choice.clock = c,
            Self::Order(o) => choice.order = o,
            Self::Decimal(d) => choice.decimal = d,
            Self::Units(u) => choice.units = u,
        }
    }
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

/// Whether a zone's name fits what is typed: the whole name begins with it,
/// or one of its parts does — `Lis` fits `Europe/Lisbon`, `york` fits
/// `America/New_York` — in any case, a space standing for the underscore.
#[must_use]
pub fn fits(zone: &str, typed: &str) -> bool {
    let typed = typed.trim().replace(' ', "_").to_lowercase();
    if typed.is_empty() {
        return false;
    }
    let zone = zone.to_lowercase();
    zone.starts_with(&typed)
        || zone.split('/').any(|part| {
            part.starts_with(&typed) || part.split('_').any(|word| word.starts_with(&typed))
        })
}

/// Every zone this system can read, out of tzdb's own list (`zone1970.tab`
/// in `RONOCTUA_ZONEINFO`, the directory every zone is read from) — read
/// once a process, the list the installer offers too.
fn all_zones() -> &'static [String] {
    static ZONES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ZONES.get_or_init(|| {
        let Some(root) = std::env::var_os("RONOCTUA_ZONEINFO") else {
            return Vec::new();
        };
        let root = std::path::PathBuf::from(root);
        let text = ["zone1970.tab", "zone.tab"]
            .iter()
            .find_map(|f| std::fs::read_to_string(root.join(f)).ok())
            .unwrap_or_default();
        zones_of_tab(&text)
    })
}

/// The zone names of a `zone1970.tab`: the third field of every line that is
/// not a comment, sorted.
#[must_use]
pub fn zones_of_tab(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(2))
        .map(|z| z.trim().to_owned())
        .filter(|z| !z.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The zones that fit what is typed, best first: a name that BEGINS with it
/// before a name with a part that does, each alphabetical. The completer of
/// the three zone questions (`line::completer_of`), in both doors.
#[must_use]
pub fn zones_matching(typed: &str) -> Vec<String> {
    zones_matching_in(all_zones(), typed)
}

/// [`zones_matching`], over a list the caller gives.
#[must_use]
pub fn zones_matching_in<Z: AsRef<str>>(all: &[Z], typed: &str) -> Vec<String> {
    let lowered = typed.trim().replace(' ', "_").to_lowercase();
    let mut fit: Vec<&str> = all
        .iter()
        .map(AsRef::as_ref)
        .filter(|z| fits(z, typed))
        .collect();
    fit.sort_by_key(|z| (!z.to_lowercase().starts_with(&lowered), *z));
    fit.into_iter().map(str::to_owned).collect()
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

    /// What the file says NOW, with one setting laid over it — what a door
    /// writes when a ring turns, so the other door's choices in the same file
    /// stay as it wrote them (see `Errand`).
    #[must_use]
    pub fn with(setting: super::Setting) -> Choice {
        if let Some(dir) = ronoctua_store::config_dir() {
            return with_at(&dir.join(FILE), setting);
        }
        let mut choice = Choice::default();
        setting.onto(&mut choice);
        choice
    }

    /// [`with`], with its path as a parameter.
    #[must_use]
    pub fn with_at(path: &std::path::Path, setting: super::Setting) -> Choice {
        let mut choice = read_at(path);
        setting.onto(&mut choice);
        choice
    }

    /// A watch on the file, ringing `on_stir` from the watcher's thread — the
    /// broadcast every door hears a choice made in another through.
    ///
    /// `None`, with a line in the log, when there is no configuration
    /// directory or no inotify instance left: the door then adopts a change
    /// at its next start, which is not silent.
    pub fn watch<F>(on_stir: F) -> Option<ronoctua_files::notice::Notice>
    where
        F: Fn() -> bool + Send + 'static,
    {
        watch_in(&ronoctua_store::config_dir()?, on_stir)
    }

    /// [`watch`], with its root as a parameter — the directory is made here,
    /// because a watch needs one to sit on and a door that starts before
    /// anybody ever chose is the common case.
    pub fn watch_in<F>(dir: &std::path::Path, on_stir: F) -> Option<ronoctua_files::notice::Notice>
    where
        F: Fn() -> bool + Send + 'static,
    {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(error = %e, "no configuration directory to watch locale.json in");
            return None;
        }
        // AT ONCE, like the language's: what arrives is a person's hand on a
        // ring, and the other door is on the screen beside it.
        match ronoctua_files::notice::Notice::on_file(
            dir,
            FILE,
            ronoctua_files::notice::Pace::AtOnce,
            on_stir,
        ) {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "no locale watcher; a change reaches this door at its next start"
                );
                None
            }
        }
    }

    /// `locale.json` as it is WRITTEN: the rings' ids, so an answer this
    /// build does not know — a newer machine's ring — is a repair the store
    /// keeps on disk, never a default written over it.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Written {
        clock: String,
        date: String,
        decimal: String,
        units: String,
        zone: String,
    }

    impl Default for Written {
        /// The defaults' ids — what a repair puts where a value will not
        /// read, and so exactly what a writer writes for a ring it did not
        /// turn: the store then puts the file's own answer back.
        fn default() -> Self {
            Self::of(&Choice::default())
        }
    }

    impl Written {
        fn of(choice: &Choice) -> Self {
            Self {
                clock: choice.clock.id().to_owned(),
                date: choice.order.id().to_owned(),
                decimal: choice.decimal.id().to_owned(),
                units: choice.units.id().to_owned(),
                zone: choice.zone.clone().unwrap_or_default(),
            }
        }

        fn choice(&self) -> Choice {
            Choice {
                clock: Clock::by_id(&self.clock).unwrap_or_default(),
                order: Order::by_id(&self.date).unwrap_or_default(),
                decimal: Decimal::by_id(&self.decimal).unwrap_or_default(),
                units: Units::by_id(&self.units).unwrap_or_default(),
                // A ZONE IS A NAME AND NOT A CHOICE OF FEW, so it is kept as it
                // was written and checked when it is READ — a zone that has
                // gone from the tzdata should say so on the screen, not vanish
                // out of the file.
                zone: Some(self.zone.clone()).filter(|z| !z.is_empty()),
            }
        }
    }

    /// The ids this build does not know, as repairs: each read as its
    /// default, and kept in the file.
    fn refused(w: &Written) -> Vec<ronoctua_store::Refused> {
        let unknown = |pointer: &str| ronoctua_store::Refused {
            pointer: pointer.to_owned(),
            why: ronoctua_store::repair::Why::UnknownVariant,
            kept: None,
        };
        let mut out = Vec::new();
        if Clock::by_id(&w.clock).is_none() {
            out.push(unknown("/clock"));
        }
        if Order::by_id(&w.date).is_none() {
            out.push(unknown("/date"));
        }
        if Decimal::by_id(&w.decimal).is_none() {
            out.push(unknown("/decimal"));
        }
        if Units::by_id(&w.units).is_none() {
            out.push(unknown("/units"));
        }
        out
    }

    /// The file as a [`ronoctua_store::Store`] in `dir`, judged by
    /// [`refused`] on every read and write.
    fn store_in(dir: &std::path::Path) -> ronoctua_store::Store<Written> {
        ronoctua_store::Store::at(Some(dir.to_path_buf()), FILE).checked(refused)
    }

    /// What the file says, or the defaults.
    ///
    /// It never fails: a value that will not read costs that value — the
    /// store repairs it, says so at `warn` and in the repair ledger — and a
    /// file that is not JSON costs the preferences in it. A writer reads it
    /// too ([`with`]), and that is safe now: the store's write puts back every
    /// repaired answer the writer did not turn, so turning one ring never
    /// erases another ring's answer this build could not read.
    #[must_use]
    pub fn read() -> Choice {
        let Some(dir) = ronoctua_store::config_dir() else {
            return Choice::default();
        };
        read_at(&dir.join(FILE))
    }

    /// [`read`], with its path as a parameter — `<dir>/locale.json`.
    #[must_use]
    pub fn read_at(path: &std::path::Path) -> Choice {
        let Some(dir) = path.parent() else {
            return Choice::default();
        };
        store_in(dir).read().choice()
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
        // THE WHOLE RECORD, every field. Writing one would take the others
        // back to their default, which is the defect `display.json`'s merge
        // exists because of — here it is cheap to avoid, because there are
        // five. The store keeps what the file holds that this build does not
        // know, and every answer it could not read that was not turned.
        store_in(dir).write(&Written::of(choice))
    }
}

/// Seconds since the epoch, now — what the readings' clock is set to by a
/// door, at the moments it draws.
#[must_use]
pub fn seconds_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Seconds since the epoch, now.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: typing a part of a zone's name stops offering it — `Lis`
    /// offers `Europe/Lisbon`, a part after the slash or the underscore
    /// counts — or a name that BEGINS with the typing is not offered first.
    #[test]
    fn a_zone_is_offered_by_any_part_of_its_name() {
        let tab = "# a comment\nPT\t+3843-00908\tEurope/Lisbon\nUS\t+404251-0740023\tAmerica/New_York\nPE\t-1203-07703\tAmerica/Lima\nGB\t+513030-0000731\tEurope/London\n";
        let all = zones_of_tab(tab);
        assert_eq!(all.len(), 4);
        assert_eq!(zones_matching_in(&all, "Lis"), ["Europe/Lisbon"]);
        assert_eq!(zones_matching_in(&all, "york"), ["America/New_York"]);
        assert_eq!(zones_matching_in(&all, "new y"), ["America/New_York"]);
        assert_eq!(
            zones_matching_in(&all, "Europe/L"),
            ["Europe/Lisbon", "Europe/London"]
        );
        // A NAME THAT BEGINS WITH IT FIRST: `America` before a part.
        assert_eq!(zones_matching_in(&all, "ame")[0], "America/Lima");
        assert!(zones_matching_in(&all, "").is_empty());
        assert!(zones_matching_in(&all, "zzz").is_empty());
    }
}
