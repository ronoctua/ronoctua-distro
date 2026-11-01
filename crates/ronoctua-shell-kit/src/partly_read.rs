//! A PERSON'S FILE READ ONLY IN PART, said in the view that shows it.
//!
//! # Why
//!
//! A store's read is lenient (`ronoctua_store::lenient`): a value this machine
//! cannot take — a newer machine's word, a typo made by hand — is repaired in
//! the value read, and the rest of the file is kept. The view that draws the
//! value would show the repair as if the file said it. The belongings list
//! every repair, with "leave it" and "write the fix", but a person changing
//! the sound settings is in the sound view, not there. So the view that owns
//! the file says one line: which places of it were not read, and why, and
//! that the file keeps them. A refusal is a value with a sentence
//! (`AGENTS.md`), and so is a part not read.
//!
//! # One piece for every view
//!
//! Every view of a person's file says it the same way: this value
//! ([`PartlyRead`]), made from the view's own read ([`read`],
//! [`PartlyRead::of`]), from a read made elsewhere and carried
//! ([`PartlyRead::of_repairs`]) or from the file read again ([`judged`]), and
//! its sentence ([`PartlyRead::said`]). `a-view-says-what-it-did-not-read`
//! holds that every view of a lenient store draws it.
//!
//! # What it is not
//!
//! A file NOTHING of which read (not JSON, nothing like the type) is not a
//! part: the views say that in their own sentence, and a writer refuses over
//! it (`Store::parsed`). [`PartlyRead::of`] answers `None` there, as it does
//! for a file read whole.

// THE STORE'S OWN WORDS for a place that did not read, so a view that carries
// a read made elsewhere, a camera and a test name them through here.
pub use ronoctua_store::repair::{Repair, Why};
use ronoctua_store::{Reading, Store};

/// A repair as a camera or a test plants it: the place and why, and nothing
/// of the value there, which no line says.
#[must_use]
pub fn planted(place: &str, why: Why) -> Repair {
    Repair {
        pointer: place.to_owned(),
        found: ronoctua_store::Node::Null,
        kept: None,
        why,
    }
}

/// How many places the line names before it counts the rest: a file a newer
/// machine wrote may differ in many, and the belongings list them all.
pub const NAMED: usize = 3;

/// The places of a person's file a lenient read could not take — never the
/// values there, which may be a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartlyRead {
    /// The file's name, as the view names it (`audio.json`).
    file: String,
    /// Each place, as an RFC 6901 pointer into the file, with why.
    places: Vec<(String, Why)>,
}

impl PartlyRead {
    /// What a read of `file` owes the person: `None` when it read whole,
    /// and when nothing of it did.
    #[must_use]
    pub fn of<T>(file: &str, reading: &Reading<T>) -> Option<Self> {
        if !reading.part_not_read() {
            return None;
        }
        Self::of_repairs(file, &reading.repairs)
    }

    /// The same, from the repairs of a read made elsewhere and carried to
    /// the view (a library's snapshot): `None` with no repair, and when one
    /// of them replaced the whole document.
    #[must_use]
    pub fn of_repairs(file: &str, repairs: &[Repair]) -> Option<Self> {
        // THE EMPTY POINTER IS THE WHOLE DOCUMENT (RFC 6901): nothing of the
        // file read, which is the view's other sentence.
        if repairs.is_empty() || repairs.iter().any(|r| r.pointer.is_empty()) {
            return None;
        }
        Some(Self {
            file: file.to_owned(),
            places: repairs.iter().map(|r| (r.pointer.clone(), r.why)).collect(),
        })
    }

    /// The file, as the line names it.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// How many places did not read.
    #[must_use]
    pub fn places(&self) -> usize {
        self.places.len()
    }

    /// Its sentence: the file, the first [`NAMED`] places each with why,
    /// how many more, and that the file keeps them.
    #[must_use]
    pub fn said(&self, texts: &ronoctua_i18n::Texts) -> String {
        use crate::key;
        let mut named = self.places.iter().take(NAMED).map(|(place, why)| {
            texts.t_with(
                key::PARTLY_READ_PLACE,
                &[("place", place), ("why", &texts.t(why_key(*why)))],
            )
        });
        let mut listed = named.next().unwrap_or_default();
        let then = |first: &str, next: &str| {
            texts.t_with(key::PARTLY_READ_THEN, &[("first", first), ("next", next)])
        };
        for place in named {
            listed = then(&listed, &place);
        }
        let more = self.places.len().saturating_sub(NAMED);
        if more > 0 {
            let more = texts.t_args(
                key::PARTLY_READ_MORE,
                &[(
                    "n",
                    ronoctua_i18n::Arg::Number(i64::try_from(more).unwrap_or(i64::MAX)),
                )],
            );
            listed = then(&listed, &more);
        }
        texts.t_with(
            key::PARTLY_READ,
            &[("file", &self.file), ("places", &listed)],
        )
    }
}

