//! What the software view promises, held down with nothing running.
//!
//! Every test compares WHAT WOULD BE DRAWN — the rows' own text — and never
//! only a field of the view: a view whose state is right and whose rows are
//! wrong looks broken and passes everything.

use super::*;
use crate::software_feed::{Arrival, DarkReading, Permits, Taught};
use ronoctua_software::found::{Batch, Picture};
use ronoctua_software::state::{Android, Generation, Supports};

fn texts(language: &str) -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh(language).expect("a declared language")
}

fn en() -> ronoctua_i18n::Texts {
    texts("en-GB")
}

fn every_language() -> Vec<ronoctua_i18n::Texts> {
    ronoctua_i18n::LANGUAGES.iter().map(|l| texts(l)).collect()
}

fn found(key: AppKey, name: &str, badges: &[Badge]) -> Found {
    Found {
        key,
        name: name.to_owned(),
        summary: format!("{name} does things"),
        version: "1.0".to_owned(),
        badges: badges.to_vec(),
        download_bytes: Some(80_000_000),
        icon: Some(Picture::Named {
            name: name.to_lowercase(),
        }),
        homepage: Some("https://example.org".to_owned()),
        license: Some("GPL-3.0-or-later".to_owned()),
        publisher: Some("Somebody".to_owned()),
        program: None,
        by_description: false,
    }
}

fn installed(key: AppKey, name: &str) -> Installed {
    Installed {
        desktop_ids: vec![format!("{}.desktop", key.id())],
        key,
        name: name.to_owned(),
        version: "2.10".to_owned(),
        size_bytes: Some(300_000_000),
        update: None,
        unfree: false,
        origin: Origin::Mine,
        icon: None,
        trouble: None,
    }
}

fn gimp_flatpak() -> AppKey {
    AppKey::Flatpak {
        id: "org.gimp.GIMP".into(),
    }
}

fn gimp_nix() -> AppKey {
    AppKey::Nix {
        attr: "gimp".into(),
    }
}

fn web() -> AppKey {
    AppKey::Web {
        slug: "mail".into(),
    }
}

/// A machine with one program of each kind, a running trial, an update, a
/// GAPPS Android and a Nix list with history.
fn full() -> Snapshot {
    let mut flatpak = installed(gimp_flatpak(), "GIMP");
    flatpak.update = Some("2.10.38".into());
    let declared = Installed {
        origin: Origin::Declared,
        ..installed(
            AppKey::Flatpak {
                id: "org.gnome.Calculator".into(),
            },
            "Calculator",
        )
    };
    Snapshot {
        installed: vec![
            installed(
                AppKey::Nix {
                    attr: "ripgrep".into(),
                },
                "ripgrep",
            ),
            flatpak,
            declared,
            installed(
                AppKey::AppImage {
                    key: "obsidian".into(),
                },
                "Obsidian",
            ),
            installed(
                AppKey::Android {
                    package: "org.fdroid.fdroid".into(),
                },
                "F-Droid",
            ),
            installed(web(), "Mail"),
        ],
        trials: vec![Trial {
            key: "hello".into(),
            attr: "hello".into(),
            name: "hello".into(),
            running: true,
            since: Some(1_000),
            traces: Traces::Scratch,
            program: Some("hello".into()),
            terminal: true,
        }],
        android: Android {
            support: true,
            prepared: true,
            preparing: false,
            image: Some(AndroidImage::Gapps),
            session: true,
            multi_window: Some(true),
            android_id: Some("3a5f9c0e71d2b4a8".into()),
        },
        generations: vec![
            Generation {
                number: 3,
                when: 1_750_000_000,
                current: true,
                summary: "+ripgrep".into(),
            },
            Generation {
                number: 2,
                when: 1_740_000_000,
                current: false,
                summary: "-vlc".into(),
            },
        ],
        supports: Supports {
            nix: true,
            flatpak: true,
            appimage: true,
            android: true,
            web: true,
        },
        nix_index_rev: Some("5b4f72e".into()),
        declarations: Declarations::default(),
    }
}

/// A machine with every source and nothing installed.
fn empty() -> Snapshot {
    Snapshot {
        supports: full().supports,
        ..Snapshot::default()
    }
}

fn readings(snapshot: Option<Snapshot>) -> Readings {
    let mut r = Readings::fresh(
        Preferences::default(),
        Taught {
            launcher: "LEADER-a".into(),
            queue: "LEADER-t-t".into(),
            brain: "LEADER-b".into(),
        },
    );
    r.snapshot = snapshot;
    r.now = 1_000 + 7 * 60;
    r.home = Some("/home/somebody".into());
    // NOBODY'S CONSENTS, whatever this machine's file says: the rows a test
    // reads do not depend on the developer's `consent.json`.
    r.consents = ronoctua_consent::Consents::default();
    r
}

/// A search for `gimp` answered by Nix and Flatpak.
fn searched(r: &mut Readings, query: &str) {
    r.asked(
        query.to_owned(),
        &[Source::Nix, Source::Flatpak, Source::AppImage],
        9,
    );
    r.took(Arrival::Batch(Batch {
        source: Source::Nix,
        generation: 9,
        found: vec![found(gimp_nix(), "GIMP", &[])],
        status: Status::Answered {
            from: Some("5b4f72e".into()),
        },
    }));
    r.took(Arrival::Batch(Batch {
        source: Source::Flatpak,
        generation: 9,
        found: vec![found(
            gimp_flatpak(),
            "GIMP",
            &[Badge::Verified, Badge::Sandboxed],
        )],
        status: Status::Answered { from: None },
    }));
}

/// GIMP's permissions: files read-write, the network, sound, Wayland, and
/// `GTK_THEME` forced into it.
fn gimp_permits() -> Permits {
    Permits {
        lines: vec!["share=network".into(), "env=GTK_THEME=Adwaita:dark".into()],
        grants: ronoctua_software::flatpak::Grants {
            files: Reach::ReadWrite,
            network: true,
            sound: true,
            devices: false,
            display: Some(Display::Wayland),
            gtk_theme: true,
        },
    }
}

/// The dark diagnosis with GIMP forcing `GTK_THEME` and both declared parts
/// in place.
fn forcing_gimp() -> DarkReading {
    DarkReading {
        forcing: vec![("org.gimp.GIMP".into(), "GIMP".into())],
        facts: ronoctua_software::flatpak::DarkFacts {
            extension: true,
            qt_unset: true,
        },
    }
}

fn open(opening: Opening, r: &Readings) -> View {
    View::fresh(opening, r, &en()).0
}

/// Every field of every row, joined — what a person reads.
fn drawn(rows: &[Row]) -> String {
    rows.iter()
        .map(|r| {
            r.fields
                .iter()
                .map(|f| f.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn focused(rows: &[Row]) -> &Row {
    rows.iter()
        .find(|r| r.focused)
        .expect("a row under the cursor")
}

/// The rows a view draws, and that nothing on them is a missing key.
fn rows_of(v: &View, r: &Readings, t: &ronoctua_i18n::Texts) -> Vec<Row> {
    let rows = v.rows(r, DECIDING_WIDTH, t);
    let said = drawn(&rows);
    assert!(!rows.is_empty());
    assert!(
        !said.contains("[software-"),
        "a key nobody wrote, in {}:\n{said}",
        t.language()
    );
    assert!(!said.contains("{ $"), "an argument nobody passed:\n{said}");
    rows
}

fn press(v: &mut View, p: Press, r: &Readings) -> Errand {
    v.key(p, r, DECIDING_WIDTH, &en())
}

/// Walk the cursor down until `want` is under it.
fn walk_to(v: &mut View, r: &Readings, want: &Spot) {
    for _ in 0..200 {
        let rows = v.rows(r, DECIDING_WIDTH, &en());
        if focused(&rows).spot == *want {
            return;
        }
        press(v, Press::Down, r);
    }
    panic!("never reached {want:?}");
}

fn queued(e: &Errand) -> Vec<Order> {
    match e {
        Errand::Queue(q) => q.iter().map(|x| x.order.clone()).collect(),
        other => panic!("not a queue: {other:?}"),
    }
}

// ─── every level draws, in every language ──────────────────────────────────

#[test]
fn every_level_draws_in_every_language() {
    let mut r = readings(Some(full()));
    searched(&mut r, "gimp");
    r.details = Some((
        gimp_nix(),
        Ok(Details {
            description: vec!["A long description.".into()],
            ..Details::default()
        }),
    ));
    r.history = vec![ronoctua_software::journal::Entry {
        when: 1_750_000_000,
        order: "install".into(),
        key: Some(gimp_flatpak()),
        name: "GIMP".into(),
        ok: true,
        refusal: None,
    }];
    r.prefs.unfree.insert("google-chrome".into());
    r.permissions = Some(("org.gimp.GIMP".into(), Some(Some(gimp_permits()))));
    r.dark = Some(Some(forcing_gimp()));
    let levels = [
        Level::Home,
        Level::Results,
        Level::Sheet(gimp_nix()),
        Level::Sheet(gimp_flatpak()),
        Level::Sheet(AppKey::System {
            desktop_id: "firefox.desktop".into(),
        }),
        Level::Sheet(AppKey::Nix {
            attr: "hello".into(),
        }),
        Level::Trials,
        Level::Android,
        Level::Maintenance,
        Level::Advanced,
        Level::Permissions("org.gimp.GIMP".into()),
        Level::Consents,
        Level::History,
    ];
    for t in every_language() {
        for level in &levels {
            let mut v = open(Opening::Home, &r);
            v.level = level.clone();
            v.query = "gimp".into();
            v.open = vec!["details", "older"];
            let rows = rows_of(&v, &r, &t);
            assert!(
                rows.iter().any(|x| !x.heading),
                "{level:?} has nothing to stand on"
            );
        }
    }
}

#[test]
fn the_first_page_says_what_is_installed_and_where_to_go() {
    let r = readings(Some(full()));
    let v = open(Opening::Home, &r);
    let rows = rows_of(&v, &r, &en());
    let said = drawn(&rows);
    for name in ["ripgrep", "GIMP", "Obsidian", "F-Droid", "Mail", "hello"] {
        assert!(said.contains(name), "{name} missing:\n{said}");
    }
    // THE FOOTER OPENS TYPING: the cursor is born on the search.
    assert_eq!(focused(&rows).spot, Spot::Search);
    // A PICTURE GOES WHERE THE GLYPH IS: a program row starts with its glyph.
    let app = rows
        .iter()
        .find(|x| x.spot == Spot::Program(gimp_flatpak()))
        .unwrap();
    assert_eq!(
        app.fields[0].text.trim(),
        words::source_glyph(Source::Flatpak).to_string()
    );
}

#[test]
fn an_empty_machine_points_at_the_launcher_with_its_keys() {
    let r = readings(Some(Snapshot::default()));
    let v = open(Opening::Home, &r);
    let said = drawn(&rows_of(&v, &r, &en()));
    assert!(said.contains("LEADER-a"), "{said}");
}

#[test]
fn before_the_first_read_the_page_says_it_is_reading() {
    let r = readings(None);
    let v = open(Opening::Home, &r);
    let said = drawn(&rows_of(&v, &r, &en()));
    assert!(
        said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_READING)),
        "{said}"
    );
}

#[test]
fn the_window_is_born_on_the_search_as_the_footer_is() {
    let r = readings(Some(full()));
    for opening in [Opening::Home, Opening::Trials] {
        let v = open(opening.clone(), &r);
        let rows = rows_of(&v, &r, &en());
        assert_eq!(focused(&rows).spot, Spot::Search, "{opening:?}");
        assert_eq!(focused(&rows).target, Target::Search, "{opening:?}");
    }
}

// ─── typing, pasting, searching ────────────────────────────────────────────

#[test]
fn a_letter_starts_a_search_in_either_door() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    let e = v.letter('g', &r, &en());
    assert_eq!(v.level, Level::Results);
    match e {
        Errand::Feed(Want::Search { query, sources }) => {
            assert_eq!(query, "g");
            assert!(
                !sources.contains(&Source::Web),
                "web apps are made, not searched"
            );
            assert!(sources.contains(&Source::Nix));
        }
        other => panic!("{other:?}"),
    }
    // THE WINDOW TYPES TOO, into the same search.
    let mut w = open(Opening::Home, &r);
    match w.letter('g', &r, &en()) {
        Errand::Feed(Want::Search { query, sources }) => {
            assert_eq!(query, "g");
            assert!(!sources.contains(&Source::Web));
            assert!(sources.contains(&Source::Nix));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(w.level, Level::Results);
}

/// The value drawn on the search's own row.
fn search_row_says(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .find(|x| x.spot == Spot::Search)
        .expect("the search's row")
        .fields
        .iter()
        .map(|f| f.text.trim().to_owned())
        .collect()
}

#[test]
fn typing_in_the_window_shows_the_query_on_the_search_row() {
    let r = readings(Some(full()));
    let mut w = open(Opening::Home, &r);
    // `RET` ON THE SEARCH, with nothing typed, says typing is already on.
    assert_eq!(
        press(&mut w, Press::Enter, &r),
        Errand::Say(en().t(ronoctua_i18n::key::SOFTWARE_TYPE_TO_SEARCH))
    );
    let mut last = Errand::Nothing;
    for c in "gimp".chars() {
        last = w.letter(c, &r, &en());
    }
    assert_eq!(w.level, Level::Results);
    match &last {
        Errand::Feed(Want::Search { query, .. }) => assert_eq!(query, "gimp"),
        other => panic!("{other:?}"),
    }
    let rows = rows_of(&w, &r, &en());
    assert!(
        search_row_says(&rows).contains(&"gimp".to_owned()),
        "{:?}",
        search_row_says(&rows)
    );
    // AND WITH SOMETHING TYPED, `RET` on it runs the search again.
    w.at = Spot::Search;
    assert_eq!(press(&mut w, Press::Enter, &r), last);
    assert_eq!(w.level, Level::Results);
}

#[test]
fn del_in_the_window_erases_and_an_empty_query_leaves_the_results() {
    let r = readings(Some(full()));
    let mut w = open(Opening::Home, &r);
    w.letter('g', &r, &en());
    w.letter('i', &r, &en());
    match w.erase(&r) {
        Errand::Feed(Want::Search { query, .. }) => assert_eq!(query, "g"),
        other => panic!("{other:?}"),
    }
    assert_eq!(w.level, Level::Results);
    assert!(search_row_says(&rows_of(&w, &r, &en())).contains(&"g".to_owned()));
    // THE LAST LETTER OFF: back where the search came from, and the search
    // forgotten.
    assert_eq!(
        w.erase(&r),
        Errand::Feed(Want::Search {
            query: String::new(),
            sources: Vec::new(),
        })
    );
    assert_eq!(w.level, Level::Home);
    assert!(w.query.is_empty());
    // `DEL` ON AN EMPTY QUERY in the results — a prefix typed alone — leaves
    // them, as in the footer.
    for c in "nix:".chars() {
        w.letter(c, &r, &en());
    }
    assert_eq!(w.level, Level::Results);
    assert!(w.query.is_empty());
    assert_eq!(w.erase(&r), Errand::Nothing);
    assert_eq!(w.level, Level::Home);
    // AND ON THE FIRST PAGE, with nothing typed, it does nothing.
    assert_eq!(w.erase(&r), Errand::Nothing);
    assert_eq!(w.level, Level::Home);
}

#[test]
fn a_paste_in_the_window_fills_the_search() {
    let r = readings(Some(full()));
    let mut w = open(Opening::Home, &r);
    assert_eq!(press(&mut w, Press::Paste, &r), Errand::Paste);
    match w.pasted("  gimp  \nsomething else", &r) {
        Errand::Feed(Want::Search { query, .. }) => assert_eq!(query, "gimp"),
        other => panic!("{other:?}"),
    }
    assert_eq!(w.level, Level::Results);
    assert!(search_row_says(&rows_of(&w, &r, &en())).contains(&"gimp".to_owned()));
    // AN EMPTY CLIPBOARD is no text, and nothing changes.
    assert_eq!(w.pasted("", &r), Errand::Nothing);
    assert_eq!(w.query, "gimp");
    // A PASTED ADDRESS offers a web app here too.
    let mut w = open(Opening::Home, &r);
    w.pasted("web.whatsapp.com", &r);
    assert!(rows_of(&w, &r, &en()).iter().any(|x| matches!(
        &x.target,
        Target::Deed(Deed::WebNew(u)) if u == "https://web.whatsapp.com/"
    )));
}

#[test]
fn typing_on_the_trials_in_the_window_is_to_try_it() {
    let mut r = readings(Some(empty()));
    searched(&mut r, "gimp");
    let mut w = open(Opening::Trials, &r);
    assert_eq!(focused(&rows_of(&w, &r, &en())).spot, Spot::Search);
    for c in "gimp".chars() {
        w.letter(c, &r, &en());
    }
    assert_eq!(w.intent, Intent::Try);
    assert_eq!(w.level, Level::Results);
    // THE SHEET OPENS ON "TRY".
    w.at = Spot::Group("gimp".into());
    assert_eq!(
        press(&mut w, Press::Enter, &r),
        Errand::Feed(Want::Details(gimp_nix()))
    );
    assert_eq!(w.level, Level::Sheet(gimp_nix()));
    assert_eq!(focused(&rows_of(&w, &r, &en())).spot, Spot::Deed("try"));
}

/// What a letter on a field of the draft says: the field is typed on its row,
/// which `RET` opens on the door's line — the same sentence and the same
/// question in both doors, since the view no longer knows its door.
fn type_on_the_row() -> Errand {
    Errand::Say(en().t_with(
        ronoctua_i18n::key::SOFTWARE_TYPE_ON_A_ROW,
        &[("keys", notation_of("software-enter").unwrap_or_default())],
    ))
}

#[test]
fn the_icons_query_is_typed_and_a_drafts_field_is_asked_on_its_row() {
    let r = readings(Some(full()));
    let (mut w, _) = View::fresh(Opening::WebNew("github.com".into()), &r, &en());
    // A LETTER ON THE EDITOR is not half a field: it says where the field is
    // typed — and teaches no paste there.
    assert_eq!(w.letter('x', &r, &en()), type_on_the_row());
    assert!(!taught(&w, &r).contains(&"C-v".to_owned()));
    // AND `RET` ON THE NAME ASKS, and the answer is the draft's.
    w.at = Spot::Ring("name");
    assert!(
        matches!(
            press(&mut w, Press::Enter, &r),
            Errand::Ask {
                ask: Ask::WebName,
                ..
            }
        ),
        "the name is not asked for"
    );
    assert_eq!(
        w.typed("software-web-name", "Octo", &r, &en()),
        Errand::Nothing
    );
    assert!(drawn(&rows_of(&w, &r, &en())).contains("Octo"));
    w.at = Spot::Ring("icon");
    press(&mut w, Press::Enter, &r);
    press(&mut w, Press::NextSource, &r);
    assert_eq!(w.level, Level::Icons(Shelf::Dashboard));
    // THE SHELF LOOKS FOR THE NAME ANSWERED ABOVE — the draft's own.
    assert_eq!(w.query(), "Octo");
    assert!(taught(&w, &r).contains(&"C-v".to_owned()));
    let icons = |query: &str| {
        Errand::Feed(Want::Icons {
            shelf: Shelf::Dashboard,
            query: query.to_owned(),
        })
    };
    assert_eq!(w.erase(&r), icons("Oct"));
    assert_eq!(w.letter('o', &r, &en()), icons("Octo"));
    assert_eq!(press(&mut w, Press::Paste, &r), Errand::Paste);
    assert_eq!(w.pasted("octocat", &r), icons("octocat"));
    let rows = rows_of(&w, &r, &en());
    assert!(
        search_row_says(&rows).contains(&"octocat".to_owned()),
        "{:?}",
        search_row_says(&rows)
    );
    assert!(
        search_row_says(&rows).contains(&en().t(ronoctua_i18n::key::SOFTWARE_ICON_LOOKING_FOR))
    );
    // THE FILE SHELF'S PATH is typed on its row too.
    w.level = Level::Icons(Shelf::File);
    assert_eq!(w.letter('x', &r, &en()), type_on_the_row());
}

/// THE DEFECT: `Delete` typed U+007F into the search, and `DEL` took one
/// code point off an accent or an emoji.
#[test]
fn a_control_character_is_no_letter_and_del_takes_a_cluster() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    assert_eq!(v.letter('\u{7f}', &r, &en()), Errand::Nothing);
    assert_eq!(v.level, Level::Home);
    assert!(v.query.is_empty());
    for c in "ge\u{301}".chars() {
        v.letter(c, &r, &en());
    }
    v.erase(&r);
    assert_eq!(v.query, "g", "half an accent was left behind");
}

#[test]
fn a_typed_prefix_picks_the_source() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    for c in "nix:rg".chars() {
        v.letter(c, &r, &en());
    }
    assert_eq!(v.filter, Some(Source::Nix));
    assert_eq!(v.query, "rg");
}

#[test]
fn erasing_the_last_letter_goes_back_home() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    v.letter('g', &r, &en());
    v.erase(&r);
    assert_eq!(v.level, Level::Home);
    assert!(v.query.is_empty());
}

