//! The listing a door draws is the walk a sync makes (`inventory`, `scope`):
//! a home with one file of every origin and every reason a file is never
//! kept, walked by the sync's walk and the plain part's, and listed by a
//! door's inventory.

use std::collections::BTreeMap;

use super::*;
use crate::inventory::{self, Here, Item, Origin, Part, State};
use crate::requests::{Request, Requests};
use crate::scope::Scope;

/// FAILS IF: a path the sync's walk keeps, sets aside as never kept, or
/// cannot read — or a file the plain part carries or leaves out for its size
/// — is missing from the listing or listed as something else; or the
/// listing says a file travels that no walk sends; or an origin (a rule, a
/// target, the person's addition here, one followed from another machine,
/// the plain part) or a reason a file is never kept has no row.
#[test]
fn belongings_list_is_the_walk() {
    let root = scratch("listing");
    let p = Places::under(&root);
    let put = |logical: &str, bytes: &[u8]| {
        let at = p.resolve(logical).unwrap();
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    };
    // ONE OF EVERY ORIGIN.
    put("config/theme.json", b"{\"theme\":\"owl\"}");
    put("config/studio/scenes.json", b"{}");
    put("state/software/history.jsonl", b"{}\n");
    put("home/.ssh/config", b"Host *\n");
    put("home/.gitconfig", b"[user]\n");
    put("home/notes/a.md", b"# a");
    put("home/music/b.ogg", b"ogg");
    put("state/person/options.json", b"{\"format\":1}");
    put("config/brain-modules/my-tea/module.rs", b"// tea");
    // EVERY REASON A FILE IS NEVER KEPT, and the two ways one is not here.
    put("config/column-player.pid", b"1");
    put("home/.ssh/agent.sock", b"");
    put("home/.gnupg/pubring.kbx", b"keybox");
    put(
        "config/belongings-keys/deploy",
        // In pieces, or the repository's secret scan takes it for a key.
        concat!("-----BEGIN OPENSSH ", "PRIVATE", " KEY-----").as_bytes(),
    );
    put(
        "config/belongings-keys/deploy.pub",
        b"ssh-ed25519 AAAA deploy\n",
    );
    std::fs::hard_link(p.deploy_key(), p.resolve("home/.ssh/belongings").unwrap()).unwrap();
    put("home/.ssh/copy.pub", b"ssh-ed25519 AAAA deploy\n");
    std::os::unix::fs::symlink(
        "/nix/store/abc-home-manager-files/settings",
        p.resolve("config/hm-settings").unwrap(),
    )
    .unwrap();
    put(
        "state/person/software/icons/big.png",
        &vec![1u8; usize::try_from(kept::PLAIN_MOST_BYTES).unwrap() + 1],
    );
    put("config/taken-out.json", b"{}");
    // WHAT CANNOT BE READ: a file, and a folder.
    put("config/locked.json", b"{}");
    put("config/hidden/inside.json", b"{}");
    let locked = p.resolve("config/locked.json").unwrap();
    let hidden = p.resolve("config/hidden").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o000)).unwrap();

    let prefs = Belongings {
        repository: "git@example.org:me/belongings.git".into(),
        extra: vec!["home/notes".into()],
        excluded: vec!["config/taken-out.json".into()],
        ..Belongings::default()
    };
    let last = Last {
        repository: prefs.repository.clone(),
        roots: ["home/music".to_owned()].into_iter().collect(),
        ..Last::default()
    };
    let mut asked = Requests::default();
    assert!(asked.ask(Request::DropEverywhere {
        target: "gnupg".into()
    }));

    let door = Scope::of(&prefs, Some(&last), &asked);
    let items = inventory::inventory(&door, &p);
    let listed: BTreeMap<&str, &Item> = items.iter().map(|i| (i.path.as_str(), i)).collect();
    let row = |path: &str| -> &Item {
        listed
            .get(path)
            .copied()
            .unwrap_or_else(|| panic!("{path} is walked and not listed: {items:#?}"))
    };
    // THE SYNC'S WALK, with what the door follows: the roots a sync of this
    // machine walks.
    let sync = Scope::for_sync(&prefs, door.elsewhere(), &BTreeSet::new());
    let found = walk::walk(&sync, &p);
    let plain = super::super::plain::walk_here(&p);

    // EVERY WALKED PATH IS LISTED, as what the walk made of it.
    assert!(!found.files.is_empty() && !found.never.is_empty());
    for path in found.files.keys() {
        let item = row(path);
        assert_eq!(item.part, Part::Sealed, "{path}");
        assert!(
            matches!(item.here, Here::Kept | Here::Nowhere),
            "{path} is sent and listed {:?}",
            item.here
        );
    }
    for (path, why) in &found.never {
        assert_eq!(row(path).here, Here::Never(why), "{path}");
    }
    for path in &found.unreadable {
        assert_eq!(row(path).state, State::Unreadable, "{path}");
    }
    for path in plain.files.keys() {
        let item = row(path);
        assert_eq!((item.part, item.here), (Part::Plain, Here::Kept), "{path}");
    }
    for path in &plain.too_large {
        assert_eq!(row(path).here, Here::Never(inventory::TOO_LARGE), "{path}");
    }
    // AND NOTHING IS SAID TO TRAVEL THAT NO WALK SENDS — a deletion travels
    // as one, and no walk meets it.
    for item in &items {
        if item.state == State::Deleted {
            let met = match item.part {
                Part::Sealed => found.files.contains_key(&item.path),
                Part::Plain => plain.files.contains_key(&item.path),
            };
            assert!(!met, "{} is walked and listed deleted", item.path);
            continue;
        }
        if item.here != Here::Kept || item.state == State::Unreadable {
            continue;
        }
        let sent = match item.part {
            Part::Sealed => found.files.contains_key(&item.path),
            Part::Plain => plain.files.contains_key(&item.path),
        };
        assert!(
            sent,
            "{} is listed as travelling and no walk sends it",
            item.path
        );
    }

    // ONE ROW OF EVERY ORIGIN.
    for origin in [
        Origin::Rule("config"),
        Origin::Rule("state/software/history.jsonl"),
        Origin::Target("ssh"),
        Origin::Target("git"),
        Origin::Added("home/notes".into()),
        Origin::AddedElsewhere("home/music".into()),
        Origin::Plain("state/person"),
        Origin::Plain("config/brain-modules"),
    ] {
        assert!(
            items
                .iter()
                .any(|i| i.origin == origin && i.here == Here::Kept),
            "no row travels from {origin:?}"
        );
    }
    // EVERY REASON A FILE IS NEVER KEPT, and not here, and nowhere.
    assert_eq!(row("config/column-player.pid").here, Here::Never("draft"));
    assert_eq!(row("home/.ssh/agent.sock").here, Here::Never("socket"));
    assert_eq!(
        row("home/.ssh/belongings").here,
        Here::Never(crate::sift::ACCESS),
        "the deploy key under another name"
    );
    assert_eq!(
        row("home/.ssh/copy.pub").here,
        Here::Never(crate::sift::ACCESS)
    );
    assert_eq!(
        row("config/hm-settings").here,
        Here::Never(crate::sift::SYSTEM_MANAGED)
    );
    assert_eq!(row("config/taken-out.json").here, Here::NotHere);
    assert_eq!(row("home/.gnupg/pubring.kbx").here, Here::Nowhere);
    // ROOT READS EVERYTHING: the test says so instead of passing blind.
    if std::fs::File::open(&locked).is_err() {
        assert_eq!(row("config/locked.json").state, State::Unreadable);
        assert_eq!(row("config/hidden").state, State::Unreadable);
    }
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

