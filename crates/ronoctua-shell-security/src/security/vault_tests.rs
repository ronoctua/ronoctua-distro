//! THE CENTRE'S `/private` SECTION, driven as a person drives it — the vault's
//! states, questions and works through the security centre's view. Beside
//! the centre, which is what they drive; `vault.rs`'s own tests test the
//! vault's arithmetic and words.

use crate::key;
use crate::vault::*;
use ronoctua_lookup::keys::Secret;
use ronoctua_private::{Op, State, Status, Why};

fn texts(language: &str) -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh(language).expect("a declared language")
}

fn scene(state: State, fido2: bool) -> crate::security::Scene {
    crate::security::Scene {
        read: true,
        settings: Some(ronoctua_security::firewall::Settings::default()),
        helper: true,
        private: Some(Status {
            state,
            cap_bytes: 64 << 30,
            apparent_bytes: 64 << 30,
            real_bytes: 3 << 30,
            fido2,
            ..Status::absent()
        }),
        private_free: Some(100 << 30),
        ..crate::security::Scene::default()
    }
}

/// The rows of the vault's section that act.
fn acts(view: &crate::security::View, scene: &crate::security::Scene) -> Vec<Act> {
    view.rows(
        scene,
        100,
        &texts("en-GB"),
        &ronoctua_files::when::Zone::utc(),
    )
    .into_iter()
    .filter(|r| r.section == crate::security::Section::Private)
    .filter_map(|r| match r.act {
        Some(crate::security::Act::Private(a)) => Some(a),
        _ => None,
    })
    .collect()
}

/// Presses the row that does `act`: a click to choose it, `RET` to run it.
fn press(
    view: &mut crate::security::View,
    scene: &crate::security::Scene,
    act: Act,
) -> crate::security::Errand {
    let t = texts("en-GB");
    let zone = ronoctua_files::when::Zone::utc();
    let rows = view.rows(scene, 100, &t, &zone);
    let n = rows
        .iter()
        .position(|r| r.act == Some(crate::security::Act::Private(act)))
        .expect("the row is drawn");
    let _ = view.clicked(n, &rows);
    view.press(crate::security::Press::Enter, scene, 100, &t, &zone)
}

/// FAILS IF: a state of the vault offers the wrong rows — creating one
/// that exists, opening one that is open, the key's opening without a
/// key, locking one that is locked.
#[test]
fn each_state_offers_what_it_can_do() {
    let v = crate::security::View::fresh();
    assert_eq!(acts(&v, &scene(State::Absent, false)), [Act::Create]);
    assert_eq!(
        acts(&v, &scene(State::Locked, false)),
        [Act::OpenPassphrase, Act::Grow, Act::Enrol, Act::Passphrase]
    );
    assert_eq!(
        acts(&v, &scene(State::Locked, true)),
        [
            Act::OpenPassphrase,
            Act::OpenKey,
            Act::Grow,
            Act::Forget,
            Act::Passphrase
        ]
    );
    assert_eq!(
        acts(&v, &scene(State::Open, false)),
        [Act::Lock, Act::Show, Act::Grow, Act::Enrol, Act::Passphrase]
    );
    assert_eq!(
        acts(&v, &scene(State::SuspendedInUse, false))[0],
        Act::Resume
    );
    // AND A SUSPENDED VAULT IS IN "NOW", in the danger colour.
    let rows = v.rows(
        &scene(State::SuspendedInUse, false),
        100,
        &texts("en-GB"),
        &ronoctua_files::when::Zone::utc(),
    );
    assert!(
        rows.iter()
            .any(|r| r.section == crate::security::Section::Now
                && r.act == Some(crate::security::Act::Private(Act::Resume)))
    );
}

