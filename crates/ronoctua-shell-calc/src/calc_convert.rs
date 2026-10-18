//! THE CONVERTER: a ring of categories, a unit to convert FROM and one to
//! convert TO, a search that finds either, a swap, and the value live.
//!
//! # What is typed where
//!
//! The AMOUNT is the calculator's own expression line (`crate::calc::View`):
//! digits, the mark and operations go there in every mode, so `2+3` is an
//! amount too. LETTERS go to the SEARCH, which finds a unit for the side being
//! chosen by any word a person types for it in their language (`metros`,
//! `polegadas`) or by its symbol (`km`, `°C`) — the same aliases the engine
//! parses (`assets/units.nix`), folded the same way (`units::fold`).
//!
//! # Currencies
//!
//! The ring's last category is the currencies the European Central Bank
//! quotes. Converting between two needs its daily rates, and those are asked
//! for only when a currency is chosen and none are cached fresh — and the
//! first time only after the person pressed the row that names the host
//! (`ronoctua-consent`): the calculator's view says so, the door asks.

use ronoctua_calc::rates::Currency;
use ronoctua_calc::units::{self, Category};
use ronoctua_calc::{Base, Quantity, Value};

/// A category of the ring: the units of one [`Category`], or the currencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The units of a category of the table.
    Units(Category),
    /// The currencies the ECB quotes, and the euro.
    Currency,
}

/// The ring, in its order: the table's categories, then the currencies.
pub const RING: [Kind; 16] = {
    let mut out = [Kind::Currency; 16];
    let mut i = 0;
    while i < Category::ALL.len() {
        out[i] = Kind::Units(Category::ALL[i]);
        i += 1;
    }
    out
};

impl Kind {
    /// Its name's key (`calc-category-<id>`).
    #[must_use]
    pub fn key(self) -> String {
        match self {
            Self::Units(c) => format!("calc-category-{}", c.id()),
            Self::Currency => "calc-category-currency".to_owned(),
        }
    }

    /// Every unit of it, in the table's order.
    #[must_use]
    pub fn bases(self) -> Vec<Base> {
        match self {
            Self::Units(c) => units::table().of(c).into_iter().map(Base::Unit).collect(),
            Self::Currency => units::CURRENCY_CODES
                .iter()
                .filter_map(|c| Currency::new(c))
                .map(Base::Currency)
                .collect(),
        }
    }
}

/// Which side a unit chosen goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The unit the amount is in.
    From,
    /// The unit it is converted into.
    To,
}

/// The converter's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converter {
    /// Where on the ring.
    ring: usize,
    /// The unit the amount is in.
    pub from: Base,
    /// The unit it is converted into.
    pub to: Base,
    /// Which side a unit chosen goes to.
    pub side: Side,
    /// What was typed to find a unit.
    pub search: String,
    /// The unit under the cursor, by its place in what the search left.
    pub cursor: usize,
}

impl Default for Converter {
    fn default() -> Self {
        Self::at(0)
    }
}

impl Converter {
    /// The converter on the ring's `ring`th category, from its first unit to
    /// its second.
    #[must_use]
    pub fn at(ring: usize) -> Self {
        let ring = ring % RING.len();
        let bases = RING[ring].bases();
        let first = bases
            .first()
            .copied()
            .unwrap_or(Base::Currency(Currency::EUR));
        let second = match RING[ring] {
            // EURO TO THE REAL, the owner's two — and whichever the table
            // lists after the euro where the real is not quoted.
            Kind::Currency => Currency::new("BRL")
                .map(Base::Currency)
                .filter(|b| bases.contains(b))
                .unwrap_or(first),
            Kind::Units(_) => bases.get(1).copied().unwrap_or(first),
        };
        let first = match RING[ring] {
            Kind::Currency => Base::Currency(Currency::EUR),
            Kind::Units(_) => first,
        };
        Self {
            ring,
            from: first,
            to: second,
            side: Side::From,
            search: String::new(),
            cursor: 0,
        }
    }

