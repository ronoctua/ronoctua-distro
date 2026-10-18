//! The clipboard's layout, which had no tests at all.
//!
//! That is not an aside: the defect the owner photographed — three lines drawn
//! above the top of the region whenever the history was short — was invisible
//! to `cargo test`, to clippy and to the whole Nix battery, because nothing
//! anywhere looked at where the clipboard put anything.

use super::*;

fn texts() -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh("en-GB").unwrap()
}

fn entry(text: &str) -> Entry {
    Entry {
        content: ronoctua_clipboard::Content::Text(text.to_owned()),
        kinds: vec!["text/plain".to_owned()],
        pinned: false,
    }
}

fn queue_of(texts: &[&str]) -> Queue {
    let mut queue = Queue::default();
    for text in texts {
        queue.register(entry(text));
    }
    queue
}

fn nothing() -> ronoctua_clipcascade::Snapshot {
    ronoctua_clipcascade::Snapshot::default()
}

/// The region is 80 wide in these tests, which is a real footer.
const WIDE: u16 = 80;

#[test]
fn an_empty_history_still_fills_the_region_and_nothing_escapes_it() {
    // THE PHOTOGRAPHED DEFECT. The old panel reserved three lines and drew
    // them at `end - 3`, `end - 2` and `end - 1`, where `end` was the top of
    // the region plus the number of rows — and with an empty history that
    // number is ONE. Nothing clamped them, so `end - 2` landed on the
    // MINIBUFFER and `end - 1` on the row that says the history is empty.
    //
    // Nothing computes a line any more, and this is what holds that down: with
    // no history at all the view still returns every row the right-hand column
    // needs, so there is nothing left over to place somewhere else.
    let t = texts();
    let queue = Queue::default();
    let sync = nothing();
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let rows = View::fresh().rows(&facts, WIDE, &t);
    let said = |needle: &str| {
        rows.iter()
            .any(|r| r.fields.iter().any(|f| f.text.contains(needle)))
    };
    // EVERY SECTION IS STILL THERE. The number of rows is not the promise —
    // what matters is that the right-hand column's content did not have to go
    // somewhere else because the left one was short, which is precisely what
    // happened before: it went onto the minibuffer.
    for heading in [
        crate::key::CLIPBOARD_HISTORY,
        crate::key::CLIPBOARD_ITEM,
        crate::key::CLIPBOARD_HEADING_SYNC,
    ] {
        assert!(said(&t.t(heading)), "`{heading}` is not drawn: {rows:#?}");
    }
    // AND THE SYNC'S OWN ROWS, which are the three lines the old panel drew at
    // absolute line numbers.
    assert!(said(&t.t(crate::key::CLIPBOARD_SYNC_SERVER)));
    assert!(said(&t.t(crate::key::CLIPBOARD_SYNC_ACCOUNT)));
    // AND EVERY FIELD IS INSIDE THE REGION'S WIDTH.
    for (n, row) in rows.iter().enumerate() {
        for field in &row.fields {
            assert!(
                field.at < WIDE,
                "row {n} has a field at column {}, past the region's {WIDE}",
                field.at
            );
        }
    }
}

#[test]
fn the_two_halves_stay_on_their_own_sides_of_the_rule() {
    // The rule is at `history_columns`, and it is the same seam the pointer
    // tests to decide which half a click fell in. A field that crossed it
    // would be drawn over the rule.
    let t = texts();
    let queue = queue_of(&["one", "two", "three"]);
    let sync = nothing();
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let seam = history_columns(WIDE);
    for row in View::fresh().rows(&facts, WIDE, &t) {
        for field in &row.fields {
            let width = u16::try_from(field.text.chars().count()).unwrap_or(u16::MAX);
            let ends = field.at.saturating_add(width);
            let on_the_left = field.at < seam;
            if on_the_left {
                assert!(
                    ends <= seam,
                    "a left field runs to {ends}, past the rule at {seam}: {:?}",
                    field.text
                );
            } else {
                assert!(
                    field.at >= detail_at(WIDE),
                    "a right field starts at {}, inside the rule's gap",
                    field.at
                );
            }
        }
    }
}

#[test]
fn the_hint_comes_from_the_table_and_every_key_has_words() {
    // THE OLD STRIP WAS HAND-ROLLED from the LONG `cmd-…` labels, in one
    // colour, with the separator written into the Rust — which is why the
    // photograph reads `d Remove from history` where every other view reads
    // `d remove`.
    let t = texts();
    let pieces = View::fresh().hint(&t);
    let ids: std::collections::BTreeSet<_> = OF_THE_VIEW.iter().map(|(_, id)| *id).collect();
    assert_eq!(pieces.len(), 3 * ids.len() - 1);
    for (notation, id) in OF_THE_VIEW {
        for key in [(*id).to_owned(), format!("cmd-{id}")] {
            assert!(!t.t(&key).starts_with('['), "`{key}` has no text");
        }
        assert!(
            ronoctua_keymap::Key::parse(notation).is_ok(),
            "`{notation}` is not a key"
        );
        assert!(
            press_of(id).is_some(),
            "`{notation}` teaches `{id}`, and no press answers to it"
        );
    }
}

