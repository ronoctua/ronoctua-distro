//! The glance — `LEADER d`, one line above the modeline that goes away.
//!
//! # Why it is a line and not a clock in the chrome
//!
//! Invariant 14 has no room for a clock, and the reason is older than the
//! invariant: a clock that is always there is a clock nobody reads. This is a
//! question one asks and gets answered, like the notification band it sits
//! beside — and like the band, it is the footer's OWN surface growing upward,
//! not a popup.
//!
//! # Why this module has no Wayland in it
//!
//! Everything here is `(what is known, width) -> pieces with columns`. No GPU,
//! no surface, no state, no clock of its own — the time arrives as a number.
//! It is `band.rs`'s shape, and it buys the same two things: it can be tested
//! headless, and it fits the frame because it is integer arithmetic over the
//! cell.
//!
//! # One line, and the height never depends on the width
//!
//! `band::rows`, `brain::rows` and `fold.rs` all argue this and it is the same
//! argument: a region whose height followed the width would reflow every
//! buffer on the screen when a window moved to the other monitor. So the
//! places sit side by side on ONE line, and when they do not fit, whole places
//! are DROPPED — by `layout::resolve`, the modeline's own arithmetic.
//!
//! Dropped and not shrunk, which is why every segment here is `removable` with
//! a `min_cols` as wide as its text: half a temperature is worse than no
//! temperature, and `15…` is not a smaller fact, it is a different one. The
//! primary place is the exception and shrinks instead — it is what the glance
//! was asked about, and a glance with nothing in it answered nothing.

use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// One drawn piece of the line. The band's shape, for the band's reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// First cell.
    pub at: u16,
    /// What is drawn, already elided.
    pub text: String,
    /// Which token colours it.
    pub color: tokens::Color,
}

/// A place's stripe: where its ground starts, how wide it is, and its tone.
///
/// Separate from [`Piece`] because a ground is a RECTANGLE and a piece is
/// text: they are drawn by two different passes, and the order between them
/// is what keeps the letters on top of their own stripe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stripe {
    /// First cell.
    pub at: u16,
    /// How many cells it covers.
    pub cells: u16,
    /// Which ground token.
    pub color: tokens::Color,
}

/// The line, as it is drawn: the stripes under it and the text on it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Drawn {
    /// The grounds, one per place, alternating.
    pub stripes: Vec<Stripe>,
    /// The text, already placed and elided.
    pub pieces: Vec<Piece>,
}

/// How many lines the glance claims: one when it is up, none when it is not.
#[must_use]
pub const fn rows(showing: bool) -> u16 {
    if showing { 1 } else { 0 }
}

/// The glance while it is on screen: when it arrived, how long it has, and
/// whether a hand is holding it there.
///
/// # Why it is not just a deadline
///
/// It was — an `Instant` to expire at — and that answers one of the three
/// questions this line has to answer. The other two are the ones the owner
/// asked for: how much of the time is LEFT, so the pie can be drawn, and
/// whether the clock is stopped, so the pie can become a pause.
///
/// It is the `ronoctua_notify::Queue`'s arrangement, deliberately: the two
/// countdowns on this footer must not behave differently, and the glyph is
/// literally the same function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Showing {
    /// When it went up — MOVED FORWARD by a hold, never a second field.
    since: std::time::Instant,
    /// How long it gets.
    life: std::time::Duration,
    /// When the hand landed, if it is on it.
    held: Option<std::time::Instant>,
}

impl Showing {
    /// A glance that has just gone up.
    #[must_use]
    pub fn new(now: std::time::Instant, life: std::time::Duration) -> Self {
        Self {
            since: now,
            life,
            held: None,
        }
    }

    /// How much of its time has been spent, with any hold taken off.
    ///
    /// The gap counts from `max(hold, since)` — the `Queue`'s rule, and it is
    /// there for the same reason: a hand that was already resting on the line
    /// when the glance went up must not hand it the time it spent hovering
    /// nothing.
    fn spent(&self, now: std::time::Instant) -> std::time::Duration {
        let gone = now.duration_since(self.since);
        match self.held {
            Some(at) => gone.saturating_sub(now.duration_since(at.max(self.since))),
            None => gone,
        }
    }