    /// The category it is on.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        RING[self.ring]
    }

    /// Its place on the ring.
    #[must_use]
    pub const fn ring(&self) -> usize {
        self.ring
    }

    /// Turns the ring by `by`, round the ends — a new category, from its
    /// first unit to its second.
    pub fn turn(&mut self, by: i64) {
        let n = i64::try_from(RING.len()).unwrap_or(1);
        let at = (i64::try_from(self.ring).unwrap_or(0) + by).rem_euclid(n);
        self.choose(usize::try_from(at).unwrap_or(0));
    }

    /// Goes to the ring's `ring`th category.
    pub fn choose(&mut self, ring: usize) {
        if ring % RING.len() != self.ring {
            *self = Self {
                side: self.side,
                ..Self::at(ring)
            };
        }
    }

    /// From becomes To, and To From.
    pub const fn swap(&mut self) {
        std::mem::swap(&mut self.from, &mut self.to);
    }

    /// Chooses the side a unit goes to.
    pub fn aim(&mut self, side: Side) {
        self.side = side;
        self.cursor = 0;
    }

    /// The units the search leaves, in the table's order — every unit of the
    /// category while nothing is typed.
    #[must_use]
    pub fn listed(&self, subtag: &str, texts: &ronoctua_i18n::Texts) -> Vec<Base> {
        let wanted = units::fold(self.search.trim());
        self.kind()
            .bases()
            .into_iter()
            .filter(|b| wanted.is_empty() || answers_to(*b, &wanted, subtag, texts))
            .collect()
    }

    /// Moves the cursor one unit down or up the list, round the ends.
    pub fn walk(&mut self, down: bool, listed: usize) {
        if listed == 0 {
            self.cursor = 0;
            return;
        }
        let at = self.cursor.min(listed - 1);
        self.cursor = if down {
            (at + 1) % listed
        } else {
            (at + listed - 1) % listed
        };
    }

    /// The unit `base` goes to the side being chosen; the search is spent, and
    /// the other side is the one chosen next — From, then To, the order a
    /// person says a conversion in.
    pub fn take(&mut self, base: Base) {
        match self.side {
            Side::From => self.from = base,
            Side::To => self.to = base,
        }
        self.side = match self.side {
            Side::From => Side::To,
            Side::To => Side::From,
        };
        self.search.clear();
        self.cursor = 0;
    }

    /// A letter typed into the search.
    pub fn letter(&mut self, c: char) {
        self.search.push(c);
        self.cursor = 0;
    }

    /// Takes the last character of the search back; `false` when there was
    /// none.
    pub fn back(&mut self) -> bool {
        let taken = crate::line::pop_cluster(&mut self.search);
        self.cursor = 0;
        taken
    }

    /// Whether converting needs the exchange rates: a currency on one side
    /// and another on the other.
    #[must_use]
    pub fn wants_rates(&self) -> bool {
        matches!((self.from, self.to), (Base::Currency(a), Base::Currency(b)) if a != b)
    }

    /// The amount, in the From unit — a bare number takes it on; a value that
    /// already has a unit keeps its own (`5 km` typed while From is a mile).
    #[must_use]
    pub fn amount_in(&self, amount: &Value) -> Value {
        let mut v = amount.clone();
        if v.unit.is_none() {
            v.unit = Some(Quantity {
                terms: vec![(self.from, 1)],
            });
        }
        v
    }

    /// The amount in the To unit — the live value.
    ///
    /// # Errors
    ///
    /// The engine's refusal: two kinds of quantity, no rates, a currency the
    /// rates do not quote.
    pub fn converted(
        &self,
        amount: &Value,
        env: &ronoctua_calc::Env,
    ) -> Result<Value, ronoctua_calc::Why> {
        ronoctua_calc::convert(
            &self.amount_in(amount),
            &Quantity {
                terms: vec![(self.to, 1)],
            },
            env,
        )
    }
}

/// Whether `base` answers to the folded `wanted`: a word a person types for
/// it in their language, its symbol, its code, or the name the door gives
/// it — each from its start.
fn answers_to(base: Base, wanted: &str, subtag: &str, texts: &ronoctua_i18n::Texts) -> bool {
    let named = units::fold(&name_of(base, 2, texts));
    if named.starts_with(wanted) {
        return true;
    }
    match base {
        Base::Unit(u) => units::table().aliases.iter().any(|a| {
            a.unit == u
                && (a.subtag == "*" || a.subtag == subtag)
                && units::fold(a.word).starts_with(wanted)
        }),
        Base::Currency(c) => {
            units::fold(c.code()).starts_with(wanted)
                || units::CURRENCY_WORDS.iter().any(|(tag, word, code)| {
                    *code == c.code()
                        && (*tag == "*" || *tag == subtag)
                        && units::fold(word).starts_with(wanted)
                })
        }
    }
}

