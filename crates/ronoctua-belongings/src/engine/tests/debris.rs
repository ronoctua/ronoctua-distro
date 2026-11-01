//! What a sync from before a rule left in the repository, against a real
//! git and a `file://` bare remote: a link into the system's store and a
//! machine's deploy key, each pushed by a build that did not sift them
//! (`crate::sift::old_sync`).
//!
//! What these prove: a new machine — by a sync or by the installer's
//! restore — writes neither, and the repository loses both, said in the
//! outcome; a machine that took them in an old sync drops them from the
//! repository too, deletes nothing of its own, and says so; and every other
//! file of the same folders keeps travelling.

use super::*;
use crate::sift::{self, old_sync};

const KEY: &str = "home/.ssh/deploy-of-alpha";
const KEY_PUB: &str = "home/.ssh/deploy-of-alpha.pub";
const MINE: &str = "home/.ssh/config";
const LINK: &str = "home/.config/app/settings.json";
const NEIGHBOUR: &str = "home/.config/app/notes.md";

/// Machine A, a build from before the rules: its own deploy key's two halves
/// copied into `~/.ssh`, a link Home Manager made into the system's store
/// in a folder the person added, and the person's own files beside them —
/// all pushed, as such a build pushed them.
fn an_old_build_pushes(root: &Path, remote: &str) -> Machine {
    let mut a = Machine::new(root, "machine-alpha", remote);
    let (private, public) = old_sync::a_key(sift::DEPLOY_TYPE, sift::DEPLOY_COMMENT, "none");
    for (logical, bytes) in [(KEY, &private), (KEY_PUB, &public)] {
        let at = a.places.resolve(logical).unwrap();
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    }
    a.write(MINE, "Host *\n  ServerAliveInterval 30\n");
    assert!(a.prefs.keep("home/.config/app"));
    a.write(NEIGHBOUR, "mine\n");
    std::os::unix::fs::symlink(
        "/nix/store/0000000000000000000000000000000-home-manager-files/settings.json",
        a.places.resolve(LINK).unwrap(),
    )
    .unwrap();
    let outcome = old_sync::during(|| a.sync());
    assert!(
        matches!(outcome, Outcome::Synced { pushed: 5, .. }),
        "the old build did not push all five: {outcome:?}"
    );
    a
}

/// What an outcome says was dropped, by path with the reason.
type Dropped = Vec<(String, String)>;

/// What a sync did: pulled, pushed, deleted here, deleted there.
type Did = (u32, u32, u32, u32);

/// What a sync's outcome says it dropped, and what it did.
fn dropped_by(outcome: &Outcome) -> (Dropped, Did) {
    match outcome {
        Outcome::Synced {
            dropped,
            pulled,
            pushed,
            deleted_here,
            deleted_there,
            ..
        } => (
            dropped.clone(),
            (*pulled, *pushed, *deleted_here, *deleted_there),
        ),
        other => panic!("{other:?}"),
    }
}

/// The three, each with its reason, as an outcome lists them.
fn the_three() -> Vec<(String, String)> {
    vec![
        (LINK.to_owned(), sift::SYSTEM_MANAGED.to_owned()),
        (KEY.to_owned(), sift::ACCESS_ELSEWHERE.to_owned()),
        (KEY_PUB.to_owned(), sift::ACCESS_ELSEWHERE.to_owned()),
    ]
}