/// FAILS IF: making the vault stops asking the cap, then a passphrase
/// twice, before the door — or two passphrases that differ reach it.
#[test]
fn creating_asks_the_cap_and_the_passphrase_twice() {
    use crate::security::Errand;
    let t = texts("en-GB");
    let s = scene(State::Absent, false);
    let mut v = crate::security::View::fresh();
    let Errand::Ask {
        id: CAP,
        secret: false,
        initial: Some(initial),
    } = press(&mut v, &s, Act::Create)
    else {
        panic!("the cap is asked first");
    };
    assert_eq!(
        ronoctua_private::parse_size(&initial),
        Some(ronoctua_private::suggested_cap(100 << 30))
    );
    // A SIZE THAT IS NOT ONE is said, and the flow ends.
    assert!(matches!(v.answered(CAP, "lots", &s, &t), Errand::Say(_)));
    assert!(matches!(
        v.answered(NEW_PASSPHRASE, "x", &s, &t),
        Errand::Nothing
    ));
    // THE WHOLE WAY, with a mistyped repetition first.
    let _ = press(&mut v, &s, Act::Create);
    assert!(matches!(
        v.answered(CAP, "64G", &s, &t),
        Errand::Ask {
            id: NEW_PASSPHRASE,
            secret: true,
            ..
        }
    ));
    assert!(matches!(
        v.answered(NEW_PASSPHRASE, "correct horse", &s, &t),
        Errand::Ask {
            id: REPEAT_PASSPHRASE,
            secret: true,
            ..
        }
    ));
    assert_eq!(
        match v.answered(REPEAT_PASSPHRASE, "correct hrose", &s, &t) {
            Errand::Say(said) => said,
            other => panic!("{other:?}"),
        },
        t.t(key::ECHO_PRIVATE_MISMATCH)
    );
    let _ = press(&mut v, &s, Act::Create);
    let _ = v.answered(CAP, "64G", &s, &t);
    let _ = v.answered(NEW_PASSPHRASE, "correct horse", &s, &t);
    match v.answered(REPEAT_PASSPHRASE, "correct horse", &s, &t) {
        Errand::Door(crate::security::Work::Private(w)) => {
            assert_eq!(
                w.op,
                Op::Create {
                    cap_bytes: 64 * ronoctua_private::GB
                }
            );
            assert_eq!(
                w.passphrase.as_ref().map(Secret::expose),
                Some("correct horse")
            );
            assert!(w.new_passphrase.is_none());
        }
        other => panic!("{other:?}"),
    }
    // TOO SMALL a cap is said with the floor.
    let _ = press(&mut v, &s, Act::Create);
    assert!(matches!(v.answered(CAP, "100M", &s, &t), Errand::Say(_)));
}

/// FAILS IF: changing the passphrase stops asking the old one and the
/// new one twice, or a question let go keeps its secrets for a later
/// answer typed by name.
#[test]
fn a_new_passphrase_asks_the_old_and_the_new_twice() {
    use crate::security::Errand;
    let t = texts("en-GB");
    let s = scene(State::Locked, false);
    let mut v = crate::security::View::fresh();
    assert!(matches!(
        press(&mut v, &s, Act::Passphrase),
        Errand::Ask {
            id: PASSPHRASE,
            secret: true,
            ..
        }
    ));
    let _ = v.answered(PASSPHRASE, "old one", &s, &t);
    let _ = v.answered(NEW_PASSPHRASE, "new one", &s, &t);
    match v.answered(REPEAT_PASSPHRASE, "new one", &s, &t) {
        Errand::Door(crate::security::Work::Private(w)) => {
            assert_eq!(w.op, Op::Passphrase);
            assert_eq!(w.passphrase.as_ref().map(Secret::expose), Some("old one"));
            assert_eq!(
                w.new_passphrase.as_ref().map(Secret::expose),
                Some("new one")
            );
        }
        other => panic!("{other:?}"),
    }
    let _ = press(&mut v, &s, Act::Passphrase);
    v.let_go();
    assert!(matches!(
        v.answered(PASSPHRASE, "old one", &s, &t),
        Errand::Nothing
    ));
}

