//! What the history view promises, and what it must never do.

use super::*;

fn texts() -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh("en-GB").unwrap()
}

const WIDE: u16 = 80;

fn shot(logged_in: bool) -> ronoctua_atuin::Snapshot {
    ronoctua_atuin::Snapshot {
        present: true,
        logged_in,
        syncing: logged_in,
        address: if logged_in {
            "https://history.example.org".to_owned()
        } else {
            String::new()
        },
        username: if logged_in {
            "someone".to_owned()
        } else {
            String::new()
        },
        entries: 1234,
        ..ronoctua_atuin::Snapshot::default()
    }
}

#[test]
fn the_cursor_opens_on_a_row_that_acts() {
    // THE LOCALE VIEW OPENED ON ROW ZERO, which is a heading — so nothing was
    // lit and `RET` fell on `Target::Nothing`. From outside that is a screen
    // where no key does anything, which is exactly what the owner reported.
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let v = View::fresh();
    assert!(
        !matches!(v.target_at(&facts, WIDE, &t), Target::Nothing),
        "the view opened on a row where RET does nothing"
    );
}

#[test]
fn the_cursor_never_stops_on_a_heading_or_a_fact() {
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    for _ in 0..40 {
        let before = v.cursor();
        let _ = v.key(Press::Down, &facts, WIDE, &t);
        if v.cursor() == before {
            break;
        }
        assert!(
            !matches!(v.target_at(&facts, WIDE, &t), Target::Nothing),
            "the cursor stopped on row {} where RET does nothing",
            v.cursor()
        );
    }
}

#[test]
fn the_filters_are_drawn_beside_the_switch_and_never_in_the_drawer() {
    // THE OBJECTION `nix/modules/home.nix` STATED was "a command with a token
    // in argv". These are what stop such a command being RECORDED, so they
    // cannot leave — and a safeguard behind a closed chevron is not one.
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let drawn = View::fresh()
        .rows(&facts, WIDE, &t)
        .iter()
        .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
        .collect::<Vec<_>>()
        .join("|");
    for key in [
        ronoctua_i18n::key::ATUIN_OPT_SECRETS_FILTER,
        ronoctua_i18n::key::ATUIN_OPT_HISTORY_FILTER,
        ronoctua_i18n::key::ATUIN_OPT_AUTO_SYNC,
    ] {
        assert!(drawn.contains(&t.t(key)), "`{key}` is not drawn: {drawn}");
    }
}

#[test]
fn the_key_is_behind_a_question_that_opens_on_no() {
    // IT IS THE PHRASE THAT DECRYPTS THE HISTORY. A `RET` that showed it
    // straight away would put it on screen because somebody was walking the
    // list — the rule the Bluetooth pairing and the format buffer follow.
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let rows = v.build(&facts, WIDE, &t);
    let at = rows
        .iter()
        .position(|(_, target)| matches!(target, Target::Key))
        .expect("no row shows the key");
    v.clicked(at);
    // THE FIRST PRESS ONLY ASKS.
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Nothing);
    assert!(v.asking());
    // THE SECOND ONE IS THE ANSWER.
    let again = v
        .build(&facts, WIDE, &t)
        .iter()
        .position(|(_, target)| matches!(target, Target::Key))
        .expect("the question has no yes");
    v.clicked(again);
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::ShowKey);
    // AND `Esc` PUTS IT AWAY BEFORE IT CLOSES THE VIEW: a recovery phrase on
    // screen is a thing that is open.
    v.took_the_key("a b c d".to_owned());
    assert_eq!(v.key(Press::Escape, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.key(Press::Escape, &facts, WIDE, &t), Errand::Close);
}

#[test]
fn a_machine_with_no_atuin_says_so_and_offers_nothing() {
    // EVERY ROW BELOW WOULD BE ABOUT A PROGRAM THAT IS NOT THERE.
    let t = texts();
    let s = ronoctua_atuin::Snapshot::default();
    let facts = Facts { shot: &s };
    let v = View::fresh();
    let rows = v.rows(&facts, WIDE, &t);
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert!(
        rows[0].fields[0]
            .text
            .contains(&t.t(ronoctua_i18n::key::ATUIN_ABSENT))
    );
}

#[test]
fn what_the_flake_owns_refuses_rather_than_lying_about_its_value() {
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    v.drawer = true;
    let rows = v.build(&facts, WIDE, &t);
    let at = rows
        .iter()
        .position(|(_, target)| matches!(target, Target::Setting("update-check")))
        .expect("update-check is not drawn");
    v.clicked(at);
    // IT IS REACHABLE AND IT REFUSES — the disabled XFS row's shape.
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.key(Press::More, &facts, WIDE, &t), Errand::Nothing);
    let said: String = rows[at].0.iter().map(|f| f.text.clone()).collect();
    assert!(
        said.contains(&t.t(ronoctua_i18n::key::ATUIN_FROM_FLAKE)),
        "{said}"
    );
}

#[test]
fn a_ring_turns_into_an_order_and_a_text_asks_to_be_typed() {
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let rows = v.build(&facts, WIDE, &t);
    let ring = rows
        .iter()
        .position(|(_, target)| matches!(target, Target::Setting("search-mode")))
        .expect("search-mode is not drawn");
    v.clicked(ring);
    let Errand::Order(ronoctua_atuin::Order::Set { key, value, .. }) =
        v.key(Press::More, &facts, WIDE, &t)
    else {
        panic!("a ring did not become an order");
    };
    assert_eq!(key, "search_mode");
    // AND THE QUOTES ARE THERE. `search_mode = fuzzy` is not TOML, and Atuin
    // then falls back to its defaults for EVERYTHING, silently.
    assert!(value.starts_with('"') && value.ends_with('"'), "{value}");

    let typed = rows
        .iter()
        .position(|(_, target)| matches!(target, Target::Setting("history-filter")))
        .expect("history-filter is not drawn");
    v.clicked(typed);
    assert_eq!(
        v.key(Press::Enter, &facts, WIDE, &t),
        Errand::Edit("history-filter")
    );
}