    /// How much of its life is left, from one to zero — what the pie draws.
    #[must_use]
    pub fn fraction_left(&self, now: std::time::Instant) -> f32 {
        if self.life.is_zero() {
            return 0.0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let left = 1.0 - (self.spent(now).as_secs_f64() / self.life.as_secs_f64()) as f32;
        // CLAMPED, for the reason `brain::left_of` gives: a clock that stepped
        // or a hold released out of order would otherwise give a pie fuller
        // than full.
        left.clamp(0.0, 1.0)
    }

    /// Is its time up?
    #[must_use]
    pub fn over(&self, now: std::time::Instant) -> bool {
        // A HELD GLANCE NEVER EXPIRES, and that is the whole point of the
        // hold: whoever is reading it must not watch it go in the middle of a
        // word. The gesture that undoes it is taking the hand away.
        self.held.is_none() && self.spent(now) >= self.life
    }

    /// Is a hand on it?
    #[must_use]
    pub const fn held(&self) -> bool {
        self.held.is_some()
    }

    /// The hand landed. Idempotent: the pointer sends a motion per pixel.
    pub const fn hold(&mut self, now: std::time::Instant) {
        if self.held.is_none() {
            self.held = Some(now);
        }
    }

    /// The hand left, and the clock goes on FROM WHERE IT STOPPED.
    ///
    /// By pushing `since` forward, not by keeping a second total: paying the
    /// whole wait at once is what a naive release does, and the line would
    /// vanish the instant the pointer left it.
    pub fn release(&mut self, now: std::time::Instant) {
        if let Some(at) = self.held.take() {
            self.since += now.duration_since(at.max(self.since));
        }
    }
}

/// What one place says, as text — the pieces it shows, in order, joined.
///
/// The SEPARATOR is `mark.separator`, the same `·` the hint line uses between
/// two things that are simply beside each other. Between PLACES the line uses
/// a wider gap, because a place is a different kind of boundary from a field.
#[must_use]
pub fn about(
    place: &ronoctua_world::prefs::Wanted,
    known: ronoctua_world::Known,
    now: i64,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(5);
    // THE DISTRO'S PLACES HAVE A TEXT KEY; the person's do not, and then the
    // ZONE is drawn. `Europe/Lisbon` reads as a place; `[world-place-…]` reads
    // as a fault, and asking somebody to write a translation for a city they
    // have just added is asking the wrong person.
    parts.push(if place.key.is_empty() {
        place.zone.clone()
    } else {
        texts.t(&place.key)
    });
    let choice = crate::locale::prefs::current();
    if place.clock {
        parts.push(crate::chrome::clock_text(now, zone, choice.clock, texts));
    }
    if place.date {
        parts.push(crate::chrome::date_text(now, zone, choice.order, texts));
    }
    // WHAT HAS NOT ARRIVED IS NOT DRAWN, and no space is kept for it. A glance
    // opened before the first fetch shows the clock and closes over the gap;
    // reserving the room would make the line jump sideways when the answer
    // came, which is the one thing a line that is read in a glance must not do.
    if place.weather
        && let Some(t) = known.temperature
    {
        // THE UNIT IS THE PERSON'S, and so is the mark between the whole part
        // and the fraction. `Arg::Fraction` applies the LANGUAGE's mark, which
        // was right until `LEADER s l` let somebody say otherwise — so the
        // number is formatted here, once, through the choice.
        parts.push(texts.t_with(
            if choice.units == crate::locale::Units::Imperial {
                ronoctua_i18n::key::WORLD_TEMPERATURE_F
            } else {
                ronoctua_i18n::key::WORLD_TEMPERATURE
            },
            &[(
                "degrees",
                &fraction(choice.units.of(t), choice.decimal, texts),
            )],
        ));
    }
    if place.rate
        && let Some(r) = known.rate
    {
        parts.push(texts.t_with(
            ronoctua_i18n::key::WORLD_RATE,
            &[
                ("from", &place.currency),
                ("to", &place.rate_in),
                ("much", &fraction(r, choice.decimal, texts)),
            ],
        ));
    }
    parts.join(&format!(" {} ", tokens::mark::SEPARATOR))
}

/// The line, as it is drawn: the stripes under it and the text on it.
///
/// `now` is epoch seconds and arrives as a parameter — the brain's line does
/// the same, and for the same reason: a clock of its own would be a second
/// wake-up for one effect. `showing` carries the countdown, because the pie
/// on the right is part of the line and not a second thing beside it.
#[must_use]
/// A number with one decimal, wearing the mark the person asked for.
///
/// # Why not `Arg::Fraction`
///
/// Because that one applies the LANGUAGE's mark, and it was right to until
/// `LEADER s l` let somebody say otherwise. The language's own is still the
/// default, and reaching it through `Decimal::mark` is what keeps that true.
fn fraction(n: f64, decimal: crate::locale::Decimal, texts: &ronoctua_i18n::Texts) -> String {
    let plain = format!("{n:.1}");
    let mark = decimal.mark(texts.decimal_mark());
    if mark == '.' {
        plain
    } else {
        plain.replace('.', &mark.to_string())
    }
}

pub fn line(
    places: &[ronoctua_world::prefs::Wanted],
    cache: &ronoctua_world::Cache,
    columns: u16,
    now: i64,
    showing: Option<Showing>,
    zones: &[(String, ronoctua_files::when::Zone)],
    texts: &ronoctua_i18n::Texts,
) -> Drawn {
    // THE PIE'S CELLS COME OFF THE BUDGET FIRST, so the places are resolved
    // against what is actually left. Taken afterwards, a full line would have
    // its last place drawn under the glyph.
    let budget = columns.saturating_sub(PIE_CELLS);

    // A PLACE THAT SHOWS NOTHING IS NOT DRAWN, and it is not refused either.
    // The generator refuses one in the DISTRO's table, where it can only be a
    // mistake; here it happens because somebody turned the fourth switch off,
    // and refusing the fourth keystroke of four is a screen that fights back.
    let showing_places: Vec<&ronoctua_world::prefs::Wanted> =
        places.iter().filter(|p| p.shows_anything()).collect();
    let segments: Vec<layout::Segment> = showing_places
        .iter()
        .enumerate()
        .map(|(n, place)| {
            let zone = zones
                .iter()
                .find(|(z, _)| *z == place.zone)
                .map_or_else(ronoctua_files::when::Zone::utc, |(_, z)| z.clone());
            let text = about(place, cache.of(&place.zone), now, &zone, texts);
            let width = layout::width(&text);
            layout::Segment {
                // THE POSITION IS THE ID. It was the zone, which stopped being
                // `&'static str` the day the places became the person's; and a
                // position is what `resolve` uses an id for anyway.
                id: ORDER.get(n).copied().unwrap_or("place-more"),
                text,
                // The primary is 0 and survives longest; the rest fall in the
                // order they are declared, which is the order they are drawn.
                priority: u8::try_from(n).unwrap_or(u8::MAX),
                // AS WIDE AS ITS TEXT, so `resolve` drops it whole rather than
                // shrinking it. See the module's header.
                min_cols: width,
                side: layout::Side::Left,
                shrink: layout::Shrink::AtTheEnd,
                // THE PRIMARY NEVER GOES. It is what the glance was asked
                // about; a glance that dropped it answered nothing.
                removable: n > 0,
            }
        })
        .collect();

    let placed = layout::resolve(budget, &segments);
    let mut drawn = Drawn::default();
    for (n, one) in placed.iter().enumerate() {
        let cells = layout::width(&one.text);
        // THE GROUND ALTERNATES, place by place, and the FIRST keeps the
        // band's own — a first place that changed colour would read as one
        // more rung of the ladder above rather than as a stripe.
        //
        // The stripe covers the gap AFTER the place as well, up to where the
        // next one starts: a ground that stopped at the last letter would
        // leave a seam of the wrong tone between every pair.
        let until = placed.get(n + 1).map_or(budget, |next| next.column);
        drawn.stripes.push(Stripe {
            at: one.column,
            cells: until.saturating_sub(one.column).max(cells),
            color: if n % 2 == 0 {
                tokens::color::GLANCE_GROUND
            } else {
                tokens::color::GLANCE_GROUND_ALTERNATE
            },
        });
        drawn.pieces.push(Piece {
            at: one.column,
            text: one.text.clone(),
            // THE PRIMARY IS THE ANSWER AND THE REST IS CONTEXT, and with the
            // grounds alternating the tone is now the second thing saying so.
            color: if one.id == ORDER[0] {
                tokens::color::TEXT_PRIMARY
            } else {
                tokens::color::TEXT_SECONDARY
            },
        });
    }

    // THE PIE, on the right edge — the notification band's glyph, from the
    // very same function, so that the two countdowns on this footer cannot
    // come to look different. It becomes a PAUSE while a hand is on the line.
    if let Some(showing) = showing {
        let glyph = if showing.held() {
            tokens::icon::PAUSE
        } else {
            crate::band::countdown_glyph(ronoctua_notify::eighths(
                showing.fraction_left(std::time::Instant::now()),
            ))
        };
        drawn.pieces.push(Piece {
            at: columns.saturating_sub(PIE_CELLS),
            text: glyph.to_string(),
            color: tokens::color::NOTIFICATION_COUNTDOWN,
        });
    }
    drawn
}

/// What the pie spends: the glyph and the air before it.
const PIE_CELLS: u16 = 2;

/// The zone of every place, read once.
///
/// # Why the zoneinfo directory comes from the wrapper
///
/// Because `tzdata` was a BUILD-time input here — the installer's list of
/// zones — and nothing needed it at run time until a place other than the
/// machine's own had to be told the time. `/etc/localtime` answers for one
/// zone; this needs several, and each is a file in that directory.
///
/// A zone that cannot be read is UTC and SAYS so (`Zone::is_utc`), which is
/// the rule `when.rs` is built on: an offset wrong by an hour looks exactly
/// like an offset that is right, in every row, for ever.
#[must_use]
pub fn zones(
    places: &[ronoctua_world::prefs::Wanted],
) -> Vec<(String, ronoctua_files::when::Zone)> {
    let root = std::env::var_os("RONOCTUA_ZONEINFO").map(std::path::PathBuf::from);
    places
        .iter()
        .map(|place| {
            let zone = root
                .as_ref()
                .map_or_else(ronoctua_files::when::Zone::utc, |root| {
                    ronoctua_files::when::Zone::read_from(&root.join(&place.zone))
                });
            if zone.is_utc() {
                tracing::debug!(
                    zone = place.zone,
                    "no zone file; the glance shows UTC for it"
                );
            }
            (place.zone.clone(), zone)
        })
        .collect()
}

/// What the fetcher has found, from the file it writes.
///
/// `read` and not `parsed`: this only READS, and a reader degrades. A cache
/// that will not parse costs a temperature, and the glance then shows the
/// clocks — which is exactly what it shows before the first fetch.
#[must_use]
pub fn cached() -> ronoctua_world::Cache {
    ronoctua_world::store::store().read()
}

/// Static names for the places' positions, because `Segment::id` is
/// `&'static str` and a place's zone is now the person's `String`.
///
/// Eight is more places than a line of any width can hold, and past it they
/// share one id — which costs nothing: the id names a POSITION for `resolve`
/// and no caller outside `line` reads it.
const ORDER: &[&str] = &[
    "place-0", "place-1", "place-2", "place-3", "place-4", "place-5", "place-6", "place-7",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the default language")
    }