#[test]
fn a_pasted_address_offers_a_web_app_and_a_path_offers_its_file() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    v.pasted("web.whatsapp.com\nsomething else", &r);
    let rows = rows_of(&v, &r, &en());
    let make = rows
        .iter()
        .find(|x| matches!(&x.target, Target::Deed(Deed::WebNew(u)) if u == "https://web.whatsapp.com/"))
        .expect("a row that makes a web app");
    assert!(drawn(std::slice::from_ref(make)).contains("web.whatsapp.com"));
    assert!(
        rows.iter()
            .any(|x| matches!(&x.target, Target::Deed(Deed::OpenOnce(_))))
    );

    let mut v = open(Opening::Home, &r);
    v.pasted("~/Downloads/Obsidian-1.6.AppImage", &r);
    let rows = rows_of(&v, &r, &en());
    assert!(rows.iter().any(|x| matches!(
        &x.target,
        Target::Deed(Deed::InstallFile { path, .. }) if path == Path::new("/home/somebody/Downloads/Obsidian-1.6.AppImage")
    )));
    // AN APP ID IS NOT AN ADDRESS: it goes to the search.
    assert_eq!(address_in("org.gimp.GIMP"), None);
    assert_eq!(address_in("gimp"), None);
    assert_eq!(
        address_in("https://mail.example.org/inbox").as_deref(),
        Some("https://mail.example.org/inbox")
    );
}

#[test]
fn results_merge_sources_mark_what_is_recommended_and_say_how_each_source_stands() {
    let mut r = readings(Some(Snapshot {
        supports: full().supports,
        ..Snapshot::default()
    }));
    searched(&mut r, "gimp");
    let mut v = open(Opening::Home, &r);
    v.query = "gimp".into();
    v.level = Level::Results;
    let rows = rows_of(&v, &r, &en());
    let groups: Vec<&Row> = rows
        .iter()
        .filter(|x| matches!(x.target, Target::Group(_)))
        .collect();
    assert_eq!(groups.len(), 1, "one program, two sources, one row");
    // NIX FIRST: the recommended member's glyph leads the row.
    assert_eq!(
        groups[0].fields[0].text.trim(),
        words::source_glyph(Source::Nix).to_string()
    );
    let said = drawn(&rows);
    assert!(
        said.contains("5b4f72e"),
        "the index's basis is said:\n{said}"
    );
    let appimage = rows
        .iter()
        .find(|x| x.spot == Spot::Status(Source::AppImage))
        .expect("the source still asking");
    assert!(
        drawn(std::slice::from_ref(appimage))
            .contains(&en().t(ronoctua_i18n::key::SOFTWARE_ASKING)),
        "{said}"
    );
}

#[test]
fn a_source_that_is_not_ready_is_one_key_from_ready() {
    let mut r = readings(Some(full()));
    r.asked("fdroid".into(), &[Source::Android], 4);
    r.took(Arrival::Batch(Batch {
        source: Source::Android,
        generation: 4,
        found: Vec::new(),
        status: Status::Refused {
            refusal: Refusal::SourceUnprepared,
        },
    }));
    let mut v = open(Opening::Home, &r);
    v.query = "fdroid".into();
    v.level = Level::Results;
    let rows = rows_of(&v, &r, &en());
    let row = rows
        .iter()
        .find(|x| x.spot == Spot::Status(Source::Android))
        .unwrap();
    assert_eq!(row.target, Target::Deed(Deed::Refresh(Source::Android)));
}

// ─── the cursor ────────────────────────────────────────────────────────────

#[test]
fn the_cursor_keeps_its_program_when_a_batch_reorders_the_list() {
    let mut r = readings(Some(Snapshot {
        supports: full().supports,
        ..Snapshot::default()
    }));
    r.asked("pa".into(), &[Source::Nix, Source::Flatpak], 1);
    r.took(Arrival::Batch(Batch {
        source: Source::Nix,
        generation: 1,
        found: vec![found(
            AppKey::Nix {
                attr: "pavucontrol".into(),
            },
            "pavucontrol",
            &[],
        )],
        status: Status::Answered { from: None },
    }));
    let mut v = open(Opening::Home, &r);
    v.query = "pa".into();
    v.level = Level::Results;
    v.at = Spot::Top;
    let before = rows_of(&v, &r, &en());
    let spot = focused(&before).spot.clone();
    assert_eq!(spot, Spot::Group("pavucontrol".into()));
    v.settle(&r, DECIDING_WIDTH, &en());
    // A BETTER MATCH ARRIVES AND GOES ABOVE IT.
    r.took(Arrival::Batch(Batch {
        source: Source::Flatpak,
        generation: 1,
        found: vec![found(
            AppKey::Flatpak {
                id: "org.x.Pa".into(),
            },
            "Pa",
            &[],
        )],
        status: Status::Answered { from: None },
    }));
    let after = rows_of(&v, &r, &en());
    assert_eq!(focused(&after).spot, spot, "the data moved the cursor");
    let at_before = View::cursor(&before);
    let at_after = View::cursor(&after);
    assert_ne!(at_before, at_after, "the list did reorder under it");
}

#[test]
fn a_removed_program_leaves_the_cursor_near_where_it_was() {
    let mut r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    walk_to(
        &mut v,
        &r,
        &Spot::Program(AppKey::AppImage {
            key: "obsidian".into(),
        }),
    );
    let i = View::cursor(&v.rows(&r, DECIDING_WIDTH, &en()));
    r.snapshot
        .as_mut()
        .unwrap()
        .installed
        .retain(|x| x.name != "Obsidian");
    let rows = rows_of(&v, &r, &en());
    let j = View::cursor(&rows);
    assert!(j.abs_diff(i) <= 1, "{i} → {j}");
    assert!(!focused(&rows).heading);
}

#[test]
fn the_first_click_selects_and_the_second_acts() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    let rows = v.rows(&r, DECIDING_WIDTH, &en());
    let n = rows
        .iter()
        .position(|x| x.spot == Spot::Program(gimp_flatpak()))
        .unwrap();
    assert!(!rows[n].focused);
    assert!(!v.clicked(n, &rows), "the first click acted");
    let rows = v.rows(&r, DECIDING_WIDTH, &en());
    assert!(rows[n].focused, "the first click did not select");
    assert!(v.clicked(n, &rows), "the second click did not act");
    // A HEADING IS NOT CLICKED.
    let h = rows.iter().position(|x| x.heading).unwrap();
    assert!(!v.clicked(h, &rows));
}

// ─── what the hint teaches ─────────────────────────────────────────────────

