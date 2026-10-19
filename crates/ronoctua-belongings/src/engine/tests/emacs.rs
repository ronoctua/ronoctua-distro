//! The person's Emacs configuration against a real git and a `file://` bare
//! remote: ONE place travels, `~/.config/emacs`, and nothing about any other
//! place Emacs may read on a machine moves, holds or deletes a file.
//!
//! What these prove: a machine whose Emacs reads `~/.emacs.d` pushes nothing
//! of it and deletes nothing of another machine's `~/.config/emacs`; an
//! addition made by hand at `~/.config/emacs` before the target is what it
//! was — a root of its own — and "not on this machine" on the target leaves
//! the repository's copies to the others; and the second review's scenarios (R1, R1b, R2, R4, R4b), on the
//! rules as they are now: a machine's base never names a file its disk did
//! not have, so every deletion that travels is of a file that machine held,
//! and lands set aside on the others.

use super::*;
use crate::emacs::{self, Reads};

const INIT: &str = "home/.config/emacs/init.el";
const LISP: &str = "home/.config/emacs/lisp/a.el";

/// Every path the last sync of `m` left in its base is on its disk: the
/// mechanism of the second review's B1 — a base naming another machine's
/// files this one never had, so that removing a folder deleted them — is
/// gone when this holds after every sync.
fn base_is_the_disk(m: &Machine) {
    let last = Last::read(&m.places.last()).expect("a base");
    for path in last.entries.keys() {
        let at = m.places.resolve(path).unwrap();
        assert!(
            std::fs::symlink_metadata(&at).is_ok(),
            "{}: its base names {path}, which its disk does not have",
            m.name
        );
    }
}

/// The copy a sync set aside on `m` before it deleted or replaced
/// `logical` (`replaced/<stamp>/<logical>`), when there is one.
fn set_aside(m: &Machine, logical: &str) -> Option<String> {
    let stamps = std::fs::read_dir(m.places.replaced()).ok()?;
    stamps
        .flatten()
        .filter_map(|s| std::fs::read_to_string(s.path().join(logical)).ok())
        .next()
}

/// What a sync of `m` did: pulled, pushed, deleted here, deleted there.
fn counts(m: &Machine) -> (u32, u32, u32, u32) {
    match m.sync() {
        Outcome::Synced {
            pulled,
            pushed,
            deleted_here,
            deleted_there,
            ..
        } => {
            base_is_the_disk(m);
            (pulled, pushed, deleted_here, deleted_there)
        }
        other => panic!("{}: {other:?}", m.name),
    }
}