    fn places() -> Vec<ronoctua_world::prefs::Wanted> {
        ronoctua_world::prefs::Preferences::default().places()
    }

    fn zones() -> Vec<(String, ronoctua_files::when::Zone)> {
        places()
            .into_iter()
            .map(|p| (p.zone, ronoctua_files::when::Zone::utc()))
            .collect()
    }

    /// The height is one line or none, and NEVER a function of the width.
    ///
    /// THE DEFECT THIS CATCHES: a region whose height followed its content
    /// would reflow every buffer on screen when the footer moved to a monitor
    /// of another size. `band::rows` and `brain::rows` both argue it; this is
    /// the third region, and the first thing it has to promise.
    #[test]
    fn the_glance_is_one_line_at_every_width() {
        let (t, z) = (texts(), zones());
        let cache = ronoctua_world::Cache::default();
        assert_eq!(rows(false), 0);
        for columns in [20u16, 40, 80, 200, 400] {
            assert_eq!(rows(true), 1, "at {columns} columns");
            // And nothing drawn ever leaves the line.
            for piece in line(&places(), &cache, columns, 0, None, &z, &t).pieces {
                let end = piece.at + layout::width(&piece.text);
                assert!(
                    end <= columns,
                    "at {columns}: `{}` ends at {end}",
                    piece.text
                );
            }
        }
    }

