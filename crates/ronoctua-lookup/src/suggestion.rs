//! A candidate for a file's tags: the fields a service would write, and a
//! cover to fetch — the one shape MusicBrainz, Wikidata and TMDB answer, so
//! the Candidate page draws one kind of row.
//!
//! The fields are the SHEET's ([`ronoctua_tags::Field`]): taking a suggestion
//! is writing those fields, so the names are the same and nothing maps one
//! vocabulary onto another.

use ronoctua_tags::Field;
use serde::{Deserialize, Serialize};

use crate::{Host, Service};

/// One field's suggested value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggested {
    /// Which field.
    pub field: Field,
    /// Its value, as it would be written.
    pub value: String,
}

/// Where a suggested cover is fetched from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "at")]
pub enum Cover {
    /// A release's front cover on the Cover Art Archive: its list is asked
    /// first, and the front image's large thumbnail fetched.
    CoverArt {
        /// The MusicBrainz release.
        release: String,
    },
    /// An image at an address on one of this crate's hosts (a Commons
    /// poster, a TMDB poster).
    Address {
        /// The host the address is on.
        host: Host,
        /// The `https` address.
        url: String,
    },
}

/// A candidate: who suggests it, the fields, a cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// Who suggests it.
    pub source: Service,
    /// The service's own id for it (a MusicBrainz recording, a Wikidata
    /// item, a TMDB number).
    pub id: String,
    /// The fields it has, in the sheet's order, none blank.
    pub fields: Vec<Suggested>,
    /// A cover to fetch, when it has one.
    pub cover: Option<Cover>,
    /// The service's own confidence, 0–100, when it gives one.
    pub score: Option<u8>,
}

impl Suggestion {
    /// A suggestion from `source` with nothing in it yet.
    #[must_use]
    pub const fn new(source: Service, id: String) -> Self {
        Self {
            source,
            id,
            fields: Vec::new(),
            cover: None,
            score: None,
        }
    }

    /// Its value for `field`.
    #[must_use]
    pub fn value(&self, field: Field) -> Option<&str> {
        self.fields
            .iter()
            .find(|s| s.field == field)
            .map(|s| s.value.as_str())
    }

    /// Add `field` with `value`, unless the value is blank or the field is
    /// already there; then put the fields in the sheet's order.
    pub fn put(&mut self, field: Field, value: Option<String>) {
        let Some(value) = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) else {
            return;
        };
        if self.value(field).is_some() {
            return;
        }
        self.fields.push(Suggested { field, value });
        self.fields.sort_by_key(|s| order(s.field));
    }
}

/// Where a field is on the sheet.
fn order(field: Field) -> usize {
    Field::ALL
        .iter()
        .position(|f| *f == field)
        .unwrap_or(usize::MAX)
}

/// Several names as one value: joined with `; `, the way a sheet shows a
/// field with several values (`ronoctua_tags`' own join).
#[must_use]
pub fn joined(names: &[String]) -> Option<String> {
    let names: Vec<&str> = names
        .iter()
        .map(|n| n.trim())
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then(|| names.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_value_is_not_a_suggestion_and_the_order_is_the_sheets() {
        let mut s = Suggestion::new(Service::MusicBrainz, "x".into());
        s.put(Field::Date, Some("2005".into()));
        s.put(Field::Title, Some(" Hoppípolla ".into()));
        s.put(Field::Genre, Some("   ".into()));
        s.put(Field::Title, Some("second".into()));
        let fields: Vec<(Field, &str)> = s
            .fields
            .iter()
            .map(|f| (f.field, f.value.as_str()))
            .collect();
        assert_eq!(
            fields,
            [(Field::Title, "Hoppípolla"), (Field::Date, "2005")]
        );
        assert_eq!(
            joined(&["a".into(), " ".into(), "b".into()]).as_deref(),
            Some("a; b")
        );
        assert_eq!(joined(&[]), None);
    }
}