/// A unit's NAME as the door says it, for `count` of it — its `unit-<id>`
/// key with the numeric selector, or a currency's code.
#[must_use]
pub fn name_of(base: Base, count: i64, texts: &ronoctua_i18n::Texts) -> String {
    match base {
        Base::Unit(u) => units::table().unit(u).map_or_else(String::new, |unit| {
            texts.t_args(
                &format!("unit-{}", unit.id),
                &[("count", ronoctua_i18n::Arg::Number(count))],
            )
        }),
        Base::Currency(c) => c.code().to_owned(),
    }
}

/// A unit's SYMBOL, as a formula writes it — the same in every language.
#[must_use]
pub fn symbol_of(base: Base) -> String {
    match base {
        Base::Unit(u) => units::table()
            .unit(u)
            .map_or_else(String::new, |unit| unit.symbol.to_owned()),
        Base::Currency(c) => c.code().to_owned(),
    }
}

/// The language's subtag — `pt` of `pt-BR` — which the unit table's words
/// are filed under.
#[must_use]
pub fn subtag_of(texts: &ronoctua_i18n::Texts) -> String {
    texts
        .language()
        .split('-')
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn unit(id: &str) -> Base {
        Base::Unit(units::table().by_id(id).expect(id))
    }

    /// FAILS IF: a category of the ring has no name in some language, or no
    /// two units to convert between.
    #[test]
    fn every_category_of_the_ring_is_named_and_has_units() {
        for lang in ["pt-BR", "en-GB"] {
            let t = texts(lang);
            for k in RING {
                let name = t.t(&k.key());
                assert!(!name.starts_with('['), "{lang}: {k:?} has no name");
                assert!(k.bases().len() >= 2, "{k:?}");
            }
        }
        assert!(RING.contains(&Kind::Currency));
    }

    /// FAILS IF: the search does not find a unit by the words of the
    /// person's language, by its symbol, or by its name.
    #[test]
    fn a_unit_is_found_by_the_words_of_the_language() {
        let pt = texts("pt-BR");
        let mut c = Converter::at(0);
        for (typed, wanted) in [("quilo", "kilometre"), ("km", "kilometre"), ("pol", "inch")] {
            c.search = typed.to_owned();
            let found = c.listed("pt", &pt);
            assert!(found.contains(&unit(wanted)), "`{typed}` → {found:?}");
        }
        c.search = "zzz".to_owned();
        assert!(c.listed("pt", &pt).is_empty());
        let en = texts("en-GB");
        c.search = "mil".to_owned();
        assert!(c.listed("en", &en).contains(&unit("mile")));
        // AND A CURRENCY by its code or its name.
        let mut money = Converter::at(RING.len() - 1);
        money.search = "real".to_owned();
        let found = money.listed("pt", &pt);
        assert!(
            found.contains(&Base::Currency(Currency::new("BRL").unwrap())),
            "{found:?}"
        );
    }

    /// FAILS IF: a unit taken goes to the wrong side, the side does not move
    /// on, or the swap does not trade them.
    #[test]
    fn taking_a_unit_fills_a_side_and_the_swap_trades_them() {
        let mut c = Converter::at(0);
        c.aim(Side::From);
        c.take(unit("mile"));
        assert_eq!((c.from, c.side), (unit("mile"), Side::To));
        c.take(unit("kilometre"));
        assert_eq!(c.to, unit("kilometre"));
        c.swap();
        assert_eq!((c.from, c.to), (unit("kilometre"), unit("mile")));
        // THE VALUE, live: 1 mile is 1.609344 km, exactly.
        c.swap();
        let env = ronoctua_calc::Env::at(0, ronoctua_files::when::Zone::utc());
        let one = ronoctua_calc::eval(&ronoctua_calc::parse("1", '.').unwrap(), &env).unwrap();
        let v = c.converted(&one, &env).unwrap();
        assert_eq!(ronoctua_calc::format(&v, '.', 6), "1.609344");
        assert!(!c.wants_rates());
    }

    /// FAILS IF: the ring does not go round, or a new category keeps the
    /// units of the last one.
    #[test]
    fn the_ring_goes_round() {
        let mut c = Converter::at(0);
        c.turn(-1);
        assert_eq!(c.kind(), Kind::Currency);
        assert_eq!(c.from, Base::Currency(Currency::EUR));
        assert!(c.wants_rates());
        c.turn(1);
        assert_eq!(c.kind(), Kind::Units(Category::Length));
        c.choose(4);
        assert_eq!(c.kind(), Kind::Units(Category::Temperature));
        assert!(Kind::Units(Category::Temperature).bases().contains(&c.from));
        c.walk(false, 3);
        assert_eq!(c.cursor, 2, "up from the first is the last");
    }
}