#[test]
fn the_key_that_reaches_the_sync_is_in_the_table() {
    // IT USED TO BE THE FIRST THING ELIDED: in the old hand-written strip `c`
    // sat 23 cells past the cut at every real width, so the key that opens
    // ClipCascade was the one the line could never teach.
    assert!(
        OF_THE_VIEW
            .iter()
            .any(|(k, id)| *k == "c" && *id == "clipboard-sync")
    );
}

#[test]
fn the_cursor_only_stops_where_a_key_does_something() {
    // WALKING THROUGH ROWS WHERE NOTHING LIGHTS UP is what the owner read on
    // the locale view, and it is what a heading or a fact would be here.
    let t = texts();
    let queue = queue_of(&["one", "two"]);
    let sync = nothing();
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let mut v = View::fresh();
    let mut seen = Vec::new();
    for _ in 0..12 {
        let before = v.cursor();
        let _ = v.key(Press::Down, &facts, WIDE, &t);
        if v.cursor() == before {
            break;
        }
        seen.push(v.cursor());
        assert!(
            !matches!(v.target_at(&facts, WIDE, &t), Target::Nothing),
            "the cursor stopped on row {} where RET does nothing",
            v.cursor()
        );
    }
    assert!(!seen.is_empty(), "the cursor never moved");
}