/// FAILS IF: a sync does not record when each file it left alike last
/// changed — the listing would call every file of a fresh restore "changed"
/// — or a file changed after it is still listed as it was; or a file deleted
/// here is not listed as deleted before the sync that deletes it from the
/// repository and from the other machine.
#[test]
fn a_sync_leaves_the_listing_in_step() {
    let root = scratch("listing-step");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    a.write("config/keyboards.json", "{}");
    a.sync();
    let last = Last::read(&a.places.last()).unwrap();
    assert!(
        last.modified_s.contains_key("config/theme.json"),
        "no time recorded: {:?}",
        last.modified_s
    );
    let listed = |m: &Machine| {
        let last = Last::read(&m.places.last());
        let scope = Scope::of(&m.prefs, last.as_ref(), &Requests::default());
        inventory::inventory(&scope, &m.places)
    };
    let state_of = |m: &Machine, path: &str| {
        listed(m)
            .into_iter()
            .find(|i| i.path == path)
            .unwrap_or_else(|| panic!("{path} is not listed"))
            .state
    };
    assert_eq!(state_of(&a, "config/theme.json"), State::Synced);
    // A SECOND MACHINE that only pulled: what the sync wrote is in step.
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    assert_eq!(state_of(&b, "config/theme.json"), State::Synced);
    assert_eq!(state_of(&b, "config/keyboards.json"), State::Synced);
    // AN EDIT OF THE SAME SIZE, a second later: changed.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    a.write("config/theme.json", "{\"theme\":\"sky\"}");
    assert_eq!(state_of(&a, "config/theme.json"), State::Changed);
    // A FILE NEW HERE: new.
    a.write("config/new.json", "{}");
    assert_eq!(state_of(&a, "config/new.json"), State::New);
    // A FILE DELETED HERE: said before the sync that takes it from every
    // machine — and that sync does.
    std::fs::remove_file(a.places.resolve("config/keyboards.json").unwrap()).unwrap();
    assert_eq!(state_of(&a, "config/keyboards.json"), State::Deleted);
    a.sync();
    assert!(
        !listed(&a).iter().any(|i| i.path == "config/keyboards.json"),
        "a deletion the sync carried is still listed"
    );
    b.sync();
    assert_eq!(
        b.read("config/keyboards.json"),
        None,
        "the other machine kept what the listing said leaves"
    );
    let _ = std::fs::remove_dir_all(&root);
}