    /// THE GROUNDS ALTERNATE, place by place, and the first keeps the band's.
    ///
    /// It is what the owner asked for in those words: London on the colour it
    /// has, Brasília with a ground of its own, and a third place back on
    /// London's. A separator alone left the eye a row of words with nothing
    /// to group them by.
    #[test]
    fn the_ground_alternates_and_the_first_keeps_the_bands() {
        let (t, z) = (texts(), zones());
        let cache = ronoctua_world::Cache::default();
        let drawn = line(&places(), &cache, 200, 0, None, &z, &t);
        assert!(drawn.stripes.len() >= 2, "fewer than two places drew");
        assert_eq!(drawn.stripes[0].color, tokens::color::GLANCE_GROUND);
        assert_eq!(
            drawn.stripes[1].color,
            tokens::color::GLANCE_GROUND_ALTERNATE
        );
        assert_ne!(
            drawn.stripes[0].color, drawn.stripes[1].color,
            "the two grounds are the same tone"
        );
        // AND THE STRIPES DO NOT OVERLAP, in either order.
        for pair in drawn.stripes.windows(2) {
            let end = pair[0].at + pair[0].cells;
            assert!(
                end <= pair[1].at,
                "a stripe ending at {end} runs into one starting at {}",
                pair[1].at
            );
        }
    }