/// FAILS IF: a grow of an open vault stops asking the passphrase its
/// mapping needs, or a locked one's asks for nothing it needs.
#[test]
fn growing_an_open_vault_asks_the_passphrase() {
    use crate::security::Errand;
    let t = texts("en-GB");
    let cap = 128 * ronoctua_private::GB;
    let mut v = crate::security::View::fresh();
    let open = scene(State::Open, false);
    assert!(matches!(
        press(&mut v, &open, Act::Grow),
        Errand::Ask {
            id: CAP,
            secret: false,
            ..
        }
    ));
    assert!(matches!(
        v.answered(CAP, "128G", &open, &t),
        Errand::Ask {
            id: PASSPHRASE,
            secret: true,
            ..
        }
    ));
    match v.answered(PASSPHRASE, "correct horse", &open, &t) {
        Errand::Door(crate::security::Work::Private(w)) => {
            assert_eq!(w.op, Op::Grow { cap_bytes: cap });
            assert!(w.passphrase.is_some());
        }
        other => panic!("{other:?}"),
    }
    let locked = scene(State::Locked, false);
    let _ = press(&mut v, &locked, Act::Grow);
    match v.answered(CAP, "128G", &locked, &t) {
        Errand::Door(crate::security::Work::Private(w)) => {
            assert_eq!(w.op, Op::Grow { cap_bytes: cap });
            assert!(w.passphrase.is_none());
        }
        other => panic!("{other:?}"),
    }
}

/// FAILS IF: the leader's two keys stop doing what the state allows —
/// the key's opening when one is enrolled, the passphrase otherwise, the
/// section where a vault is made, locking only an open one.
#[test]
fn the_leaders_keys_follow_the_state() {
    use crate::security::Errand;
    let t = texts("en-GB");
    let mut v = crate::security::View::fresh();
    let errand = v.private_open(&scene(State::Locked, true), &t);
    assert!(
        matches!(&errand, Errand::Door(crate::security::Work::Private(w)) if w.touches()),
        "{errand:?}"
    );
    assert!(matches!(
        v.private_open(&scene(State::Locked, false), &t),
        Errand::Ask {
            id: PASSPHRASE,
            secret: true,
            ..
        }
    ));
    assert!(matches!(
        v.private_open(&scene(State::SuspendedInUse, true), &t),
        Errand::Ask { id: PASSPHRASE, .. }
    ));
    assert!(matches!(
        v.private_open(&scene(State::Absent, false), &t),
        Errand::Show(crate::security::Section::Private)
    ));
    assert!(matches!(
        v.private_open(&scene(State::Open, false), &t),
        Errand::Command("private-show")
    ));
    match v.private_lock(&scene(State::Open, false), &t) {
        Errand::Door(crate::security::Work::Private(w)) => assert_eq!(w.op, Op::Close),
        other => panic!("{other:?}"),
    }
    for state in [State::Locked, State::Absent, State::SuspendedInUse] {
        assert!(matches!(
            v.private_lock(&scene(state, false), &t),
            Errand::Say(_)
        ));
    }
    let none = crate::security::Scene {
        read: true,
        ..crate::security::Scene::default()
    };
    assert_eq!(
        match v.private_open(&none, &t) {
            Errand::Say(said) => said,
            other => panic!("{other:?}"),
        },
        t.t(key::SECURITY_PRIVATE_MISSING)
    );
}