/// FAILS IF: a new machine's sync writes the link into the system's store
/// or another machine's deploy key (either half) that an old sync left in
/// the repository; or the repository keeps them after it; or its outcome
/// does not name them with their reasons; or the person's own files in the
/// same folders stop travelling.
#[test]
fn what_an_old_sync_left_is_never_pulled_and_leaves_the_repository() {
    let root = scratch("debris-new-machine");
    let remote = bare(&root.join("remote.git"));
    let _a = an_old_build_pushes(&root, &remote);
    let mut c = Machine::new(&root, "machine-gamma", &remote);
    c.prefs.extra.push("home/.config/app".to_owned());
    let (dropped, (pulled, pushed, _, deleted_there)) = dropped_by(&c.sync());
    assert_eq!(dropped, the_three());
    assert_eq!((pulled, pushed, deleted_there), (2, 0, 3));
    for logical in [KEY, KEY_PUB, LINK] {
        let at = c.places.resolve(logical).unwrap();
        assert!(
            std::fs::symlink_metadata(&at).is_err(),
            "{logical} was written on the new machine"
        );
    }
    assert!(c.read(MINE).is_some() && c.read(NEIGHBOUR).is_some());
    // GONE FROM THE REPOSITORY: the next machine finds nothing to drop.
    let mut d = Machine::new(&root, "machine-delta", &remote);
    d.prefs.extra.push("home/.config/app".to_owned());
    let (dropped, (pulled, ..)) = dropped_by(&d.sync());
    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(pulled, 2);
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a machine that took the three in an old sync keeps them in the
/// repository once it runs this build, deletes its own copies (the person
/// decides what to do with a key in their `~/.ssh`), or does not say what
/// it dropped; or the machine that pushed them deletes anything of its own
/// afterwards.
#[test]
fn what_an_old_sync_carried_here_leaves_the_repository_from_here() {
    let root = scratch("debris-carried");
    let remote = bare(&root.join("remote.git"));
    let a = an_old_build_pushes(&root, &remote);
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.prefs.extra.push("home/.config/app".to_owned());
    let outcome = old_sync::during(|| b.sync());
    assert!(
        matches!(outcome, Outcome::Synced { pulled: 5, .. }),
        "the old build did not take all five: {outcome:?}"
    );
    let (dropped, (pulled, pushed, deleted_here, deleted_there)) = dropped_by(&b.sync());
    assert_eq!(dropped, the_three());
    assert_eq!((pulled, pushed, deleted_here, deleted_there), (0, 0, 0, 3));
    for logical in [KEY, KEY_PUB, LINK] {
        let at = b.places.resolve(logical).unwrap();
        assert!(
            std::fs::symlink_metadata(&at).is_ok(),
            "{logical} was deleted here"
        );
    }
    // THE MACHINE THAT PUSHED THEM, on this build: nothing of its own goes.
    let (dropped, (pulled, pushed, deleted_here, _)) = dropped_by(&a.sync());
    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!((pulled, pushed, deleted_here), (0, 0, 0));
    for logical in [KEY, KEY_PUB, LINK, MINE, NEIGHBOUR] {
        let at = a.places.resolve(logical).unwrap();
        assert!(std::fs::symlink_metadata(&at).is_ok(), "{logical} went");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's restore of a new home writes what an old sync
/// left, or does not say so; or the first sync of that home does not take
/// it out of the repository.
#[test]
fn a_restore_never_writes_what_an_old_sync_left() {
    let root = scratch("debris-restore");
    let remote = bare(&root.join("remote.git"));
    let _a = an_old_build_pushes(&root, &remote);
    let home = root.join("new/home");
    std::fs::create_dir_all(&home).unwrap();
    let mut prefs = Belongings::with_repository(&remote);
    prefs.extra.push("home/.config/app".to_owned());
    let c = Machine {
        name: "machine-gamma",
        places: Places::for_home(&home, &root.join("new/run")),
        prefs,
        vault: None,
        pinned: None,
    };
    c.say(PASSPHRASE);
    let outcome = c.engine().restore(&mut |_| {}).unwrap();
    let (dropped, (pulled, ..)) = dropped_by(&outcome);
    assert_eq!(dropped, the_three());
    assert_eq!(pulled, 2);
    for logical in [KEY, KEY_PUB, LINK] {
        let at = c.places.resolve(logical).unwrap();
        assert!(
            std::fs::symlink_metadata(&at).is_err(),
            "{logical} was restored"
        );
    }
    let (dropped, (_, _, deleted_here, deleted_there)) = dropped_by(&c.sync());
    assert_eq!(dropped, the_three());
    assert_eq!((deleted_here, deleted_there), (0, 3));
    let _ = std::fs::remove_dir_all(root);
}