#[test]
fn the_hint_teaches_every_key_and_every_key_has_words() {
    let t = texts();
    let ids: std::collections::BTreeSet<_> = OF_THE_VIEW.iter().map(|(_, id)| *id).collect();
    assert_eq!(View::fresh().hint(&t).len(), 3 * ids.len() - 1);
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
            "`{notation}` teaches `{id}` with no press"
        );
    }
}

#[test]
fn a_machine_that_has_not_logged_in_is_offered_the_way_in() {
    let t = texts();
    let s = shot(false);
    let facts = Facts { shot: &s };
    let targets: Vec<Target> = View::fresh()
        .build(&facts, WIDE, &t)
        .into_iter()
        .map(|(_, target)| target)
        .collect();
    assert!(targets.contains(&Target::LogIn), "{targets:?}");
    // AND NOT THE WAY OUT, nor a key that does not exist yet.
    assert!(!targets.contains(&Target::LogOut), "{targets:?}");
    assert!(!targets.contains(&Target::Key), "{targets:?}");
}

#[test]
fn the_modeline_ring_has_a_row_and_turns_both_ways() {
    // IT HAD A RING, A DEFAULT AND A READER, AND NO ROW. `prefs::Mark` was
    // written, read by the modeline and drawn by nothing, so its value was
    // whatever the file was born with — for ever.
    use ronoctua_atuin::prefs::Mark;
    let t = texts();
    let turn = |mark, forward| {
        let shot = ronoctua_atuin::Snapshot {
            present: true,
            logged_in: true,
            mark,
            ..ronoctua_atuin::Snapshot::default()
        };
        let facts = Facts { shot: &shot };
        let mut v = View::fresh();
        for _ in 0..200 {
            if v.target_at(&facts, WIDE, &t) == Target::Mark {
                let press = if forward { Press::More } else { Press::Less };
                let Errand::Order(ronoctua_atuin::Order::Mark(next)) =
                    v.key(press, &facts, WIDE, &t)
                else {
                    panic!("the mark row asked for nothing");
                };
                return next;
            }
            let _ = v.key(Press::Down, &facts, WIDE, &t);
        }
        panic!("`<down>` never stopped on the mark row");
    };
    for mark in Mark::ALL {
        assert_ne!(turn(mark, true), mark, "{mark:?} did not move");
        // BACKWARDS IS `len - 1` FORWARD: `at - 1` underflows at zero, and in
        // `usize` that is not a smaller number.
        assert_eq!(
            turn(turn(mark, true), false),
            mark,
            "{mark:?} did not return"
        );
    }
}

#[test]
fn a_switch_says_on_or_off_and_never_what_the_service_is_doing() {
    // `Kind::Switch` RESOLVED TO THE GLANCE'S WORDS. Every declared switch
    // drew `atuin-syncing` / `atuin-not-syncing`, so `update_check` read
    // "check for a new version   staying on this machine".
    let t = texts();
    let syncing = t.t(ronoctua_i18n::key::ATUIN_SYNCING);
    let not = t.t(ronoctua_i18n::key::ATUIN_NOT_SYNCING);
    let mut settings = std::collections::BTreeMap::new();
    for setting in crate::history::options::SETTINGS {
        if matches!(setting.kind, ronoctua_options::Kind::Switch) {
            settings.insert(setting.id.replace('-', "_"), "true".to_owned());
        }
    }
    let shot = ronoctua_atuin::Snapshot {
        present: true,
        logged_in: true,
        settings,
        ..ronoctua_atuin::Snapshot::default()
    };
    let facts = Facts { shot: &shot };
    let mut v = View::fresh();
    // The drawer holds the rest of the table; open it so every switch is drawn.
    for _ in 0..200 {
        if v.target_at(&facts, WIDE, &t) == Target::Drawer {
            let _ = v.key(Press::Enter, &facts, WIDE, &t);
            break;
        }
        let _ = v.key(Press::Down, &facts, WIDE, &t);
    }
    // THE GLANCE'S OWN ROW KEEPS THOSE WORDS, and should: there the question
    // IS whether the history is syncing. What may not carry them is a
    // DECLARED SETTING, so only those rows are read.
    let rows = v.rows(&facts, WIDE, &t);
    let mut switches = 0;
    for (n, row) in rows.iter().enumerate() {
        v.clicked(n);
        let Target::Setting(id) = v.target_at(&facts, WIDE, &t) else {
            continue;
        };
        let Some(setting) = crate::history::options::SETTINGS
            .iter()
            .find(|s| s.id == id)
        else {
            continue;
        };
        if !matches!(setting.kind, ronoctua_options::Kind::Switch) {
            continue;
        }
        switches += 1;
        for field in &row.fields {
            assert_ne!(
                field.text, syncing,
                "`{id}` still answers with the glance's word"
            );
            assert_ne!(
                field.text, not,
                "`{id}` still answers with the glance's word"
            );
        }
    }
    assert!(switches > 0, "no switch was drawn, so nothing was measured");
}