/// FAILS IF: "touch your key" is said before sudo let the person
/// through (a password question may still come), is not said once the
/// unit started, or a step of the vault's is taken for the fingerprint's.
#[test]
fn the_touch_is_asked_when_the_unit_starts() {
    use crate::security::{Came, Errand, Work as Page};
    let t = texts("en-GB");
    let mut v = crate::security::View::fresh();
    let mut s = scene(State::Locked, true);
    let Errand::Door(work) = v.private_open(&s, &t) else {
        panic!("the key's opening goes to the door");
    };
    v.sent(&work);
    let busy = |v: &crate::security::View, s: &crate::security::Scene| {
        v.rows(s, 100, &t, &ronoctua_files::when::Zone::utc())
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<String>()
    };
    assert!(!busy(&v, &s).contains(&t.t(key::SECURITY_PRIVATE_TOUCH)));
    // SUDO ASKS IN THE BAR (a password, or the key for sudo): nothing of
    // the vault's touch is said while it does — the unit has not started.
    assert!(matches!(&work, Page::Private(w) if w.op == Op::Open && w.touches()));
    // THE UNIT STARTED: now, and only now.
    let out = v.came(Came::Step(ronoctua_private::STARTED.to_owned()), &mut s, &t);
    assert_eq!(out.say, Some(t.t(key::SECURITY_PRIVATE_TOUCH)));
    assert!(busy(&v, &s).contains(&t.t(key::SECURITY_PRIVATE_TOUCH)));
    // A PASSPHRASE OPENING waits for nobody's touch.
    let mut v = crate::security::View::fresh();
    v.sent(&Page::Private(Box::new(Work::with(
        Op::Open,
        Secret::new("x"),
    ))));
    let out = v.came(Came::Step(ronoctua_private::STARTED.to_owned()), &mut s, &t);
    assert_eq!(out.say, None);
}

/// FAILS IF: the row pressed to send the vault's work is no longer under
/// the cursor while the work runs — a busy line drawn above it moves the
/// index-kept cursor onto the row before, and a second `RET` on "open
/// with the key" would ask the passphrase instead.
#[test]
fn the_cursor_stays_on_the_row_that_sent_the_work() {
    let t = texts("en-GB");
    let zone = ronoctua_files::when::Zone::utc();
    for (state, act) in [
        (State::Locked, Act::OpenKey),
        (State::Locked, Act::Forget),
        (State::Open, Act::Lock),
        (State::Open, Act::Forget),
    ] {
        let s = scene(state, true);
        let mut v = crate::security::View::fresh();
        let crate::security::Errand::Door(work) = press(&mut v, &s, act) else {
            panic!("{act:?} goes to the door");
        };
        v.sent(&work);
        let rows = v.rows(&s, 100, &t, &zone);
        assert!(
            rows.iter().any(|r| r
                .fields
                .iter()
                .any(|f| f.text == t.t(key::SECURITY_PRIVATE_BUSY))),
            "{act:?}: no busy line"
        );
        assert_eq!(
            rows.get(v.cursor()).and_then(|r| r.act.clone()),
            Some(crate::security::Act::Private(act)),
            "{state:?}: the cursor left {act:?} while its work runs"
        );
    }
}

/// FAILS IF: the unit's reason stops being what the door says — a wrong
/// passphrase said as "it did not work".
#[test]
fn the_units_reason_is_said() {
    let t = texts("pt-BR");
    let mut v = crate::security::View::fresh();
    let mut s = scene(State::Locked, false);
    let mut work = Work::with(Op::Open, Secret::new("x"));
    work.refused = Some(Why::WrongPassphrase);
    let out = v.came(
        crate::security::Came::Door {
            work: crate::security::Work::Private(Box::new(work)),
            result: Err(ronoctua_security::Why::Failed),
        },
        &mut s,
        &t,
    );
    assert_eq!(out.say, Some(t.t(key::PRIVATE_REFUSED_WRONG_PASSPHRASE)));
}

/// FAILS IF: a state of the section draws a key that has no words, in
/// either language.
#[test]
fn every_state_is_drawn_in_both_languages() {
    for language in ["en-GB", "pt-BR"] {
        let t = texts(language);
        for state in State::ALL {
            for fido2 in [false, true] {
                let mut s = scene(state, fido2);
                s.private_free = Some(1 << 30);
                let rows = crate::security::View::fresh().rows(
                    &s,
                    100,
                    &t,
                    &ronoctua_files::when::Zone::utc(),
                );
                let drawn: String = rows
                    .iter()
                    .filter(|r| r.section == crate::security::Section::Private)
                    .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                    .collect();
                assert!(!drawn.contains('['), "{language} {state:?}: {drawn}");
                assert!(!drawn.is_empty(), "{language} {state:?}");
            }
        }
    }
}