/// The keys the hint line teaches, as notations.
fn taught(v: &View, r: &Readings) -> Vec<String> {
    let drawn: Vec<String> = v
        .hint(r, DECIDING_WIDTH, &en())
        .iter()
        .filter(|p| p.is_a_key())
        .map(|p| p.text.clone())
        .collect();
    OF_THE_VIEW
        .iter()
        .map(|(n, _, _)| *n)
        .filter(|n| {
            ronoctua_keymap::Key::parse(n)
                .is_ok_and(|k| drawn.contains(&crate::chrome::key_text(&k)))
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_hint_teaches_only_the_keys_that_work_on_the_row() {
    let mut fresh = readings(Some(empty()));
    searched(&mut fresh, "gimp");
    let mut r = readings(Some(full()));
    searched(&mut r, "gimp");
    // A RESULT NOT INSTALLED: install and try, never remove.
    let mut v = open(Opening::Home, &r);
    v.query = "gimp".into();
    v.level = Level::Results;
    v.at = Spot::Group("gimp".into());
    let k = taught(&v, &fresh);
    assert!(
        k.contains(&"C-RET".to_owned()) && k.contains(&"C-t".to_owned()),
        "{k:?}"
    );
    assert!(!k.contains(&"C-d".to_owned()), "{k:?}");
    assert!(
        k.contains(&"C-v".to_owned()),
        "the footer pastes into its search: {k:?}"
    );
    // THE SAME RESULT, INSTALLED: open and remove, and never install again.
    let k = taught(&v, &r);
    assert!(
        k.contains(&"C-o".to_owned()) && k.contains(&"C-d".to_owned()),
        "{k:?}"
    );
    assert!(!k.contains(&"C-RET".to_owned()), "{k:?}");
    // AN INSTALLED PROGRAM OF ONE'S OWN WITH AN UPDATE: open, update, remove.
    let mut v = open(Opening::Home, &r);
    v.at = Spot::Program(gimp_flatpak());
    let k = taught(&v, &r);
    for want in ["C-o", "C-u", "C-d"] {
        assert!(k.contains(&want.to_owned()), "{want} missing: {k:?}");
    }
    assert!(
        !k.contains(&"C-RET".to_owned()) && !k.contains(&"C-e".to_owned()),
        "{k:?}"
    );
    assert!(
        k.contains(&"C-v".to_owned()),
        "the window pastes into its search too: {k:?}"
    );
    // WHAT THE FLAKE DECLARED IS NOT REMOVED FROM HERE.
    v.at = Spot::Program(AppKey::Flatpak {
        id: "org.gnome.Calculator".into(),
    });
    assert!(!taught(&v, &r).contains(&"C-d".to_owned()));
    // A WEB APP IS EDITED; A RUNNING TRIAL IS STOPPED.
    v.at = Spot::Program(web());
    assert!(taught(&v, &r).contains(&"C-e".to_owned()));
    v.at = Spot::Trial("hello".into());
    let k = taught(&v, &r);
    assert!(
        k.contains(&"C-k".to_owned()) && k.contains(&"C-RET".to_owned()),
        "{k:?}"
    );
    // A RING TURNS.
    let mut v = open(Opening::Home, &r);
    v.at = Spot::Ring("filter");
    assert!(taught(&v, &r).contains(&"S-<right>".to_owned()));
}

#[test]
fn every_key_of_the_table_is_a_press_and_every_press_id_is_in_the_table() {
    for (notation, id, _) in OF_THE_VIEW {
        assert!(press_of(id).is_some(), "{id} has no press");
        assert_eq!(pressed(notation), press_of(id), "{notation}");
        assert!(
            ronoctua_keymap::Key::parse(notation).is_ok(),
            "{notation} does not parse"
        );
    }
    assert_eq!(pressed("ESC"), Some(Press::Escape));
    assert_eq!(notation_of("software-enter"), Some("RET"));
}

// ─── questions ─────────────────────────────────────────────────────────────

#[test]
fn removing_opens_on_no_and_no_does_nothing() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    v.at = Spot::Program(gimp_flatpak());
    assert_eq!(press(&mut v, Press::Remove, &r), Errand::Nothing);
    let rows = rows_of(&v, &r, &en());
    // NO FIRST, AND THE CURSOR BORN ON IT.
    let first = rows.iter().find(|x| !x.heading).unwrap();
    assert_eq!(first.target, Target::Answer(Answer::No));
    assert!(first.focused);
    assert_eq!(press(&mut v, Press::Enter, &r), Errand::Nothing);
    assert!(v.asking.is_none());
    assert_eq!(
        v.at,
        Spot::Program(gimp_flatpak()),
        "the cursor came back where it was"
    );
}

#[test]
fn every_question_opens_on_no() {
    let r = readings(Some(full()));
    let questions = [
        Question::Consent {
            kind: ConsentKind::Unfree,
            deed: Box::new(Deed::Launch("x".into())),
            name: "x".into(),
            bytes: None,
        },
        Question::Consent {
            kind: ConsentKind::Big,
            deed: Box::new(Deed::Launch("x".into())),
            name: "x".into(),
            bytes: Some(900_000_000),
        },
        Question::Consent {
            kind: ConsentKind::Unverified,
            deed: Box::new(Deed::Launch("x".into())),
            name: "x".into(),
            bytes: None,
        },
        Question::Consent {
            kind: ConsentKind::ArmOnly,
            deed: Box::new(Deed::Launch("x".into())),
            name: "x".into(),
            bytes: None,
        },
        Question::Remove {
            key: gimp_flatpak(),
            name: "GIMP".into(),
        },
        Question::RemoveWeb {
            slug: "mail".into(),
            name: "Mail".into(),
        },
        Question::Rollback(2),
        Question::CleanUp(None),
        Question::PrepareAndroid(AndroidImage::Gapps),
        Question::EraseAndroid,
        Question::Foreign("/tmp/x.apk".into()),
        Question::LeaveDraft,
        Question::Forget {
            key: gimp_flatpak(),
            name: "GIMP".into(),
        },
        Question::Decline {
            id: "gimp".into(),
            name: "GIMP".into(),
            everywhere: false,
        },
        Question::Decline {
            id: "gimp".into(),
            name: "GIMP".into(),
            everywhere: true,
        },
    ];
    for t in every_language() {
        for q in &questions {
            let mut v = open(Opening::Home, &r);
            v.ask_it(q.clone());
            let rows = rows_of(&v, &r, &t);
            let answers: Vec<&Row> = rows.iter().filter(|x| !x.heading).collect();
            assert_eq!(answers[0].target, Target::Answer(Answer::No), "{q:?}");
            assert!(answers[0].focused, "{q:?}: the cursor was not born on no");
            assert!(answers.len() >= 2, "{q:?}: no yes to say");
        }
    }
}

#[test]
fn removing_with_its_data_is_the_second_yes() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    v.at = Spot::Program(gimp_flatpak());
    press(&mut v, Press::Remove, &r);
    v.at = Spot::Answer(Answer::YesAndMore);
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(
        queued(&e),
        vec![Order::Remove {
            key: gimp_flatpak(),
            keep_data: false
        }]
    );
    v.at = Spot::Program(gimp_flatpak());
    press(&mut v, Press::Remove, &r);
    v.at = Spot::Answer(Answer::Yes);
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(
        queued(&e),
        vec![Order::Remove {
            key: gimp_flatpak(),
            keep_data: true
        }]
    );
}

#[test]
fn an_unfree_package_asks_first_and_the_yes_carries_the_consent() {
    let mut r = readings(Some(full()));
    let chrome = AppKey::Nix {
        attr: "google-chrome".into(),
    };
    r.asked("chrome".into(), &[Source::Nix], 3);
    r.took(Arrival::Batch(Batch {
        source: Source::Nix,
        generation: 3,
        found: vec![found(chrome.clone(), "Google Chrome", &[Badge::Unfree])],
        status: Status::Answered { from: None },
    }));
    let mut v = open(Opening::Home, &r);
    v.query = "chrome".into();
    v.level = Level::Results;
    v.at = Spot::Group("googlechrome".into());
    assert_eq!(press(&mut v, Press::Install, &r), Errand::Nothing);
    assert!(matches!(
        v.asking,
        Some(Question::Consent {
            kind: ConsentKind::Unfree,
            ..
        })
    ));
    v.at = Spot::Answer(Answer::Yes);
    let e = press(&mut v, Press::Enter, &r);
    // `C-RET` IS "INSTALL ONLY FOR YOU": declared, the consent carried.
    match &queued(&e)[..] {
        [
            Order::Declare {
                key,
                consent,
                machines: None,
            },
        ] => {
            assert_eq!(*key, chrome);
            assert!(consent.unfree);
        }
        other => panic!("{other:?}"),
    }
    // AND ONCE ALLOWED, IT IS NOT ASKED AGAIN.
    r.prefs.unfree.insert("google-chrome".into());
    v.at = Spot::Group("googlechrome".into());
    assert!(matches!(
        press(&mut v, Press::Install, &r),
        Errand::Queue(_)
    ));
}

#[test]
fn a_download_above_the_threshold_asks_and_a_broken_one_refuses() {
    let mut r = readings(Some(empty()));
    r.prefs.ask_above_mb = 50;
    searched(&mut r, "gimp");
    let mut v = open(Opening::Home, &r);
    v.query = "gimp".into();
    v.level = Level::Results;
    v.at = Spot::Group("gimp".into());
    press(&mut v, Press::Install, &r);
    assert!(matches!(
        v.asking,
        Some(Question::Consent {
            kind: ConsentKind::Big,
            bytes: Some(80_000_000),
            ..
        })
    ));
    let rows = rows_of(&v, &r, &en());
    assert!(drawn(&rows).contains("80 MB"), "{}", drawn(&rows));

    let mut r = readings(Some(full()));
    r.asked("old".into(), &[Source::Nix], 1);
    r.took(Arrival::Batch(Batch {
        source: Source::Nix,
        generation: 1,
        found: vec![found(
            AppKey::Nix { attr: "old".into() },
            "Old",
            &[Badge::Broken],
        )],
        status: Status::Answered { from: None },
    }));
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(AppKey::Nix { attr: "old".into() });
    v.at = Spot::Primary;
    let rows = rows_of(&v, &r, &en());
    let install = focused(&rows);
    assert!(install.disabled, "a broken package installs");
    match press(&mut v, Press::Enter, &r) {
        Errand::Say(s) => assert_eq!(s, en().t(words::refusal_key(Refusal::Broken))),
        other => panic!("{other:?}"),
    }
}

// ─── sheets ────────────────────────────────────────────────────────────────

#[test]
fn a_sheet_opens_on_its_first_verb_and_asks_for_its_details() {
    let mut r = readings(Some(empty()));
    searched(&mut r, "gimp");
    let mut v = open(Opening::Home, &r);
    v.query = "gimp".into();
    v.level = Level::Results;
    v.at = Spot::Group("gimp".into());
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(e, Errand::Feed(Want::Details(gimp_nix())));
    assert_eq!(v.level, Level::Sheet(gimp_nix()));
    let rows = rows_of(&v, &r, &en());
    assert_eq!(
        focused(&rows).target,
        Target::Deed(Deed::DeclareForMe {
            key: gimp_nix(),
            name: "GIMP".into(),
            consent: Consent::default(),
            machines: None,
        })
    );
    // AND INSTALLING HERE ONLY, undeclared, is the row after it.
    assert!(rows.iter().any(|x| x.spot == Spot::Deed("install")
        && x.target
            == Target::Deed(Deed::Install {
                key: gimp_nix(),
                name: "GIMP".into(),
                consent: Consent::default(),
            })));
    // THE SOURCE RING turns to the Flatpak and asks for ITS details.
    walk_to(&mut v, &r, &Spot::Ring("member"));
    let e = press(&mut v, Press::More, &r);
    assert_eq!(e, Errand::Feed(Want::Details(gimp_flatpak())));
    assert_eq!(v.level, Level::Sheet(gimp_flatpak()));
    // `Esc` goes back to the results, with the cursor where it was.
    press(&mut v, Press::Escape, &r);
    assert_eq!(v.level, Level::Results);
    assert_eq!(v.at, Spot::Group("gimp".into()));
}

#[test]
fn a_trial_opens_its_sheet_on_try_and_keeps_without_downloading() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Trials, &r);
    walk_to(&mut v, &r, &Spot::Trial("hello".into()));
    press(&mut v, Press::Enter, &r);
    assert_eq!(
        v.level,
        Level::Sheet(AppKey::Nix {
            attr: "hello".into()
        })
    );
    assert_eq!(v.at, Spot::Deed("try"));
    let rows = rows_of(&v, &r, &en());
    assert!(rows.iter().any(|x| x.spot == Spot::Primary
        && matches!(&x.target, Target::Deed(Deed::Keep { trial, .. }) if trial == "hello")));
    v.at = Spot::Primary;
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(
        queued(&e),
        vec![Order::Keep {
            trial: "hello".into()
        }]
    );
}

