//! What the Syncthing view promises, held down with nothing running.
//!
//! Every test here compares WHAT WOULD BE DRAWN, not what a field holds. It is
//! the rule this repository paid for twice: a view whose state is right and
//! whose rows are wrong looks broken and passes everything.

use super::*;
use ronoctua_syncthing::{Device, Folder, Running, Service, State, Traffic, unit};

fn texts() -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
}

fn folder(id: &str) -> Folder {
    Folder {
        id: id.to_owned(),
        kind: "sendreceive".to_owned(),
        completion: 100,
        raw: serde_json::json!({
            "id": id,
            "label": "",
            "path": "/somewhere",
            "type": "sendreceive",
            "paused": false,
            "rescanIntervalS": 3600,
            "fsWatcherEnabled": true,
            "devices": [],
            "versioning": { "type": "" },
        }),
        ..Folder::default()
    }
}

fn device(id: &str, name: &str) -> Device {
    Device {
        id: id.to_owned(),
        name: name.to_owned(),
        raw: serde_json::json!({
            "deviceID": id,
            "name": name,
            "paused": false,
            "compression": "metadata",
            "numConnections": 3,
        }),
        ..Device::default()
    }
}

fn running() -> Snapshot {
    Snapshot {
        service: Service::Up(Box::new(Running {
            version: "v2.1.3".to_owned(),
            id: "ME".to_owned(),
            uptime: 7_200,
            address: "127.0.0.1:8384".to_owned(),
            ..Running::default()
        })),
        folders: vec![folder("papers")],
        devices: vec![
            Device {
                me: true,
                ..device("ME", "here")
            },
            device("THEM", "the other one"),
        ],
        options: serde_json::json!({ "natEnabled": false, "maxFolderConcurrency": 0 }),
        gui: serde_json::json!({ "address": "127.0.0.1:8384" }),
        completion: 100,
        traffic: Traffic {
            sent: 1_000,
            received: 2_000,
        },
        unit: unit::Standing::Running,
        ..Snapshot::default()
    }
}

/// The label of every row, which is what a person actually reads.
fn labels(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|r| !r.fields.is_empty())
        .map(|r| r.fields[0].text.clone())
        .collect()
}