/// FAILS IF: a machine whose Emacs reads `~/.emacs.d` pushes anything of it
/// — its configuration, its packages, a stub `~/.emacs` Customize wrote —,
/// deletes anything in another machine's `~/.config/emacs`, or loses its
/// own `~/.emacs.d`; or its page does not say which place Emacs reads.
#[test]
fn a_machine_whose_emacs_reads_emacs_d_moves_nothing_of_anothers() {
    let root = scratch("emacs-reads-emacs-d");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write(INIT, "(alpha)\n");
    a.write(LISP, "(provide 'a)\n");
    assert_eq!(counts(&a), (0, 2, 0, 0));
    // B: ITS CONFIGURATION IN `~/.emacs.d`, its packages beside it, and a
    // `~/.emacs` Customize wrote.
    let mine = [
        ("home/.emacs.d/init.el", "(beta)\n"),
        ("home/.emacs.d/lisp/b.el", "(provide 'b)\n"),
        ("home/.emacs.d/elpa/magit-4.1/magit.el", ";; magit\n"),
        ("home/.emacs", "(custom-set-variables)\n"),
    ];
    for (logical, text) in mine {
        b.write(logical, text);
    }
    // ITS FIRST SYNC takes the repository's `~/.config/emacs` — Emacs does
    // not read it there — and sends nothing.
    assert_eq!(counts(&b), (2, 0, 0, 0));
    assert_eq!(
        emacs::reads(&b.places),
        Some(Reads::Elsewhere(emacs::EMACS))
    );
    std::fs::remove_file(b.places.resolve(emacs::EMACS).unwrap()).unwrap();
    assert_eq!(
        emacs::reads(&b.places),
        Some(Reads::Elsewhere(emacs::EMACS_D))
    );
    // B GOES ON in `~/.emacs.d`: nothing moves.
    b.write("home/.emacs.d/init.el", "(beta, again)\n");
    b.write("home/.emacs.d/lisp/c.el", "(provide 'c)\n");
    assert_eq!(counts(&b), (0, 0, 0, 0));
    // A CHANGES ITS OWN: B takes it, and A loses nothing.
    a.write(INIT, "(alpha, again)\n");
    assert_eq!(counts(&a), (0, 1, 0, 0));
    assert_eq!(counts(&b), (1, 0, 0, 0));
    assert_eq!(counts(&a), (0, 0, 0, 0));
    assert_eq!(a.read(INIT).as_deref(), Some("(alpha, again)\n"));
    assert_eq!(a.read(LISP).as_deref(), Some("(provide 'a)\n"));
    // B's OWN is where it was, and no machine ever saw it.
    assert_eq!(
        b.read("home/.emacs.d/init.el").as_deref(),
        Some("(beta, again)\n")
    );
    let c = Machine::new(&root, "machine-gamma", &remote);
    assert_eq!(counts(&c), (2, 0, 0, 0));
    for (logical, _) in mine {
        assert!(c.read(logical).is_none(), "{logical} travelled");
        assert!(a.read(logical).is_none(), "{logical} travelled");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: an addition made by hand at `~/.config/emacs` before the target
/// existed (and one under it) stops being a root of its own whose files
/// travel as the person's — what it was on a88e849, and what a machine on
/// that build sharing the repository still makes of it — or adds what the
/// target skips; or "not on this machine" for the target takes a file out
/// of the repository (the others keep theirs, and a new machine still takes
/// it); or another machine deletes or changes a file because of it.
#[test]
fn an_old_addition_at_the_emacs_folder_is_a_root_and_deletes_nothing() {
    let root = scratch("emacs-old-addition");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    let package = "home/.config/emacs/elpa/magit-4.1/magit.el";
    a.prefs.extra = vec![
        "home/.config/emacs".to_owned(),
        "home/.config/emacs/lisp".to_owned(),
    ];
    a.write(INIT, "(alpha)\n");
    a.write(LISP, "(provide 'a)\n");
    a.write(package, ";; magit\n");
    assert_eq!(counts(&a), (0, 2, 0, 0));
    // A ROOT OF ITS OWN, its files the person's additions: what B follows.
    let last = Last::read(&a.places.last()).unwrap();
    assert!(
        last.roots.contains("home/.config/emacs"),
        "no root: {:?}",
        last.roots
    );
    assert!(last.entries.values().all(|k| k.added), "{:?}", last.entries);
    assert!(!last.entries.contains_key(package), "the package is kept");
    assert_eq!(counts(&b), (2, 0, 0, 0));
    let followed = Last::read(&b.places.last()).unwrap();
    assert!(followed.roots.contains("home/.config/emacs"));
    assert!(followed.entries.get(INIT).is_some_and(|k| k.added));
    assert!(b.read(package).is_none());
    // "NOT ON THIS MACHINE" for the target on A: the old addition goes with
    // it, its root leaves the index, and no file leaves the repository —
    // `lisp`, added deeper, is still A's.
    for path in kept::target_by_id(emacs::TARGET).unwrap().paths {
        a.prefs.stop_keeping(path);
    }
    assert_eq!(a.prefs.extra, ["home/.config/emacs/lisp"]);
    assert_eq!(counts(&a), (0, 0, 0, 0), "a file left the repository");
    let last = Last::read(&a.places.last()).unwrap();
    assert!(
        !last.roots.contains("home/.config/emacs"),
        "{:?}",
        last.roots
    );
    // B KEEPS THE TARGET: in step, nothing deleted; its edit travels, and A,
    // which no longer keeps the folder, takes nothing.
    assert_eq!(counts(&b), (0, 0, 0, 0));
    assert_eq!(b.read(INIT).as_deref(), Some("(alpha)\n"));
    b.write(INIT, "(beta)\n");
    assert_eq!(counts(&b), (0, 1, 0, 0));
    assert_eq!(counts(&a), (0, 0, 0, 0));
    assert_eq!(
        a.read(INIT).as_deref(),
        Some("(alpha)\n"),
        "A takes nothing"
    );
    assert_eq!(a.read(LISP).as_deref(), Some("(provide 'a)\n"));
    assert_eq!(a.read(package).as_deref(), Some(";; magit\n"));
    let c = Machine::new(&root, "machine-gamma", &remote);
    assert_eq!(counts(&c), (2, 0, 0, 0));
    assert_eq!(c.read(INIT).as_deref(), Some("(beta)\n"));
    assert_eq!(c.read(LISP).as_deref(), Some("(provide 'a)\n"));
    assert!(c.read(package).is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// The files Doom's checkout has (the second review's `doom_on`): with no
/// framework recognised, they are files of `~/.config/emacs` like any.
fn doom_on(m: &Machine) {
    for f in ["bin/doom", "lisp/doom.el", "early-init.el"] {
        m.write(&format!("home/.config/emacs/{f}"), ";; doom\n");
    }
    m.write("home/.config/doom/config.el", "(doom)\n");
}

/// R1, on the rules as they are now. B had Doom's checkout at
/// `~/.config/emacs`, A a plain configuration there; B drops Doom by
/// removing the folder.
///
/// FAILS IF: B's base names a file B's disk never had (the review's B1: A's
/// files deleted by a machine that never held them), `~/.config/doom` is
/// carried, a deletion reaches A that is not one of the files B held and
/// removed, or A does not keep each of them set aside.
#[test]
fn review_r1_a_removed_folder_deletes_only_what_it_held() {
    let root = scratch("emacs-review-r1");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write(INIT, "(alpha)\n");
    a.write(LISP, "(provide 'a)\n");
    assert_eq!(counts(&a), (0, 2, 0, 0));
    doom_on(&b);
    // B's FIRST SYNC: A's two files land in B's folder, where its page lists
    // them — B holds them now — and Doom's three go.
    assert_eq!(counts(&b), (2, 3, 0, 0));
    assert_eq!(b.read(INIT).as_deref(), Some("(alpha)\n"));
    let held: Vec<String> = Last::read(&b.places.last())
        .unwrap()
        .entries
        .into_keys()
        .collect();
    assert_eq!(held.len(), 5, "{held:?}");
    assert!(!held.iter().any(|p| p.starts_with("home/.config/doom")));
    // THE FOLDER REMOVED on B: what B held, and only that, leaves.
    std::fs::remove_dir_all(b.places.resolve("home/.config/emacs").unwrap()).unwrap();
    assert_eq!(counts(&b), (0, 0, 0, 5));
    // A: Doom's three never reached it before they left; its own two are
    // the person's deletion on B, each set aside here.
    assert_eq!(counts(&a), (0, 0, 2, 0));
    assert_eq!(set_aside(&a, INIT).as_deref(), Some("(alpha)\n"));
    assert_eq!(set_aside(&a, LISP).as_deref(), Some("(provide 'a)\n"));
    let _ = std::fs::remove_dir_all(root);
}

/// R1b, on the rules as they are now: B replaces Doom's checkout with a
/// configuration of its own.
///
/// FAILS IF: B's base names a file B's disk never had, or what reaches A
/// is anything but B's own edit of a file B held (A's `init.el`, pulled
/// there before) and B's deletion of another it held — each set aside on A.
#[test]
fn review_r1b_a_replaced_folder_sends_only_what_it_held() {
    let root = scratch("emacs-review-r1b");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write(INIT, "(alpha)\n");
    a.write(LISP, "(provide 'a)\n");
    assert_eq!(counts(&a), (0, 2, 0, 0));
    doom_on(&b);
    assert_eq!(counts(&b), (2, 3, 0, 0));
    std::fs::remove_dir_all(b.places.resolve("home/.config/emacs").unwrap()).unwrap();
    b.write(INIT, "(beta)\n");
    // B's `init.el` — the one it pulled — is changed, the other four gone.
    assert_eq!(counts(&b), (0, 1, 0, 4));
    assert_eq!(counts(&a), (1, 0, 1, 0));
    assert_eq!(a.read(INIT).as_deref(), Some("(beta)\n"));
    assert_eq!(set_aside(&a, INIT).as_deref(), Some("(alpha)\n"));
    assert_eq!(set_aside(&a, LISP).as_deref(), Some("(provide 'a)\n"));
    let _ = std::fs::remove_dir_all(root);
}

/// R2, on the rules as they are now: A moves its configuration from
/// `~/.config/emacs` to `~/.emacs.d`, while B has a new file of its own in
/// `~/.config/emacs`.
///
/// FAILS IF: anything of A's `~/.emacs.d` travels, B's new file is deleted
/// or does not travel, a deletion reaches B that is not one of the files A
/// held and moved away, B does not keep them set aside, or A's page does
/// not say where its Emacs reads now.
#[test]
fn review_r2_a_move_out_of_the_carried_place_is_said_and_sets_aside() {
    let root = scratch("emacs-review-r2");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write(INIT, "(i)\n");
    a.write(LISP, "(a)\n");
    assert_eq!(counts(&a), (0, 2, 0, 0));
    assert_eq!(counts(&b), (2, 0, 0, 0));
    b.write("home/.config/emacs/lisp/new.el", "(new)\n");
    std::fs::remove_dir_all(a.places.resolve("home/.config/emacs").unwrap()).unwrap();
    a.write("home/.emacs.d/init.el", "(i)\n");
    a.write("home/.emacs.d/lisp/a.el", "(a)\n");
    assert_eq!(
        emacs::reads(&a.places),
        Some(Reads::Elsewhere(emacs::EMACS_D))
    );
    assert_eq!(counts(&a), (0, 0, 0, 2));
    assert_eq!(counts(&b), (0, 1, 2, 0));
    assert_eq!(
        b.read("home/.config/emacs/lisp/new.el").as_deref(),
        Some("(new)\n")
    );
    assert_eq!(set_aside(&b, INIT).as_deref(), Some("(i)\n"));
    assert_eq!(set_aside(&b, LISP).as_deref(), Some("(a)\n"));
    assert!(
        b.read("home/.emacs.d/init.el").is_none(),
        "~/.emacs.d travelled"
    );
    // A TAKES B's new file — and Emacs there still reads `~/.emacs.d`.
    assert_eq!(counts(&a), (1, 0, 0, 0));
    assert_eq!(a.read("home/.emacs.d/init.el").as_deref(), Some("(i)\n"));
    let _ = std::fs::remove_dir_all(root);
}

/// R4, on the rules as they are now: A keeps its configuration in
/// `~/.emacs.d` and ADDED it by hand (an addition travels to every
/// machine); B's is in `~/.config/emacs`, and B takes A's folder out.
///
/// FAILS IF: B's configuration does not reach A's `~/.config/emacs`, A's
/// `~/.emacs.d` is deleted or changed anywhere, B's page does not say that
/// its Emacs now reads the `~/.emacs.d` that arrived, or B's "not on this
/// machine" deletes a file on either disk — A, which added the folder
/// itself, sends it back, and B keeps it out.
#[test]
fn review_r4_an_added_emacs_d_taken_out_deletes_nothing() {
    let root = scratch("emacs-review-r4");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    let mut b = Machine::new(&root, "machine-beta", &remote);
    assert!(a.prefs.keep("home/.emacs.d"));
    a.write("home/.emacs.d/init.el", "(alpha)\n");
    assert_eq!(counts(&a), (0, 1, 0, 0));
    b.write(INIT, "(beta)\n");
    // B FOLLOWS A's addition, and sends its own configuration.
    assert_eq!(counts(&b), (1, 1, 0, 0));
    assert_eq!(
        emacs::reads(&b.places),
        Some(Reads::Elsewhere(emacs::EMACS_D))
    );
    assert_eq!(counts(&a), (1, 0, 0, 0));
    assert_eq!(a.read(INIT).as_deref(), Some("(beta)\n"));
    // B TAKES `~/.emacs.d` OUT: it leaves the repository, every machine
    // forgets it, and both disks keep their copy.
    assert!(b.prefs.stop_keeping("home/.emacs.d"));
    assert_eq!(counts(&b), (0, 0, 0, 1));
    assert_eq!(counts(&a), (0, 1, 0, 0));
    assert_eq!(counts(&b), (0, 0, 0, 0));
    assert_eq!(
        a.read("home/.emacs.d/init.el").as_deref(),
        Some("(alpha)\n")
    );
    assert_eq!(
        b.read("home/.emacs.d/init.el").as_deref(),
        Some("(alpha)\n")
    );
    assert_eq!(b.read(INIT).as_deref(), Some("(beta)\n"));
    let c = Machine::new(&root, "machine-gamma", &remote);
    assert_eq!(counts(&c), (2, 0, 0, 0));
    let _ = std::fs::remove_dir_all(root);
}

/// R4b, on the rules as they are now: B pulled A's added `~/.emacs.d`, took
/// its `init.el` out here ("not here", its row) and removed the folder,
/// then wrote its own configuration in `~/.config/emacs`.
///
/// FAILS IF: A's file is deleted from A's disk or from the repository, or
/// B's own configuration does not travel.
#[test]
fn review_r4b_a_file_taken_out_here_deletes_nothing() {
    let root = scratch("emacs-review-r4b");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    let mut b = Machine::new(&root, "machine-beta", &remote);
    assert!(a.prefs.keep("home/.emacs.d"));
    a.write("home/.emacs.d/init.el", "(alpha)\n");
    assert_eq!(counts(&a), (0, 1, 0, 0));
    assert_eq!(counts(&b), (1, 0, 0, 0));
    b.prefs.stop_keeping("home/.emacs.d/init.el");
    std::fs::remove_dir_all(b.places.resolve("home/.emacs.d").unwrap()).unwrap();
    b.write(INIT, "(beta)\n");
    assert_eq!(counts(&b), (0, 1, 0, 0));
    assert_eq!(counts(&a), (1, 0, 0, 0));
    assert_eq!(
        a.read("home/.emacs.d/init.el").as_deref(),
        Some("(alpha)\n")
    );
    let c = Machine::new(&root, "machine-gamma", &remote);
    assert_eq!(counts(&c), (2, 0, 0, 0));
    assert_eq!(c.read(INIT).as_deref(), Some("(beta)\n"));
    assert_eq!(
        c.read("home/.emacs.d/init.el").as_deref(),
        Some("(alpha)\n")
    );
    let _ = std::fs::remove_dir_all(root);
}