#[test]
fn trying_carries_the_trials_traces() {
    let mut r = readings(Some(full()));
    searched(&mut r, "gimp");
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(gimp_nix());
    walk_to(&mut v, &r, &Spot::Ring("traces"));
    press(&mut v, Press::More, &r);
    press(&mut v, Press::More, &r);
    assert_eq!(v.traces, Traces::Sandbox);
    let rows = rows_of(&v, &r, &en());
    assert!(
        rows.iter().any(|x| x.spot == Spot::Ring("network")),
        "the sealed trial's network ring"
    );
    v.at = Spot::Deed("try");
    let e = press(&mut v, Press::Enter, &r);
    match &queued(&e)[..] {
        [Order::Try { attr, traces, .. }] => {
            assert_eq!(attr, "gimp");
            assert_eq!(*traces, Traces::Sandbox);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_flatpak_cannot_be_tried_and_says_so() {
    let mut r = readings(Some(full()));
    searched(&mut r, "gimp");
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(gimp_flatpak());
    let rows = rows_of(&v, &r, &en());
    // GIMP's Flatpak is installed in the fixture; a not-installed one:
    let mut r2 = readings(Some(Snapshot {
        supports: full().supports,
        ..Snapshot::default()
    }));
    searched(&mut r2, "gimp");
    let rows2 = rows_of(&v, &r2, &en());
    let try_row = rows2.iter().find(|x| x.spot == Spot::Deed("try")).unwrap();
    assert!(try_row.disabled);
    assert_eq!(
        try_row.said.as_deref(),
        Some(en().t(words::refusal_key(Refusal::CannotTry)).as_str())
    );
    assert!(
        rows.iter()
            .any(|x| x.spot == Spot::Primary && matches!(x.target, Target::Deed(Deed::Launch(_))))
    );
}

#[test]
fn the_flake_s_own_are_opened_and_never_removed() {
    let r = readings(Some(full()));
    let (mut v, e) = View::fresh(
        Opening::Sheet {
            key: AppKey::System {
                desktop_id: "firefox.desktop".into(),
            },
            name: "Firefox".into(),
        },
        &r,
        &en(),
    );
    assert_eq!(e, Errand::Nothing, "a system app has no details to fetch");
    let rows = rows_of(&v, &r, &en());
    assert!(drawn(&rows).contains("Firefox"));
    let remove = rows
        .iter()
        .find(|x| x.spot == Spot::Deed("remove"))
        .unwrap();
    assert!(remove.disabled);
    v.at = Spot::Primary;
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Launch("firefox.desktop".into())
    );
}

#[test]
fn keeping_a_program_declares_it_in_one_press() {
    // "MAKE IT PERMANENT" WAS A LINE TO COPY; now it is the declaration.
    let r = readings(Some(full()));
    let rg = AppKey::Nix {
        attr: "ripgrep".into(),
    };
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(rg.clone());
    v.at = Spot::Deed("declare-for-me");
    let rows = rows_of(&v, &r, &en());
    assert!(
        drawn(&rows).contains(&en().t(ronoctua_i18n::key::SOFTWARE_DECLARED_ROW_KEEP)),
        "{}",
        drawn(&rows)
    );
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(
        queued(&e),
        vec![Order::Declare {
            key: rg,
            consent: Consent::default(),
            machines: None,
        }]
    );
    assert!(
        !drawn(&rows).contains("host.nix"),
        "a line for host.nix is still offered"
    );
}

/// FAILS IF: a change of the declaration made while the session keeps no
/// identity goes to the queue unsigned instead of asking the belongings'
/// passphrase first; the passphrase is not written where `sign` reads it,
/// as typed and private; an empty answer still queues the change; or the
/// question is asked with the identity kept, or with no repository synced.
#[test]
fn a_change_of_the_declaration_asks_the_passphrase_to_sign_it() {
    use ronoctua_belongings::places::Places;
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!(
        "ronoctua-shell-software-sign-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let places = Places::under(&root);
    let repository = "git@example.org:someone/belongings.git";
    crate::belongings::store_of(&places)
        .write(&ronoctua_belongings::prefs::Belongings::with_repository(
            repository,
        ))
        .unwrap();
    let rg = AppKey::Nix {
        attr: "ripgrep".into(),
    };
    let declare = || Order::Declare {
        key: rg.clone(),
        consent: Consent::default(),
        machines: None,
    };
    let mut r = readings(Some(full()));
    r.belongings = Some(places.clone());
    let keep = |v: &mut View, r: &Readings| {
        v.level = Level::Sheet(rg.clone());
        v.at = Spot::Deed("declare-for-me");
        press(v, Press::Enter, r)
    };

    // NO CLONE YET: there is no key to open, so nothing is asked.
    let mut v = open(Opening::Home, &r);
    assert_eq!(queued(&keep(&mut v, &r)), vec![declare()]);

    // SYNCED, AND NO IDENTITY KEPT: the passphrase first.
    std::fs::create_dir_all(places.repo()).unwrap();
    std::fs::write(places.key_age(), b"age").unwrap();
    let mut v = open(Opening::Home, &r);
    assert_eq!(
        keep(&mut v, &r),
        Errand::Ask {
            ask: Ask::Sign,
            initial: None
        }
    );
    assert!(Ask::Sign.secret() && !Ask::FilePath.secret());
    assert_eq!(Ask::of_command("software-sign"), Some(Ask::Sign));
    // AS TYPED — a space is part of a passphrase — and then the change.
    let e = v.typed("software-sign", " a passphrase ", &r, &en());
    assert_eq!(queued(&e), vec![declare()]);
    let written = places.passphrase();
    assert_eq!(std::fs::read(&written).unwrap(), b" a passphrase ");
    assert_eq!(
        std::fs::metadata(&written).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_file(&written).unwrap();
    // AN EMPTY ANSWER IS `Esc`: the change is dropped, and a later answer
    // has nothing to queue.
    assert!(matches!(keep(&mut v, &r), Errand::Ask { .. }));
    assert_eq!(v.typed("software-sign", "", &r, &en()), Errand::Nothing);
    assert_eq!(v.typed("software-sign", "late", &r, &en()), Errand::Nothing);
    assert!(!written.exists(), "a passphrase was written for nothing");
    // A DECLINE asks the same way.
    v.asking = Some(Question::Decline {
        id: "nix:ripgrep".into(),
        name: "ripgrep".into(),
        everywhere: true,
    });
    assert!(matches!(
        v.answered(Answer::Yes, &r, &en()),
        Errand::Ask { ask: Ask::Sign, .. }
    ));

    // THE IDENTITY KEPT FOR THE SESSION: no question, the change goes.
    ronoctua_belongings::unlock::Marker {
        format: ronoctua_belongings::unlock::MARKER_FORMAT,
        repository: ronoctua_belongings::unlock::repository_tag(repository),
        until_s: i64::try_from(r.now).unwrap() + 600,
    }
    .write(&places)
    .unwrap();
    let mut v = open(Opening::Home, &r);
    assert_eq!(queued(&keep(&mut v, &r)), vec![declare()]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failed_job_is_said_on_the_sheet_and_its_consent_is_asked_next_time() {
    use ronoctua_transfers::{Engine, Job, Stage, Work};
    let mut r = readings(Some(Snapshot {
        supports: full().supports,
        ..Snapshot::default()
    }));
    let key = AppKey::AppImage {
        key: "upscayl".into(),
    };
    let mut job = Job::fresh_one(
        4,
        Engine::Software,
        Work::Software {
            order: Box::new(Order::Install {
                key: key.clone(),
                consent: Consent::default(),
            }),
            label: "Upscayl".into(),
        },
    );
    job.stage = Stage::Failed;
    job.refusal = Some(Refusal::UnverifiedNeedsConsent);
    r.jobs = vec![job];
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(key);
    v.at = Spot::Primary;
    let said = drawn(&rows_of(&v, &r, &en()));
    assert!(
        said.contains(&en().t(words::refusal_key(Refusal::UnverifiedNeedsConsent))[..20]),
        "{said}"
    );
    press(&mut v, Press::Enter, &r);
    assert!(matches!(
        v.asking,
        Some(Question::Consent {
            kind: ConsentKind::Unverified,
            ..
        })
    ));
}

#[test]
fn a_working_job_shows_on_its_row() {
    use ronoctua_transfers::{Engine, Job, Stage, Work};
    let mut r = readings(Some(full()));
    let mut job = Job::fresh_one(
        7,
        Engine::Software,
        Work::Software {
            order: Box::new(Order::Upgrade {
                scope: Scope::One {
                    key: gimp_flatpak(),
                },
            }),
            label: "GIMP".into(),
        },
    );
    job.stage = Stage::Running;
    job.phase = Some(ronoctua_software::progress::Phase::Downloading);
    job.done = 25;
    job.total = 100;
    r.jobs = vec![job];
    let v = open(Opening::Home, &r);
    let rows = rows_of(&v, &r, &en());
    let row = rows
        .iter()
        .find(|x| x.spot == Spot::Program(gimp_flatpak()))
        .unwrap();
    let said = drawn(std::slice::from_ref(row));
    assert!(
        said.contains("25 %") && said.contains("downloading"),
        "{said}"
    );
}

// ─── web apps and icons ────────────────────────────────────────────────────

#[test]
fn a_new_web_app_is_named_by_its_page_and_saved_as_one_order() {
    use ronoctua_software::icons::Candidate;
    let mut r = readings(Some(full()));
    let (mut v, e) = View::fresh(Opening::WebNew("mail.example.org".into()), &r, &en());
    assert_eq!(
        e,
        Errand::Feed(Want::Site("https://mail.example.org/".into()))
    );
    assert_eq!(v.level, Level::WebEditor);
    // BEFORE THE PAGE ANSWERS: the host is the name, and it says it is reading.
    r.site = Some(("https://mail.example.org/".into(), None));
    let said = drawn(&rows_of(&v, &r, &en()));
    assert!(
        said.contains("mail.example.org")
            && said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_WEB_FETCHING)),
        "{said}"
    );
    let save = rows_of(&v, &r, &en())
        .into_iter()
        .find(|x| x.spot == Spot::Primary)
        .unwrap();
    assert!(save.disabled, "saved with no icon");
    // THE PAGE ANSWERS: its name and its best icon fill what nobody typed.
    let icon = IconChoice::Site {
        url: "https://mail.example.org/icon.png".into(),
        page: None,
    };
    r.site = Some((
        "https://mail.example.org/".into(),
        Some(Ok((
            Some("Example Mail".into()),
            vec![Candidate {
                choice: icon.clone(),
                about: "manifest · 512 px".into(),
            }],
        ))),
    ));
    r.profiles = vec![ronoctua_software::web::ChromeProfile {
        directory: "Profile 1".into(),
        name: "Work".into(),
        last_used: true,
    }];
    let rows = rows_of(&v, &r, &en());
    assert!(drawn(&rows).contains("Example Mail") && drawn(&rows).contains("Work"));
    let icon_row = rows.iter().find(|x| x.spot == Spot::Ring("icon")).unwrap();
    assert_eq!(icon_row.picture, Some(Portrait::Candidate(icon.clone())));
    v.at = Spot::Primary;
    let e = press(&mut v, Press::Enter, &r);
    match &queued(&e)[..] {
        [Order::WebCreate { spec }] => {
            assert_eq!(spec.name, "Example Mail");
            assert_eq!(spec.url, "https://mail.example.org/");
            assert_eq!(spec.profile, "Profile 1");
            assert_eq!(spec.icon, icon);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(v.level, Level::Home, "saving leaves the editor");
}

#[test]
fn a_changed_draft_asks_before_it_is_left_and_letters_do_not_type_into_it() {
    let r = readings(Some(full()));
    let (mut v, _) = View::fresh(Opening::WebNew("mail.example.org".into()), &r, &en());
    assert_eq!(
        v.typed("software-web-name", "My Mail", &r, &en()),
        Errand::Nothing
    );
    match v.letter('x', &r, &en()) {
        Errand::Say(s) => assert!(s.contains("RET"), "{s}"),
        other => panic!("{other:?}"),
    }
    press(&mut v, Press::Escape, &r);
    assert_eq!(v.asking, Some(Question::LeaveDraft));
    assert_eq!(v.at, Spot::Answer(Answer::No));
    press(&mut v, Press::Escape, &r);
    assert_eq!(v.level, Level::WebEditor, "no keeps the draft");
}

#[test]
fn an_icon_is_picked_from_a_shelf_and_the_editor_comes_back() {
    use ronoctua_software::icons::Candidate;
    let mut r = readings(Some(full()));
    let (mut v, _) = View::fresh(Opening::WebNew("github.com".into()), &r, &en());
    v.at = Spot::Ring("icon");
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(v.level, Level::Icons(Shelf::Site));
    assert_eq!(
        e,
        Errand::Nothing,
        "the site's shelf is the page's own read"
    );
    let e = press(&mut v, Press::NextSource, &r);
    assert_eq!(v.level, Level::Icons(Shelf::Dashboard));
    let query = match e {
        Errand::Feed(Want::Icons {
            shelf: Shelf::Dashboard,
            query,
        }) => query,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        query, "github.com",
        "the first screen already looks for something"
    );
    let choice = IconChoice::Dashboard {
        name: "github-light".into(),
        variant: "light".into(),
    };
    r.candidates = Some(crate::software_feed::Shelved {
        generation: 1,
        shelf: Shelf::Dashboard,
        query,
        answer: Some(Ok(vec![Candidate {
            choice: choice.clone(),
            about: "dashboardicons · github-light · SVG".into(),
        }])),
    });
    let rows = rows_of(&v, &r, &en());
    let row = rows
        .iter()
        .find(|x| x.target == Target::Candidate(choice.clone()))
        .expect("the candidate");
    assert_eq!(row.picture, Some(Portrait::Candidate(choice.clone())));
    v.at = Spot::Candidate(choice.clone());
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.level, Level::WebEditor);
    assert_eq!(v.draft.as_ref().and_then(|d| d.icon.clone()), Some(choice));
    // AND THE FILE SHELF offers the path, the clipboard and the manager.
    v.level = Level::Icons(Shelf::File);
    let said = drawn(&rows_of(&v, &r, &en()));
    assert!(
        said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_ICON_CLIPBOARD)),
        "{said}"
    );
}

#[test]
fn a_dropped_image_is_the_icon_and_a_dropped_package_is_asked_about() {
    let r = readings(Some(full()));
    let (mut v, _) = View::fresh(Opening::WebNew("github.com".into()), &r, &en());
    v.dropped(&["/home/somebody/me.png".into()], &r, &en());
    assert_eq!(
        v.draft.as_ref().and_then(|d| d.icon.clone()),
        Some(IconChoice::File {
            path: "/home/somebody/me.png".into()
        })
    );
    let mut v = open(Opening::Home, &r);
    v.dropped(&["/home/somebody/x.apk".into()], &r, &en());
    assert_eq!(
        v.asking,
        Some(Question::Foreign("/home/somebody/x.apk".into()))
    );
    v.at = Spot::Answer(Answer::Yes);
    let e = press(&mut v, Press::Enter, &r);
    match &queued(&e)[..] {
        [Order::InstallFile { path, consent }] => {
            assert_eq!(path, Path::new("/home/somebody/x.apk"));
            assert!(consent.unverified);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn editing_a_web_app_reads_its_file_first() {
    let mut r = readings(Some(full()));
    let mut v = open(Opening::Home, &r);
    v.at = Spot::Program(web());
    let e = press(&mut v, Press::Edit, &r);
    assert_eq!(e, Errand::Feed(Want::WebApp("mail".into())));
    assert_eq!(v.level, Level::WebEditor);
    r.web_app = Some((
        "mail".into(),
        Some(Some(ronoctua_software::web::WebApp {
            slug: "mail".into(),
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon: Some(IconChoice::Collection {
                name: "mail".into(),
            }),
            icon_file: None,
            category: Some("Office".into()),
        })),
    ));
    let rows = rows_of(&v, &r, &en());
    let said = drawn(&rows);
    assert!(
        said.contains("https://mail.example.org/")
            && said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_CATEGORY_OFFICE)),
        "{said}"
    );
    assert!(rows.iter().any(|x| x.spot == Spot::Deed("web-remove")));
}

// ─── android, maintenance, advanced ────────────────────────────────────────

#[test]
fn android_with_google_play_shows_the_id_to_register_and_the_page() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Android, &r);
    let rows = rows_of(&v, &r, &en());
    let id = rows
        .iter()
        .find(|x| x.spot == Spot::Fact("android-id"))
        .unwrap();
    assert!(drawn(std::slice::from_ref(id)).contains("3a5f9c0e71d2b4a8"));
    let page = rows
        .iter()
        .find(|x| x.spot == Spot::Deed("register"))
        .unwrap();
    assert_eq!(
        page.target,
        Target::Link(ronoctua_software::android::UNCERTIFIED.to_owned())
    );
    v.at = Spot::Fact("android-id");
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Copy("3a5f9c0e71d2b4a8".into())
    );
    v.at = Spot::Primary;
    assert_eq!(
        queued(&press(&mut v, Press::Stop, &r)),
        vec![Order::AndroidSession { on: false }]
    );
}

#[test]
fn android_not_prepared_offers_google_play_first_and_asks() {
    let mut snap = full();
    snap.android = Android {
        support: true,
        ..Android::default()
    };
    let r = readings(Some(snap));
    let mut v = open(Opening::Android, &r);
    let rows = rows_of(&v, &r, &en());
    let image = rows.iter().find(|x| x.spot == Spot::Ring("image")).unwrap();
    assert!(
        drawn(std::slice::from_ref(image)).contains("Google Play"),
        "GAPPS is the default"
    );
    v.at = Spot::Primary;
    press(&mut v, Press::Enter, &r);
    assert_eq!(
        v.asking,
        Some(Question::PrepareAndroid(AndroidImage::Gapps))
    );
    v.at = Spot::Answer(Answer::Yes);
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Prepare {
            source: Source::Android,
            image: Some(AndroidImage::Gapps)
        }]
    );
    // VANILLA IS STILL OFFERED.
    v.at = Spot::Ring("image");
    press(&mut v, Press::More, &r);
    assert_eq!(v.image, AndroidImage::Vanilla);
}