/// Why a place did not read, in words — one key per reason, the ones the
/// belongings' list of repairs says too.
#[must_use]
pub const fn why_key(why: Why) -> &'static str {
    use crate::key;
    match why {
        Why::UnknownVariant => key::BELONGINGS_REPAIR_WHY_UNKNOWN_VARIANT,
        Why::WrongType => key::BELONGINGS_REPAIR_WHY_WRONG_TYPE,
        Why::Missing => key::BELONGINGS_REPAIR_WHY_MISSING,
        Why::Invalid => key::BELONGINGS_REPAIR_WHY_INVALID,
    }
}

/// A view's own read of its file: the value, repaired where it must be, and
/// what the read owes the person.
#[must_use]
pub fn read<T>(store: &Store<T>) -> (T, Option<PartlyRead>)
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    let reading = store.reading();
    let partly = PartlyRead::of(store.file(), &reading);
    (reading.value, partly)
}

/// What the file owes the person NOW, read again to be judged — for a view
/// whose values came from a read made elsewhere (the footer's copy, kept
/// since it started), on the event that brought them.
#[must_use]
pub fn judged<T>(store: &Store<T>) -> Option<PartlyRead>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    PartlyRead::of(store.file(), &store.reading())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-partly-read-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Prefs {
        step: u8,
        over: bool,
        names: Vec<String>,
    }

    impl Default for Prefs {
        fn default() -> Self {
            Self {
                step: 5,
                over: false,
                names: Vec::new(),
            }
        }
    }

    /// FAILS IF: a file read only in part reads as whole to its view, a file
    /// read whole or not at all owes this line, or the line does not name the
    /// file and the place — in every language.
    #[test]
    fn a_file_read_in_part_owes_a_line_naming_what_was_not_read() {
        let dir = scratch("read");
        // A store under no root of this process writes no ledger.
        let store: Store<Prefs> = Store::at(Some(dir.clone()), "sound.json");
        // NO FILE, and a WHOLE one: nothing is owed.
        assert_eq!(read(&store).1, None);
        std::fs::write(dir.join("sound.json"), br#"{"step": 2, "over": true}"#).unwrap();
        assert_eq!(
            read(&store),
            (
                Prefs {
                    step: 2,
                    over: true,
                    names: Vec::new()
                },
                None
            )
        );
        // A PART: the value repaired, the rest kept, and the line owed.
        std::fs::write(
            dir.join("sound.json"),
            br#"{"step": "loud", "over": true, "names": ["a", 3]}"#,
        )
        .unwrap();
        let (value, partly) = read(&store);
        assert_eq!(value.step, 5, "the repair is the default there");
        assert!(value.over, "the rest of the file was not kept");
        let partly = partly.expect("a file read in part owes nothing");
        assert_eq!(partly.file(), "sound.json");
        assert_eq!(partly.places(), 2);
        assert_eq!(judged(&store), Some(partly.clone()));
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("the language");
            let said = partly.said(&t);
            assert!(said.contains("sound.json"), "{language}: {said}");
            assert!(said.contains("/step"), "{language}: {said}");
            assert!(said.contains("/names/1"), "{language}: {said}");
            assert!(
                said.contains(&t.t(why_key(Why::WrongType))),
                "{language}: {said}"
            );
            assert!(!said.contains('['), "{language}: a key is missing: {said}");
        }
        // NOTHING OF IT: another sentence, the view's own.
        std::fs::write(dir.join("sound.json"), b"{ not json").unwrap();
        assert_eq!(read(&store).1, None);
        std::fs::write(dir.join("sound.json"), b"[1, 2]").unwrap();
        assert_eq!(read(&store).1, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a carried read that replaced the whole file owes the line, or
    /// the line names more places than [`NAMED`] instead of counting them.
    #[test]
    fn the_line_names_a_few_places_and_counts_the_rest() {
        assert_eq!(PartlyRead::of_repairs("x.json", &[]), None);
        assert_eq!(
            PartlyRead::of_repairs(
                "x.json",
                &[planted("/a", Why::Invalid), planted("", Why::WrongType)]
            ),
            None,
            "the whole file is not a part of it"
        );
        let many: Vec<Repair> = (0..NAMED + 2)
            .map(|i| planted(&format!("/place{i}"), Why::UnknownVariant))
            .collect();
        let partly = PartlyRead::of_repairs("x.json", &many).unwrap();
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("the language");
            let said = partly.said(&t);
            for i in 0..NAMED {
                assert!(said.contains(&format!("/place{i}")), "{language}: {said}");
            }
            assert!(
                !said.contains(&format!("/place{NAMED}")),
                "{language}: {said}"
            );
            assert!(
                said.contains('2'),
                "{language}: the rest is not counted: {said}"
            );
            assert!(!said.contains('['), "{language}: a key is missing: {said}");
        }
        for why in Why::ALL {
            for language in ["en-GB", "pt-BR"] {
                let t = ronoctua_i18n::Texts::fresh(language).expect("the language");
                assert!(t.has(why_key(why)), "{language}: {}", why.id());
            }
        }
    }
}
