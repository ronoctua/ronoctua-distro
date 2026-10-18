//! What the history view promises, and what it must never do.

use super::*;

fn texts() -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh("en-GB").unwrap()
}

const WIDE: u16 = 80;

/// Puts the cursor on row `at`, as a key walking there would — the row's
/// identity with it.
fn aim(v: &mut View, at: usize) {
    v.cursor = at;
    v.on = None;
}

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
        crate::key::ATUIN_OPT_SECRETS_FILTER,
        crate::key::ATUIN_OPT_HISTORY_FILTER,
        crate::key::ATUIN_OPT_AUTO_SYNC,
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
    aim(&mut v, at);
    // THE FIRST PRESS ONLY ASKS.
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Nothing);
    assert!(v.asking());
    // THE SECOND ONE IS THE ANSWER.
    let again = v
        .build(&facts, WIDE, &t)
        .iter()
        .position(|(_, target)| matches!(target, Target::Key))
        .expect("the question has no yes");
    aim(&mut v, again);
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
            .contains(&t.t(crate::key::ATUIN_ABSENT))
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
    aim(&mut v, at);
    // IT IS REACHABLE AND IT REFUSES — the disabled XFS row's shape.
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.key(Press::More, &facts, WIDE, &t), Errand::Nothing);
    let said: String = rows[at].0.iter().map(|f| f.text.clone()).collect();
    assert!(said.contains(&t.t(crate::key::ATUIN_FROM_FLAKE)), "{said}");
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
    aim(&mut v, ring);
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
    aim(&mut v, typed);
    assert_eq!(
        v.key(Press::Enter, &facts, WIDE, &t),
        Errand::Ask(Question::SETTING)
    );
    assert_eq!(v.question(), Some(Question::SETTING));
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
    let syncing = t.t(crate::key::ATUIN_SYNCING);
    let not = t.t(crate::key::ATUIN_NOT_SYNCING);
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
    let built = v.build(&facts, WIDE, &t);
    let mut switches = 0;
    for (row, (_, target)) in rows.iter().zip(&built) {
        let Target::Setting(id) = *target else {
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

#[test]
fn the_row_the_keyboard_is_on_is_drawn_in_another_colour() {
    // WHAT THE OWNER READ ON THE SCREEN: nothing said which line the cursor
    // was on. `Row::focused` was set and is read by NO door in this system —
    // neither the footer nor the window draws a band behind the cursor — so
    // the colour is the only thing that could say it, and this view was the
    // one that never set it.
    let t = texts();
    let shot = ronoctua_atuin::Snapshot {
        present: true,
        logged_in: true,
        ..ronoctua_atuin::Snapshot::default()
    };
    let facts = Facts { shot: &shot };
    let mut v = View::fresh();
    // Every row the cursor can stop on, one after another.
    for _ in 0..200 {
        let rows = v.rows(&facts, WIDE, &t);
        let Some(here) = rows.iter().position(|r| r.focused) else {
            panic!("no row was focused at all");
        };
        assert!(
            rows[here]
                .fields
                .iter()
                .any(|f| f.color == tokens::color::STATE_ACCENT),
            "the focused row carries no accent: {:?}",
            rows[here]
                .fields
                .iter()
                .map(|f| &f.text)
                .collect::<Vec<_>>()
        );
        // AND NOBODY ELSE WEARS IT. An accent on two rows says the keyboard is
        // in two places, which is the same as saying nothing.
        for (n, row) in rows.iter().enumerate() {
            if n == here {
                continue;
            }
            assert!(
                !row.fields
                    .iter()
                    .any(|f| f.color == tokens::color::STATE_ACCENT),
                "row {n} wears the cursor's accent and is not the cursor"
            );
        }
        let before = v.cursor();
        let _ = v.key(Press::Down, &facts, WIDE, &t);
        if v.cursor() == before {
            break;
        }
    }
}

#[test]
fn a_row_the_flake_owns_keeps_saying_so_under_the_cursor() {
    // THE VALUE'S HALF IS THE OTHER ANSWER. A setting the flake fixes is drawn
    // and reachable and REFUSES (invariant 6), so the cursor may not make it
    // look like something that changes: the label takes the accent and the
    // value takes `text.here.read`, never `text.here.acts`.
    let t = texts();
    let Some(fixed) = crate::history::options::SETTINGS.iter().find(|s| s.fixed) else {
        return;
    };
    let shot = ronoctua_atuin::Snapshot {
        present: true,
        logged_in: true,
        ..ronoctua_atuin::Snapshot::default()
    };
    let facts = Facts { shot: &shot };
    let mut v = View::fresh();
    // THE DRAWER FIRST. What the flake owns is rarely touched by definition,
    // so it is in there — a walk over the closed view never reaches it.
    for _ in 0..200 {
        if v.target_at(&facts, WIDE, &t) == Target::Drawer {
            let _ = v.key(Press::Enter, &facts, WIDE, &t);
            break;
        }
        let before = v.cursor();
        let _ = v.key(Press::Down, &facts, WIDE, &t);
        if v.cursor() == before {
            break;
        }
    }
    for _ in 0..200 {
        if v.target_at(&facts, WIDE, &t) == Target::Setting(fixed.id) {
            let rows = v.rows(&facts, WIDE, &t);
            let here = rows.iter().position(|r| r.focused).expect("no cursor");
            let colours: Vec<_> = rows[here].fields.iter().map(|f| f.color).collect();
            assert!(
                colours.contains(&tokens::color::TEXT_HERE_READ),
                "`{}` is owned by the flake and reads as a row that answers",
                fixed.id
            );
            assert!(
                !colours.contains(&tokens::color::TEXT_HERE_ACTS),
                "`{}` reads as a row that answers",
                fixed.id
            );
            return;
        }
        let before = v.cursor();
        let _ = v.key(Press::Down, &facts, WIDE, &t);
        if v.cursor() == before {
            break;
        }
    }
    panic!("the flake-owned row was never reached, drawer open");
}

/// Where the first row carrying `target` is.
fn row_of(v: &View, facts: &Facts<'_>, target: &Target) -> usize {
    v.build(facts, WIDE, &texts())
        .iter()
        .position(|(_, t)| t == target)
        .unwrap_or_else(|| panic!("no row carries {target:?}"))
}

/// THE LOGIN IS FOUR QUESTIONS AND ONE ORDER, asked by the view in either
/// door: the address and the account in the clear, the password and the key
/// hidden — and the order carries all four.
#[test]
fn a_login_is_four_questions_and_one_order() {
    let t = texts();
    let s = shot(false);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let login = row_of(&v, &facts, &Target::LogIn);
    aim(&mut v, login);
    assert_eq!(
        v.key(Press::Enter, &facts, WIDE, &t),
        Errand::Ask(Question::SERVER)
    );
    assert_eq!(
        v.answered("atuin-server", "  https://history.example.org "),
        Errand::Ask(Question::ACCOUNT)
    );
    assert_eq!(
        v.answered("atuin-account", " someone "),
        Errand::Ask(Question::PASSWORD)
    );
    // HIDDEN: the password and the key, and nothing else.
    let hidden: Vec<&str> = Question::ALL
        .iter()
        .filter(|q| q.secret)
        .map(|q| q.id)
        .collect();
    assert_eq!(hidden, ["atuin-password", "atuin-key"]);
    assert_eq!(
        v.answered("atuin-password", " s3nh4 "),
        Errand::Ask(Question::KEY)
    );
    assert_eq!(
        v.answered("atuin-key", " one two three "),
        Errand::Order(ronoctua_atuin::Order::LogIn {
            address: "https://history.example.org".to_owned(),
            username: "someone".to_owned(),
            // AS TYPED: a space is a password's own.
            password: " s3nh4 ".to_owned(),
            key: "one two three".to_owned(),
        })
    );
    assert_eq!(v.question(), None, "the login outlived its order");
    // AND EVERY QUESTION HAS ITS WORDS, in every language.
    for language in ronoctua_i18n::LANGUAGES {
        let texts = ronoctua_i18n::Texts::fresh(language).expect(language);
        for q in Question::ALL {
            let key = format!("ask-{}", q.id);
            assert!(!texts.t(&key).starts_with('['), "`{key}` in {language}");
        }
    }
}

/// `RET` ON NOTHING ENDS A LOGIN, and an answer nobody is waiting for — a
/// question let go, or another question's id — is nothing: never a path
/// typed into a password.
#[test]
fn an_empty_or_stray_answer_is_nothing() {
    let t = texts();
    let s = shot(false);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let login = row_of(&v, &facts, &Target::LogIn);

    aim(&mut v, login);
    let _ = v.key(Press::Enter, &facts, WIDE, &t);
    assert_eq!(v.answered("atuin-server", "   "), Errand::Nothing);
    assert_eq!(v.question(), None);

    aim(&mut v, login);
    let _ = v.key(Press::Enter, &facts, WIDE, &t);
    let _ = v.answered("atuin-server", "https://h.example.org");
    assert_eq!(v.answered("atuin-account", ""), Errand::Nothing);
    assert_eq!(v.question(), None);

    // ANOTHER QUESTION'S ID is not this one's answer, and it ends it.
    aim(&mut v, login);
    let _ = v.key(Press::Enter, &facts, WIDE, &t);
    assert_eq!(v.answered("atuin-setting", "x"), Errand::Nothing);
    assert_eq!(v.answered("atuin-server", "h"), Errand::Nothing);

    // NOTHING ASKED, nothing taken.
    assert_eq!(v.answered("atuin-key", "k"), Errand::Nothing);
}

/// A QUESTION LET GO TAKES THE PASSWORD WITH IT — by the line (`Esc`), or by
/// a key reaching the view, which in either door means the line is gone.
/// And while it waits for the key, the password is in no `Debug`.
#[test]
fn a_question_let_go_takes_the_password_with_it() {
    let t = texts();
    let s = shot(false);
    let facts = Facts { shot: &s };
    let to_the_key = |v: &mut View| {
        let login = row_of(v, &facts, &Target::LogIn);
        aim(v, login);
        let _ = v.key(Press::Enter, &facts, WIDE, &t);
        let _ = v.answered("atuin-server", "https://h.example.org");
        let _ = v.answered("atuin-account", "someone");
        let _ = v.answered("atuin-password", "s3nh4-secreta");
        assert_eq!(v.question(), Some(Question::KEY));
    };

    let mut v = View::fresh();
    to_the_key(&mut v);
    let printed = format!("{v:?}");
    assert!(
        !printed.contains("s3nh4"),
        "the password reached Debug: {printed}"
    );
    v.let_go();
    assert_eq!(v.question(), None);
    assert_eq!(v.answered("atuin-key", "k"), Errand::Nothing);

    let mut v = View::fresh();
    to_the_key(&mut v);
    let _ = v.key(Press::Down, &facts, WIDE, &t);
    assert_eq!(v.question(), None, "a key left the login standing");
    assert_eq!(v.answered("atuin-key", "k"), Errand::Nothing);
}

/// A TYPED VALUE IS THE SETTING'S, in TOML's spelling — and nothing typed
/// writes nothing: `key = ` is not TOML, and Atuin then drops EVERY setting.
#[test]
fn a_typed_value_is_the_settings_and_nothing_typed_writes_nothing() {
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let filter = row_of(&v, &facts, &Target::Setting("history-filter"));
    aim(&mut v, filter);
    assert_eq!(
        v.key(Press::Enter, &facts, WIDE, &t),
        Errand::Ask(Question::SETTING)
    );
    let Errand::Order(ronoctua_atuin::Order::Set { key, value, .. }) =
        v.answered("atuin-setting", r#"["^ ", "^secret"]"#)
    else {
        panic!("a typed value became no order");
    };
    assert_eq!(key, "history_filter");
    assert_eq!(value, r#"["^ ", "^secret"]"#);

    aim(&mut v, filter);
    let _ = v.key(Press::Enter, &facts, WIDE, &t);
    assert_eq!(v.answered("atuin-setting", "   "), Errand::Nothing);
}

/// THE DEFECT: the key's question grows a sentence ABOVE its row, the cursor
/// was left on the sentence, and the second `RET` — "yes, show it" — turned
/// the modeline's ring at the top of the page. The cursor keeps the row it
/// was on, through all three of its states.
#[test]
fn the_second_ret_on_the_key_answers_its_question_where_the_cursor_stands() {
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let key_row = row_of(&v, &facts, &Target::Key);
    // BY KEYS ONLY, walking down to it.
    for _ in 0..200 {
        if v.target_at(&facts, WIDE, &t) == Target::Key {
            break;
        }
        let _ = v.key(Press::Down, &facts, WIDE, &t);
    }
    assert_eq!(v.cursor(), key_row);
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Key);
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::ShowKey);
    v.took_the_key("one two three".to_owned());
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Key);
    let shown: String = v
        .rows(&facts, WIDE, &t)
        .iter()
        .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
        .collect();
    assert!(shown.contains("one two three"), "{shown}");
    // `RET` ON THE SHOWN KEY PUTS IT AWAY, as `Esc` does, and the cursor is
    // back on the row that asks.
    assert_eq!(v.key(Press::Enter, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Key);
    assert_eq!(v.cursor(), key_row);
    // `Esc` FROM THE QUESTION leaves the cursor on the key's row too, and not
    // on the row below it — which is "log out".
    let _ = v.key(Press::Enter, &facts, WIDE, &t);
    assert_eq!(v.key(Press::Escape, &facts, WIDE, &t), Errand::Nothing);
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Key);
}

/// A ROW THAT GOES AWAY leaves the cursor NEAR it: logged in, "log in to a
/// server" is gone, and the cursor lands in the account's own section — not
/// on the modeline's ring at the top of the page, where a `RET` turned it.
#[test]
fn a_row_that_goes_away_leaves_the_cursor_near_where_it_was() {
    let t = texts();
    let out = shot(false);
    let mut v = View::fresh();
    let login = row_of(&v, &Facts { shot: &out }, &Target::LogIn);
    aim(&mut v, login);
    let _ = v.key(Press::Enter, &Facts { shot: &out }, WIDE, &t);
    v.let_go();
    let inside = shot(true);
    assert_eq!(
        v.target_at(&Facts { shot: &inside }, WIDE, &t),
        Target::Sync
    );
}

/// INVARIANT 19: the first click selects, and a click on the row the cursor
/// is already on is its `RET`. A heading or a fact takes no click — it went
/// through `settle` to the first row that acts, the modeline's ring.
#[test]
fn the_first_click_selects_and_the_second_acts() {
    let t = texts();
    let s = shot(true);
    let facts = Facts { shot: &s };
    let mut v = View::fresh();
    let sync = row_of(&v, &facts, &Target::Sync);
    assert!(!v.clicked(sync, &facts, WIDE, &t), "the first click acted");
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Sync);
    assert!(
        v.clicked(sync, &facts, WIDE, &t),
        "the second click did not"
    );
    // A HEADING: row zero.
    assert!(!v.clicked(0, &facts, WIDE, &t));
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Sync);
    // AND A ROW PAST THE END.
    assert!(!v.clicked(10_000, &facts, WIDE, &t));
    assert_eq!(v.target_at(&facts, WIDE, &t), Target::Sync);
}

/// ONE READER FOR BOTH DOORS, by the whole notation: the shifted arrows are
/// the value keys, a bare arrow is not, and a chord is not the letter under
/// it.
#[test]
fn a_key_is_read_by_its_whole_notation() {
    let read = |notation: &str| press_of_key(ronoctua_keymap::Key::parse(notation).unwrap());
    assert_eq!(read("S-<right>"), Some(Press::More));
    assert_eq!(read("S-<left>"), Some(Press::Less));
    assert_eq!(read("<right>"), None);
    assert_eq!(read("<down>"), Some(Press::Down));
    assert_eq!(read("RET"), Some(Press::Enter));
    assert_eq!(read("s"), Some(Press::Sync));
    assert_eq!(read("C-s"), None);
    assert_eq!(read("ESC"), Some(Press::Escape));
    // EVERY KEY THE HINT TEACHES reads back as its own press.
    for (notation, id) in OF_THE_VIEW {
        assert_eq!(read(notation), press_of(id), "`{notation}`");
    }
}