#[test]
fn maintenance_updates_by_source_and_frees_space_after_a_question() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Maintenance, &r);
    let rows = rows_of(&v, &r, &en());
    let said = drawn(&rows);
    assert!(said.contains("5b4f72e"), "{said}");
    assert!(
        said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_WEB_ALWAYS_LATEST)),
        "{said}"
    );
    v.at = Spot::Primary;
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Upgrade {
            scope: Scope::Everything
        }]
    );
    v.at = Spot::Status(Source::Flatpak);
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Upgrade {
            scope: Scope::Source {
                source: Source::Flatpak
            }
        }]
    );
    v.at = Spot::Deed("tidy-all");
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.asking, Some(Question::CleanUp(None)));
    v.at = Spot::Answer(Answer::Yes);
    let tidied = queued(&press(&mut v, Press::Enter, &r));
    assert_eq!(tidied.len(), 3, "{tidied:?}");
    // GOING BACK asks too; the version in use is only a fact.
    v.at = Spot::Generation(2);
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.asking, Some(Question::Rollback(2)));
    let rows = {
        v.close_question();
        rows_of(&v, &r, &en())
    };
    assert_eq!(
        rows.iter()
            .find(|x| x.spot == Spot::Generation(3))
            .unwrap()
            .target,
        Target::Nothing
    );
    // AUTOMATICALLY is the brain's page, and the sentence names its keys.
    assert!(drawn(&rows).contains("LEADER-b"));
}

/// FAILS IF: a catalogue that is on and whose hosts nobody let draws no
/// consent row — and asks nothing, so a search finds nothing with no word on
/// why — or the row stays once its hosts are let, or pressing it does not
/// let them, or a catalogue switched off asks for a consent it would not use.
#[test]
fn a_catalogue_not_let_draws_its_consent_row() {
    let mut r = readings(Some(full()));
    let (mut v, _) = View::fresh(Opening::Advanced, &r, &en());
    let rows = rows_of(&v, &r, &en());
    for s in [
        OnlineSource::Flathub,
        OnlineSource::Fdroid,
        OnlineSource::AppImageHub,
    ] {
        let row = rows
            .iter()
            .find(|x| x.spot == Spot::Let(s.id()))
            .unwrap_or_else(|| panic!("{s:?} has no consent row"));
        assert!(drawn(std::slice::from_ref(row)).contains(s.hosts()[0]));
    }
    v.at = Spot::Let(OnlineSource::Fdroid.id());
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Let(OnlineSource::Fdroid)
    );
    r.consents = r.consents.clone().with(OnlineSource::Fdroid.hosts(), true);
    r.prefs.sources.flathub = false;
    let rows = rows_of(&v, &r, &en());
    assert!(
        !rows.iter().any(|x| x.spot == Spot::Let("fdroid")),
        "let, and still asking"
    );
    assert!(
        !rows.iter().any(|x| x.spot == Spot::Let("flathub")),
        "off, and asking"
    );
    assert!(rows.iter().any(|x| x.spot == Spot::Let("appimagehub")));
}

#[test]
fn a_preference_is_one_tweak_and_the_ring_turns_from_what_the_file_says() {
    let mut r = readings(Some(full()));
    let (mut v, e) = View::fresh(Opening::Advanced, &r, &en());
    assert_eq!(e, Errand::Feed(Want::Dark));
    v.at = Spot::Ring("prefer");
    let e = press(&mut v, Press::Enter, &r);
    assert_eq!(e, Errand::Tweak(Tweak::Prefer(Prefer::FlatpakFirst)));
    let mut p = r.prefs.clone();
    if let Errand::Tweak(t) = &e {
        t.apply(&mut p);
    }
    assert_eq!(p.prefer, Prefer::FlatpakFirst);
    // THE VIEW HOLDS NO COPY: the next turn reads the file's value.
    r.prefs = p;
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Tweak(Tweak::Prefer(Prefer::NixFirst))
    );
    v.at = Spot::Ring("ask-above");
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Tweak(Tweak::AskAboveMb(1_000))
    );
    v.at = Spot::Ring("flathub");
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Tweak(Tweak::Switch(OnlineSource::Flathub, false))
    );
    // A SWITCH UNDER A SEARCH THAT IS OFF refuses, and says why.
    r.prefs.online = false;
    let rows = rows_of(&v, &r, &en());
    assert!(
        rows.iter()
            .find(|x| x.spot == Spot::Ring("flathub"))
            .unwrap()
            .disabled
    );
}

#[test]
fn the_dark_diagnosis_names_the_app_that_forces_gtk_theme_in_red() {
    let mut r = readings(Some(full()));
    r.dark = Some(Some(forcing_gimp()));
    let v = open(Opening::Advanced, &r);
    let rows = rows_of(&v, &r, &en());
    let row = rows.iter().find(|x| {
        x.spot == Spot::Permissions("org.gimp.GIMP".into())
            && x.fields
                .iter()
                .any(|f| f.color == tokens::color::STATE_ERROR)
    });
    assert!(row.is_some(), "{}", drawn(&rows));
}

#[test]
fn a_consent_is_taken_back_with_remove() {
    let mut r = readings(Some(full()));
    r.prefs.unfree.insert("google-chrome".into());
    let mut v = open(Opening::Home, &r);
    v.level = Level::Consents;
    v.at = Spot::Consent("google-chrome".into());
    assert_eq!(
        press(&mut v, Press::Remove, &r),
        Errand::Tweak(Tweak::Unfree {
            attr: "google-chrome".into(),
            allowed: false
        })
    );
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Copy("google-chrome".into())
    );
}

#[test]
fn every_row_shape_a_door_draws_is_drawn() {
    // THE SHAPES, one each: a title with a picture, a program with its glyph
    // and lit letters, a drawer, a refusing row, a danger answer, a note.
    let mut r = readings(Some(empty()));
    searched(&mut r, "gi");
    let mut v = open(Opening::Home, &r);
    v.query = "gi".into();
    v.level = Level::Results;
    let rows = rows_of(&v, &r, &en());
    let program = rows
        .iter()
        .find(|x| matches!(x.target, Target::Group(_)))
        .unwrap();
    assert_eq!(
        program.picture,
        Some(Portrait::Program(Picture::Named {
            name: "gimp".into()
        }))
    );
    assert!(
        program.fields.len() > 3,
        "the matched letters are not lit: {:?}",
        program.fields
    );
    assert!(program.fields.iter().any(
        |f| f.color == tokens::color::STATE_ACCENT || f.color == tokens::color::TEXT_MATCH_HERE
    ));

    v.level = Level::Sheet(gimp_nix());
    let rows = rows_of(&v, &r, &en());
    let title = rows
        .iter()
        .find(|x| x.heading && x.picture.is_some())
        .expect("a title with a picture");
    assert_eq!(title.fields.len(), 2, "the glyph and the name");
    let drawer = rows
        .iter()
        .find(|x| x.target == Target::Drawer("details"))
        .expect("the details drawer");
    assert_eq!(
        drawer.fields[0].text.trim(),
        tokens::icon::EXPAND.to_string()
    );
    assert!(
        rows.iter().any(|x| x.heading
            && x.fields
                .first()
                .is_some_and(|f| f.color == tokens::color::TEXT_DIM)),
        "a note"
    );

    v.ask_it(Question::Remove {
        key: gimp_nix(),
        name: "GIMP".into(),
    });
    let rows = rows_of(&v, &r, &en());
    let danger = rows
        .iter()
        .find(|x| x.target == Target::Answer(Answer::YesAndMore))
        .unwrap();
    assert!(
        danger
            .fields
            .iter()
            .any(|f| f.color == tokens::color::STATE_ERROR),
        "{:?}",
        danger.fields
    );

    let mut snap = empty();
    snap.supports.web = false;
    let r = readings(Some(snap));
    let v = open(Opening::Home, &r);
    let refusing = v
        .rows(&r, DECIDING_WIDTH, &en())
        .into_iter()
        .find(|x| x.disabled)
        .expect("a refusing row");
    assert!(refusing.said.is_some());
}

#[test]
fn the_history_opens_the_program_it_names() {
    let mut r = readings(Some(full()));
    r.history = vec![ronoctua_software::journal::Entry {
        when: 1_750_000_000,
        order: "remove".into(),
        key: Some(gimp_flatpak()),
        name: "GIMP".into(),
        ok: false,
        refusal: Some(Refusal::DeclaredInTheFlake),
    }];
    let mut v = open(Opening::Home, &r);
    v.level = Level::History;
    let rows = rows_of(&v, &r, &en());
    let said = drawn(&rows);
    assert!(
        said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_ORDER_REMOVE)),
        "{said}"
    );
    v.at = Spot::Entry(0);
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Feed(Want::Details(gimp_flatpak()))
    );
}

// ─── notes and budgets ─────────────────────────────────────────────────────

#[test]
fn a_note_wraps_and_is_never_cut() {
    let r = readings(Some(full()));
    let v = open(Opening::Home, &r);
    let note = en().t(ronoctua_i18n::key::SOFTWARE_NOTE_YOURS);
    let lines_at = |columns: u16| -> usize {
        let rows = v.rows(&r, columns, &en());
        let said = drawn(&rows);
        assert!(
            !said.contains('…') || !note.split(' ').any(|w| said.contains(&format!("{w}…"))),
            "cut at {columns}"
        );
        // EVERY WORD OF THE NOTE IS ON THE SCREEN.
        for word in note.split_whitespace() {
            assert!(said.contains(word), "`{word}` lost at {columns}:\n{said}");
        }
        rows.iter()
            .filter(|x| {
                x.heading
                    && x.fields
                        .first()
                        .is_some_and(|f| note.contains(f.text.trim()) && !f.text.trim().is_empty())
            })
            .count()
    };
    assert!(
        lines_at(40) > lines_at(160),
        "the note did not wrap on a narrow door"
    );
}

