//! The golden fixtures: every good file reads and writes back byte for byte,
//! and every bad one is refused for the reason it was written to show.

use crate::choice::Choice;
use crate::declared::Declared;
use crate::declined::Declined;
use crate::distro_pins::DistroPins;
use crate::local::{Approved, Signer};
use crate::options::Options;
use crate::reports::{DeclaredAndroid, DistroApps, PersonApplied};
use crate::state::{MachineState, Whole};
use crate::{Document, Why, parse, to_json};

fn round_trips<T: Document + std::fmt::Debug>(name: &str, text: &str) {
    let doc: T = parse(text.as_bytes()).unwrap_or_else(|r| panic!("{name}: {r}"));
    assert_eq!(
        to_json(&doc),
        text,
        "{name} does not write back byte for byte"
    );
}

/// FAILS IF: a field is renamed, reordered or retyped, a writer stops
/// writing what every reader reads, or a good file stops being accepted.
#[test]
fn every_good_fixture_reads_and_writes_back_byte_for_byte() {
    round_trips::<Declared>("declared", include_str!("../fixtures/good/declared.json"));
    round_trips::<Declined>("declined", include_str!("../fixtures/good/declined.json"));
    round_trips::<Approved>("approved", include_str!("../fixtures/good/approved.json"));
    round_trips::<PersonApplied>(
        "person-applied",
        include_str!("../fixtures/good/person-applied.json"),
    );
    round_trips::<DistroApps>(
        "distro-apps",
        include_str!("../fixtures/good/distro-apps.json"),
    );
    round_trips::<DeclaredAndroid>(
        "declared-android",
        include_str!("../fixtures/good/declared-android.json"),
    );
    round_trips::<MachineState>("state", include_str!("../fixtures/good/state.json"));
    round_trips::<Whole>(
        "last-apply",
        include_str!("../fixtures/good/last-apply.json"),
    );
    round_trips::<Options>("options", include_str!("../fixtures/good/options.json"));
    round_trips::<Signer>("signer", include_str!("../fixtures/good/signer.json"));
    round_trips::<Choice>("choice", include_str!("../fixtures/good/choice.json"));
    round_trips::<DistroPins>(
        "distro-pins",
        include_str!("../fixtures/good/distro-pins.json"),
    );
}

fn refused<T: Document + std::fmt::Debug>(name: &str, text: &str, why: Why) {
    match parse::<T>(text.as_bytes()) {
        Ok(doc) => panic!("{name} was accepted: {doc:?}"),
        Err(r) => assert_eq!(r.why, why, "{name}: {r}"),
    }
}

/// FAILS IF: an unknown field, a newer format, an id that does not name its
/// entry, an unknown source or reason, an empty machine list, a stamp that is
/// not exact, a duplicate id or a module without `my-` is taken in — or is
/// refused for another reason than the one it shows.
#[test]
fn every_bad_fixture_is_refused_for_its_reason() {
    refused::<Declared>(
        "newer",
        include_str!("../fixtures/bad/declared-newer.json"),
        Why::Newer,
    );
    refused::<Declared>(
        "unknown field",
        include_str!("../fixtures/bad/declared-unknown-field.json"),
        Why::Shape,
    );
    refused::<Declared>(
        "unknown entry field",
        include_str!("../fixtures/bad/declared-unknown-entry-field.json"),
        Why::Shape,
    );
    refused::<Declared>(
        "unknown source",
        include_str!("../fixtures/bad/declared-unknown-source.json"),
        Why::Shape,
    );
    refused::<Declared>(
        "bad stamp",
        include_str!("../fixtures/bad/declared-bad-stamp.json"),
        Why::Shape,
    );
    refused::<Declared>(
        "id mismatch",
        include_str!("../fixtures/bad/declared-id-mismatch.json"),
        Why::Invalid,
    );
    refused::<Declared>(
        "empty machines",
        include_str!("../fixtures/bad/declared-empty-machines.json"),
        Why::Invalid,
    );
    refused::<Declined>(
        "duplicate",
        include_str!("../fixtures/bad/declined-duplicate.json"),
        Why::Duplicate,
    );
    refused::<Approved>(
        "not a person module",
        include_str!("../fixtures/bad/approved-not-a-person-module.json"),
        Why::Invalid,
    );
    refused::<Declared>(
        "not json",
        include_str!("../fixtures/bad/not-json.json"),
        Why::NotJson,
    );
    refused::<Signer>(
        "not a key",
        include_str!("../fixtures/bad/signer-not-a-key.json"),
        Why::Invalid,
    );
    refused::<DistroPins>(
        "not the tag's version",
        include_str!("../fixtures/bad/distro-pins-not-the-tags-version.json"),
        Why::Invalid,
    );
    refused::<DistroPins>(
        "not a release",
        include_str!("../fixtures/bad/distro-pins-not-a-release.json"),
        Why::Invalid,
    );
    refused::<DistroPins>(
        "a pin twice",
        include_str!("../fixtures/bad/distro-pins-duplicate.json"),
        Why::Duplicate,
    );
}

/// FAILS IF: an entry's accessors stop agreeing with its fields, or `null`
/// stops meaning every machine.
#[test]
fn an_entry_says_where_it_applies() {
    let declared: Declared =
        parse(include_str!("../fixtures/good/declared.json").as_bytes()).expect("the golden file");
    let calculator = declared.entry("flatpak:org.gnome.Calculator").unwrap();
    assert_eq!(calculator.source_id(), "org.gnome.Calculator");
    assert!(calculator.applies_to("vm") && !calculator.applies_to("laptop"));
    assert_eq!(declared.on("laptop").count(), declared.apps.len() - 1);
}