/// Everything a row draws, labels and values both.
fn drawn(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| {
            r.fields
                .iter()
                .map(|f| f.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

#[test]
fn with_no_service_the_screen_says_so_and_does_not_go_blank() {
    // A BLANK PANEL AND A REFUSED ONE LOOK ALIKE, and only one of them has a
    // sentence. This is the first thing anybody sees on a machine where the
    // unit is stopped.
    let v = View::fresh(Preferences::default());
    let rows = v.rows(None, 80, &texts());
    assert!(!rows.is_empty());
    let said = labels(&rows).join(" | ");
    assert!(said.contains("Syncthing"), "{said}");
    assert!(!said.contains('['), "a key nobody wrote: {said}");
}

#[test]
fn the_list_draws_the_folders_the_machines_and_the_service() {
    let v = View::fresh(Preferences::default());
    let rows = v.rows(Some(&running()), 80, &texts());
    let said = labels(&rows).join(" | ");
    assert!(said.contains("papers"), "{said}");
    assert!(said.contains("the other one"), "{said}");
    // AND NOT THIS MACHINE, which is in Syncthing's device list and is not one
    // of the others: a list of one that says "here" is a list that answers the
    // wrong question.
    assert!(!said.contains("here"), "{said}");
    assert!(
        !said.contains('['),
        "some key is missing from the base language: {said}"
    );
}

#[test]
fn the_invitations_section_exists_only_when_it_has_something_in_it() {
    // An empty "asking to be let in" heading on every screen, for ever, is a
    // heading that teaches people to stop reading headings.
    let v = View::fresh(Preferences::default());
    let quiet = labels(&v.rows(Some(&running()), 80, &texts())).join(" | ");
    let asking = Snapshot {
        invitations: vec![Invitation::Device {
            id: "NEWONE".to_owned(),
            name: "a phone".to_owned(),
            address: "192.0.2.5:22000".to_owned(),
        }],
        ..running()
    };
    let loud = labels(&v.rows(Some(&asking), 80, &texts())).join(" | ");
    assert!(!quiet.contains("asking to be let in"), "{quiet}");
    assert!(loud.contains("asking to be let in"), "{loud}");
    assert!(loud.contains("a phone"), "{loud}");
}

#[test]
fn a_row_the_flake_owns_is_drawn_reachable_and_refuses() {
    // INVARIANT 6 ON THE SCREEN. Not hidden — hidden would leave a person
    // wondering where a setting went — and not editable, because the next
    // `nixos-rebuild` would undo it with nothing saying so.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.level = Level::Global;
    v.cursor = 1;
    let rows = v.rows(Some(&snap), 80, &texts());
    let at = rows
        .iter()
        .position(|r| r.spot == Spot::Setting("nat-traversal"))
        .expect("the flake's own row is drawn");
    assert!(rows[at].disabled, "it is offered as if it could be changed");
    v.cursor = at;
    v.moved = true;
    assert_eq!(
        v.key(Press::Enter, Some(&snap), &texts()),
        Errand::Speak(ronoctua_i18n::key::ECHO_SYNCTHING_FLAKE_OWNS),
        "it refused in silence, which is the same as a key that is broken"
    );
    assert_eq!(
        v.key(Press::More, Some(&snap), &texts()),
        Errand::Speak(ronoctua_i18n::key::ECHO_SYNCTHING_FLAKE_OWNS)
    );
}

#[test]
fn the_drawer_hides_eighty_odd_rows_until_it_is_opened() {
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.level = Level::Global;
    let shut = v.rows(Some(&snap), 80, &texts()).len();
    let drawer = v
        .rows(Some(&snap), 80, &texts())
        .iter()
        .position(|r| r.spot == Spot::Drawer("global"))
        .expect("there is a drawer");
    v.cursor = drawer;
    v.moved = true;
    assert_eq!(v.key(Press::Enter, Some(&snap), &texts()), Errand::Nothing);
    let open = v.rows(Some(&snap), 80, &texts()).len();
    assert!(
        open > shut + 20,
        "the drawer opened and almost nothing came out: {shut} -> {open}"
    );
    // AND IT SHUTS AGAIN, on the same key, with the cursor still on it.
    v.key(Press::Enter, Some(&snap), &texts());
    assert_eq!(v.rows(Some(&snap), 80, &texts()).len(), shut);
}

#[test]
fn only_the_drawer_draws_a_chevron() {
    // THE OWNER'S OWN ARGUMENT: they refused a chevron on a row that acts,
    // saying it is what collapse and expand use. By that argument it belongs
    // to the drawer, and to nothing else in this view.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.level = Level::Global;
    let rows = v.rows(Some(&snap), 80, &texts());
    for row in &rows {
        let drawn: String = row.fields.iter().map(|f| f.text.clone()).collect();
        let has = drawn.contains(ronoctua_tokens::icon::EXPAND)
            || drawn.contains(ronoctua_tokens::icon::COLLAPSE);
        assert_eq!(
            has,
            matches!(row.spot, Spot::Drawer(_)),
            "a chevron on a row that is not a drawer: {drawn:?}"
        );
    }
}

#[test]
fn escape_goes_up_one_level_and_only_then_out() {
    // The Bluetooth rule, and it is measured on what is DRAWN: a level field
    // that is right while the rows are somebody else's is exactly the failure
    // this repository keeps paying for.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    let rows = v.rows(Some(&snap), 80, &texts());
    let at = rows
        .iter()
        .position(|r| r.spot == Spot::Folder("papers".to_owned()))
        .expect("the folder is in the list");
    v.cursor = at;
    v.moved = true;
    assert_eq!(v.key(Press::Enter, Some(&snap), &texts()), Errand::Nothing);
    let sheet = labels(&v.rows(Some(&snap), 80, &texts())).join(" | ");
    assert!(
        sheet.contains("folder id"),
        "not the folder's sheet: {sheet}"
    );

    assert_eq!(v.key(Press::Escape, Some(&snap), &texts()), Errand::Nothing);
    let back = labels(&v.rows(Some(&snap), 80, &texts())).join(" | ");
    assert!(back.contains("the service"), "not back at the list: {back}");

    assert_eq!(v.key(Press::Escape, Some(&snap), &texts()), Errand::Close);
}

#[test]
fn a_sheet_about_something_that_went_away_goes_back_to_the_list() {
    // A folder removed from another machine leaves a sheet of empty rows with
    // nothing saying why.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.level = Level::Folder("papers".to_owned());
    let gone = Snapshot {
        folders: Vec::new(),
        ..snap
    };
    v.settle_in(Some(&gone), &texts());
    let back = labels(&v.rows(Some(&gone), 80, &texts())).join(" | ");
    assert!(back.contains("the service"), "still on the sheet: {back}");
}

#[test]
fn the_mark_turns_through_three_and_writes_the_file() {
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    let rows = v.rows(Some(&snap), 80, &texts());
    let at = rows
        .iter()
        .position(|r| r.spot == Spot::Mark)
        .expect("the one preference this view keeps");
    v.cursor = at;
    v.moved = true;
    assert_eq!(v.preferences().mark, Mark::WhenOff);
    assert_eq!(
        v.key(Press::More, Some(&snap), &texts()),
        Errand::Keep(Preferences { mark: Mark::WhenOn })
    );
    assert_eq!(
        v.key(Press::More, Some(&snap), &texts()),
        Errand::Keep(Preferences { mark: Mark::Never })
    );
    // AND IT COMES BACK ROUND, because every value of a ring is as good as the
    // next — unlike a range, which stops at its ends.
    assert_eq!(
        v.key(Press::More, Some(&snap), &texts()),
        Errand::Keep(Preferences {
            mark: Mark::WhenOff
        })
    );
}

#[test]
fn a_typed_setting_asks_the_footer_rather_than_turning() {
    // Text and lists are typed, and the one place anything is typed in this
    // system is the footer's minibuffer. A shifted arrow on one of them would
    // otherwise be a key that plainly does nothing on a row that plainly has a
    // value.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.level = Level::Folder("papers".to_owned());
    let rows = v.rows(Some(&snap), 80, &texts());
    let at = rows
        .iter()
        .position(|r| r.spot == Spot::Setting("folder-path"))
        .expect("a folder's path is a setting like any other");
    v.cursor = at;
    v.moved = true;
    assert_eq!(
        v.key(Press::More, Some(&snap), &texts()),
        Errand::Ask(Ask::Setting("folder-path", Some("papers".to_owned())))
    );
    // AND EVERY QUESTION'S COMMAND IS ONE A KEY REACHES. The keymap's own
    // test refuses a command with no path at all, and an earlier draft
    // invented three that nothing else could run.
    assert_eq!(Ask::NewDevice.command(), "syncthing-add-device");
    assert_eq!(Ask::NewFolder.command(), "syncthing-add-folder");
    assert_eq!(Ask::Invited(0).command(), "syncthing-add-folder");
    assert_eq!(
        Ask::Setting("folder-path", None).command(),
        "syncthing-enter"
    );
}

#[test]
fn what_was_typed_becomes_an_order_for_the_question_that_was_asked() {
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.asked(Ask::Setting("folder-path", Some("papers".to_owned())));
    let order = v
        .typed("/home/somebody/papers", Some(&snap))
        .expect("an order");
    match order {
        Order::Set { what, at, body, .. } => {
            assert_eq!(what, "folder-path");
            assert_eq!(at.as_deref(), Some("papers"));
            assert_eq!(body, serde_json::json!({ "path": "/home/somebody/papers" }));
        }
        other => panic!("{other:?}"),
    }
    // NOTHING TYPED IS `Esc` IN THE MINIBUFFER, which is how a person says
    // never mind — not an order to write an empty path.
    v.asked(Ask::NewFolder);
    assert!(v.typed("   ", Some(&snap)).is_none());
}

#[test]
fn a_fact_copies_itself_and_a_machine_id_is_the_reason() {
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    v.level = Level::Device("THEM".to_owned());
    let rows = v.rows(Some(&snap), 80, &texts());
    let at = rows
        .iter()
        .position(|r| r.spot == Spot::Service("device-id"))
        .expect("the sheet says which machine it is about");
    v.cursor = at;
    v.moved = true;
    match v.key(Press::Enter, Some(&snap), &texts()) {
        Errand::Copy(text) => assert!(text.contains("THEM"), "{text}"),
        other => panic!("a 63-character id nobody can retype: {other:?}"),
    }
}

#[test]
fn every_key_the_hint_teaches_is_a_key_the_view_answers() {
    // A hint that teaches a key nothing answers is worse than no hint: the
    // person presses it and something else happens, or nothing does.
    for (notation, id, _) in OF_THE_VIEW {
        assert!(
            pressed(notation).is_some(),
            "`{notation}` is taught and reaches no key"
        );
        assert!(
            press_of(id).is_some(),
            "`{id}` is in the table and has no press"
        );
    }
}

#[test]
fn the_hint_teaches_only_what_works_where_the_cursor_is() {
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    let rows = v.rows(Some(&snap), 80, &texts());
    let folder_at = rows
        .iter()
        .position(|r| r.spot == Spot::Folder("papers".to_owned()))
        .unwrap();
    v.cursor = folder_at;
    v.moved = true;
    let on_a_folder: Vec<String> = v
        .hint(Some(&snap), &texts())
        .iter()
        .map(|p| p.text.clone())
        .collect();
    assert!(
        on_a_folder.iter().any(|p| p.contains('r')),
        "rescan is not taught on a folder: {on_a_folder:?}"
    );

    // ON A SETTING, `r` MEANS NOTHING and is not taught.
    v.level = Level::Global;
    v.cursor = 1;
    v.settle_in(Some(&snap), &texts());
    let on_a_setting = v.hint(Some(&snap), &texts()).len();
    assert!(
        on_a_setting < on_a_folder.len(),
        "the same keys are taught everywhere: {on_a_setting} vs {}",
        on_a_folder.len()
    );
}

#[test]
fn the_footer_reaches_every_key_the_table_holds() {
    // THE FOOTER BUILDS ITS NOTATION WITH SHIFT, unlike the sound view's. A
    // notation built by `Key::simple` drops the modifier, and `S-<right>`
    // would arrive as a plain one — which walks the list instead of turning
    // the ring, in silence. This is the guard against that, and against a
    // modified key entering the table with no door able to make it.
    for (notation, _, _) in OF_THE_VIEW {
        let made = match *notation {
            "RET" => ronoctua_keymap::Key {
                mods: ronoctua_keymap::Mods::default(),
                sym: ronoctua_keymap::Sym::Enter,
            },
            "<up>" => ronoctua_keymap::Key {
                mods: ronoctua_keymap::Mods::default(),
                sym: ronoctua_keymap::Sym::Up,
            },
            "<down>" => ronoctua_keymap::Key {
                mods: ronoctua_keymap::Mods::default(),
                sym: ronoctua_keymap::Sym::Down,
            },
            "S-<left>" => ronoctua_keymap::Key {
                mods: ronoctua_keymap::Mods {
                    shift: true,
                    ..ronoctua_keymap::Mods::default()
                },
                sym: ronoctua_keymap::Sym::Left,
            },
            "S-<right>" => ronoctua_keymap::Key {
                mods: ronoctua_keymap::Mods {
                    shift: true,
                    ..ronoctua_keymap::Mods::default()
                },
                sym: ronoctua_keymap::Sym::Right,
            },
            one => {
                let mut letters = one.chars();
                let only = letters.next().expect("a notation is not empty");
                assert!(
                    letters.next().is_none(),
                    "`{one}` is neither a named key this test makes nor one letter"
                );
                ronoctua_keymap::Key {
                    mods: ronoctua_keymap::Mods::default(),
                    sym: ronoctua_keymap::Sym::Char(only),
                }
            }
        };
        assert_eq!(
            made.notation(),
            *notation,
            "the footer cannot spell `{notation}`"
        );
        assert!(
            pressed(&made.notation()).is_some(),
            "`{notation}` is spelt and reaches nothing"
        );
    }
}

#[test]
fn the_rows_a_key_decides_at_are_the_rows_that_get_drawn() {
    // A key decides at one width because the two doors measure differently.
    // That is only safe while the row COUNT and the row TARGETS do not depend
    // on the width — only the cutting does.
    let v = View::fresh(Preferences::default());
    let snap = running();
    let narrow = v.rows(Some(&snap), 40, &texts());
    let wide = v.rows(Some(&snap), 200, &texts());
    assert_eq!(narrow.len(), wide.len(), "the row count follows the width");
    for (a, b) in narrow.iter().zip(&wide) {
        assert_eq!(a.spot, b.spot);
        assert_eq!(a.target, b.target);
        assert_eq!(a.heading, b.heading);
        assert_eq!(a.disabled, b.disabled);
    }
}

#[test]
fn no_row_runs_out_of_its_container() {
    // The owner has had to point at a glyph against an edge before. Every
    // field is placed and cut by `chrome::field_pair`; this is what says the
    // cutting held.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    for level in [
        Level::List,
        Level::Folder("papers".to_owned()),
        Level::Device("THEM".to_owned()),
        Level::Global,
        Level::Interface,
    ] {
        v.level = level.clone();
        v.open = vec!["folder", "device", "global", "interface"];
        for columns in [40_u16, 80, 200] {
            let (left, room) = extent(columns);
            for row in v.rows(Some(&snap), columns, &texts()) {
                for field in &row.fields {
                    let ends = field.at + ronoctua_render::layout::width(&field.text);
                    assert!(
                        ends <= left + room,
                        "{level:?} at {columns}: `{}` ends at {ends}, past {}",
                        field.text,
                        left + room
                    );
                }
            }
        }
    }
}

#[test]
fn pausing_a_folder_and_pausing_a_machine_are_different_orders() {
    // One is configuration and the other is runtime. Getting them the wrong
    // way round makes a paused folder come back by itself after a reboot.
    let mut v = View::fresh(Preferences::default());
    let snap = running();
    let rows = v.rows(Some(&snap), 80, &texts());
    v.cursor = rows
        .iter()
        .position(|r| r.spot == Spot::Folder("papers".to_owned()))
        .unwrap();
    v.moved = true;
    assert_eq!(
        v.key(Press::Pause, Some(&snap), &texts()),
        Errand::Order(Box::new(Order::PauseFolder("papers".to_owned())))
    );

    let rows = v.rows(Some(&snap), 80, &texts());
    v.cursor = rows
        .iter()
        .position(|r| r.spot == Spot::Device("THEM".to_owned()))
        .unwrap();
    assert_eq!(
        v.key(Press::Pause, Some(&snap), &texts()),
        Errand::Order(Box::new(Order::Pause(Some("THEM".to_owned()))))
    );
}

#[test]
fn the_service_offers_stopping_only_when_systemd_says_it_is_running() {
    // `systemctl` that cannot be asked is not the same as a unit that is off,
    // and a button that guesses which would be a button that lies half the
    // time.
    let up = running();
    let v = View::fresh(Preferences::default());
    let said = labels(&v.rows(Some(&up), 80, &texts())).join(" | ");
    assert!(said.contains("stop the service"), "{said}");

    let unaskable = Snapshot {
        unit: unit::Standing::Unknown,
        ..running()
    };
    let said = drawn(&v.rows(Some(&unaskable), 80, &texts())).join(" | ");
    assert!(!said.contains("stop the service"), "{said}");
    assert!(!said.contains("start the service"), "{said}");
    // AND IT SAYS SO IN THE VALUE, which is where a row of this shape puts
    // what it knows: `systemctl` that cannot be asked is not a unit that is
    // off, and the screen tells the two apart.
    assert!(said.contains("cannot tell"), "{said}");
    // AND `restart it` IS DRAWN ONCE. It was reached by two paths and put on
    // the screen twice, which is a row that says the same word about itself.
    assert_eq!(
        said.matches("restart it").count(),
        1,
        "the same row twice: {said}"
    );
}

#[test]
fn a_folder_still_moving_draws_a_bar_and_a_finished_one_draws_a_word() {
    let v = View::fresh(Preferences::default());
    let half = Snapshot {
        folders: vec![Folder {
            completion: 50,
            need_bytes: 1_000,
            state: State::Syncing,
            ..folder("papers")
        }],
        ..running()
    };
    let rows = v.rows(Some(&half), 80, &texts());
    let row = rows
        .iter()
        .find(|r| r.spot == Spot::Folder("papers".to_owned()))
        .unwrap();
    let drawn: String = row.fields.iter().map(|f| f.text.clone()).collect();
    assert!(drawn.contains("50%"), "{drawn}");

    let done = v.rows(Some(&running()), 80, &texts());
    let row = done
        .iter()
        .find(|r| r.spot == Spot::Folder("papers".to_owned()))
        .unwrap();
    let drawn: String = row.fields.iter().map(|f| f.text.clone()).collect();
    assert!(
        !drawn.contains('%'),
        "a finished folder still shows a bar: {drawn}"
    );
}

#[test]
fn the_uptime_says_one_unit_and_the_biggest_that_fits() {
    let t = texts();
    assert!(for_how_long(30, &t).contains("30"));
    assert!(for_how_long(600, &t).contains("10"));
    assert!(for_how_long(7_200, &t).contains('2'));
    assert!(for_how_long(172_800, &t).contains('2'));
    // AND NEVER A MISSING KEY.
    for seconds in [0_u64, 59, 60, 3_599, 3_600, 86_399, 86_400] {
        let said = for_how_long(seconds, &t);
        assert!(!said.starts_with('['), "{seconds}: {said}");
    }
}

#[test]
fn every_label_this_view_draws_exists_in_the_base_language() {
    // `Texts::t` answers `[key]` for a key nobody wrote, which is what a
    // missing translation looks like on screen. A hundred and twenty-one of
    // these come out of a generated table, so this is the only place the two
    // sides meet.
    let mut v = View::fresh(Preferences::default());
    let snap = Snapshot {
        invitations: vec![
            Invitation::Device {
                id: "NEWONE".to_owned(),
                name: String::new(),
                address: String::new(),
            },
            Invitation::Folder {
                folder: "shared".to_owned(),
                label: String::new(),
                from: "THEM".to_owned(),
                who: String::new(),
            },
        ],
        ..running()
    };
    for level in [
        Level::List,
        Level::Folder("papers".to_owned()),
        Level::Device("THEM".to_owned()),
        Level::Global,
        Level::Interface,
    ] {
        v.level = level.clone();
        v.open = vec!["folder", "device", "global", "interface"];
        for row in v.rows(Some(&snap), 200, &texts()) {
            for field in &row.fields {
                assert!(
                    !field.text.starts_with('['),
                    "{level:?}: `{}` is a key nobody wrote",
                    field.text
                );
            }
        }
    }
}