#[test]
fn every_label_fits_its_row_in_the_longest_language() {
    // THE FOOTER'S NARROWEST WIDTH WORTH HAVING, and a label is never cut
    // there in either language: the reserved room is the longest
    // translation's, not the English one's.
    let mut r = readings(Some(full()));
    searched(&mut r, "gimp");
    r.dark = Some(Some(DarkReading::default()));
    let levels = [
        Level::Home,
        Level::Results,
        Level::Sheet(gimp_nix()),
        Level::Sheet(gimp_flatpak()),
        Level::Trials,
        Level::Android,
        Level::Maintenance,
        Level::Advanced,
        Level::Consents,
        Level::History,
    ];
    for t in every_language() {
        for level in &levels {
            let mut v = open(Opening::Home, &r);
            v.level = level.clone();
            v.query = "gimp".into();
            for row in v
                .rows(&r, 80, &t)
                .iter()
                .filter(|x| !x.heading && x.fields.len() >= 2)
            {
                // The label is the field before the value: the first, or the
                // second on a row with a glyph in front.
                let label = if row.fields.len() >= 3 {
                    &row.fields[1]
                } else {
                    &row.fields[0]
                };
                assert!(
                    !label.text.ends_with('…'),
                    "{}: a label cut at 80: {:?} in {level:?}",
                    t.language(),
                    label.text
                );
            }
        }
    }
}

#[test]
fn every_disabled_row_says_why() {
    let mut snap = full();
    snap.supports.web = false;
    let r = readings(Some(snap));
    let mut v = open(Opening::Home, &r);
    let rows = rows_of(&v, &r, &en());
    let web_new = rows
        .iter()
        .find(|x| x.spot == Spot::Deed("web-new"))
        .unwrap();
    assert!(web_new.disabled);
    assert_eq!(
        web_new.said.as_deref(),
        Some(en().t(words::refusal_key(Refusal::NoBrowser)).as_str())
    );
    v.at = Spot::Deed("web-new");
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Say(en().t(words::refusal_key(Refusal::NoBrowser)))
    );
    for level in [
        Level::Home,
        Level::Maintenance,
        Level::Advanced,
        Level::Android,
    ] {
        v.level = level;
        for row in v.rows(&r, DECIDING_WIDTH, &en()) {
            assert_eq!(row.disabled, row.said.is_some(), "{:?}", row.spot);
        }
    }
}

#[test]
fn every_spot_is_one_row() {
    // THE CURSOR KEEPS AN IDENTITY: two rows with one spot would be the
    // cursor on two rows at once.
    let mut r = readings(Some(full()));
    searched(&mut r, "gimp");
    r.dark = Some(Some(DarkReading::default()));
    for level in [
        Level::Home,
        Level::Results,
        Level::Sheet(gimp_nix()),
        Level::Sheet(gimp_flatpak()),
        Level::Trials,
        Level::Android,
        Level::Maintenance,
        Level::Advanced,
    ] {
        let mut v = open(Opening::Home, &r);
        v.level = level.clone();
        v.query = "gimp".into();
        v.open = vec!["details", "older"];
        let rows = v.rows(&r, DECIDING_WIDTH, &en());
        let spots: Vec<&Spot> = rows
            .iter()
            .filter(|x| !x.heading)
            .map(|x| &x.spot)
            .collect();
        for (i, s) in spots.iter().enumerate() {
            assert!(!spots[i + 1..].contains(s), "{level:?}: {s:?} twice");
        }
    }
}

#[test]
fn a_tweak_changes_one_thing() {
    let mut p = Preferences::default();
    Tweak::Unfree {
        attr: "x".into(),
        allowed: true,
    }
    .apply(&mut p);
    assert!(p.unfree.contains("x"));
    Tweak::Switch(OnlineSource::Dashboard, false).apply(&mut p);
    assert!(!p.sources.dashboard && p.sources.flathub);
    Tweak::WebProfile(Some("Profile 1".into())).apply(&mut p);
    assert_eq!(p.web_profile.as_deref(), Some("Profile 1"));
    Tweak::Unfree {
        attr: "x".into(),
        allowed: false,
    }
    .apply(&mut p);
    assert!(p.unfree.is_empty());
}

#[test]
fn a_desktop_id_finds_its_program_or_is_the_system_s() {
    let snap = full();
    assert_eq!(
        key_of_desktop(Some(&snap), "org.gimp.GIMP.desktop"),
        gimp_flatpak()
    );
    assert_eq!(
        key_of_desktop(Some(&snap), "firefox.desktop"),
        AppKey::System {
            desktop_id: "firefox.desktop".into()
        }
    );
    // THE LAUNCHER'S SPELLING, without the extension, finds the same program.
    assert_eq!(key_of_desktop(Some(&snap), "org.gimp.GIMP"), gimp_flatpak());
    assert!(same_desktop("org.gimp.GIMP", "org.gimp.GIMP.desktop"));
    assert!(!same_desktop("org.gimp.GIMP", "org.gimp.GIMP2.desktop"));
}

// ─── permissions, Android's windows and the dark facts ─────────────────────

/// The permissions page of GIMP, read.
fn permissions_page() -> (View, Readings) {
    let mut r = readings(Some(full()));
    r.permissions = Some(("org.gimp.GIMP".into(), Some(Some(gimp_permits()))));
    let mut v = open(Opening::Home, &r);
    v.level = Level::Permissions("org.gimp.GIMP".into());
    v.at = Spot::Top;
    (v, r)
}

#[test]
fn a_permission_ring_is_an_order_from_what_the_app_reaches_now() {
    let (mut v, r) = permissions_page();
    let rows = rows_of(&v, &r, &en());
    let files = rows
        .iter()
        .find(|x| x.spot == Spot::Ring("grant-files"))
        .expect("the files ring");
    assert!(
        drawn(std::slice::from_ref(files)).contains(&en().t(words::reach_key(Reach::ReadWrite))),
        "{}",
        drawn(&rows)
    );
    v.at = Spot::Ring("grant-files");
    assert_eq!(
        queued(&press(&mut v, Press::More, &r)),
        vec![Order::Permission {
            id: "org.gimp.GIMP".into(),
            grant: Grant::Files {
                reach: Reach::ReadOnly
            }
        }]
    );
    // A SWITCH TURNS TO ITS OTHER SIDE, from what the file says.
    v.at = Spot::Ring("grant-network");
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Permission {
            id: "org.gimp.GIMP".into(),
            grant: Grant::Network { on: false }
        }]
    );
    v.at = Spot::Ring("grant-display");
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Permission {
            id: "org.gimp.GIMP".into(),
            grant: Grant::Display {
                through: Display::X11
            }
        }]
    );
    // AND THE PAGE ASKS AGAIN once the queue is done.
    assert_eq!(
        v.rereads(),
        vec![Want::Reread, Want::Permissions("org.gimp.GIMP".into())]
    );
}

#[test]
fn gtk_theme_is_only_taken_away_after_a_question_that_opens_on_no() {
    let (mut v, r) = permissions_page();
    let rows = rows_of(&v, &r, &en());
    let row = rows
        .iter()
        .find(|x| x.spot == Spot::Deed("unset-gtk-theme"))
        .expect("the row that takes GTK_THEME away");
    assert!(
        row.fields
            .iter()
            .any(|f| f.color == tokens::color::STATE_ERROR),
        "danger is a colour"
    );
    v.at = Spot::Deed("unset-gtk-theme");
    assert_eq!(press(&mut v, Press::Enter, &r), Errand::Nothing);
    let asked = rows_of(&v, &r, &en());
    assert_eq!(focused(&asked).spot, Spot::Answer(Answer::No));
    assert_eq!(
        press(&mut v, Press::Enter, &r),
        Errand::Nothing,
        "no does nothing"
    );
    v.at = Spot::Deed("unset-gtk-theme");
    press(&mut v, Press::Enter, &r);
    v.at = Spot::Answer(Answer::Yes);
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Permission {
            id: "org.gimp.GIMP".into(),
            grant: Grant::UnsetGtkTheme
        }]
    );
    // AN APP THAT DOES NOT CARRY IT HAS NO SUCH ROW: there is nothing to
    // take away, and nothing here can put it in.
    let mut r = r;
    if let Some((_, Some(Some(p)))) = r.permissions.as_mut() {
        p.grants.gtk_theme = false;
    }
    let rows = rows_of(&v, &r, &en());
    assert!(
        !rows.iter().any(|x| x.spot == Spot::Deed("unset-gtk-theme")),
        "{}",
        drawn(&rows)
    );
}

#[test]
fn restoring_the_defaults_is_asked_first_and_is_one_reset() {
    let (mut v, r) = permissions_page();
    v.at = Spot::Deed("reset-permissions");
    press(&mut v, Press::Enter, &r);
    assert!(matches!(v.asking, Some(Question::Permission { .. })));
    v.at = Spot::Answer(Answer::Yes);
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Permission {
            id: "org.gimp.GIMP".into(),
            grant: Grant::Reset
        }]
    );
}

#[test]
fn a_program_with_no_window_has_no_display_ring() {
    let (v, mut r) = permissions_page();
    if let Some((_, Some(Some(p)))) = r.permissions.as_mut() {
        p.grants.display = None;
    }
    let rows = rows_of(&v, &r, &en());
    assert!(!rows.iter().any(|x| x.spot == Spot::Ring("grant-display")));
    assert!(rows.iter().any(|x| x.spot == Spot::Ring("grant-sound")));
}

#[test]
fn the_permission_rows_draw_in_every_language() {
    let (mut v, r) = permissions_page();
    v.open = vec!["asks"];
    for t in every_language() {
        let rows = rows_of(&v, &r, &t);
        assert!(rows.iter().any(|x| x.spot == Spot::Permission(0)));
    }
    v.at = Spot::Deed("reset-permissions");
    press(&mut v, Press::Enter, &r);
    for t in every_language() {
        rows_of(&v, &r, &t);
    }
}

#[test]
fn android_s_windows_are_a_ring_and_say_when_they_change() {
    let r = readings(Some(full()));
    let mut v = open(Opening::Android, &r);
    let rows = rows_of(&v, &r, &en());
    assert!(
        drawn(&rows).contains(&en().t(ronoctua_i18n::key::SOFTWARE_NOTE_WINDOWS_NEXT_SESSION)),
        "{}",
        drawn(&rows)
    );
    v.at = Spot::Ring("windows");
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::AndroidWindows { multi: false }]
    );
    // UNKNOWN IS WAYDROID'S OWN DEFAULT, one window for all: the turn asks
    // for one per app.
    let mut snap = full();
    snap.android.multi_window = None;
    let r = readings(Some(snap));
    let rows = rows_of(&v, &r, &en());
    let ring = rows
        .iter()
        .find(|x| x.spot == Spot::Ring("windows"))
        .unwrap();
    assert!(
        drawn(std::slice::from_ref(ring))
            .contains(&en().t(ronoctua_i18n::key::SOFTWARE_ANDROID_ONE_FOR_ALL))
    );
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::AndroidWindows { multi: true }]
    );
}

#[test]
fn the_dark_facts_are_read_and_not_assumed() {
    let mut r = readings(Some(full()));
    let v = open(Opening::Advanced, &r);
    // STILL ASKING: neither fact is stated before it is read.
    r.dark = Some(None);
    let rows = rows_of(&v, &r, &en());
    let gtk3 = rows
        .iter()
        .find(|x| x.spot == Spot::Fact("dark-gtk3"))
        .unwrap();
    assert!(
        drawn(std::slice::from_ref(gtk3)).contains(&en().t(ronoctua_i18n::key::SOFTWARE_ASKING))
    );
    r.dark = Some(Some(DarkReading {
        forcing: Vec::new(),
        facts: ronoctua_software::flatpak::DarkFacts {
            extension: false,
            qt_unset: false,
        },
    }));
    for t in every_language() {
        let rows = rows_of(&v, &r, &t);
        let said = drawn(&rows);
        assert!(
            said.contains(&t.t(ronoctua_i18n::key::SOFTWARE_DARK_GTK3_MISSING)),
            "{said}"
        );
        assert!(
            said.contains(&t.t(ronoctua_i18n::key::SOFTWARE_DARK_QT_LEAKS)),
            "{said}"
        );
    }
    assert_eq!(v.rereads(), vec![Want::Reread, Want::Dark]);
}

// ─── what the doors are handed ──────────────────────────────────────────────

/// A job of the software engine, at a stage.
fn job(order: Order, stage: ronoctua_transfers::Stage) -> ronoctua_transfers::Job {
    let mut j = ronoctua_transfers::Job::fresh_one(
        1,
        ronoctua_transfers::Engine::Software,
        ronoctua_transfers::Work::Software {
            order: Box::new(order),
            label: "GIMP".into(),
        },
    );
    j.stage = stage;
    j
}

#[test]
fn an_end_is_a_notification_only_when_it_is_worth_one() {
    use ronoctua_transfers::Stage;
    let install = Order::Install {
        key: gimp_flatpak(),
        consent: Consent::default(),
    };
    assert_eq!(
        ended_of(&job(install.clone(), Stage::Done)),
        Some(Ended::Installed)
    );
    assert_eq!(
        ended_of(&job(install.clone(), Stage::Failed)),
        Some(Ended::Failed)
    );
    // THE PERSON CANCELLED IT, and knows.
    assert_eq!(ended_of(&job(install, Stage::Cancelled)), None);
    assert_eq!(
        ended_of(&job(
            Order::Prepare {
                source: Source::Android,
                image: Some(AndroidImage::Gapps)
            },
            Stage::Done
        )),
        Some(Ended::AndroidReady)
    );
    assert_eq!(
        ended_of(&job(
            Order::Prepare {
                source: Source::Flatpak,
                image: None
            },
            Stage::Done
        )),
        None
    );
    // A TRIAL STARTING is on screen already; its END is the unit's.
    assert_eq!(
        ended_of(&job(
            Order::Try {
                attr: "hello".into(),
                traces: Traces::Scratch,
                network: true,
                consent: Consent::default()
            },
            Stage::Done
        )),
        None
    );
    // AND A JOB OF ANOTHER ENGINE is never one of these.
    let mut copy = job(Order::AndroidErase, Stage::Done);
    copy.work = ronoctua_transfers::Work::Measure {
        path: "/tmp".into(),
    };
    assert_eq!(ended_of(&copy), None);
}