    /// The pie is drawn, and it BECOMES A PAUSE while a hand is on the line.
    ///
    /// The same glyph as the notification band's, from the same function —
    /// two countdowns on one footer must not come to look different.
    #[test]
    fn the_pie_becomes_a_pause_while_a_hand_holds_it() {
        let (t, z) = (texts(), zones());
        let cache = ronoctua_world::Cache::default();
        let now = std::time::Instant::now();
        let life = std::time::Duration::from_secs(6);

        let running = line(
            &places(),
            &cache,
            200,
            0,
            Some(Showing::new(now, life)),
            &z,
            &t,
        );
        let last = running.pieces.last().expect("a pie");
        assert_eq!(
            last.text,
            crate::band::countdown_glyph(8).to_string(),
            "a fresh glance draws a full pie"
        );

        let mut held = Showing::new(now, life);
        held.hold(now);
        let paused = line(&places(), &cache, 200, 0, Some(held), &z, &t);
        assert_eq!(
            paused.pieces.last().expect("a pie").text,
            tokens::icon::PAUSE.to_string()
        );
        // IN THE SAME CELL, so the line does not shift when the hand lands.
        assert_eq!(last.at, paused.pieces.last().expect("a pie").at);
    }

    /// The clock STOPS while held and goes on FROM WHERE IT STOPPED.
    ///
    /// THE DEFECT THIS CATCHES: a release that pays the whole wait at once,
    /// which makes the line vanish the instant the pointer leaves it.
    #[test]
    fn a_hold_stops_the_clock_and_it_goes_on_from_there() {
        let start = std::time::Instant::now();
        let life = std::time::Duration::from_secs(10);
        let mut showing = Showing::new(start, life);

        let two = start + std::time::Duration::from_secs(2);
        assert!((showing.fraction_left(two) - 0.8).abs() < 0.01);

        showing.hold(two);
        // TIME PASSES AND NOTHING IS SPENT.
        let much_later = start + std::time::Duration::from_secs(60);
        assert!(
            (showing.fraction_left(much_later) - 0.8).abs() < 0.01,
            "the held clock ran on"
        );
        assert!(!showing.over(much_later), "a held glance expired");

        showing.release(much_later);
        // AND IT GOES ON FROM 2 s, not from 60.
        assert!(
            (showing.fraction_left(much_later) - 0.8).abs() < 0.01,
            "the release paid the whole wait at once"
        );
        let after = much_later + std::time::Duration::from_secs(3);
        assert!((showing.fraction_left(after) - 0.5).abs() < 0.01);
        assert!(showing.over(much_later + std::time::Duration::from_secs(9)));
    }

    /// A hand already resting there when the glance goes up is not handed the
    /// time it spent hovering nothing — the `Queue`'s `max(hold, since)` rule.
    #[test]
    fn a_hold_never_pays_out_to_a_glance_that_was_not_there() {
        let early = std::time::Instant::now();
        let later = early + std::time::Duration::from_secs(30);
        let mut showing = Showing::new(later, std::time::Duration::from_secs(10));
        showing.hold(early);
        showing.release(later + std::time::Duration::from_secs(4));
        // Four seconds of hold, all of them after it went up: six left.
        let f = showing.fraction_left(later + std::time::Duration::from_secs(4));
        assert!((f - 1.0).abs() < 0.01, "fraction was {f}");
    }

