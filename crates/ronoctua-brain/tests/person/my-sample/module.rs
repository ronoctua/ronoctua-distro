//! A PERSON'S SAMPLE MODULE — the fixture a machine's brain is compiled with
//! in `brain-person-modules-load`, loaded from the person's folder
//! (`RONOCTUA_BRAIN_PERSON_MODULES_DIR`) and never from this crate's
//! `modules/`.
//!
//! It shows two of its three seats: the second, and the third under the name
//! the person gave it — a mark with instances, as a person's module may draw
//! one.

use crate::module::{Birth, Face, Look, Manifest, Mark, Module, Tone};

/// The entry every module has.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    Sample
}

/// The module. No state: its seats are fixed.
struct Sample;

impl Module for Sample {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        Face {
            marks: vec![
                Mark {
                    id: super::mark::SEAT.numbered(2, None),
                    tone: Tone::Watching,
                    count: 0,
                    act: None,
                },
                Mark {
                    id: super::mark::SEAT.numbered(3, Some("desk".to_owned())),
                    tone: Tone::Warning,
                    count: 0,
                    act: None,
                },
            ],
            ..Face::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the person's sample is not loaded with its manifest's three
    /// instances, or shows a seat its manifest does not declare.
    #[test]
    fn its_seats_are_the_declared_ones() {
        assert_eq!(super::super::mark::SEAT.instances, 3);
        let pulse = crate::pulse::Pulse::default();
        let mine = crate::prefs::TaskPrefs::default();
        let look = Look {
            now: 1,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        let face = Sample.face(&look);
        let shown: Vec<&Mark> = Vec::new();
        for mark in &face.marks {
            assert_eq!(super::super::MANIFEST.refusal_of(mark, &shown), None);
        }
    }
}
