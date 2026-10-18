//! THE CENTRE'S POST-QUANTUM AND ENCRYPTION SECTIONS, driven through the
//! security centre's view — beside the centre, which is what they drive;
//! `seal.rs`'s own tests test the seal's questions and words.

use crate::key;
use crate::report::{Opening, SealDoing, SealReport};
use crate::seal::*;
use ronoctua_pq::Kind;
use std::path::PathBuf;

/// What the security centre's page draws, as text.
fn drawn(rows: &[crate::security::Row]) -> String {
    rows.iter()
        .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// FAILS IF: the post-quantum section leaves a channel out, draws one
/// without its status and its sentence, or a key is missing in a
/// language — under the facts of a classical machine and of a
/// post-quantum one, with the machine's secrets and without.
#[test]
fn the_quantum_section_draws_every_channel_in_both_languages() {
    use ronoctua_security::quantum::{self, Belongings, Built, Facts, Syncthing, Tls};
    let shipped = Built {
        openssh: Some("10.4p1".into()),
        openssl: Some("3.6.3".into()),
        chrome: Some("151.0.7922.71".into()),
        syncthing: Some(Syncthing {
            version: "2.1.1".into(),
            go: "1.26.5".into(),
            go_mod: "1.25.0".into(),
            pins_curves: false,
        }),
        tor: Some("0.4.9.11".into()),
        tailscale: Some("1.98.10".into()),
        ships: None,
    };
    let machines = [
        Facts::default(),
        Facts {
            belongings: Belongings::PostQuantum,
            root_disk: Some(true),
            tls: Tls {
                quantum: 3,
                classical: 2,
                classical_hosts: vec!["old.example".into(), "older.example".into()],
            },
            ssh_kex: quantum::SSH_PQ_KEX.map(ToOwned::to_owned).to_vec(),
            sops: true,
            built: shipped,
        },
    ];
    for language in ["en-GB", "pt-BR"] {
        let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
        for facts in &machines {
            let scene = crate::security::Scene {
                read: true,
                quantum: facts.clone(),
                ..crate::security::Scene::default()
            };
            let view = crate::security::View::at(Some(crate::security::Section::Quantum));
            let rows = view.rows(&scene, 120, &texts, &ronoctua_files::when::Zone::utc());
            let page = drawn(&rows);
            assert!(!page.contains('['), "{language}: a key is missing: {page}");
            for reading in quantum::channels(facts) {
                let label = texts.t(&format!(
                    "security-quantum-channel-{}",
                    reading.channel.id()
                ));
                let status = texts.t(&format!("security-quantum-status-{}", reading.status.id()));
                // UNDER THE SECTION'S HEADING: another section may say
                // the same word (the vault's own is `/private`).
                let heading = texts.t(key::SECURITY_HEADING_QUANTUM);
                let from = rows
                    .iter()
                    .position(|r| r.fields.iter().any(|f| f.text == heading))
                    .unwrap_or_else(|| panic!("{language}: no post-quantum heading"));
                let at = rows
                    .iter()
                    .skip(from)
                    .position(|r| r.fields.iter().any(|f| f.text == label))
                    .map(|n| n + from)
                    .unwrap_or_else(|| panic!("{language}: no row for {:?}", reading.channel));
                assert!(
                    rows[at].fields.iter().any(|f| f.text == status),
                    "{language}: {:?} without its status",
                    reading.channel
                );
                // ITS SENTENCE, with its numbers and names, starting on
                // the line right after it (a note wraps, never cuts).
                let sentence = crate::security::quantum_sentence(&reading, &texts);
                assert!(
                    !sentence.contains('{') && !sentence.starts_with('['),
                    "{language}: {:?} has a hole: {sentence}",
                    reading.why
                );
                let next: String = rows
                    .get(at + 1)
                    .map(|r| r.fields.iter().map(|f| f.text.trim()).collect())
                    .unwrap_or_default();
                assert!(
                    !next.is_empty() && sentence.starts_with(&next),
                    "{language}: {:?} without its sentence: {next:?} / {sentence:?}",
                    reading.channel
                );
            }
        }
    }
}

/// FAILS IF: a count reaches its sentence as text — "every one of the 1
/// hosts heard", the state right after the first request — or a plural
/// branch is missing in a language.
#[test]
fn one_host_is_said_as_one_in_both_languages() {
    use ronoctua_security::quantum::{self, Channel, Facts, Tls};
    let heard = |quantum: usize, classical: usize| {
        let facts = Facts {
            tls: Tls {
                quantum,
                classical,
                classical_hosts: vec!["old.example".into(); classical],
            },
            ..Facts::default()
        };
        quantum::channels(&facts)
            .into_iter()
            .find(|r| r.channel == Channel::Https)
            .unwrap()
    };
    for (language, one, one_old, some_old) in [
        (
            "en-GB",
            "The one host heard agreed X25519MLKEM768.",
            "The one host heard still agrees a classical key: old.example.",
            "One of the 3 hosts heard still agrees a classical key: old.example.",
        ),
        (
            "pt-BR",
            "O único host ouvido combinou X25519MLKEM768.",
            "O único host ouvido ainda combina uma chave clássica: old.example.",
            "Um dos 3 hosts ouvidos ainda combina uma chave clássica: old.example.",
        ),
    ] {
        let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
        let said = |q, c| crate::security::quantum_sentence(&heard(q, c), &texts);
        assert_eq!(said(1, 0), one, "{language}");
        assert_eq!(said(0, 1), one_old, "{language}");
        assert_eq!(said(2, 1), some_old, "{language}");
        assert!(said(2, 0).contains('2'), "{language}: {}", said(2, 0));
        assert!(said(1, 2).contains("2 "), "{language}: {}", said(1, 2));
    }
}

/// FAILS IF: the security centre does not ask again for a passphrase the
/// queue refused on an open IT sent, or asks again about one somebody
/// else sent (the file manager's), or asks twice for one refusal.
#[test]
fn the_centre_asks_again_only_about_its_own_opens() {
    use crate::security::{Came, Errand, Scene, View};
    let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("a language");
    let mut view = View::fresh();
    let mut scene = Scene::default();
    let path = PathBuf::from("/home/p/letter.txt.age");
    let out = view.came(
        Came::Sniffed {
            path: path.clone(),
            kind: Some(Kind::Passphrase),
        },
        &mut scene,
        &texts,
    );
    assert!(matches!(
        out.errand,
        Errand::Ask {
            id: PASSPHRASE,
            secret: true,
            ..
        }
    ));
    let sent = view.answered(PASSPHRASE, "not it", &scene, &texts);
    assert!(
        matches!(&sent, Errand::Seal(SealDoing::Open { path: p, .. }) if *p == path),
        "the open was not handed to the queue"
    );
    let refusal = || {
        Came::Sealed(SealReport::Refused {
            from: path.clone(),
            opening: Some(Opening::Passphrase),
            why: ronoctua_pq::Why::WrongPassphrase,
        })
    };
    let somebody_else = Came::Sealed(SealReport::Refused {
        from: PathBuf::from("/home/p/other.age"),
        opening: Some(Opening::Passphrase),
        why: ronoctua_pq::Why::WrongPassphrase,
    });
    assert!(matches!(
        view.came(somebody_else, &mut scene, &texts).errand,
        Errand::Nothing
    ));
    assert!(matches!(
        view.came(refusal(), &mut scene, &texts).errand,
        Errand::Ask {
            id: PASSPHRASE_WRONG,
            secret: true,
            ..
        }
    ));
    assert!(matches!(
        view.came(refusal(), &mut scene, &texts).errand,
        Errand::Nothing
    ));
}

/// FAILS IF: the security centre hands a seal to the queue and then says
/// nothing when the queue refuses it — the case the file manager's
/// passphrase seal of an unreadable file left silent.
#[test]
fn the_centre_says_a_refused_seal_it_sent() {
    use crate::security::{Act, Came, Errand, Press, Scene, View};
    let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("a language");
    let zone = ronoctua_files::when::Zone::utc();
    let mut scene = Scene::default();
    let file = PathBuf::from("/home/p/unreadable.txt");
    let mut view = View::fresh();
    view.seal_these(vec![file.clone()]);
    let rows = view.rows(&scene, 120, &texts, &zone);
    let at = rows
        .iter()
        .position(|r| r.act == Some(Act::SealMode(Mode::Passphrase)))
        .expect("no 'with a passphrase' row");
    view.clicked(at, &rows);
    assert!(matches!(
        view.press(Press::Enter, &scene, 120, &texts, &zone),
        Errand::Ask {
            id: PASSPHRASE_NEW,
            ..
        }
    ));
    assert!(matches!(
        view.answered(PASSPHRASE_NEW, "horse", &scene, &texts),
        Errand::Ask {
            id: PASSPHRASE_AGAIN,
            ..
        }
    ));
    assert!(matches!(
        view.answered(PASSPHRASE_AGAIN, "horse", &scene, &texts),
        Errand::Seal(SealDoing::Seal { .. })
    ));
    let refusal = || {
        Came::Sealed(SealReport::Refused {
            from: file.clone(),
            opening: None,
            why: ronoctua_pq::Why::Failed,
        })
    };
    assert!(matches!(
        View::fresh().came(refusal(), &mut scene, &texts).errand,
        Errand::Nothing
    ));
    match view.came(refusal(), &mut scene, &texts).errand {
        Errand::Say(said) => assert!(said.contains("unreadable.txt"), "{said}"),
        other => panic!("the refused seal was not said: {other:?}"),
    }
}