    /// The primary place survives every width; the complementary ones go
    /// first. It is what the glance was asked about.
    #[test]
    fn the_primary_is_the_last_to_go() {
        let (t, z) = (texts(), zones());
        let cache = ronoctua_world::Cache::default();
        let wide = line(&places(), &cache, 200, 0, None, &z, &t).pieces;
        let narrow = line(&places(), &cache, 24, 0, None, &z, &t).pieces;
        assert!(wide.len() >= narrow.len(), "narrower drew more");
        assert!(!narrow.is_empty(), "the glance emptied itself");
        // The primary's name is in the first piece at either width.
        let name = t.t(&places()[0].key);
        assert!(wide[0].text.contains(&name), "{:?}", wide[0].text);
        assert!(narrow[0].text.contains(&name), "{:?}", narrow[0].text);
    }

    /// WHAT HAS NOT ARRIVED IS NOT DRAWN. The default cache is empty, so the
    /// glance is clocks — which is exactly what a machine with no network
    /// shows, and it must not be a row of blanks or a placeholder.
    #[test]
    fn a_place_with_no_answer_yet_draws_what_it_has() {
        let (t, z) = (texts(), zones());
        let cache = ronoctua_world::Cache::default();
        let all = places();
        let primary = &all[0];
        let empty = about(primary, ronoctua_world::Known::default(), 0, &z[0].1, &t);
        assert!(!empty.is_empty());
        assert!(!empty.contains("None"), "{empty}");

        let known = ronoctua_world::Known {
            temperature: Some(8.0),
            rate: Some(7.42),
        };
        let full = about(primary, known, 0, &z[0].1, &t);
        assert!(
            full.len() > empty.len(),
            "an answer added nothing: {empty:?} -> {full:?}"
        );
        let _ = cache;
    }

    /// What the owner will actually read, printed — the check that no
    /// assertion replaces. `cargo test -- --nocapture` shows the line.
    #[test]
    fn the_line_reads_the_way_it_will_be_read() {
        let (t, z) = (texts(), zones());
        let mut cache = ronoctua_world::Cache::default();
        cache.places.insert(
            places()[0].zone.clone(),
            ronoctua_world::Known {
                temperature: Some(15.3),
                rate: Some(6.9233),
            },
        );
        for columns in [90u16, 60, 40, 24] {
            let drawn: Vec<String> = line(&places(), &cache, columns, 1_788_000_000, None, &z, &t)
                .pieces
                .into_iter()
                .map(|p| format!("[{}]{}", p.at, p.text))
                .collect();
            println!("  {columns:>3} cells: {}", drawn.join("  "));
        }
    }

    /// The rate and the temperature are drawn with the LANGUAGE's decimal
    /// mark, and that is not free: `fluent-bundle` does not localise a number.
    ///
    /// THE DEFECT THIS CATCHES: `t_with` instead of `t_args`, which takes a
    /// `&str` and would put whatever Rust's `{:.1}` wrote — a full stop, in
    /// every language — into a Brazilian's line for ever.
    #[test]
    fn a_number_on_the_line_wears_the_mark_of_its_language() {
        let known = ronoctua_world::Known {
            temperature: Some(15.3),
            rate: Some(6.9),
        };
        let zone = ronoctua_files::when::Zone::utc();
        let all = places();
        let primary = &all[0];
        let mut seen = std::collections::BTreeSet::new();
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a declared language");
            let said = about(primary, known, 0, &zone, &texts);
            let mark = texts.decimal_mark();
            assert!(
                said.contains(&format!("15{mark}3")),
                "{language} wrote the temperature without its own mark: {said}"
            );
            seen.insert(mark);
        }
        assert!(
            seen.len() > 1,
            "every declared language uses the same decimal mark, so this proves nothing"
        );
    }

    /// A place that shows only its clock shows ONLY its clock, whatever has
    /// arrived for it. It is the owner's default for Brasília, and the
    /// `shows` switches are the whole reason the table has four of them.
    #[test]
    fn the_switches_decide_and_not_what_arrived() {
        let (t, z) = (texts(), zones());
        let known = ronoctua_world::Known {
            temperature: Some(30.0),
            rate: Some(0.14),
        };
        for place in places().iter().filter(|p| !p.weather && !p.rate) {
            let said = about(place, known, 0, &z[0].1, &t);
            assert!(
                !said.contains("30"),
                "{} does not show weather and drew it: {said}",
                place.zone
            );
        }
    }
}
