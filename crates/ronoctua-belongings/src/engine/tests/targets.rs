//! The predefined targets and the identity kept for the session, against a
//! real git and a `file://` bare remote.
//!
//! What these prove: `~/.ssh` and `~/.gnupg` travel encrypted and come back
//! private — files with their owner's bits alone, folders `0700` even where
//! something made them wider first — and what a target skips never travels
//! (the person's Emacs configuration goes, its packages and caches do not);
//! a target dropped on every machine leaves the repository from the machine
//! that dropped it, is forgotten (and kept on disk) by the others, and comes
//! back when kept everywhere again; "not here" leaves the repository's
//! copies to the others; the files a sync brought offer what they make
//! possible; the identity a typed passphrase opened is kept for the session
//! and never written to a disk; a second sync asks nothing, and an identity
//! that no longer opens the repository is forgotten and asks the passphrase.

use std::os::unix::fs::PermissionsExt as _;

use age::secrecy::ExposeSecret as _;

use super::*;
use crate::requests::{Request, Requests};
use crate::status::Offer;
use crate::unlock::{Marker, Vault as _, description};

fn mode_of(m: &Machine, logical: &str) -> u32 {
    std::fs::symlink_metadata(m.places.resolve(logical).unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o7777
}

fn chmod(m: &Machine, logical: &str, mode: u32) {
    std::fs::set_permissions(
        m.places.resolve(logical).unwrap(),
        std::fs::Permissions::from_mode(mode),
    )
    .unwrap();
}

/// A private key's first line, assembled in pieces: written whole, the
/// repository's secret scan (`nix/scripts/scan-secrets.py`) would take this
/// fixture for a key that leaked.
const A_KEY: &str = concat!("-----BEGIN OPENSSH ", "PRIVATE", " KEY----- mine");

/// A person's `~/.ssh`, `~/.gnupg`, and two of their own files.
fn person_files(a: &Machine) {
    a.write("home/.ssh/id_ed25519", A_KEY);
    a.write(
        "home/.ssh/id_ed25519.pub",
        "ssh-ed25519 AAAAC3Nza me@laptop\n",
    );
    a.write("home/.ssh/config", "Host *\n  ServerAliveInterval 60\n");
    a.write("home/.ssh/known_hosts", "github.com ssh-ed25519 AAAA\n");
    // WHAT THE TARGETS SKIP, as files (a socket is no file anyway).
    a.write("home/.ssh/agent.sock", "not a socket, and still skipped");
    a.write("home/.ssh/control-git@github.com:22", "a master");
    a.write("home/.gnupg/private-keys-v1.d/A1.key", "(private-key …)");
    a.write("home/.gnupg/pubring.kbx", "keybox");
    a.write("home/.gnupg/trustdb.gpg", "trust");
    a.write("home/.gnupg/random_seed", "this machine's seed");
    a.write("home/.gnupg/.#lk0x0000.laptop.42", "a lock");
    a.write("home/.gnupg/public-keys.d/pubring.db.lock", "a lock");
    a.write("home/.shellrc.local", "export TOKEN=mine\n");
    a.write("home/.gitconfig", "[user]\n\tname = me\n");
    chmod(a, "home/.ssh", 0o755);
    chmod(a, "home/.ssh/id_ed25519", 0o600);
    chmod(a, "home/.ssh/id_ed25519.pub", 0o644);
    chmod(a, "home/.ssh/config", 0o644);
    chmod(a, "home/.gnupg", 0o700);
    chmod(a, "home/.gnupg/private-keys-v1.d", 0o700);
    chmod(a, "home/.gnupg/pubring.kbx", 0o644);
    chmod(a, "home/.shellrc.local", 0o644);
}

/// FAILS IF: a private target's file comes back with group or other bits,
/// its folder stays wider than `0700` (made so before the sync, or never
/// remembered), a name a target skips travels, a target that is not private
/// loses its mode — or the clamped modes make the next syncs push again.
#[test]
fn belongings_targets_come_back_private() {
    let root = scratch("targets-private");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    person_files(&a);
    a.sync();
    // SOMETHING MADE B's `~/.ssh` WIDE before the belongings came.
    std::fs::create_dir_all(b.places.home.join(".ssh")).unwrap();
    chmod(&b, "home/.ssh", 0o755);
    let outcome = b.sync();
    let Outcome::Synced { pulled, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(pulled, 9, "four of ssh, three of gnupg, the shell's, git's");
    for (logical, mode) in [
        ("home/.ssh/id_ed25519", 0o600),
        ("home/.ssh/id_ed25519.pub", 0o600),
        ("home/.ssh/config", 0o600),
        ("home/.ssh/known_hosts", 0o600),
        ("home/.gnupg/pubring.kbx", 0o600),
        ("home/.gnupg/private-keys-v1.d/A1.key", 0o600),
        ("home/.ssh", 0o700),
        ("home/.gnupg", 0o700),
        ("home/.gnupg/private-keys-v1.d", 0o700),
        ("home/.shellrc.local", 0o644),
        ("home/.gitconfig", 0o644),
    ] {
        assert_eq!(
            mode_of(&b, logical),
            mode,
            "{logical} came back {:o}",
            mode_of(&b, logical)
        );
    }
    assert_eq!(b.read("home/.ssh/id_ed25519").as_deref(), Some(A_KEY));
    for skipped in [
        "home/.ssh/agent.sock",
        "home/.ssh/control-git@github.com:22",
        "home/.gnupg/random_seed",
        "home/.gnupg/.#lk0x0000.laptop.42",
        "home/.gnupg/public-keys.d/pubring.db.lock",
    ] {
        assert!(b.read(skipped).is_none(), "{skipped} travelled");
        let last = Last::read(&a.places.last()).unwrap();
        assert!(!last.entries.contains_key(skipped), "{skipped} is kept");
    }
    // THE MODES READ AS THEY TRAVEL: nothing moves again, on either side —
    // though A's own files are still `0644`.
    assert_eq!(mode_of(&a, "home/.ssh/id_ed25519.pub"), 0o644);
    for m in [&a, &b] {
        let Outcome::Synced { pushed, pulled, .. } = m.sync() else {
            panic!("{}", m.name);
        };
        assert_eq!((pushed, pulled), (0, 0), "{} moved a mode", m.name);
    }
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the person's Emacs configuration does not reach another
/// machine as it was written — its saved choices and lockfiles carried out
/// of the folders that hold them, a snippet named like Emacs's state deeper
/// down included — or anything Emacs and its package managers made from it
/// does: the packages, what was compiled, the caches and the state of this
/// machine, a checkout's `.git`, an editor's drafts.
#[test]
fn belongings_emacs_travels_without_its_packages() {
    let root = scratch("targets-emacs");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    let carried = [
        (
            "home/.config/emacs/init.el",
            "(load-theme 'modus-vivendi t)\n",
        ),
        (
            "home/.config/emacs/early-init.el",
            "(setq package-enable-at-startup nil)\n",
        ),
        ("home/.config/emacs/lisp/my-keys.el", "(provide 'my-keys)\n"),
        ("home/.config/emacs/custom.el", "(custom-set-variables)\n"),
        ("home/.config/emacs/snippets/org-mode/url", "# name: url\n"),
        ("home/.config/emacs/transient/values.el", "((magit-log))\n"),
        (
            "home/.config/emacs/straight/versions/default.el",
            "((\"x\" . \"1\"))\n",
        ),
    ];
    for (logical, text) in carried {
        a.write(logical, text);
    }
    let skipped = [
        "home/.config/emacs/elpa/magit-4.1/magit.el",
        "home/.config/emacs/elpa/magit-4.1/magit.elc",
        "home/.config/emacs/eln-cache/30.2-1a2b/magit-3c4d.eln",
        "home/.config/emacs/straight/repos/vertico/vertico.el",
        "home/.config/emacs/.local/straight/build-30.2/x/x.el",
        "home/.config/emacs/lisp/my-keys.elc",
        "home/.config/emacs/auto-save-list/.saves-42-alpha~",
        "home/.config/emacs/transient/history.el",
        "home/.config/emacs/var/elfeed/db/index",
        "home/.config/emacs/history",
        "home/.config/emacs/url/cookies",
        "home/.config/emacs/server/server",
        "home/.config/emacs/.git/index",
        "home/.config/emacs/#init.el#",
        "home/.config/emacs/init.el~",
        "home/.config/emacs/tree-sitter/libtree-sitter-rust.so",
    ];
    for logical in skipped {
        a.write(logical, "this machine's");
    }
    a.sync();
    let outcome = b.sync();
    let Outcome::Synced { pulled, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(pulled, 7, "the seven files of the configuration");
    for (logical, text) in carried {
        assert_eq!(b.read(logical).as_deref(), Some(text), "{logical}");
    }
    let last = Last::read(&a.places.last()).unwrap();
    for logical in skipped {
        assert!(b.read(logical).is_none(), "{logical} travelled");
        assert!(!last.entries.contains_key(logical), "{logical} is kept");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: "not on any machine" leaves a copy in the repository, deletes
/// a file anywhere, keeps the target in another machine's base or lets it
/// push it again; a request a door made while the sync ran is consumed with
/// the one it applied; "everywhere again" does not bring it back; or "not on
/// this machine" takes the repository's copies from the others.
#[test]
fn belongings_dropped_everywhere_is_forgotten_elsewhere() {
    let root = scratch("targets-dropped");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let mut b = Machine::new(&root, "machine-beta", &remote);
    person_files(&a);
    a.sync();
    b.sync();
    let ssh_kept = |m: &Machine| {
        Last::read(&m.places.last())
            .unwrap()
            .entries
            .keys()
            .filter(|p| p.starts_with("home/.ssh/"))
            .count()
    };
    assert_eq!(ssh_kept(&b), 4);

    // A: NOT ON ANY MACHINE — and, while that sync runs, a door asks the
    // same of GnuPG: that request is not this sync's to consume.
    let mut asked = Requests::default();
    assert!(asked.ask(Request::DropEverywhere {
        target: "ssh".to_owned()
    }));
    asked.write(&a.places.requests()).unwrap();
    a.say(PASSPHRASE);
    let requests = a.places.requests();
    let meanwhile = move || {
        let mut more = Requests::read(&requests);
        more.ask(Request::DropEverywhere {
            target: "gnupg".to_owned(),
        });
        more.write(&requests).unwrap();
    };
    let mut engine = a.engine();
    engine.before_push = Some(&meanwhile);
    let Ok(Done::Synced(Outcome::Synced { deleted_there, .. })) = engine.prepare(&mut |_| {})
    else {
        panic!("the drop did not sync");
    };
    assert_eq!(
        deleted_there, 4,
        "the four files of ssh left the repository"
    );
    assert_eq!(
        Requests::read(&a.places.requests()).requests,
        [Request::DropEverywhere {
            target: "gnupg".to_owned()
        }],
        "the request the sync applied stayed, or the one made meanwhile went"
    );
    let _ = std::fs::remove_file(a.places.requests());
    let last = Last::read(&a.places.last()).unwrap();
    assert_eq!(ssh_kept(&a), 0);
    assert!(last.dropped.contains("ssh"));
    assert!(
        a.read("home/.ssh/config").is_some(),
        "the drop deleted a file here"
    );

    // B: FORGETS, keeps its files, and never pushes them again.
    let Outcome::Synced {
        deleted_here,
        pushed,
        ..
    } = b.sync()
    else {
        panic!();
    };
    assert_eq!((deleted_here, pushed), (0, 0));
    assert_eq!(
        ssh_kept(&b),
        0,
        "another machine still keeps a dropped target"
    );
    assert!(
        Last::read(&b.places.last())
            .unwrap()
            .dropped
            .contains("ssh")
    );
    assert!(
        b.read("home/.ssh/id_ed25519").is_some(),
        "the drop deleted a file elsewhere"
    );
    b.write("home/.ssh/config", "Host * changed on beta\n");
    b.write("home/.ssh/extra", "a key made while it was dropped");
    let Outcome::Synced { pushed, .. } = b.sync() else {
        panic!();
    };
    assert_eq!(pushed, 0, "a dropped target was pushed again");
    let Outcome::Synced { pulled, .. } = a.sync() else {
        panic!();
    };
    assert_eq!(pulled, 0);
    assert_eq!(
        a.read("home/.ssh/config").as_deref(),
        Some("Host *\n  ServerAliveInterval 60\n")
    );

    // EVERYWHERE AGAIN, from B: its files go up, A takes the new one.
    b.write("home/.ssh/config", "Host *\n  ServerAliveInterval 60\n");
    let mut again = Requests::default();
    again.ask(Request::KeepEverywhere {
        target: "ssh".to_owned(),
    });
    again.write(&b.places.requests()).unwrap();
    let Outcome::Synced { pushed, .. } = b.sync() else {
        panic!();
    };
    assert_eq!(pushed, 5, "the four of before and the new one");
    assert!(!b.places.requests().exists(), "a settled request stayed");
    assert!(Last::read(&b.places.last()).unwrap().dropped.is_empty());
    a.sync();
    assert_eq!(
        a.read("home/.ssh/extra").as_deref(),
        Some("a key made while it was dropped")
    );
    assert_eq!(ssh_kept(&a), 5);

    // NOT ON THIS MACHINE: B takes GnuPG out; the repository keeps A's.
    assert!(b.prefs.stop_keeping("home/.gnupg"));
    let Outcome::Synced { deleted_there, .. } = b.sync() else {
        panic!();
    };
    assert_eq!(deleted_there, 0, "not here took the copies of the others");
    a.write("home/.gnupg/trustdb.gpg", "trust, changed on alpha");
    let Outcome::Synced { pushed, .. } = a.sync() else {
        panic!();
    };
    assert_eq!(
        pushed, 1,
        "the target left the repository for every machine"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a sync that brought the person's SSH key and their security
/// keys offers neither, offers what was already turned on, or a sync that
/// brought nothing offers anything.
#[test]
fn a_sync_that_brings_a_key_offers_what_it_makes_possible() {
    let root = scratch("targets-offers");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    person_files(&a);
    a.write(
        "home/.config/Yubico/u2f_keys",
        "me:handle,key,es256,+presence\n",
    );
    let Outcome::Synced { offers, .. } = a.sync() else {
        panic!();
    };
    assert!(
        offers.is_empty(),
        "the machine that pushed was offered {offers:?}"
    );
    let Outcome::Synced { offers, .. } = b.sync() else {
        panic!();
    };
    assert_eq!(
        offers,
        [
            Offer::SshSigning {
                key: "home/.ssh/id_ed25519.pub".to_owned()
            },
            Offer::PamU2f
        ]
    );
    let Outcome::Synced { offers, .. } = b.sync() else {
        panic!();
    };
    assert!(
        offers.is_empty(),
        "a sync that brought nothing offered {offers:?}"
    );
    // ANOTHER FILE OF THOSE TARGETS changes on A — a new host, an alias —
    // and B, whose "no" keeps no memory, is offered nothing again.
    a.write(
        "home/.ssh/known_hosts",
        "github.com ssh-ed25519 AAAA\ncodeberg.org ssh-ed25519 BBBB\n",
    );
    a.write(
        "home/.gitconfig",
        "[user]\n\tname = me\n[alias]\n\tst = status\n",
    );
    a.sync();
    let Outcome::Synced { offers, pulled, .. } = b.sync() else {
        panic!();
    };
    assert_eq!(pulled, 2, "known_hosts and .gitconfig");
    assert!(
        offers.is_empty(),
        "a file that makes nothing possible offered {offers:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A commit in a repository of its own at `work`, by git as `m`'s person
/// runs it: their home, their global configuration — what a signing setup
/// that does not hold breaks.
fn commit_as(m: &Machine, work: &Path) -> Result<(), String> {
    std::fs::create_dir_all(work).unwrap();
    std::fs::write(work.join("note"), "a note").unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "-A"],
        &["commit", "-q", "-m", "a commit of the person's"],
    ] {
        let mut command = ronoctua_git::base(&tools(), work, None).unwrap();
        command
            .env("HOME", &m.places.home)
            .env("XDG_CONFIG_HOME", m.places.home.join(".config"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_CONFIG_GLOBAL");
        ronoctua_git::subcommand(&mut command, args[0]).args(&args[1..]);
        let out = command.output().unwrap();
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
    }
    Ok(())
}

/// FAILS IF: a yes to SSH signing on one machine breaks every commit on
/// another that keeps `~/.ssh` "not here" — the settings travelling in
/// `.gitconfig`, which every machine keeps, without the key they name — or
/// a machine that has the key does not sign with it after the sync.
#[test]
fn a_yes_to_signing_travels_with_the_key_alone() {
    let root = scratch("targets-signing");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let mut b = Machine::new(&root, "machine-beta", &remote);
    let c = Machine::new(&root, "machine-gamma", &remote);
    person_files(&a);
    ronoctua_git::set_global(&tools(), &a.places.home, "user.email", "me@example.org").unwrap();
    a.sync();
    // B: `~/.ssh` NOT HERE.
    assert!(b.prefs.stop_keeping("home/.ssh"));
    b.sync();
    assert!(b.read("home/.ssh/id_ed25519.pub").is_none());
    assert!(commit_as(&b, &root.join("work-beta-before")).is_ok());
    // C: brought the key, and says yes.
    let Outcome::Synced { offers, .. } = c.sync() else {
        panic!();
    };
    let [offer @ Offer::SshSigning { .. }] = offers.as_slice() else {
        panic!("{offers:?}");
    };
    crate::offers::accept(offer, &c.places, &tools()).unwrap();
    c.sync();
    // THE OPTIONS THE YES WROTE wait for the person on every other machine
    // (`crate::plain::HELD`), who takes them there.
    a.sync_taking_held();
    b.sync_taking_held();
    let get =
        |m: &Machine, key: &str| ronoctua_git::get_global(&tools(), &m.places.home, key).unwrap();
    // A HAS THE KEY, and signs with it.
    assert_eq!(get(&a, "commit.gpgsign").as_deref(), Some("true"));
    assert_eq!(
        get(&a, "user.signingkey").as_deref(),
        Some("~/.ssh/id_ed25519.pub")
    );
    // B HAS THE INCLUDE and not what it names: git reads nothing from it,
    // and a commit there goes, unsigned.
    assert!(
        b.read("home/.gitconfig")
            .is_some_and(|g| g.contains("git-signing.inc")),
        "the global file did not travel"
    );
    assert!(b.read(crate::offers::SIGNING_INCLUDE).is_none());
    assert!(b.read("home/.ssh/id_ed25519.pub").is_none());
    assert_eq!(get(&b, "commit.gpgsign"), None);
    if let Err(e) = commit_as(&b, &root.join("work-beta")) {
        panic!("a yes on another machine broke every commit here: {e}");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// Every file under `root`, read whole.
fn every_file(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            match std::fs::symlink_metadata(&path) {
                Ok(m) if m.is_dir() => todo.push(path),
                Ok(m) if m.is_file() => {
                    if let Ok(bytes) = std::fs::read(&path) {
                        out.push((path, bytes));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// FAILS IF: the identity a typed passphrase opened is not kept for the
/// session, is written into ANY file — the home, the state, the runtime
/// folder, the clone, the remote — the marker does not say which
/// repository and until when, or a person who keeps none has one kept.
#[test]
fn belongings_identity_never_touches_disk() {
    let root = scratch("identity-disk");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.vault = Some(Memory::default());
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    a.sync();
    let identity = a
        .vault
        .as_ref()
        .unwrap()
        .take(&description(&a.prefs.repository))
        .expect("the identity was not kept for the session");
    assert!(
        identity.starts_with("AGE-SECRET-KEY-PQ-1"),
        "kept: not an identity"
    );
    let files = every_file(&root);
    assert!(files.len() > 20, "the scan read {} files", files.len());
    for (path, bytes) in &files {
        assert!(
            !contains(bytes, identity.as_bytes()),
            "the identity is in {}",
            path.display()
        );
    }
    let marker = Marker::read(&a.places).expect("no marker for the doors");
    assert!(marker.holds(&a.prefs.repository, now_s()));
    assert!(marker.until_s <= now_s() + i64::try_from(crate::unlock::KEPT_SECONDS).unwrap());
    assert!(!marker.holds(&a.prefs.repository, marker.until_s));

    // THE PERSON KEEPS NONE: the next sync forgets it, marker and all.
    a.prefs.keep_unlocked = false;
    a.sync();
    assert!(
        a.vault
            .as_ref()
            .unwrap()
            .take(&description(&a.prefs.repository))
            .is_none()
    );
    assert!(Marker::read(&a.places).is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a sync with the identity the session kept asks for a
/// passphrase; one with nothing kept does not; an identity that no longer
/// opens the repository (another machine migrated it, or made it again) is
/// used, or kept — or one kept for a repository made empty since makes it.
#[test]
fn belongings_a_second_sync_asks_nothing() {
    let root = scratch("second-sync");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.vault = Some(Memory::default());
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    a.sync();
    b.sync();
    b.write("config/theme.json", "{\"theme\":\"lark\"}");
    b.sync();

    // NOTHING TYPED, and the sync goes: the change of B comes here.
    assert!(!a.places.passphrase().exists());
    let done = a.engine().prepare(&mut |_| {});
    assert!(matches!(done, Ok(Done::Synced(_))), "{done:?}");
    assert_eq!(
        a.read("config/theme.json").as_deref(),
        Some("{\"theme\":\"lark\"}")
    );
    assert!(matches!(
        Status::read(&a.places.status()).map(|s| s.outcome),
        Some(Outcome::Synced { .. })
    ));

    // ANOTHER IDENTITY kept (the repository was made again elsewhere):
    // never used, forgotten, and the passphrase asked.
    let (other, _) = Keyring::new(&crate::crypt::passphrase("x".to_owned()), WORK).unwrap();
    let vault = a.vault.as_ref().unwrap();
    vault.keep(
        &description(&a.prefs.repository),
        other.secret().expose_secret(),
        60,
    );
    let refused = a.engine().prepare(&mut |_| {});
    assert_eq!(refused.unwrap_err().why, Why::NoPassphrase);
    assert!(vault.take(&description(&a.prefs.repository)).is_none());
    assert!(
        Marker::read(&a.places).is_none(),
        "the marker outlived the identity"
    );

    // AND NOTHING KEPT asks too; typed, it is kept again.
    let refused = a.engine().prepare(&mut |_| {});
    assert_eq!(refused.unwrap_err().why, Why::NoPassphrase);
    a.sync();
    assert!(vault.take(&description(&a.prefs.repository)).is_some());

    // A REPOSITORY MADE EMPTY since (and this machine's clone gone with
    // it): the kept identity makes nothing.
    std::fs::remove_dir_all(&remote_dir).unwrap();
    std::fs::remove_dir_all(a.places.repo()).unwrap();
    bare(&remote_dir);
    let refused = a.engine().prepare(&mut |_| {});
    assert_eq!(refused.unwrap_err().why, Why::NoPassphrase);
    assert!(vault.take(&description(&a.prefs.repository)).is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: `forget --everywhere` (the end of a session, a sleep) leaves
/// an identity of this user kept, forgets another program's key, or leaves
/// an instance's marker — or a plain `forget` (the queue's, when the person
/// turns `keep_unlocked` off) takes ANOTHER instance's identity or marker:
/// the development loop's copy would make the installed session ask its
/// passphrase again.
#[test]
fn forgetting_takes_this_instance_or_every_one() {
    let root = scratch("forget");
    let here = ronoctua_xdg::name();
    let other = if here == ronoctua_xdg::NAME {
        format!("{}-dev", ronoctua_xdg::NAME)
    } else {
        ronoctua_xdg::NAME.to_owned()
    };
    let run = root.join("run");
    let plant = |vault: &Memory| {
        for name in [&here, &other] {
            vault.keep(&format!("{}{name}:aaaa", crate::unlock::PREFIX), "id", 60);
            let leaf = run.join(format!("{name}-{}", crate::places::RUNTIME_LEAF));
            std::fs::create_dir_all(&leaf).unwrap();
            std::fs::write(leaf.join(crate::places::UNLOCKED_FILE), "{}").unwrap();
        }
        vault.keep("someone-else:cccc", "theirs", 60);
    };
    let marked = |name: &str| {
        run.join(format!("{name}-{}", crate::places::RUNTIME_LEAF))
            .join(crate::places::UNLOCKED_FILE)
            .exists()
    };
    let kept = |vault: &Memory, name: &str| {
        vault
            .take(&format!("{}{name}:aaaa", crate::unlock::PREFIX))
            .is_some()
    };

    // THIS INSTANCE: its key and its marker, and nobody else's.
    let vault = Memory::default();
    plant(&vault);
    assert_eq!(forget_here_under(&vault, &run), 1);
    assert!(!kept(&vault, &here) && !marked(&here));
    assert!(
        kept(&vault, &other) && marked(&other),
        "a plain forget took another instance's identity"
    );
    assert!(vault.take("someone-else:cccc").is_some());

    // EVERYWHERE: every instance's, and still nobody else's.
    let vault = Memory::default();
    plant(&vault);
    assert_eq!(forget_everywhere_under(&vault, &run), 2);
    for name in [&here, &other] {
        assert!(!kept(&vault, name) && !marked(name), "{name} was left");
    }
    assert!(vault.take("someone-else:cccc").is_some());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a target's path past the "large" line is not said — only the
/// person's additions were, and an Emacs configuration carrying a gigabyte
/// said nothing —, an addition of the person's past it stops being said (one
/// under a target's path included, as before the target), or a path that is
/// both an addition and a target's is said twice.
#[test]
fn a_target_past_the_large_line_is_said() {
    let big = kept::LARGE_BYTES + 1;
    let entries: BTreeMap<String, Known> = [
        ("home/.config/emacs/data/big.el", big),
        ("home/.config/emacs/init.el", 10),
        ("home/.ssh/config", 3),
        ("home/videos/a.mkv", big),
        ("home/notes/a.md", 5),
    ]
    .into_iter()
    .map(|(p, size_bytes)| {
        (
            p.to_owned(),
            Known {
                size_bytes,
                ..Known::default()
            },
        )
    })
    .collect();
    let prefs = Belongings {
        extra: vec![
            "home/.config/emacs".to_owned(),
            "home/.config/emacs/data".to_owned(),
            "home/notes".to_owned(),
            "home/videos".to_owned(),
        ],
        ..Belongings::default()
    };
    assert_eq!(
        large_of(&prefs, &entries),
        [
            ("home/.config/emacs".to_owned(), big + 10),
            ("home/.config/emacs/data".to_owned(), big),
            ("home/videos".to_owned(), big),
        ]
    );
}

/// FAILS IF: a machine that takes out an addition over a target (the whole
/// home, added by hand) takes the target's files out of the repository, so
/// that a machine which did not sync while the addition lasted — its base
/// still says the file is the target's, not an addition — deletes its own
/// copy (`~/.ssh/config`): the deletion a88e849 makes, and the guard in
/// `merge::plan` closes. The addition's own files still leave, and are
/// forgotten (kept on disk) elsewhere.
#[test]
fn a_root_taken_out_over_a_target_deletes_nothing_elsewhere() {
    let root = scratch("targets-root-over");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    let counts = |m: &Machine| match m.sync() {
        Outcome::Synced {
            pulled,
            pushed,
            deleted_here,
            deleted_there,
            ..
        } => (pulled, pushed, deleted_here, deleted_there),
        other => panic!("{}: {other:?}", m.name),
    };
    let ssh = "home/.ssh/config";
    let notes = "home/notes/a.md";
    assert!(a.prefs.keep("home/notes"));
    a.write(notes, "a\n");
    a.write(ssh, "Host x\n");
    assert_eq!(counts(&a), (0, 2, 0, 0));
    assert_eq!(counts(&b), (2, 0, 0, 0));
    // A ADDS THE WHOLE HOME in place of its notes, and every file of it
    // travels as an addition now — B, asleep, does not hear of it: its base
    // still says the SSH file is the target's.
    a.prefs.extra = vec!["home".to_owned()];
    assert_eq!(counts(&a), (0, 0, 0, 0));
    let last = Last::read(&a.places.last()).unwrap();
    assert!(last.roots.contains("home"), "{:?}", last.roots);
    assert!(last.entries.get(ssh).is_some_and(|k| k.added));
    let asleep = Last::read(&b.places.last()).unwrap();
    assert!(asleep.entries.get(ssh).is_some_and(|k| !k.added));
    // A TAKES THE HOME OUT: the notes leave the repository, the SSH file
    // stays for the machines that keep the target.
    a.prefs.stop_keeping("home");
    assert_eq!(counts(&a), (0, 0, 0, 1), "the target's file left");
    // B WAKES: nothing deleted, nothing set aside.
    assert_eq!(counts(&b), (0, 0, 0, 0));
    assert_eq!(b.read(ssh).as_deref(), Some("Host x\n"));
    assert_eq!(b.read(notes).as_deref(), Some("a\n"), "forgotten, kept");
    assert!(
        std::fs::read_dir(b.places.replaced()).map_or(true, |mut d| d.next().is_none()),
        "something was set aside on B"
    );
    let c = Machine::new(&root, "machine-gamma", &remote);
    assert_eq!(counts(&c), (1, 0, 0, 0));
    assert_eq!(c.read(ssh).as_deref(), Some("Host x\n"));
    let _ = std::fs::remove_dir_all(root);
}