#[test]
fn a_failure_is_said_with_the_refusal_s_sentence_first() {
    use ronoctua_transfers::Stage;
    let t = en();
    let mut broke = job(Order::AndroidErase, Stage::Failed);
    broke.refusal = Some(Refusal::DeclaredInTheFlake);
    broke.trouble = Some("declared-in-the-flake".into());
    assert_eq!(
        failure_said(&broke, &t),
        t.t(words::refusal_key(Refusal::DeclaredInTheFlake))
    );
    broke.refusal = None;
    broke.trouble = Some("error: out of disk\n".into());
    assert_eq!(failure_said(&broke, &t), "error: out of disk");
    broke.trouble = None;
    assert_eq!(
        failure_said(&broke, &t),
        t.t(ronoctua_i18n::key::SOFTWARE_FAILED)
    );
}

#[test]
fn the_marks_say_nothing_at_rest_and_one_glyph_each_otherwise() {
    use crate::software_feed::Running;
    assert!(segments(&Running::default()).is_empty(), "a mark at rest");
    let both = segments(&Running {
        trials: vec![("hello".into(), "hello".into())],
        android: true,
    });
    let ids: Vec<&str> = both.iter().map(|s| s.id).collect();
    assert_eq!(ids, [TRIAL_MARK, ANDROID_MARK]);
    assert_eq!(both[0].text, tokens::icon::SOFTWARE_TRIAL.to_string());
    assert_eq!(both[1].text, tokens::icon::SOFTWARE_ANDROID.to_string());
    assert!(
        both.iter().all(|s| s.removable),
        "a mark gives way on a narrow line"
    );
    // AND THEY FOLLOW A READING of the machine.
    let r = Running::of(&full());
    assert_eq!(r.trials, vec![("hello".to_owned(), "hello".to_owned())]);
    assert!(r.android);
}

#[test]
fn the_flake_s_program_is_not_asked_about_and_the_sheet_says_why() {
    let r = readings(Some(full()));
    let t = en();
    let system = AppKey::System {
        desktop_id: "firefox".into(),
    };
    let (v, errand) = View::fresh(
        Opening::Remove {
            key: system.clone(),
            name: "Firefox".into(),
        },
        &r,
        &t,
    );
    assert_eq!(v.asking, None, "a question whose yes the CLI would refuse");
    assert_eq!(v.level, Level::Sheet(system));
    assert_eq!(
        errand,
        Errand::Say(t.t(words::refusal_key(Refusal::DeclaredInTheFlake)))
    );
    // THE PERSON'S OWN is asked about, on no.
    let (v, _) = View::fresh(
        Opening::Remove {
            key: gimp_flatpak(),
            name: "GIMP".into(),
        },
        &r,
        &t,
    );
    assert!(matches!(v.asking, Some(Question::Remove { .. })));
    assert_eq!(v.at, Spot::Answer(Answer::No));
}

#[test]
fn a_second_key_turns_the_open_view_and_esc_comes_back() {
    let r = readings(Some(full()));
    let t = en();
    let mut v = open(Opening::Home, &r);
    v.reopen(Opening::Trials, &r, &t);
    assert_eq!(v.level, Level::Trials);
    press(&mut v, Press::Escape, &r);
    assert_eq!(v.level, Level::Home, "the level it left is the way back");
    assert_eq!(v.at, Spot::Search, "and the cursor where it was");
    // HOME FROM DEEP DOWN goes all the way up.
    v.reopen(Opening::Maintenance, &r, &t);
    v.reopen(Opening::Android, &r, &t);
    v.reopen(Opening::Home, &r, &t);
    assert_eq!(v.level, Level::Home);
    assert_eq!(press(&mut v, Press::Escape, &r), Errand::Close);
}

#[test]
fn ctrl_return_is_install_in_the_notation_both_doors_write() {
    // THE FOOTER builds `Key { ctrl, Enter }` from the keysym and writes its
    // notation; the window's hint presses the same key back through
    // `door::keys::event_of`. Both meet at `C-RET`.
    let key = ronoctua_keymap::Key {
        mods: ronoctua_keymap::Mods {
            ctrl: true,
            ..ronoctua_keymap::Mods::default()
        },
        sym: ronoctua_keymap::Sym::Enter,
    };
    assert_eq!(key.notation(), "C-RET");
    assert_eq!(pressed(&key.notation()), Some(Press::Install));
    let (event, mods) = crate::door::keys::event_of(key);
    assert_eq!(
        event.keysym,
        smithay_client_toolkit::seat::keyboard::Keysym::Return
    );
    assert!(mods.ctrl);
}

// ─── the declaration ───────────────────────────────────────────────────────

/// A machine whose person declared five programs — applied, left out for a
/// name nixpkgs dropped, held for a download that changed, on another
/// computer only, and waiting — and declined six of the distro's, one of
/// each state.
fn declared_machine() -> Snapshot {
    use ronoctua_software::declaration::{
        DeclaredRow, DeclineState, DeclinedRow, DistroRow, Standing,
    };
    use ronoctua_software::person::Stamp;
    use ronoctua_software::person::declared::{AppImageApp, Entry, NixApp, Pin};
    use ronoctua_software::person::declined::Declining;
    use ronoctua_software::person::reports::{DistroApp, SkipWhy};
    let at = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
    let nix = |attr: &str, machines: Option<Vec<String>>| {
        Entry::Nix(NixApp {
            id: format!("nix:{attr}"),
            machines,
            edited_at: at.clone(),
            edited_on: "noctua".into(),
            attr: attr.into(),
            unfree: false,
        })
    };
    let obsidian = Entry::AppImage(AppImageApp {
        id: "appimage:obsidian".into(),
        machines: None,
        edited_at: at.clone(),
        edited_on: "laptop".into(),
        key: "obsidian".into(),
        name: "Obsidian".into(),
        pin: Pin {
            url: "https://github.com/obsidianmd/obsidian-releases/releases/download/v1.6.7/Obsidian-1.6.7.AppImage".into(),
            hash: "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=".into(),
            version: "v1.6.7".into(),
            pinned_at: at.clone(),
        },
        follows: None,
        icon: None,
    });
    let declining = |id: &str, machines: Option<Vec<String>>| Declining {
        id: id.into(),
        machines,
        edited_at: at.clone(),
        edited_on: "noctua".into(),
    };
    let distro = |id: &str, name: &str, essential: Option<&str>| DistroRow {
        app: DistroApp {
            id: id.into(),
            source: ronoctua_software::person::declared::Source::Nix,
            desktop_ids: vec![format!("{id}.desktop")],
            essential: essential.map(str::to_owned),
            group: "base".into(),
            on_this_machine: true,
            declined_here: false,
        },
        name: name.into(),
    };
    let mut s = full();
    s.installed[0].origin = Origin::MineDeclared;
    s.declarations = Declarations {
        host: "noctua".into(),
        declared: vec![
            DeclaredRow {
                entry: nix("ripgrep", None),
                standing: Standing::Applied,
            },
            DeclaredRow {
                entry: nix("gone", None),
                standing: Standing::Skipped {
                    reason: SkipWhy::AttrMissing,
                    detail: String::new(),
                },
            },
            DeclaredRow {
                entry: obsidian,
                standing: Standing::Skipped {
                    reason: SkipWhy::Held,
                    detail: "upstream-changed".into(),
                },
            },
            DeclaredRow {
                entry: nix("rarely", Some(vec!["laptop".into()])),
                standing: Standing::Elsewhere,
            },
            DeclaredRow {
                entry: nix("fresh", None),
                standing: Standing::Waiting,
            },
        ],
        declined: vec![
            DeclinedRow {
                declining: declining("gimp", None),
                name: "GIMP".into(),
                state: DeclineState::Honoured,
            },
            DeclinedRow {
                declining: declining("vlc", None),
                name: "VLC".into(),
                state: DeclineState::StillPresent,
            },
            DeclinedRow {
                declining: declining("zsh", None),
                name: "Zsh".into(),
                state: DeclineState::Essential {
                    reason: "the-login-shell".into(),
                },
            },
            DeclinedRow {
                declining: declining("old-thing", None),
                name: "old-thing".into(),
                state: DeclineState::Unknown,
            },
            DeclinedRow {
                declining: declining("htop", Some(vec!["laptop".into()])),
                name: "htop".into(),
                state: DeclineState::Elsewhere,
            },
            DeclinedRow {
                declining: declining("btop", None),
                name: "btop".into(),
                state: DeclineState::Waiting,
            },
        ],
        distro: vec![
            distro("gimp", "GIMP", None),
            distro("vlc", "VLC", None),
            distro("zsh", "Zsh", Some("the-login-shell")),
            distro("firefox", "Firefox", None),
            distro("htop", "htop", None),
            distro("btop", "btop", None),
        ],
        unreadable: Vec::new(),
        reported: true,
        android_pending: Vec::new(),
        leaving: Vec::new(),
    };
    s
}

/// DECLINED-LIST-IS-DRAWN: "Declined by you" names every declined program
/// with what the decline did here — removed, still present (in the danger
/// tone), kept with the distro's reason, unknown, elsewhere with its
/// computers, waiting — in every language; the rest of the distro's
/// programs follow, each one press from being kept uninstalled; and a
/// declined one brings itself back.
#[test]
fn declined_list_is_drawn() {
    let r = readings(Some(declared_machine()));
    for t in every_language() {
        let v = open(Opening::Declined, &r);
        let rows = rows_of(&v, &r, &t);
        let said = drawn(&rows);
        use ronoctua_software::declaration::DeclineState;
        for (name, state) in [
            ("GIMP", DeclineState::Honoured),
            ("VLC", DeclineState::StillPresent),
            ("old-thing", DeclineState::Unknown),
            ("btop", DeclineState::Waiting),
        ] {
            let row = rows
                .iter()
                .find(|x| x.fields.iter().any(|f| f.text == name))
                .unwrap_or_else(|| panic!("{name} is not drawn in {}:\n{said}", t.language()));
            let line = drawn(std::slice::from_ref(row));
            let sentence = t.t(words::decline_key(&state));
            assert!(
                line.contains(&sentence[..sentence.len().min(12)]),
                "{name}: {line}"
            );
        }
        assert!(
            said.contains(&t.t(ronoctua_i18n::key::SOFTWARE_ESSENTIAL_WHY_THE_LOGIN_SHELL)[..16]),
            "the distro's reason is not said in {}:\n{said}",
            t.language()
        );
        assert!(said.contains("laptop"), "where it is declined:\n{said}");
        let vlc = rows
            .iter()
            .find(|x| x.spot == Spot::Declined("vlc".into()))
            .expect("vlc's row");
        assert!(
            vlc.fields
                .iter()
                .any(|f| f.color == tokens::color::STATE_ERROR),
            "still present is not in the danger tone"
        );
        // THE REST OF THE DISTRO'S: firefox, which nothing declined.
        let firefox = rows
            .iter()
            .find(|x| x.spot == Spot::Distro("firefox".into()))
            .unwrap_or_else(|| panic!("the distro's other programs are not listed:\n{said}"));
        assert!(matches!(
            firefox.can.remove,
            Some(Deed::KeepUninstalled {
                everywhere: false,
                ..
            })
        ));
        assert!(
            !rows.iter().any(|x| x.spot == Spot::Distro("gimp".into())),
            "listed twice"
        );
    }
    // A DECLINED ONE, PRESSED, IS ITS PAGE; BRINGING IT BACK IS ONE PRESS.
    let mut v = open(Opening::Declined, &r);
    walk_to(&mut v, &r, &Spot::Declined("gimp".into()));
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.level, Level::Distro("gimp".into()));
    walk_to(&mut v, &r, &Spot::Deed("bring-back"));
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Undecline { id: "gimp".into() }]
    );
}

/// FORGET-EXPLAINS-ITSELF: removing a declared program for good is asked on
/// "no", and the question says what the yes does beyond this computer — it
/// leaves the declaration, the other computers remove it at their next
/// update, what it saved stays unless erased — in every language; the
/// second yes erases, the first keeps.
#[test]
fn forget_explains_itself() {
    let r = readings(Some(declared_machine()));
    let rg = AppKey::Nix {
        attr: "ripgrep".into(),
    };
    for t in every_language() {
        let mut v = open(Opening::Home, &r);
        v.level = Level::Sheet(rg.clone());
        walk_to(&mut v, &r, &Spot::Deed("remove"));
        let rows = rows_of(&v, &r, &t);
        assert!(
            matches!(
                &focused(&rows).target,
                Target::Deed(Deed::ForgetForever { .. })
            ),
            "a declared program is removed only for good"
        );
        press(&mut v, Press::Enter, &r);
        let rows = rows_of(&v, &r, &t);
        let said = drawn(&rows);
        for k in [
            ronoctua_i18n::key::SOFTWARE_DECLARED_FORGET_HERE,
            ronoctua_i18n::key::SOFTWARE_DECLARED_FORGET_ELSEWHERE,
            ronoctua_i18n::key::SOFTWARE_DECLARED_FORGET_DATA,
        ] {
            let sentence = t.t(k);
            let first: String = sentence.chars().take(20).collect();
            assert!(
                said.contains(&first),
                "{k} is not said in {}:\n{said}",
                t.language()
            );
        }
        let answers: Vec<&Row> = rows.iter().filter(|x| !x.heading).collect();
        assert_eq!(answers[0].target, Target::Answer(Answer::No));
        assert!(answers[0].focused, "the cursor was not born on no");
        assert_eq!(answers.len(), 3, "no, yes, and yes with the data");
    }
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(rg.clone());
    walk_to(&mut v, &r, &Spot::Deed("remove"));
    press(&mut v, Press::Enter, &r);
    assert_eq!(press(&mut v, Press::Enter, &r), Errand::Nothing, "no");
    walk_to(&mut v, &r, &Spot::Deed("remove"));
    press(&mut v, Press::Enter, &r);
    v.at = Spot::Answer(Answer::YesAndMore);
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Forget {
            key: rg.clone(),
            keep_data: false
        }]
    );
    // THE LAUNCHER'S `C-d` on a declared program opens the same question.
    let (v, _) = View::fresh(
        Opening::Remove {
            key: rg,
            name: "ripgrep".into(),
        },
        &r,
        &en(),
    );
    assert!(matches!(v.asking, Some(Question::Forget { .. })));
}