#[test]
fn the_qr_is_a_toggle_and_escape_puts_it_away_before_it_closes() {
    // WITHOUT THE TOGGLE the only way out of a code on screen is `Esc`, which
    // also closes the view — so looking at a QR code and going back to the
    // list would be impossible.
    let t = texts();
    let queue = queue_of(&["a thing to encode"]);
    let sync = nothing();
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let mut v = View::fresh();
    // NO `<down>` FIRST. It was here because the cursor was born on row zero,
    // which is the `History` HEADING, so one press was spent reaching the
    // first entry — this view is settled now, like every other, and the
    // opening row is one that acts.
    assert_eq!(v.key(Press::Qr, &facts, WIDE, &t), Errand::Qr(0));
    assert_eq!(v.qr(), Some(0));
    assert_eq!(v.key(Press::Qr, &facts, WIDE, &t), Errand::Qr(0));
    assert_eq!(v.qr(), None, "a second press did not put it away");

    let _ = v.key(Press::Qr, &facts, WIDE, &t);
    assert_eq!(v.key(Press::Escape, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.qr(), None);
    // AND ONLY THEN does it close.
    assert_eq!(v.key(Press::Escape, &facts, WIDE, &t), Errand::Close);
}

#[test]
fn an_unconfigured_sync_offers_the_two_rows_that_set_it_up() {
    // THE OLD LINE SAID `ClipCascade: not integrated` and could say nothing
    // else — it was a build-time fact in the warning tone. What is drawn now
    // is what is true of THIS machine, and it is reachable.
    let t = texts();
    let queue = Queue::default();
    let sync = nothing();
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let v = View::fresh();
    let targets: Vec<Target> = (0..v.rows(&facts, WIDE, &t).len())
        .map(|n| {
            let mut probe = View::fresh();
            probe.clicked(n);
            probe.target_at(&facts, WIDE, &t)
        })
        .collect();
    assert!(targets.contains(&Target::Server), "{targets:?}");
    assert!(targets.contains(&Target::Account), "{targets:?}");
    // AND NO SWITCH, because there is nothing yet to switch on.
    assert!(
        !targets.iter().any(|t| matches!(t, Target::Sync(_))),
        "{targets:?}"
    );
}

#[test]
fn a_configured_sync_says_which_of_the_three_states_it_is_in() {
    // OFF, DOWN AND UP ARE THREE SENTENCES. Collapsing them would leave a
    // person unable to tell "I turned it off" from "the server is not
    // answering", which are acted on differently.
    let t = texts();
    let queue = Queue::default();
    let shot = |enabled, connected| ronoctua_clipcascade::Snapshot {
        configured: true,
        enabled,
        connected,
        server: "https://clip.example.org".to_owned(),
        username: "someone".to_owned(),
        ..ronoctua_clipcascade::Snapshot::default()
    };
    let said = |sync: &ronoctua_clipcascade::Snapshot| {
        let facts = Facts {
            queue: &queue,
            sync,
        };
        View::fresh()
            .rows(&facts, WIDE, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|")
    };
    let off = said(&shot(false, false));
    let down = said(&shot(true, false));
    let up = said(&shot(true, true));
    assert_ne!(off, down, "off and down read the same");
    assert_ne!(down, up, "down and up read the same");
    // AND THE SERVER IS NAMED IN ALL THREE, because "where is it syncing to"
    // is the question somebody has while reading any of them.
    //
    // AT A WIDTH WHERE IT FITS. In an eighty-column footer the right-hand
    // column is forty-six cells and the label column eats into them, so the
    // address is elided — which is correct, and is what the window is for. The
    // promise is that it is DRAWN and cut with an ellipsis, never dropped.
    for said in [&off, &down, &up] {
        assert!(said.contains("clip.example"), "{said}");
        assert!(
            said.contains('…') || said.contains("clip.example.org"),
            "the address was cut with no ellipsis: {said}"
        );
    }
    let wide = {
        let sync = shot(true, true);
        let facts = Facts {
            queue: &queue,
            sync: &sync,
        };
        View::fresh()
            .rows(&facts, 160, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|")
    };
    assert!(
        wide.contains("https://clip.example.org"),
        "a wide region still cut the address: {wide}"
    );
}

#[test]
fn the_history_column_leaves_the_rest_to_the_detail_and_never_overlaps() {
    // The split is written once here and was recomputed by the painting code
    // before. At every width from a narrow footer to a wide window, the two
    // have to stay apart.
    for columns in [40u16, 60, 80, 120, 200] {
        let seam = history_columns(columns);
        assert!(seam < columns, "{columns}: the history took the whole line");
        assert!(
            detail_at(columns) > seam,
            "{columns}: the detail starts on the rule"
        );
        assert!(
            detail_at(columns) < columns,
            "{columns}: the detail starts off the screen"
        );
    }
}

#[test]
fn the_cursor_reaches_every_settings_row_and_every_preference_has_a_door() {
    // EVERY FIELD OF `Preferences` IS REACHABLE FROM THIS VIEW.
    //
    // Six of them were not. `images`, `notify`, `cap`, `salt`, `rounds` and
    // `mark` existed in the crate, were written to disk, were read by the
    // modeline — and no row anywhere drew them, so their value was whatever
    // the default was, for ever. This walks the view the way a finger does and
    // demands that each one be stopped on.
    let t = texts();
    let queue = queue_of(&["one", "two", "three"]);
    let sync = ronoctua_clipcascade::Snapshot {
        configured: true,
        enabled: true,
        connected: true,
        server: "https://clip.example.org".to_owned(),
        username: "someone".to_owned(),
        ..ronoctua_clipcascade::Snapshot::default()
    };
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let mut v = View::fresh();
    let mut seen = std::collections::BTreeSet::new();
    // One press per row, plus slack: the walk stops at the bottom by itself.
    for _ in 0..64 {
        seen.insert(format!("{:?}", v.target_at(&facts, WIDE, &t)));
        let _ = v.key(Press::Down, &facts, WIDE, &t);
    }
    for wanted in [
        "Images",
        "Notify",
        "Cap",
        "Salt",
        "Rounds",
        "Mark",
        "Server",
        "Account",
        "Forget",
        "Sync(false)",
    ] {
        assert!(
            seen.iter().any(|s| s == wanted),
            "`<down>` never stopped on {wanted}; it saw {seen:?}"
        );
    }
}

#[test]
fn turning_a_ring_keeps_the_whole_record_and_moves_one_field() {
    // THE MERGE, which is why `Settings` carries six fields and not one. A
    // door that sent back only what it changed would let the boundary keep a
    // value it read some time ago; a door that sent the whole `Preferences`
    // would carry a `server` and a `username` that a login may have moved in
    // between.
    use ronoctua_clipcascade::prefs::Mark;
    let t = texts();
    let queue = Queue::default();
    let sync = ronoctua_clipcascade::Snapshot {
        configured: true,
        enabled: true,
        server: "https://clip.example.org".to_owned(),
        username: "someone".to_owned(),
        salt: "pepper".to_owned(),
        rounds: 1234,
        cap: 4096,
        images: false,
        notify: true,
        mark: Mark::Never,
        ..ronoctua_clipcascade::Snapshot::default()
    };
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let mut v = View::fresh();
    // Walk to the images row and turn it.
    let mut kept = None;
    for _ in 0..64 {
        if format!("{:?}", v.target_at(&facts, WIDE, &t)) == "Images" {
            let Errand::Keep(s) = v.key(Press::More, &facts, WIDE, &t) else {
                panic!("turning the images row asked for nothing");
            };
            kept = Some(s);
            break;
        }
        let _ = v.key(Press::Down, &facts, WIDE, &t);
    }
    let kept = kept.expect("the images row was never reached");
    assert!(kept.images, "the switch did not move");
    assert_eq!(kept.salt, "pepper", "the salt was not carried");
    assert_eq!(kept.rounds, 1234, "the rounds were not carried");
    assert_eq!(kept.cap, 4096, "the cap was not carried");
    assert!(kept.notify, "the other switch was not carried");
    assert_eq!(kept.mark, Mark::Never, "the ring was not carried");
}

#[test]
fn the_ring_turns_both_ways_and_comes_back_to_where_it_started() {
    use ronoctua_clipcascade::prefs::Mark;
    let t = texts();
    let queue = Queue::default();
    let at = |mark| ronoctua_clipcascade::Snapshot {
        configured: true,
        enabled: true,
        server: "https://clip.example.org".to_owned(),
        username: "someone".to_owned(),
        mark,
        ..ronoctua_clipcascade::Snapshot::default()
    };
    let turn = |mark, forward| {
        let sync = at(mark);
        let facts = Facts {
            queue: &queue,
            sync: &sync,
        };
        let mut v = View::fresh();
        for _ in 0..64 {
            if format!("{:?}", v.target_at(&facts, WIDE, &t)) == "Mark" {
                let press = if forward { Press::More } else { Press::Less };
                let Errand::Keep(s) = v.key(press, &facts, WIDE, &t) else {
                    panic!("the mark row asked for nothing");
                };
                return s.mark;
            }
            let _ = v.key(Press::Down, &facts, WIDE, &t);
        }
        panic!("the mark row was never reached");
    };
    for mark in Mark::ALL {
        // BACKWARDS IS `len - 1` FORWARD, and this is what holds that: `at - 1`
        // would underflow at zero, which in `usize` is not a smaller number.
        assert_eq!(
            turn(turn(mark, true), false),
            mark,
            "{mark:?} did not return"
        );
        assert_ne!(turn(mark, true), mark, "{mark:?} did not move");
    }
}

#[test]
fn every_key_of_the_table_is_a_notation_a_key_can_produce() {
    // A HINT THAT TEACHES A KEY NOBODY CAN PRESS.
    //
    // The shifted arrows went in as one row reading `S-<left>/<right>`, which
    // is what a person would write — and this table is matched against a real
    // key's own notation, so that row would have been drawn on the hint line
    // and answered nothing at all.
    for (notation, id) in OF_THE_VIEW {
        assert!(
            press_of(id).is_some(),
            "`{notation}` names `{id}`, which this view does not answer"
        );
        use ronoctua_keymap::Sym;
        let named = [
            Sym::Space,
            Sym::Enter,
            Sym::Tab,
            Sym::Esc,
            Sym::Backspace,
            Sym::Left,
            Sym::Right,
            Sym::Up,
            Sym::Down,
        ];
        let letters = (0x21u8..=0x7e).map(|b| Sym::Char(b as char));
        let made = named.into_iter().chain(letters).any(|sym| {
            [false, true].into_iter().any(|shift| {
                ronoctua_keymap::Key {
                    mods: ronoctua_keymap::Mods {
                        shift,
                        ..ronoctua_keymap::Mods::default()
                    },
                    sym,
                }
                .notation()
                    == *notation
            })
        });
        assert!(made, "no key produces the notation `{notation}`");
    }
}

#[test]
fn a_fresh_view_draws_its_cursor_on_the_row_ret_acts_on() {
    // THE CURSOR IS A COLOUR, AND IT IS WHERE `RET` AND A CLICK ACT
    // (`View::here`). The entries were coloured by the raw cursor, which a
    // fresh view keeps on the `History` heading: no entry was lit, `RET`
    // copied the first one, and a first click on that unlit entry copied it
    // too, where invariant 19 has it select.
    let t = texts();
    let queue = queue_of(&["one", "two"]);
    let sync = nothing();
    let facts = Facts {
        queue: &queue,
        sync: &sync,
    };
    let mut v = View::fresh();
    let here = v.here(&facts, WIDE, &t);
    let lit: Vec<usize> = v
        .rows(&facts, WIDE, &t)
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.fields
                .iter()
                .any(|f| f.color == tokens::color::TEXT_HERE_ACTS)
        })
        .map(|(n, _)| n)
        .collect();
    assert_eq!(
        lit,
        vec![here],
        "the cursor is drawn elsewhere than `RET` acts"
    );
    let Target::Entry(n) = v.target_at(&facts, WIDE, &t) else {
        panic!("a fresh view over two entries is not on one");
    };
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Copy(n));
}