/// Every declared entry says how this computer stands to it; a left-out one
/// is in the danger tone and its page offers the fix its reason calls for —
/// a newer download for a held pin, another program for a name nixpkgs
/// dropped, this computer too for one declared elsewhere.
#[test]
fn the_declared_say_how_they_stand_and_offer_their_fix() {
    let r = readings(Some(declared_machine()));
    for t in every_language() {
        let v = open(Opening::Declared, &r);
        let rows = rows_of(&v, &r, &t);
        let said = drawn(&rows);
        for id in [
            "nix:ripgrep",
            "nix:gone",
            "appimage:obsidian",
            "nix:rarely",
            "nix:fresh",
        ] {
            assert!(
                rows.iter().any(|x| x.spot == Spot::Declared(id.into())),
                "{id} is not listed in {}:\n{said}",
                t.language()
            );
        }
        let gone = rows
            .iter()
            .find(|x| x.spot == Spot::Declared("nix:gone".into()))
            .unwrap();
        assert!(
            gone.fields
                .iter()
                .any(|f| f.color == tokens::color::STATE_ERROR)
        );
        assert!(rows.iter().any(|x| x.spot == Spot::Deed("refresh-pins")));
    }
    // A NAME NIXPKGS DROPPED: another program is searched for.
    let mut v = open(Opening::Declared, &r);
    walk_to(&mut v, &r, &Spot::Declared("nix:gone".into()));
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.level, Level::Declaration("nix:gone".into()));
    walk_to(&mut v, &r, &Spot::Deed("find-replacement"));
    let e = press(&mut v, Press::Enter, &r);
    assert!(matches!(e, Errand::Feed(Want::Search { ref query, .. }) if query == "gone"));
    // A HELD PIN: a newer download is looked for.
    let mut v = open(Opening::Declared, &r);
    v.level = Level::Declaration("appimage:obsidian".into());
    let said = drawn(&rows_of(&v, &r, &en()));
    assert!(
        said.contains(&en().t(ronoctua_i18n::key::SOFTWARE_DECLARED_HELD_UPSTREAM_CHANGED)),
        "{said}"
    );
    walk_to(&mut v, &r, &Spot::Deed("refresh-pins"));
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::RefreshPins]
    );
    // DECLARED ELSEWHERE: this computer too.
    let mut v = open(Opening::Declared, &r);
    v.level = Level::Declaration("nix:rarely".into());
    walk_to(&mut v, &r, &Spot::Deed("declare-for-me"));
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Declare {
            key: AppKey::Nix {
                attr: "rarely".into()
            },
            consent: Consent::default(),
            machines: Some(vec!["laptop".into(), "noctua".into()]),
        }]
    );
}

/// One of the distro's programs is kept uninstalled after a question on
/// "no" — here, or on every computer — and the launcher's `C-d` on it asks
/// the same; one the distro needs says why and offers nothing.
#[test]
fn a_distro_program_is_declined_here_or_everywhere_after_a_question() {
    let r = readings(Some(declared_machine()));
    let firefox = AppKey::System {
        desktop_id: "firefox.desktop".into(),
    };
    let mut v = open(Opening::Home, &r);
    v.level = Level::Sheet(firefox.clone());
    for (spot, machines) in [
        ("keep-uninstalled-here", Some(vec!["noctua".to_owned()])),
        ("keep-uninstalled-everywhere", None),
    ] {
        walk_to(&mut v, &r, &Spot::Deed(spot));
        assert_eq!(press(&mut v, Press::Enter, &r), Errand::Nothing);
        let rows = rows_of(&v, &r, &en());
        assert!(focused(&rows).target == Target::Answer(Answer::No));
        assert!(
            drawn(&rows).contains(&en().t(ronoctua_i18n::key::SOFTWARE_DECLARED_DECLINE_REBUILD))
        );
        v.at = Spot::Answer(Answer::Yes);
        assert_eq!(
            queued(&press(&mut v, Press::Enter, &r)),
            vec![Order::Decline {
                id: "firefox".into(),
                machines
            }]
        );
    }
    let (v, e) = View::fresh(
        Opening::Remove {
            key: firefox,
            name: "Firefox".into(),
        },
        &r,
        &en(),
    );
    assert_eq!(e, Errand::Nothing);
    assert!(matches!(
        v.asking,
        Some(Question::Decline {
            everywhere: false,
            ..
        })
    ));
    let mut v = open(Opening::Home, &r);
    v.level = Level::Distro("zsh".into());
    let rows = rows_of(&v, &r, &en());
    assert!(
        drawn(&rows).contains(&en().t(ronoctua_i18n::key::SOFTWARE_ESSENTIAL_WHY_THE_LOGIN_SHELL))
    );
    assert!(
        !rows
            .iter()
            .any(|x| matches!(x.target, Target::Deed(Deed::KeepUninstalled { .. })))
    );
}

/// A decline is always reachable from its page, whatever came of it.
///
/// FAILS IF: a decline of other computers only does not also offer keeping
/// the program uninstalled HERE (the launcher's removal asks exactly that);
/// a decline the system refused because the distro needs the program, or
/// one of a program the distro no longer carries, cannot be brought back
/// from either door; or a program no longer declined here, which the
/// system still leaves out, does not say it comes back with the next update.
#[test]
fn every_decline_can_be_brought_back_and_kept_here_too() {
    let r = readings(Some(declared_machine()));
    let deeds = |level: Level, r: &Readings| -> Vec<Deed> {
        let mut v = open(Opening::Home, r);
        v.level = level;
        rows_of(&v, r, &en())
            .into_iter()
            .filter_map(|x| match x.target {
                Target::Deed(d) => Some(d),
                _ => None,
            })
            .collect()
    };
    // DECLINED ON LAPTOP ONLY: here too, or everywhere, or back.
    let htop = deeds(Level::Distro("htop".into()), &r);
    assert!(
        htop.iter()
            .any(|d| matches!(d, Deed::BringBack { id, .. } if id == "htop"))
    );
    assert!(htop.iter().any(|d| matches!(
        d,
        Deed::KeepUninstalled { id, everywhere: false, .. } if id == "htop"
    )));
    assert!(htop.iter().any(|d| matches!(
        d,
        Deed::KeepUninstalled { id, everywhere: true, .. } if id == "htop"
    )));
    let mut v = open(Opening::Home, &r);
    v.level = Level::Distro("htop".into());
    walk_to(&mut v, &r, &Spot::Deed("keep-uninstalled-here"));
    press(&mut v, Press::Enter, &r);
    v.at = Spot::Answer(Answer::Yes);
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Decline {
            id: "htop".into(),
            machines: Some(vec!["noctua".into()])
        }]
    );
    // DECLINED EVERYWHERE ALREADY: nothing more to keep uninstalled.
    let gimp = deeds(Level::Distro("gimp".into()), &r);
    assert!(
        !gimp
            .iter()
            .any(|d| matches!(d, Deed::KeepUninstalled { .. }))
    );
    // REFUSED, the distro needing it: said, and still brought back.
    for t in every_language() {
        let mut v = open(Opening::Home, &r);
        v.level = Level::Distro("zsh".into());
        let said = drawn(&rows_of(&v, &r, &t));
        assert!(
            said.contains(&t.t(ronoctua_i18n::key::SOFTWARE_DECLARED_ROW_DECLINED)),
            "{said}"
        );
        assert!(
            said.contains(&t.t(ronoctua_i18n::key::SOFTWARE_DECLARED_ROW_BRING_BACK)),
            "{said}"
        );
    }
    let mut v = open(Opening::Declined, &r);
    walk_to(&mut v, &r, &Spot::Declined("zsh".into()));
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.level, Level::Distro("zsh".into()));
    walk_to(&mut v, &r, &Spot::Deed("bring-back"));
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Undecline { id: "zsh".into() }]
    );
    // A PROGRAM THE DISTRO NO LONGER CARRIES: its page, and back.
    let mut v = open(Opening::Declined, &r);
    walk_to(&mut v, &r, &Spot::Declined("old-thing".into()));
    press(&mut v, Press::Enter, &r);
    assert_eq!(v.level, Level::Distro("old-thing".into()));
    walk_to(&mut v, &r, &Spot::Deed("bring-back"));
    assert_eq!(
        queued(&press(&mut v, Press::Enter, &r)),
        vec![Order::Undecline {
            id: "old-thing".into()
        }]
    );
    // NO LONGER DECLINED, STILL LEFT OUT: it comes back with the update.
    let mut snap = declared_machine();
    let firefox = snap
        .declarations
        .distro
        .iter_mut()
        .find(|d| d.app.id == "firefox")
        .unwrap();
    firefox.app.on_this_machine = false;
    firefox.app.declined_here = true;
    let r = readings(Some(snap));
    for t in every_language() {
        let mut v = open(Opening::Home, &r);
        v.level = Level::Distro("firefox".into());
        let said = drawn(&rows_of(&v, &r, &t));
        let returning = t.t(ronoctua_i18n::key::SOFTWARE_DECLARED_RETURNING);
        let first: String = returning.chars().take(20).collect();
        assert!(said.contains(&first), "{said}");
    }
}

/// FAILS IF: what left the declaration and is still here (the update held
/// the machine when it was removed, or another computer removed it) does
/// not say so on its row, or cannot be asked again to go.
#[test]
fn what_leaves_at_the_next_update_says_so() {
    let mut snap = declared_machine();
    snap.installed[0].trouble = Some(Refusal::LeavesAtNextApply);
    let key = snap.installed[0].key.clone();
    let r = readings(Some(snap));
    for t in every_language() {
        let v = open(Opening::Home, &r);
        let rows = rows_of(&v, &r, &t);
        let row = rows
            .iter()
            .find(|x| x.spot == Spot::Program(key.clone()))
            .expect("still listed");
        let sentence = t.t(words::refusal_key(Refusal::LeavesAtNextApply));
        let first: String = sentence.chars().take(16).collect();
        assert!(
            drawn(std::slice::from_ref(row)).contains(&first),
            "{}",
            drawn(std::slice::from_ref(row))
        );
        assert!(matches!(row.can.remove, Some(Deed::ForgetForever { .. })));
        // ITS PAGE does not call it declared, and says why it is here.
        let mut v = open(Opening::Home, &r);
        v.level = Level::Sheet(key.clone());
        let page = drawn(&rows_of(&v, &r, &t));
        assert!(
            !page.contains(&t.t(words::origin_key(Origin::MineDeclared))),
            "{page}"
        );
        assert!(page.contains(&first), "{page}");
    }
}

/// FAILS IF: the editor of a DECLARED web app does not say that saving
/// changes the declaration — the person's other computers take it — or the
/// editor of one of the person's own says it.
#[test]
fn a_declared_web_app_says_its_editor_declares() {
    use ronoctua_software::declaration::{DeclaredRow, Standing};
    use ronoctua_software::person::declared::{Entry, WebApp as Declared};
    let mut snap = declared_machine();
    snap.declarations.declared.push(DeclaredRow {
        entry: Entry::Web(Declared {
            id: "web:mail".into(),
            machines: None,
            edited_at: ronoctua_software::person::Stamp::parse("2026-10-05T12:00:00Z").unwrap(),
            edited_on: "noctua".into(),
            slug: "mail".into(),
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon: None,
            category: "Network".into(),
        }),
        standing: Standing::Applied,
    });
    let r = readings(Some(snap));
    for (slug, declared) in [("mail", true), ("notes", false)] {
        for t in every_language() {
            let mut v = open(Opening::Home, &r);
            v.draft = Some(WebDraft {
                slug: Some(slug.into()),
                url: "https://mail.example.org/".into(),
                adopted: true,
                ..WebDraft::default()
            });
            v.level = Level::WebEditor;
            let said = drawn(&rows_of(&v, &r, &t));
            let note = t.t(ronoctua_i18n::key::SOFTWARE_DECLARED_WEB_EDIT);
            let first: String = note.chars().take(20).collect();
            assert_eq!(said.contains(&first), declared, "{slug}: {said}");
        }
    }
}

/// The first page has the two doors of the declaration, with how many.
/// Every level of the declaration draws, in every language, with something
/// to stand on.
#[test]
fn every_level_of_the_declaration_draws_in_every_language() {
    let r = readings(Some(declared_machine()));
    let levels = [
        Level::Declared,
        Level::Declaration("nix:ripgrep".into()),
        Level::Declaration("nix:gone".into()),
        Level::Declaration("appimage:obsidian".into()),
        Level::Declaration("nix:rarely".into()),
        Level::Declaration("nix:fresh".into()),
        Level::Declined,
        Level::Distro("gimp".into()),
        Level::Distro("zsh".into()),
        Level::Distro("firefox".into()),
        Level::Sheet(AppKey::Nix {
            attr: "ripgrep".into(),
        }),
    ];
    for t in every_language() {
        for level in &levels {
            let mut v = open(Opening::Home, &r);
            v.level = level.clone();
            let rows = rows_of(&v, &r, &t);
            assert!(
                rows.iter().any(|x| !x.heading),
                "{level:?} has nothing to stand on"
            );
        }
    }
}

#[test]
fn the_first_page_has_the_declarations_doors() {
    let r = readings(Some(declared_machine()));
    let v = open(Opening::Home, &r);
    let rows = rows_of(&v, &r, &en());
    assert!(rows.iter().any(|x| x.spot == Spot::Door("declared")));
    assert!(rows.iter().any(|x| x.spot == Spot::Door("declined")));
    let said = drawn(&rows);
    assert!(said.contains("5"), "{said}");
}
