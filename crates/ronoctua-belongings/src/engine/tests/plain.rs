//! The plain part, the merges and the installer's verbs, against a real git
//! and a `file://` bare remote — two or three machines, one repository.
//!
//! What these prove: the plain part travels with no passphrase, in clear,
//! only under `plain/`, and never sealed; the person's brain modules go from
//! one machine to another; two machines that changed different fields of a
//! JSON file, or different hunks of a text, are joined without a question,
//! and what clashes is decided field by field or merged one way; a merge
//! never happens without a base, and the base cache is sealed and pruned;
//! a restore into a new home leaves nothing for the first sync to move; and
//! probe, verify and init answer the installer.

use super::*;
use crate::engine::merging::{BASE_SUFFIX, sha256};
use crate::engine::plain::{Opened, copy_out};
use crate::engine::restore::{Initialized, probe_with};
use crate::plain::{ChoiceThere, HeldWhy, Report};
use crate::plan::{DecisionsFile, Merging};
use ronoctua_person::choice::Choice;
use ronoctua_person::declined::Declined;

/// The plain part alone, as `sync-plain` (push) or `plain pull` runs it.
fn plain(m: &Machine, push: bool) -> Report {
    match m.engine().plain_part(push, &mut |_| {}) {
        Ok(report) => report,
        Err(r) => panic!("{}: plain run refused: {r}", m.name),
    }
}

/// What the remote's head holds at `name`, if anything.
fn remote_file(remote_dir: &Path, root: &Path, name: &str) -> Option<Vec<u8>> {
    let (names, bytes) = remote_files(remote_dir, root);
    names
        .iter()
        .position(|n| n == name)
        .map(|at| bytes[at].clone())
}

/// The person's options as `ronoctua-person` writes them.
const OPTIONS: &str =
    "{\n  \"format\": 1,\n  \"ssh-signing\": null,\n  \"binary-cache\": null\n}\n";

/// `options` naming the person's binary cache, read by `visibility` — one
/// field of [`OPTIONS`] changed, for the merges below.
fn with_cache(options: &str, visibility: &str) -> String {
    options.replace(
        "\"binary-cache\": null",
        &format!(
            "\"binary-cache\": {{\"name\": \"someone\", \"public-key\": \"someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=\", \"visibility\": \"{visibility}\"}}"
        ),
    )
}

/// A machine's record, as the machine package publishes it.
const RECORD: &str = "{\n  \"format\": 1,\n  \"last-good\": null,\n  \"trial\": null,\n  \"hold\": null,\n  \"last-apply\": null\n}\n";

/// The decisions file a door writes, whole.
fn decide(m: &Machine, file: &DecisionsFile) {
    write_private(&m.places.decisions(), &serde_json::to_vec(file).unwrap()).unwrap();
}

/// A whole prepare that must wait, and its plan.
fn waiting(m: &Machine) -> Plan {
    m.say(PASSPHRASE);
    match m.engine().prepare(&mut |_| {}) {
        Ok(Done::Waiting(plan)) => plan,
        other => panic!("{}: {other:?}", m.name),
    }
}

/// An apply that must sync.
fn applied(m: &Machine) -> Outcome {
    match m.engine().apply(&mut |_| {}) {
        Ok(Done::Synced(o)) => o,
        other => panic!("{}: {other:?}", m.name),
    }
}

/// FAILS IF: the plain part asks for the passphrase, or a plain run touches
/// the sealed part — `index.age`, `key.age` or a chunk changes when only a
/// machine's record was sent — or another machine cannot take it with no
/// passphrase either, or that run passes for a sync of the sealed part.
#[test]
fn belongings_plain_needs_no_passphrase() {
    let root = scratch("plain-no-passphrase");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}\n");
    a.write("state/person/options.json", OPTIONS);
    a.sync();
    let sealed_of = |names: &[String], bytes: &[Vec<u8>]| -> Vec<(String, Vec<u8>)> {
        names
            .iter()
            .zip(bytes)
            .filter(|(n, _)| !n.starts_with("plain/"))
            .map(|(n, b)| (n.clone(), b.clone()))
            .collect()
    };
    let (names, bytes) = remote_files(&remote_dir, &root);
    let sealed_before = sealed_of(&names, &bytes);
    assert!(sealed_before.iter().any(|(n, _)| n == INDEX_FILE));

    // THE MACHINE WRITES ITS RECORD, AND NOBODY TYPES ANYTHING.
    a.write("state/person/machines/machine-alpha.json", RECORD);
    assert!(!a.places.passphrase().exists());
    let report = plain(&a, true);
    assert_eq!(report.pushed, ["state/person/machines/machine-alpha.json"]);
    assert!(report.conflicts.is_empty());
    assert_eq!(
        remote_file(
            &remote_dir,
            &root,
            "plain/person/machines/machine-alpha.json"
        )
        .as_deref(),
        Some(RECORD.as_bytes()),
        "the record is not there in clear"
    );
    let (names, bytes) = remote_files(&remote_dir, &root);
    assert_eq!(
        sealed_of(&names, &bytes),
        sealed_before,
        "a plain run changed the sealed part"
    );

    // ANOTHER MACHINE, never synced, takes it with no passphrase either —
    // and its sealed part is still never synced. The OPTIONS are not taken:
    // they change how root authenticates, and wait for the person here.
    let b = Machine::new(&root, "machine-beta", &remote);
    let report = plain(&b, false);
    assert_eq!(report.changed, ["state/person/machines/machine-alpha.json"]);
    assert_eq!(report.conflicts, ["state/person/options.json"]);
    assert_eq!(b.read("state/person/options.json"), None);
    let last = Last::read(&b.places.last()).unwrap();
    assert!(
        last.repository.is_empty() && last.entries.is_empty(),
        "a plain run passed for a sync: {last:?}"
    );
    assert_eq!(last.plain.repository, remote);
    assert_eq!(last.plain.files.len(), 1);
    // AND A SECOND RUN MOVES NOTHING, and still holds the options.
    let again = plain(&b, true);
    assert!(
        again.changed.is_empty() && again.pushed.is_empty(),
        "{again:?}"
    );
    assert_eq!(again.conflicts, ["state/person/options.json"]);
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a plain file is sealed too, a sealed file shows in clear, a
/// draft, a link or a file past `plainMostKiB` travels in the plain part, or
/// what was left out for its size is not named.
#[test]
fn belongings_plain_is_only_plain() {
    let root = scratch("plain-only");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"sealed-needle\"}\n");
    a.write("state/person/options.json", OPTIONS);
    a.write("config/brain-modules/my-tea/module.rs", "pub fn tea() {}\n");
    a.write("state/person/.options.json.swp", "an editor's swap");
    a.write("state/person/software/declared.json.77.new", "{");
    std::os::unix::fs::symlink(
        "options.json",
        a.places.resolve("state/person/latest").unwrap(),
    )
    .unwrap();
    let big = a
        .places
        .resolve("state/person/software/icons/big.png")
        .unwrap();
    std::fs::create_dir_all(big.parent().unwrap()).unwrap();
    std::fs::write(
        &big,
        vec![7u8; usize::try_from(kept::PLAIN_MOST_BYTES).unwrap() + 1],
    )
    .unwrap();
    a.sync();

    let (names, bytes) = remote_files(&remote_dir, &root);
    let plain_names: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| n.starts_with("plain/"))
        .collect();
    assert_eq!(
        plain_names,
        [
            "plain/brain-modules/my-tea/module.rs",
            "plain/person/options.json",
            "plain/signed.json",
            "plain/signed.sig",
            "plain/signer.pub",
        ],
        "a draft, a link or a large file travelled in clear"
    );
    for name in &names {
        let fine = name.starts_with("plain/")
            || [
                KEY_FILE,
                INDEX_FILE,
                FORMAT_FILE,
                RECOVER_FILE,
                ATTRIBUTES_FILE,
            ]
            .contains(&name.as_str())
            || crate::chunks::id_of(name).is_some();
        assert!(fine, "the repository names {name}");
    }
    for (name, content) in names.iter().zip(&bytes) {
        assert!(
            !contains(content, b"sealed-needle"),
            "{name} holds a sealed file in clear"
        );
    }
    // NEVER SEALED TOO: the index names no plain path.
    let p = crate::crypt::passphrase(PASSPHRASE.to_owned());
    let (_, index) = a.engine().open_repository(&p).unwrap();
    let sealed: Vec<&str> = index.entries.iter().map(|b| b.path.as_str()).collect();
    assert_eq!(sealed, ["config/theme.json"]);
    let last = Last::read(&a.places.last()).unwrap();
    assert_eq!(
        last.plain.too_large,
        ["state/person/software/icons/big.png"]
    );
    assert!(!last.entries.keys().any(|k| kept::plain_root(k).is_some()));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a brain module the person signed does not reach another
/// machine by itself — pinned, nobody typing anything — an edit there does
/// not go out signed by the identity that machine's session keeps and come
/// back to the first by itself, or a module deleted elsewhere (and signed
/// so) is not deleted here.
#[test]
fn belongings_brain_modules_travel() {
    let root = scratch("brain-modules");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let mut a = Machine::keeping(&root, "machine-alpha", &remote);
    let manifest = "{ id = \"my-tea\"; }\n";
    a.write("config/brain-modules/my-tea/manifest.nix", manifest);
    a.write("config/brain-modules/my-tea/module.rs", "pub fn new() {}\n");
    a.sync();
    assert_eq!(
        remote_file(
            &remote_dir,
            &root,
            "plain/brain-modules/my-tea/manifest.nix"
        )
        .as_deref(),
        Some(manifest.as_bytes())
    );
    // A NEW MACHINE, its key pinned as the installer pins it: its plain
    // pull takes the signed module, and asks nobody.
    let mut b = Machine::keeping(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    let report = plain(&b, false);
    assert!(report.conflicts.is_empty(), "{report:?}");
    assert_eq!(
        report.changed,
        [
            "config/brain-modules/my-tea/manifest.nix",
            "config/brain-modules/my-tea/module.rs"
        ]
    );
    assert_eq!(
        b.read("config/brain-modules/my-tea/manifest.nix")
            .as_deref(),
        Some(manifest)
    );
    // ITS SESSION KEEPS THE IDENTITY once a sync opened it, and an edit goes
    // out signed with no passphrase typed.
    b.sync();
    b.write(
        "config/brain-modules/my-tea/module.rs",
        "pub fn new() { /* from beta */ }\n",
    );
    let report = plain(&b, true);
    assert_eq!(report.pushed, ["config/brain-modules/my-tea/module.rs"]);
    assert!(report.unsigned.is_empty(), "{report:?}");
    // THE FIRST MACHINE takes it by itself too: signed by the person.
    a.pin_from(&remote_dir, &root);
    a.vault = None;
    let report = plain(&a, false);
    assert!(report.conflicts.is_empty(), "{report:?}");
    assert_eq!(
        a.read("config/brain-modules/my-tea/module.rs").as_deref(),
        Some("pub fn new() { /* from beta */ }\n")
    );
    // A MODULE DELETED ELSEWHERE, the deletion signed, is deleted here.
    std::fs::remove_dir_all(b.places.resolve("config/brain-modules/my-tea").unwrap()).unwrap();
    plain(&b, true);
    let report = plain(&a, false);
    assert!(report.conflicts.is_empty(), "{report:?}");
    assert_eq!(a.read("config/brain-modules/my-tea/module.rs"), None);
    let _ = std::fs::remove_dir_all(root);
}

/// Changes the remote's head as whoever can write the repository could —
/// a deploy key leaked, the git host — with no identity and no signing key:
/// `change` edits a clone of it, and the result is committed and pushed.
fn tamper(remote_dir: &Path, root: &Path, change: impl FnOnce(&Path)) {
    let work = root.join("tamper");
    let _ = std::fs::remove_dir_all(&work);
    let mut clone = ronoctua_git::base(&tools(), root, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(remote_dir)
        .arg(&work);
    assert!(clone.output().unwrap().status.success());
    change(&work);
    commit_all(&work, "belongings");
    Repo::discover(&work)
        .unwrap()
        .with_tools(tools())
        .output(
            None,
            "push",
            &["-q", "origin", "HEAD:refs/heads/main"],
            1 << 20,
        )
        .unwrap();
    let _ = std::fs::remove_dir_all(&work);
}

/// The remote's head commit.
fn remote_head(remote_dir: &Path) -> String {
    let mut rev = ronoctua_git::base(&tools(), remote_dir, None).unwrap();
    ronoctua_git::subcommand(&mut rev, "rev-parse").arg("refs/heads/main");
    let out = rev.output().unwrap();
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// FAILS IF: a plain file somebody changed in the repository without the
/// person's key — a declaration, a brain module added, the signature
/// replaced by another key's or taken away — is taken by itself on a pinned
/// machine, or held without its reason; or the person cannot take it on
/// this machine, after which their machine signs it and the others take it.
#[test]
fn belongings_tampered_plain_is_held() {
    let root = scratch("plain-tampered");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    const DECLARED: &str = "state/person/software/declared.json";
    let mine = declarations(&[]);
    a.write(DECLARED, &mine);
    a.sync();
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    assert!(plain(&b, false).conflicts.is_empty());
    assert_eq!(b.read(DECLARED).as_deref(), Some(mine.as_str()));

    // A DECLARATION CHANGED and a MODULE ADDED, the signature left as it was.
    let evil = declarations(&[declared_app("evil", "6.6", "2026-10-11T10:00:00Z")]);
    tamper(&remote_dir, &root, |w| {
        std::fs::write(w.join("plain/person/software/declared.json"), &evil).unwrap();
        std::fs::create_dir_all(w.join("plain/brain-modules/my-evil")).unwrap();
        std::fs::write(w.join("plain/brain-modules/my-evil/module.rs"), "evil()").unwrap();
    });
    for _ in 0..2 {
        let report = plain(&b, true);
        assert!(report.changed.is_empty(), "{report:?}");
        assert_eq!(report.held.get(DECLARED), Some(&HeldWhy::NotAsSigned));
        assert_eq!(
            report.held.get("config/brain-modules/my-evil/module.rs"),
            Some(&HeldWhy::NotAsSigned)
        );
        assert_eq!(b.read(DECLARED).as_deref(), Some(mine.as_str()));
        assert_eq!(b.read("config/brain-modules/my-evil/module.rs"), None);
    }
    let last = Last::read(&b.places.last()).unwrap();
    assert_eq!(last.plain.held.get(DECLARED), Some(&HeldWhy::NotAsSigned));

    // THE SIGNATURE REPLACED by another key's, over the tampered tree.
    let stranger = ronoctua_pq::sign::SigningKey::generate().unwrap();
    tamper(&remote_dir, &root, |w| {
        let files = [
            ("person/software/declared.json", evil.as_bytes().to_vec()),
            ("brain-modules/my-evil/module.rs", b"evil()".to_vec()),
        ]
        .into_iter()
        .map(|(p, b)| (p.to_owned(), ronoctua_pq::sign::sha256_hex(&b)))
        .collect();
        let manifest = ronoctua_pq::sign::Manifest {
            format: ronoctua_pq::sign::MANIFEST_FORMAT,
            counter: 99,
            signed_at_s: 0,
            signer: String::new(),
            files,
        };
        let (json, sig) = ronoctua_pq::sign::sign_manifest(&stranger, &manifest).unwrap();
        std::fs::write(w.join("plain/signed.json"), json).unwrap();
        std::fs::write(w.join("plain/signed.sig"), sig).unwrap();
        std::fs::write(
            w.join("plain/signer.pub"),
            format!("{}\n", stranger.public()),
        )
        .unwrap();
    });
    let report = plain(&b, false);
    assert_eq!(report.held.get(DECLARED), Some(&HeldWhy::BadSignature));
    assert_eq!(b.read(DECLARED).as_deref(), Some(mine.as_str()));

    // AND TAKEN AWAY.
    tamper(&remote_dir, &root, |w| {
        std::fs::remove_file(w.join("plain/signed.json")).unwrap();
        std::fs::remove_file(w.join("plain/signed.sig")).unwrap();
    });
    let report = plain(&b, false);
    assert_eq!(report.held.get(DECLARED), Some(&HeldWhy::Unsigned));

    // THE PERSON, in a sync on the first machine: asked with the reason, no
    // suggestion; what they keep goes up signed by them, and the module
    // they did not take stays out of what they sign.
    let plan = waiting(&a);
    let c = plan.conflicts.iter().find(|c| c.path == DECLARED).unwrap();
    assert!(c.suggested.is_none() && c.merge.is_none(), "{c:?}");
    assert!(c.held.is_some(), "{c:?}");
    let module = plan
        .conflicts
        .iter()
        .find(|c| c.path == "config/brain-modules/my-evil/module.rs")
        .unwrap();
    assert!(module.held.is_some(), "{module:?}");
    decide(
        &a,
        &DecisionsFile::of_paths(
            [
                (DECLARED.to_owned(), Decision::KeepLocal),
                (module.path.clone(), Decision::KeepLocal),
            ]
            .into_iter()
            .collect(),
        ),
    );
    applied(&a);
    assert_eq!(a.read(DECLARED).as_deref(), Some(mine.as_str()));
    assert_eq!(a.read("config/brain-modules/my-evil/module.rs"), None);
    let report = plain(&b, false);
    assert!(report.held.is_empty(), "{report:?}");
    assert_eq!(b.read("config/brain-modules/my-evil/module.rs"), None);
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a machine that signs while a change of the repository's is held
/// signs that change in — laundering it for every other machine — instead
/// of its own file for that path.
#[test]
fn a_signature_never_vouches_for_what_it_held() {
    let root = scratch("plain-launder");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    const DECLARED: &str = "state/person/software/declared.json";
    a.write("state/person/options.json", OPTIONS);
    a.write(DECLARED, &declarations(&[]));
    a.sync();
    tamper(&remote_dir, &root, |w| {
        std::fs::write(w.join("plain/person/options.json"), "{\"format\": 1}\n").unwrap();
    });
    let one = declarations(&[declared_app("one", "1.0", "2026-10-11T10:00:00Z")]);
    a.write(DECLARED, &one);
    let report = plain(&a, true);
    assert_eq!(report.pushed, [DECLARED]);
    assert_eq!(
        report.held.get("state/person/options.json"),
        Some(&HeldWhy::NotAsSigned)
    );
    let mut c = Machine::new(&root, "machine-gamma", &remote);
    c.pin_from(&remote_dir, &root);
    let key = c.pinned.clone().unwrap();
    let json = remote_file(&remote_dir, &root, "plain/signed.json").unwrap();
    let sig = remote_file(&remote_dir, &root, "plain/signed.sig").unwrap();
    let manifest = ronoctua_pq::sign::open_manifest(&json, &sig, &key).unwrap();
    assert_eq!(
        manifest.files["person/options.json"],
        sha256(OPTIONS.as_bytes()),
        "the held change was signed in"
    );
    let report = plain(&c, false);
    assert_eq!(c.read(DECLARED).as_deref(), Some(one.as_str()));
    assert_eq!(c.read("state/person/options.json"), None);
    assert_eq!(
        report.held.get("state/person/options.json"),
        Some(&HeldWhy::NotAsSigned)
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: an older plain part the person really signed, put back in the
/// repository by somebody who kept a copy, is taken over a newer one this
/// machine already took.
#[test]
fn belongings_plain_rollback_is_refused() {
    let root = scratch("plain-rollback");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    const DECLARED: &str = "state/person/software/declared.json";
    let old = declarations(&[declared_app("vulnerable", "1.0", "2026-10-01T10:00:00Z")]);
    let new = declarations(&[declared_app("vulnerable", "1.1", "2026-10-02T10:00:00Z")]);
    a.write(DECLARED, &old);
    a.sync();
    let signed_old = remote_head(&remote_dir);
    a.write(DECLARED, &new);
    assert_eq!(plain(&a, true).pushed, [DECLARED]);
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    plain(&b, false);
    assert_eq!(b.read(DECLARED).as_deref(), Some(new.as_str()));
    let seen = Last::read(&b.places.last()).unwrap().plain.seen;
    assert!(seen.counter >= 2, "{seen:?}");

    // THE OLD SIGNED TREE, played again on top.
    tamper(&remote_dir, &root, |w| {
        let repo = Repo::discover(w).unwrap().with_tools(tools());
        repo.output(
            None,
            "checkout",
            &[signed_old.as_str(), "--", "plain"],
            1 << 20,
        )
        .unwrap();
    });
    let report = plain(&b, false);
    assert_eq!(report.held.get(DECLARED), Some(&HeldWhy::RolledBack));
    assert_eq!(b.read(DECLARED).as_deref(), Some(new.as_str()));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a plain run without the identity sends this machine's change
/// to a signed file unsigned (or signed by nothing it vouched for), does not
/// say it waits, or loses it — or a manifest signed here at a gesture
/// (`sign`) does not carry it up with no identity open, and the other
/// machines take it.
#[test]
fn a_push_of_signed_files_waits_for_an_unlock() {
    let root = scratch("plain-locked");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    const DECLARED: &str = "state/person/software/declared.json";
    a.write(DECLARED, &declarations(&[]));
    a.sync();
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    plain(&b, false);
    let mut a = a;
    a.pin_from(&remote_dir, &root);

    // LOCKED: the declaration and the record — only the record goes.
    let one = declarations(&[declared_app("one", "1.0", "2026-10-11T10:00:00Z")]);
    a.write(DECLARED, &one);
    a.write("state/person/machines/machine-alpha.json", RECORD);
    let before = remote_file(&remote_dir, &root, "plain/signed.json");
    let report = plain(&a, true);
    assert_eq!(report.pushed, ["state/person/machines/machine-alpha.json"]);
    assert_eq!(report.unsigned, [DECLARED]);
    assert_eq!(remote_file(&remote_dir, &root, "plain/signed.json"), before);
    assert_ne!(
        remote_file(&remote_dir, &root, "plain/person/software/declared.json").as_deref(),
        Some(one.as_bytes())
    );
    assert_eq!(a.read(DECLARED).as_deref(), Some(one.as_str()));
    // AND AGAIN: it still waits, and is still this machine's.
    assert_eq!(plain(&a, true).unsigned, [DECLARED]);
    assert_eq!(
        Last::read(&a.places.last()).unwrap().plain.unsigned,
        [DECLARED]
    );
    // A PULL TRIES NOTHING, and the change waits on.
    assert_eq!(plain(&a, false).unsigned, [DECLARED]);

    // SIGNED HERE AT A GESTURE (the passphrase typed for it), no network.
    a.say(PASSPHRASE);
    assert!(matches!(
        a.engine().sign_here().unwrap(),
        crate::engine::SignedHere::Signed { .. }
    ));
    assert!(
        !a.places.passphrase().exists(),
        "the passphrase outlived it"
    );
    assert_eq!(
        a.engine().sign_here().unwrap_err().why,
        Why::NoPassphrase,
        "a second sign asked nobody and found nothing"
    );
    // AND THE PUSH CARRIES WHAT WAS SIGNED, with no identity open.
    let report = plain(&a, true);
    assert_eq!(report.pushed, [DECLARED]);
    assert!(report.unsigned.is_empty(), "{report:?}");
    let report = plain(&b, false);
    assert!(report.held.is_empty(), "{report:?}");
    assert_eq!(b.read(DECLARED).as_deref(), Some(one.as_str()));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: two machines that changed different fields of one file are
/// asked anything, the joined file does not reach both, or a field both
/// changed is not a conflict that says where — decided field by field.
#[test]
fn belongings_json_merges_by_field_in_a_sync() {
    let root = scratch("json-merge");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write("config/display.json", "{\"theme\":\"owl\",\"size\":12}\n");
    a.sync();
    b.sync();
    a.write("config/display.json", "{\"theme\":\"cyber\",\"size\":12}\n");
    a.sync();
    b.write("config/display.json", "{\"theme\":\"owl\",\"size\":14}\n");
    let outcome = b.sync();
    assert!(matches!(outcome, Outcome::Synced { .. }), "{outcome:?}");
    let joined = b.read("config/display.json").unwrap();
    let node = ronoctua_store::json::Node::parse(joined.as_bytes()).unwrap();
    assert_eq!(node.at("/theme").unwrap().to_string(), "\"cyber\"");
    assert_eq!(node.at("/size").unwrap().to_string(), "14");
    a.sync();
    assert_eq!(a.read("config/display.json"), Some(joined));

    // ONE FIELD BOTH CHANGED: a conflict that says where, decided field by
    // field — the other field joined as before.
    a.write("config/display.json", "{\"theme\":\"paper\",\"size\":14}\n");
    a.sync();
    b.write("config/display.json", "{\"theme\":\"rose\",\"size\":16}\n");
    let plan = waiting(&b);
    let c = &plan.conflicts[0];
    let Some(Merging::Json { fields }) = &c.merge else {
        panic!("{c:?}");
    };
    let pointers: Vec<&str> = fields.iter().map(|f| f.pointer.as_str()).collect();
    assert_eq!(pointers, ["/theme"]);
    assert_eq!(fields[0].ours.as_deref(), Some("\"rose\""));
    assert_eq!(fields[0].theirs.as_deref(), Some("\"paper\""));
    let mut file = DecisionsFile::default();
    file.fields
        .entry(c.path.clone())
        .or_default()
        .insert("/theme".to_owned(), Decision::TakeRemote);
    decide(&b, &file);
    applied(&b);
    let node = ronoctua_store::json::Node::parse(b.read("config/display.json").unwrap().as_bytes())
        .unwrap();
    assert_eq!(node.at("/theme").unwrap().to_string(), "\"paper\"");
    assert_eq!(node.at("/size").unwrap().to_string(), "16");
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: two machines that changed different hunks of a text are asked
/// anything, or a hunk both changed is not offered the two merged decisions
/// — and "merge, taking the repository's" does not keep this machine's
/// other hunks.
#[test]
fn belongings_text_merges_clean_hunks() {
    let root = scratch("text-merge");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    assert!(a.prefs.keep("home/notes"));
    let lines = |l: [&str; 7]| l.iter().map(|x| format!("{x}\n")).collect::<String>();
    a.write(
        "home/notes/a.md",
        &lines(["one", "two", "three", "four", "five", "six", "seven"]),
    );
    a.sync();
    b.sync();
    a.write(
        "home/notes/a.md",
        &lines(["ONE", "two", "three", "four", "five", "six", "seven"]),
    );
    a.sync();
    b.write(
        "home/notes/a.md",
        &lines(["one", "two", "three", "four", "five", "six", "SEVEN"]),
    );
    b.sync();
    let both = lines(["ONE", "two", "three", "four", "five", "six", "SEVEN"]);
    assert_eq!(b.read("home/notes/a.md"), Some(both.clone()));
    a.sync();
    assert_eq!(a.read("home/notes/a.md"), Some(both));

    // A HUNK BOTH CHANGED.
    a.write(
        "home/notes/a.md",
        &lines(["ONE", "two", "three", "four-a", "five", "six", "SEVEN"]),
    );
    a.sync();
    b.write(
        "home/notes/a.md",
        &lines(["ONE", "two", "three", "four-b", "five", "six", "seven-b"]),
    );
    let plan = waiting(&b);
    let c = &plan.conflicts[0];
    assert_eq!(c.merge, Some(Merging::Text { hunks: 1 }));
    assert!(Decision::TakeMergedTheirs.allowed(c));
    decide(
        &b,
        &DecisionsFile::of_paths(
            [(c.path.clone(), Decision::TakeMergedTheirs)]
                .into_iter()
                .collect(),
        ),
    );
    applied(&b);
    let merged = lines(["ONE", "two", "three", "four-a", "five", "six", "seven-b"]);
    assert_eq!(b.read("home/notes/a.md"), Some(merged.clone()));
    a.sync();
    assert_eq!(a.read("home/notes/a.md"), Some(merged));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a merge is made with no base — the cache lost, or a machine's
/// first sync — instead of asking.
#[test]
fn belongings_merge_needs_a_base() {
    let root = scratch("merge-base");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    let b = Machine::new(&root, "machine-beta", &remote);
    a.write("config/display.json", "{\"theme\":\"owl\",\"size\":12}\n");
    a.sync();
    b.sync();
    a.write("config/display.json", "{\"theme\":\"cyber\",\"size\":12}\n");
    a.sync();
    b.write("config/display.json", "{\"theme\":\"owl\",\"size\":14}\n");
    std::fs::remove_dir_all(b.places.base_cache()).unwrap();
    let plan = waiting(&b);
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].merge, None, "merged with no base");
    assert!(!Decision::TakeMergedOurs.allowed(&plan.conflicts[0]));
    assert_eq!(
        b.read("config/display.json").as_deref(),
        Some("{\"theme\":\"owl\",\"size\":14}\n"),
        "written while it waits"
    );
    // A FIRST SYNC has no base at all.
    let c = Machine::new(&root, "machine-gamma", &remote);
    c.write("config/display.json", "{\"theme\":\"owl\",\"size\":20}\n");
    let plan = waiting(&c);
    assert_eq!(plan.conflicts[0].merge, None);
    assert_eq!(plan.conflicts[0].suggested, Some(Decision::TakeRemote));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a merge base is kept in clear, outside the belongings' private
/// folder, under a name that is not its content's hash, for a file that is
/// not text, or after `last.json` stops naming it.
#[test]
fn belongings_base_cache_is_sealed() {
    let root = scratch("base-cache");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    assert!(a.prefs.keep("home/notes"));
    a.write("config/theme.json", "{\"theme\":\"base-needle\"}\n");
    a.write("home/notes/a.md", "a note, base-needle\n");
    let binary = a.places.resolve("home/notes/b.bin").unwrap();
    std::fs::write(&binary, [0u8, 1, 2, 3, 0]).unwrap();
    a.sync();
    let dir = a.places.base_cache();
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let last = Last::read(&a.places.last()).unwrap();
    let text_shas: BTreeSet<String> = ["config/theme.json", "home/notes/a.md"]
        .iter()
        .map(|p| last.entries[*p].sha256.clone())
        .collect();
    let kept_now = |dir: &Path| -> BTreeSet<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    };
    let names = kept_now(&dir);
    let wanted: BTreeSet<String> = text_shas
        .iter()
        .map(|s| format!("{s}{BASE_SUFFIX}"))
        .collect();
    assert_eq!(
        names, wanted,
        "the cache is not the text the last sync named"
    );
    let p = crate::crypt::passphrase(PASSPHRASE.to_owned());
    let ring =
        crate::crypt::Keyring::open(&std::fs::read(a.places.repo().join(KEY_FILE)).unwrap(), &p)
            .unwrap();
    for name in &names {
        let sealed = std::fs::read(dir.join(name)).unwrap();
        assert_eq!(ronoctua_pq::sniff(&sealed), ronoctua_pq::Kind::Pq, "{name}");
        assert!(!contains(&sealed, b"base-needle"), "{name} is in clear");
        assert_eq!(
            std::fs::metadata(dir.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let sha = name.strip_suffix(BASE_SUFFIX).unwrap();
        let opened = a.engine().base_of(&ring, sha).unwrap();
        assert_eq!(super::super::merging::sha256(&opened), sha);
    }
    // PRUNED: a file no longer kept takes its base with it.
    std::fs::remove_file(a.places.resolve("home/notes/a.md").unwrap()).unwrap();
    a.sync();
    let theme = format!("{}{BASE_SUFFIX}", last.entries["config/theme.json"].sha256);
    assert_eq!(kept_now(&dir), [theme].into_iter().collect());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a restore into a new home misses a file, a mode, a link, an
/// addition or the plain part, keeps the passphrase, pushes anything — or
/// leaves the first sync of that home something to move.
#[test]
fn belongings_restore_makes_a_home() {
    let root = scratch("restore");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}\n");
    a.write("config/player-keys/tmdb", "a key");
    std::fs::set_permissions(
        a.places.resolve("config/player-keys/tmdb").unwrap(),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(a.prefs.keep("home/notes"));
    a.write("home/notes/diary.md", "dear diary\n");
    std::os::unix::fs::symlink("diary.md", a.places.home.join("notes/latest.md")).unwrap();
    a.write("state/person/options.json", OPTIONS);
    a.write("config/brain-modules/my-tea/module.rs", "pub fn tea() {}\n");
    a.sync();
    let head_before = remote_files(&remote_dir, &root).0;

    // THE NEW HOME, as the installer makes it: its own runtime folder.
    let home = root.join("new/home");
    std::fs::create_dir_all(&home).unwrap();
    let c = Machine {
        name: "machine-gamma",
        places: Places::for_home(&home, &root.join("new/run")),
        prefs: Belongings::with_repository(&remote),
        vault: None,
        pinned: None,
    };
    c.say(PASSPHRASE);
    let outcome = c.engine().restore(&mut |_| {}).unwrap();
    let Outcome::Synced { pulled, pushed, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!((pulled, pushed), (6, 0));
    assert!(
        !c.places.passphrase().exists(),
        "the passphrase outlived it"
    );
    for (logical, text) in [
        ("config/theme.json", "{\"theme\":\"owl\"}\n"),
        ("home/notes/diary.md", "dear diary\n"),
        ("state/person/options.json", OPTIONS),
        ("config/brain-modules/my-tea/module.rs", "pub fn tea() {}\n"),
    ] {
        assert_eq!(c.read(logical).as_deref(), Some(text), "{logical}");
    }
    assert_eq!(
        c.places.config,
        home.join(".config").join(ronoctua_xdg::name()),
        "restored where a session of that home does not look"
    );
    let tmdb = c.places.resolve("config/player-keys/tmdb").unwrap();
    assert_eq!(
        std::fs::metadata(tmdb).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_link(c.places.home.join("notes/latest.md")).unwrap(),
        Path::new("diary.md")
    );
    assert_eq!(remote_files(&remote_dir, &root).0, head_before, "it pushed");

    // AND THE FIRST SYNC OF THE NEW HOME MOVES NOTHING.
    let outcome = c.sync();
    let Outcome::Synced {
        pulled,
        pushed,
        deleted_here,
        deleted_there,
        ..
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!((pulled, pushed, deleted_here, deleted_there), (0, 0, 0, 0));
    let report = plain(&c, true);
    assert!(
        report.changed.is_empty() && report.pushed.is_empty(),
        "{report:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a restore takes a repository whose `FORMAT` is lower than this
/// build's (format 1 was never deployed: it is not ours) or writes a single
/// file of it into the new home.
#[test]
fn a_restore_refuses_a_format_not_its_own() {
    let root = scratch("restore-format");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}\n");
    assert!(matches!(a.sync(), Outcome::Synced { .. }));
    let work = root.join("lower");
    let mut clone = ronoctua_git::base(&tools(), &root, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(&remote_dir)
        .arg(&work);
    assert!(clone.output().unwrap().status.success());
    std::fs::write(work.join(FORMAT_FILE), "ronoctua-belongings 1\n").unwrap();
    commit_all(&work, "belongings");
    Repo::discover(&work)
        .unwrap()
        .with_tools(tools())
        .output(
            None,
            "push",
            &["-q", "origin", "HEAD:refs/heads/main"],
            1 << 20,
        )
        .unwrap();
    let c = Machine::new(&root, "machine-gamma", &remote);
    c.say(PASSPHRASE);
    assert_eq!(
        c.engine().restore(&mut |_| {}).unwrap_err().why,
        Why::NotOurs
    );
    assert_eq!(c.read("config/theme.json"), None, "a file was restored");
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a plain run asks the person, writes over this machine's side of
/// a conflict or sends it — or a sync with the person does not list it and
/// carry the decision.
#[test]
fn a_plain_conflict_waits_for_a_sync_with_the_person() {
    let root = scratch("plain-conflict");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    let b = Machine::keeping(&root, "machine-beta", &remote);
    a.write("state/person/fields.json", OPTIONS);
    a.sync();
    b.sync();
    let theirs = with_cache(OPTIONS, "public");
    let ours = OPTIONS.replace("\"ssh-signing\": null", "\"ssh-signing\": {\"key\": \"x\"}");
    // The cache dropped here while the other machine changes who reads it.
    let both = ours.clone();
    a.write("state/person/fields.json", &theirs);
    plain(&a, true);
    // DIFFERENT FIELDS: joined, no question, even unattended.
    b.write("state/person/fields.json", &ours);
    let report = plain(&b, true);
    assert!(report.conflicts.is_empty(), "{report:?}");
    let joined = b.read("state/person/fields.json").unwrap();
    assert!(joined.contains("\"visibility\": \"public\"") && joined.contains("\"key\": \"x\""));
    plain(&a, false);
    assert_eq!(a.read("state/person/fields.json"), Some(joined.clone()));

    // THE SAME FIELD: left as each side has it, named, its base kept.
    a.write(
        "state/person/fields.json",
        &joined.replace("\"public\"", "\"private\""),
    );
    plain(&a, true);
    b.write("state/person/fields.json", &both);
    let report = plain(&b, true);
    assert_eq!(report.conflicts, ["state/person/fields.json"]);
    assert_eq!(b.read("state/person/fields.json"), Some(both.clone()));
    let there = remote_file(&remote_dir, &root, "plain/person/fields.json").unwrap();
    assert!(String::from_utf8_lossy(&there).contains("\"visibility\": \"private\""));
    let last = Last::read(&b.places.last()).unwrap();
    assert_eq!(last.plain.conflicts, ["state/person/fields.json"]);

    // A SYNC WITH THE PERSON lists it, and carries the decision.
    let plan = waiting(&b);
    let c = plan
        .conflicts
        .iter()
        .find(|c| c.path == "state/person/fields.json")
        .unwrap();
    assert!(matches!(c.merge, Some(Merging::Json { .. })), "{c:?}");
    // NO MACHINE NAMED: no plain commit says which one wrote it, and the
    // last sealed sync's machine is not the one that changed this file.
    assert!(c.machine.is_empty(), "{c:?}");
    decide(
        &b,
        &DecisionsFile::of_paths(
            [(c.path.clone(), Decision::TakeRemote)]
                .into_iter()
                .collect(),
        ),
    );
    applied(&b);
    assert_eq!(
        b.read("state/person/fields.json").as_deref(),
        Some(String::from_utf8_lossy(&there).as_ref())
    );
    let last = Last::read(&b.places.last()).unwrap();
    assert!(last.plain.conflicts.is_empty());
    // WHAT IT OVERWROTE, SET ASIDE.
    let replaced: Vec<PathBuf> = std::fs::read_dir(b.places.replaced())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert!(replaced.iter().any(|r| {
        std::fs::read_to_string(r.join("state/person/fields.json")).ok() == Some(both.clone())
    }));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the person's options, signed, do not reach a pinned machine by
/// itself, or a machine with no key pinned and no identity open takes them
/// (or anything signed) — held, with the reason.
#[test]
fn the_options_arrive_as_the_person_signed_them() {
    let root = scratch("options-signed");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    a.write("state/person/options.json", OPTIONS);
    a.sync();
    let unpinned = Machine::new(&root, "machine-beta", &remote);
    let report = plain(&unpinned, false);
    assert_eq!(
        report.held.get("state/person/options.json"),
        Some(&HeldWhy::NoKey)
    );
    assert_eq!(unpinned.read("state/person/options.json"), None);
    let mut c = Machine::new(&root, "machine-gamma", &remote);
    c.pin_from(&remote_dir, &root);
    assert!(plain(&c, false).conflicts.is_empty());
    assert_eq!(
        c.read("state/person/options.json").as_deref(),
        Some(OPTIONS)
    );
    let on = with_cache(OPTIONS, "private");
    a.write("state/person/options.json", &on);
    assert_eq!(plain(&a, true).pushed, ["state/person/options.json"]);
    let report = plain(&c, true);
    assert!(report.conflicts.is_empty(), "{report:?}");
    assert_eq!(
        c.read("state/person/options.json").as_deref(),
        Some(on.as_str())
    );
    let _ = std::fs::remove_dir_all(root);
}

/// One AppImage of the person's declarations, pinned at `version`.
fn declared_app(id: &str, version: &str, at: &str) -> String {
    format!(
        "{{\"source\": \"appimage\", \"id\": \"appimage:{id}\", \"machines\": null, \"edited-at\": \"{at}\", \"edited-on\": \"laptop\", \"key\": \"{id}\", \"name\": \"{id}\", \"pin\": {{\"url\": \"https://example.org/{id}-{version}.AppImage\", \"hash\": \"sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=\", \"version\": \"{version}\", \"pinned-at\": \"{at}\"}}, \"follows\": null, \"icon\": null}}"
    )
}

/// FAILS IF: two machines refreshing the pin of one declared program — and
/// one of them declaring another — meet as a conflict, the later pin does
/// not win, or the declared program is lost.
#[test]
fn the_persons_declarations_settle_their_pins_without_asking() {
    let root = scratch("declared");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    let b = Machine::keeping(&root, "machine-beta", &remote);
    let file = |apps: &[String]| format!("{{\"format\": 1, \"apps\": [{}]}}\n", apps.join(", "));
    const PATH: &str = "state/person/software/declared.json";
    a.write(
        PATH,
        &file(&[declared_app("one", "1.0", "2026-10-01T10:00:00Z")]),
    );
    plain_first(&a, &b);
    a.write(
        PATH,
        &file(&[declared_app("one", "1.2", "2026-10-04T10:00:00Z")]),
    );
    plain(&a, true);
    b.write(
        PATH,
        &file(&[
            declared_app("one", "1.1", "2026-10-03T10:00:00Z"),
            declared_app("two", "2.0", "2026-10-03T10:00:00Z"),
        ]),
    );
    let report = plain(&b, true);
    assert!(report.conflicts.is_empty(), "{report:?}");
    let merged: ronoctua_person::declared::Declared =
        ronoctua_person::parse(b.read(PATH).unwrap().as_bytes()).unwrap();
    let ids: Vec<&str> = merged.apps.iter().map(|e| e.id()).collect();
    assert_eq!(ids, ["appimage:one", "appimage:two"]);
    assert!(b.read(PATH).unwrap().contains("\"version\": \"1.2\""));
    let _ = std::fs::remove_dir_all(root);
}

/// `a` sends its plain part, and `b` takes it: both on the same base, both
/// sessions keeping the identity their first sync opened.
fn plain_first(a: &Machine, b: &Machine) {
    // THE IDENTITY FIRST: a plain run refuses a repository with none.
    assert_eq!(
        b.engine().plain_part(true, &mut |_| {}).unwrap_err().why,
        Why::Empty
    );
    a.sync();
    b.sync();
}

/// FAILS IF: a probe does not say whether the access reads and writes, or
/// whether the repository is empty — or it writes anything to it.
#[test]
fn a_probe_says_read_write_and_empty() {
    let root = scratch("probe");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    let probed = a.engine().probe().unwrap();
    assert_eq!(
        (probed.read, probed.write, probed.empty),
        (true, true, true)
    );
    assert_eq!(probed.why, None);
    a.write("config/theme.json", "{}\n");
    a.sync();
    let before = remote_files(&remote_dir, &root);
    let probed = a.engine().probe().unwrap();
    assert_eq!(
        (probed.read, probed.write, probed.empty),
        (true, true, false)
    );
    assert_eq!(remote_files(&remote_dir, &root), before, "a probe wrote");
    assert!(
        !a.places.probing().exists(),
        "the scratch outlived the probe"
    );
    // NOWHERE: nothing reads.
    let gone = Machine::new(
        &root,
        "machine-beta",
        &format!("file://{}", root.join("none.git").display()),
    );
    let probed = gone.engine().probe().unwrap();
    assert!(!probed.read && !probed.write);
    assert!(probed.why.is_some());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: `init` does not make an empty repository with the passphrase,
/// pushes over a repository another machine made, or `verify-passphrase`
/// touches anything, keeps a passphrase that does not open, or loses one
/// that does.
#[test]
fn init_makes_an_empty_repository_and_verify_touches_nothing() {
    let root = scratch("init");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.say(PASSPHRASE);
    assert!(matches!(
        a.engine().init(&mut |_| {}),
        Ok(Initialized::Made(Done::Synced(_)))
    ));
    assert!(remote_file(&remote_dir, &root, KEY_FILE).is_some());
    let made = remote_files(&remote_dir, &root);

    let b = Machine::new(&root, "machine-beta", &remote);
    b.say(PASSPHRASE);
    assert_eq!(b.engine().verify_passphrase(&mut |_| {}).unwrap(), 2);
    assert!(
        b.places.passphrase().exists(),
        "the restore's passphrase went"
    );
    std::fs::remove_file(b.places.passphrase()).unwrap();
    b.say("not the passphrase");
    assert_eq!(
        b.engine().verify_passphrase(&mut |_| {}).unwrap_err().why,
        Why::WrongPassphrase
    );
    assert!(
        !b.places.passphrase().exists(),
        "a wrong passphrase was kept"
    );
    // INIT OVER A REPOSITORY ANOTHER MACHINE MADE only verifies.
    b.say(PASSPHRASE);
    assert_eq!(
        b.engine().init(&mut |_| {}).unwrap(),
        Initialized::Verified { format: 2 }
    );
    assert_eq!(
        remote_files(&remote_dir, &root),
        made,
        "init pushed over it"
    );
    assert!(b.places.passphrase().exists());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's copy, given the passphrase, does not hold the
/// plain part to the key `key.age` holds and leave that key for the
/// installer to pin — or copies a plain part that key did not sign; or a
/// restore and an `init` do not leave the key they opened in their runtime
/// folder, or a restore takes a file the signature does not vouch for.
#[test]
fn the_installer_pins_the_key_the_passphrase_opened() {
    let root = scratch("plain-pin");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.say(PASSPHRASE);
    assert!(matches!(
        a.engine().init(&mut |_| {}),
        Ok(Initialized::Made(Done::Synced(_)))
    ));
    let made = std::fs::read_to_string(a.places.runtime.join(crate::engine::SIGNER_OUT)).unwrap();
    let mut probe = Machine::new(&root, "probe", &remote);
    probe.pin_from(&remote_dir, &root);
    let key = probe.pinned.clone().unwrap();
    assert_eq!(made.trim(), key.to_string(), "init left another key");

    a.write("state/person/options.json", OPTIONS);
    a.sync();
    let keys = root.join("keys");
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::write(
        keys.join(crate::places::DEPLOY_KEY),
        "a local remote asks for none",
    )
    .unwrap();
    let copy = |out: &Path, signer: &Path| {
        let pass = root.join("pass");
        say_into(&pass, PASSPHRASE);
        let got = copy_out(
            &remote,
            &keys,
            out,
            None,
            tools(),
            Opened {
                passphrase: Some(&pass),
                signer: Some(signer),
                choice: None,
            },
        );
        assert!(!pass.exists(), "the passphrase outlived the copy");
        got
    };
    let signer = root.join("signer.pub");
    copy(&root.join("staged"), &signer).unwrap();
    assert_eq!(
        std::fs::read_to_string(&signer).unwrap().trim(),
        key.to_string()
    );
    assert!(root.join("staged/signed.json").is_file());
    // A FILE NOT AS SIGNED: left out of the copy, and named.
    tamper(&remote_dir, &root, |w| {
        std::fs::write(w.join("plain/person/options.json"), "{\"format\": 1}").unwrap();
    });
    let report = copy(&root.join("staged-2"), &signer).unwrap();
    assert_eq!(
        report.held.get("state/person/options.json"),
        Some(&HeldWhy::NotAsSigned)
    );
    assert!(!root.join("staged-2/person/options.json").exists());
    // A PLAIN PART THE KEY DID NOT SIGN AT ALL: refused, nothing to pin.
    tamper(&remote_dir, &root, |w| {
        std::fs::remove_file(w.join("plain/signed.sig")).unwrap();
    });
    let _ = std::fs::remove_file(&signer);
    assert_eq!(
        copy(&root.join("staged-3"), &signer).unwrap_err().why,
        Why::Corrupt
    );
    assert!(!signer.exists());

    // A RESTORE into a new home leaves the key, and holds what is not signed.
    let home = root.join("new/home");
    std::fs::create_dir_all(&home).unwrap();
    let c = Machine {
        name: "machine-gamma",
        places: Places::for_home(&home, &root.join("new/run")),
        prefs: Belongings::with_repository(&remote),
        vault: None,
        pinned: None,
    };
    c.say(PASSPHRASE);
    c.engine().restore(&mut |_| {}).unwrap();
    assert_eq!(
        c.read("state/person/options.json"),
        None,
        "a tampered file restored"
    );
    let last = Last::read(&c.places.last()).unwrap();
    assert_eq!(
        last.plain.held.get("state/person/options.json"),
        Some(&HeldWhy::Unsigned),
        "the signature was taken away: nothing of the plain part is vouched for"
    );
    assert_eq!(
        std::fs::read_to_string(c.places.runtime.join(crate::engine::SIGNER_OUT))
            .unwrap()
            .trim(),
        key.to_string()
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's copy of the plain part misses a file, puts it
/// elsewhere than the repository's layout, or brings a sealed one.
#[test]
fn the_plain_part_is_copied_out_as_the_repository_lays_it() {
    let root = scratch("plain-out");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{}\n");
    a.write("state/person/options.json", OPTIONS);
    a.write("config/brain-modules/my-tea/module.rs", "pub fn tea() {}\n");
    a.sync();
    let e = Machine::new(&root, "installer", &remote);
    let out = root.join("staged");
    let report = e.engine().copy_plain(&out, None, None, None).unwrap();
    assert_eq!(
        report.changed,
        [
            "config/brain-modules/my-tea/module.rs",
            "state/person/options.json"
        ]
    );
    assert_eq!(
        std::fs::read_to_string(out.join("person/options.json")).unwrap(),
        OPTIONS
    );
    assert!(out.join("brain-modules/my-tea/module.rs").is_file());
    let mut all = Vec::new();
    for entry in std::fs::read_dir(&out).unwrap().flatten() {
        all.push(entry.file_name().to_string_lossy().into_owned());
    }
    all.sort();
    assert_eq!(
        all,
        ["brain-modules", "person", "signed.json", "signed.sig"]
    );
    let _ = std::fs::remove_dir_all(root);
}

/// The person's declarations, with these programs.
fn declarations(apps: &[String]) -> String {
    format!("{{\"format\": 1, \"apps\": [{}]}}\n", apps.join(", "))
}

/// FAILS IF: what a push sent is not that path's base — a file changed back
/// while the push was on its way comes back as the repository's at the next
/// run (a program the person took back declared again, and installed by the
/// next update) — or the taking back is not sent.
#[test]
fn a_push_is_the_base_of_what_it_sent() {
    let root = scratch("plain-sent");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    const PATH: &str = "state/person/software/declared.json";
    const THERE: &str = "plain/person/software/declared.json";
    let without = declarations(&[]);
    let with = declarations(&[declared_app("x", "1.0", "2026-10-05T10:00:00Z")]);
    a.write(PATH, &without);
    a.sync();
    // THE PERSON DECLARES X, and takes it back while the push is on its way.
    a.write(PATH, &with);
    let back = || a.write(PATH, &without);
    let mut engine = a.engine();
    engine.before_push = Some(&back);
    let report = engine.plain_part(true, &mut |_| {}).unwrap();
    assert_eq!(report.pushed, [PATH]);
    assert_eq!(
        remote_file(&remote_dir, &root, THERE).as_deref(),
        Some(with.as_bytes())
    );
    let last = Last::read(&a.places.last()).unwrap();
    assert_eq!(
        last.plain.files[PATH].sha256,
        sha256(with.as_bytes()),
        "the base is not what the push sent"
    );
    // THE NEXT RUN SENDS THE TAKING BACK, and never brings X back here.
    let report = plain(&a, true);
    assert_eq!(report.pushed, [PATH], "{report:?}");
    assert!(report.changed.is_empty(), "{report:?}");
    assert_eq!(a.read(PATH).as_deref(), Some(without.as_str()));
    assert_eq!(
        remote_file(&remote_dir, &root, THERE).as_deref(),
        Some(without.as_bytes())
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a run writes over a plain file this machine changed after the
/// run read it — a JSON file the software centre writes, a text being
/// edited — or takes the repository's side for its base, so the next run
/// cannot see that both sides changed; in a plain run, and in a sync with
/// the person (where it is also said not written, and nothing of it set
/// aside). Files a run takes by itself: as the person signed them, both
/// sessions keeping the identity.
#[test]
fn a_file_written_meanwhile_is_never_landed_over() {
    let root = scratch("plain-moved");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    let b = Machine::keeping(&root, "machine-beta", &remote);
    const OPTIONS_PATH: &str = "state/person/fields.json";
    const MODULE: &str = "state/person/notes.txt";
    let lines = |l: [&str; 5]| l.iter().map(|x| format!("{x}\n")).collect::<String>();
    a.write(OPTIONS_PATH, OPTIONS);
    a.write(MODULE, &lines(["one", "two", "three", "four", "five"]));
    a.sync();
    b.sync();

    // THE REPOSITORY CHANGES ONE FIELD; THIS MACHINE HAS ITS RECORD TO SEND,
    // and while it goes the software centre writes another field here.
    let theirs = with_cache(OPTIONS, "public");
    b.write(OPTIONS_PATH, &theirs);
    plain(&b, true);
    a.write("state/person/machines/machine-alpha.json", RECORD);
    let ours = OPTIONS.replace("\"ssh-signing\": null", "\"ssh-signing\": {\"key\": \"x\"}");
    let meanwhile = || a.write(OPTIONS_PATH, &ours);
    let mut engine = a.engine();
    engine.before_push = Some(&meanwhile);
    let report = engine.plain_part(true, &mut |_| {}).unwrap();
    assert!(
        report.changed.is_empty(),
        "landed over a file that moved: {report:?}"
    );
    assert_eq!(a.read(OPTIONS_PATH).as_deref(), Some(ours.as_str()));
    let last = Last::read(&a.places.last()).unwrap();
    assert_eq!(
        last.plain.files[OPTIONS_PATH].sha256,
        sha256(OPTIONS.as_bytes()),
        "the base moved with a file that was not written"
    );
    // THE NEXT RUN SEES BOTH SIDES, and joins them field by field.
    let report = plain(&a, true);
    assert!(report.conflicts.is_empty(), "{report:?}");
    let joined = a.read(OPTIONS_PATH).unwrap();
    assert!(
        joined.contains("\"visibility\": \"public\"") && joined.contains("\"key\": \"x\""),
        "{joined}"
    );

    // IN A SYNC WITH THE PERSON: the other machine changed a brain module's
    // first line, and this one edits its last while the sync pushes.
    b.sync();
    b.write(MODULE, &lines(["ONE", "two", "three", "four", "five"]));
    b.sync();
    let editing = || a.write(MODULE, &lines(["one", "two", "three", "four", "FIVE"]));
    let mut engine = a.engine();
    engine.before_push = Some(&editing);
    a.say(PASSPHRASE);
    let outcome = match engine.prepare(&mut |_| {}) {
        Ok(Done::Synced(o)) => o,
        other => panic!("{other:?}"),
    };
    let Outcome::Synced { unwritten, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(unwritten, [MODULE], "not said as not written");
    assert_eq!(
        a.read(MODULE),
        Some(lines(["one", "two", "three", "four", "FIVE"])),
        "a sync landed over a module being edited"
    );
    let replaced = a.places.replaced();
    let aside = std::fs::read_dir(&replaced)
        .map(|d| d.flatten().any(|stamp| stamp.path().join(MODULE).exists()))
        .unwrap_or(false);
    assert!(!aside, "set aside what was never overwritten");
    // AND THE NEXT SYNC JOINS THE TWO EDITS.
    a.sync();
    assert_eq!(
        a.read(MODULE),
        Some(lines(["ONE", "two", "three", "four", "FIVE"]))
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a repository made again, empty, at the same address takes the
/// old plain base for its own — every plain file here deleted as if the
/// repository had deleted it, and never sent to the new one.
#[test]
fn an_empty_repository_again_at_the_same_address_takes_every_plain_file() {
    let root = scratch("plain-again");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    const MODULE: &str = "config/brain-modules/my-tea/module.rs";
    a.write("state/person/options.json", OPTIONS);
    a.write(MODULE, "pub fn tea() {}\n");
    a.sync();
    assert_eq!(Last::read(&a.places.last()).unwrap().plain.files.len(), 2);
    // THE PERSON DELETES THE REPOSITORY AND MAKES IT AGAIN, EMPTY, at the
    // same address — and this machine's clone is made anew.
    std::fs::remove_dir_all(&remote_dir).unwrap();
    assert_eq!(bare(&remote_dir), remote);
    std::fs::remove_dir_all(a.places.repo()).unwrap();
    a.sync();
    assert_eq!(
        a.read("state/person/options.json").as_deref(),
        Some(OPTIONS),
        "deleted here as if the new repository had deleted it"
    );
    assert_eq!(a.read(MODULE).as_deref(), Some("pub fn tea() {}\n"));
    assert_eq!(
        remote_file(&remote_dir, &root, "plain/person/options.json").as_deref(),
        Some(OPTIONS.as_bytes()),
        "never sent to the new repository"
    );
    assert!(remote_file(&remote_dir, &root, "plain/brain-modules/my-tea/module.rs").is_some());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a plain run covers in `status.json` what a run a door started
/// said — a sealed sync's failure (the reminder's mark would go), a plan
/// waiting for the person, what a sync synced and could not write — or
/// says a sync failed when only the plain part was refused, touches what a
/// door typed, or writes anything while another run holds the lock.
#[test]
fn a_plain_run_speaks_only_where_no_door_has() {
    let root = scratch("plain-status");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("state/person/machines/machine-alpha.json", RECORD);
    a.sync();
    let status = |m: &Machine| std::fs::read(m.places.status()).ok();
    let sync_plain = |m: &Machine| m.engine().sync_plain(&mut |_| {});

    // EVERY ENDING A DOOR SHOWS stays as it is.
    let synced = status(&a);
    sync_plain(&a).unwrap();
    assert_eq!(status(&a), synced, "a plain run covered a sync's ending");
    for outcome in [
        Outcome::Refused {
            why: Why::WrongPassphrase,
        },
        Outcome::Waiting { conflicts: 2 },
        Outcome::Abandoned,
    ] {
        write_status(&a.places, 5, outcome.clone());
        let before = status(&a);
        sync_plain(&a).unwrap();
        assert_eq!(status(&a), before, "a plain run covered {outcome:?}");
    }

    // A MACHINE NO DOOR RAN: the plain run says how it ended, and the next
    // one says it again — never a failure.
    let b = Machine::new(&root, "machine-beta", &remote);
    assert_eq!(status(&b), None);
    sync_plain(&b).unwrap();
    let said = Status::read(&b.places.status()).unwrap();
    assert_eq!(
        said.outcome,
        Outcome::PlainSynced {
            pulled: 1,
            pushed: 0
        }
    );
    assert!(!said.failed());
    b.write("state/person/machines/machine-beta.json", RECORD);
    sync_plain(&b).unwrap();
    assert_eq!(
        Status::read(&b.places.status()).unwrap().outcome,
        Outcome::PlainSynced {
            pulled: 0,
            pushed: 1
        }
    );

    // BUSY: another run holds the lock, and nothing is written.
    let before = status(&b);
    let held = crate::running::hold(&b.places).unwrap();
    assert_eq!(sync_plain(&b).unwrap_err().why, Why::Busy);
    drop(held);
    assert_eq!(status(&b), before);

    // REFUSED: not a failed sync, and what a door typed stays.
    let c = Machine::new(
        &root,
        "machine-gamma",
        &format!("file://{}", root.join("none.git").display()),
    );
    c.say(PASSPHRASE);
    assert!(sync_plain(&c).is_err());
    assert_eq!(
        status(&c),
        None,
        "a plain run's refusal passed for a sync's"
    );
    assert!(
        c.places.passphrase().exists(),
        "a plain run took what a door typed"
    );

    // REFUSED BEFORE IT STARTS (an address no order reaches): nothing either.
    let places = Places::under(&root.join("nowhere"));
    std::fs::create_dir_all(&places.config).unwrap();
    std::fs::write(places.prefs(), r#"{"repository":"file:///srv/b.git"}"#).unwrap();
    assert_eq!(
        order(Op::SyncPlain, &places, &mut |_| {}).unwrap_err().why,
        Why::NoRepository
    );
    assert!(!places.status().exists());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's copy of the plain part goes on without the
/// access the keys folder holds for the address's kind, lays it anywhere
/// but as the repository does, brings a sealed file — or leaves its scratch
/// clone beside the folder it fills (beside the target's `/etc/nixos`),
/// whether it ends well or not.
#[test]
fn the_installers_copy_of_the_plain_part_leaves_no_clone_behind() {
    let root = scratch("plain-out-keys");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{}\n");
    a.write("state/person/options.json", OPTIONS);
    a.sync();
    let keys = root.join("keys");
    let target = root.join("target/etc/nixos");
    std::fs::create_dir_all(&target).unwrap();
    let out = target.join("plain");
    let beside = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        copy_out(&remote, &keys, &out, None, tools(), Opened::default())
            .unwrap_err()
            .why,
        Why::NeedsAccess
    );
    assert!(beside(&target).is_empty());
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::write(
        keys.join(crate::places::DEPLOY_KEY),
        "not a key: a local remote asks for none",
    )
    .unwrap();
    let report = copy_out(&remote, &keys, &out, None, tools(), Opened::default()).unwrap();
    assert_eq!(report.changed, ["state/person/options.json"]);
    assert_eq!(
        std::fs::read_to_string(out.join("person/options.json")).unwrap(),
        OPTIONS
    );
    assert_eq!(beside(&out), ["person", "signed.json", "signed.sig"]);
    assert_eq!(
        beside(&target),
        ["plain"],
        "the scratch clone outlived the copy"
    );
    // AN EMPTY REPOSITORY: refused, and its scratch clone gone all the same.
    let empty = bare(&root.join("empty.git"));
    let other = root.join("other");
    std::fs::create_dir_all(&other).unwrap();
    assert_eq!(
        copy_out(
            &empty,
            &keys,
            &other.join("plain"),
            None,
            tools(),
            Opened::default(),
        )
        .unwrap_err()
        .why,
        Why::Empty
    );
    assert!(
        beside(&other).is_empty(),
        "the scratch clone outlived a refusal"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's probe of an address goes on without the
/// credential's file, does not answer read, write and empty with the key it
/// was given, or leaves its scratch repository behind.
#[test]
fn the_installers_probe_takes_the_credential_it_names() {
    let root = scratch("probe-address");
    let remote = bare(&root.join("remote.git"));
    let places = Places::for_home(&root.join("live"), &root.join("run"));
    let key = root.join("keys").join(crate::places::DEPLOY_KEY);
    assert_eq!(
        probe_with(&places, &remote, &key, None, None, tools())
            .unwrap_err()
            .why,
        Why::NeedsAccess
    );
    std::fs::create_dir_all(key.parent().unwrap()).unwrap();
    std::fs::write(&key, "not a key: a local remote asks for none").unwrap();
    let probed = probe_with(&places, &remote, &key, None, None, tools()).unwrap();
    assert_eq!(
        (probed.read, probed.write, probed.empty, probed.why),
        (true, true, true, None)
    );
    assert!(!places.probing().exists(), "the scratch outlived the probe");
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's questions delete the passphrase for a refusal
/// that says nothing about it — a repository they could not reach — or
/// `init` of an empty repository leaves the restore that follows without
/// it, or remembers another machine than the one it was told.
#[test]
fn the_installers_questions_keep_the_passphrase_through_the_network() {
    let root = scratch("asks-kept");
    let gone = Machine::new(
        &root,
        "machine-gamma",
        &format!("file://{}", root.join("none.git").display()),
    );
    gone.say(PASSPHRASE);
    assert!(gone.engine().verify_passphrase(&mut |_| {}).is_err());
    assert!(
        gone.places.passphrase().exists(),
        "verify took it for a repository it never reached"
    );
    assert!(gone.engine().init(&mut |_| {}).is_err());
    assert!(
        gone.places.passphrase().exists(),
        "init took it for a repository it never reached"
    );

    // INIT MAKES IT — as the machine it was told — and the restore that
    // follows still has the passphrase, and is the last to use it.
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "installer-live", &remote);
    a.write("state/person/options.json", OPTIONS);
    a.say(PASSPHRASE);
    let mut engine = a.engine();
    engine.machine = "new-laptop".to_owned();
    assert!(matches!(
        engine.init(&mut |_| {}),
        Ok(Initialized::Made(Done::Synced(_)))
    ));
    assert!(
        a.places.passphrase().exists(),
        "init left the restore without the passphrase"
    );
    let p = crate::crypt::passphrase(PASSPHRASE.to_owned());
    let (_, index) = a.engine().open_repository(&p).unwrap();
    assert_eq!(
        index
            .machines
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["new-laptop"]
    );
    a.engine().restore(&mut |_| {}).unwrap();
    assert!(!a.places.passphrase().exists(), "the restore kept it");
    let _ = std::fs::remove_dir_all(root);
}

/// The person's declined programs, where the plain part keeps them.
const DECLINED: &str = "state/person/software/declined.json";
/// The same, in the repository.
const DECLINED_THERE: &str = "plain/person/software/declined.json";

/// When the installer's person chose.
fn chosen_at() -> ronoctua_person::Stamp {
    ronoctua_person::Stamp::parse("2026-10-20T10:00:00Z").unwrap()
}

/// The installer's choice for `host`: `declined` left out, `kept` kept.
fn choice(host: &str, declined: &[&str], kept: &[&str]) -> Choice {
    let ids = |l: &[&str]| l.iter().map(|&i| i.to_owned()).collect();
    Choice::new(host, chosen_at(), ids(declined), ids(kept))
}

/// A `declined.json` of `host`'s declines, as a machine of the person's
/// writes it.
fn declined_on(host: &str, ids: &[&str]) -> String {
    let mut doc = Declined::default();
    for id in ids {
        doc.decline(id, Some(vec![host.to_owned()]), &chosen_at(), host);
    }
    ronoctua_person::to_json(&doc)
}

/// The remote's manifest, opened with `key`.
fn remote_manifest(
    remote_dir: &Path,
    root: &Path,
    key: &ronoctua_pq::sign::PublicKey,
) -> ronoctua_pq::sign::Manifest {
    let json = remote_file(remote_dir, root, "plain/signed.json").expect("a manifest");
    let sig = remote_file(remote_dir, root, "plain/signed.sig").expect("a signature");
    ronoctua_pq::sign::open_manifest(&json, &sig, key).expect("signed by the person's key")
}

/// Where each decline of the remote's `declined.json` holds, by id.
fn remote_declines(remote_dir: &Path, root: &Path) -> Vec<(String, Option<Vec<String>>)> {
    remote_file(remote_dir, root, DECLINED_THERE)
        .map(|b| ronoctua_person::parse::<Declined>(&b).unwrap())
        .unwrap_or_default()
        .apps
        .into_iter()
        .map(|d| (d.id, d.machines))
        .collect()
}

fn on(hosts: &[&str]) -> Option<Vec<String>> {
    Some(hosts.iter().map(|&h| h.to_owned()).collect())
}

/// FAILS IF: the installer's choice is not said in the repository's
/// declines — another machine's decline lost, ours not scoped to the new
/// machine — not signed by the key the passphrase opened one past the
/// repository's counter, not pushed, or not copied out as pushed; or a
/// machine that pins the key, and one that saw the older counter, do not
/// take it; or the same choice a second time pushes anything; or a choice
/// that ticks now what an earlier attempt left out does not bring it back,
/// or undoes the person's decline of every machine without saying so.
#[test]
fn the_installers_choice_is_signed_pushed_and_copied_out() {
    let root = scratch("plain-decline");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    a.write("state/person/options.json", OPTIONS);
    a.write(DECLINED, &declined_on("machine-alpha", &["gimp"]));
    a.sync();
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    let key = b.pinned.clone().unwrap();
    let before = remote_manifest(&remote_dir, &root, &key).counter;

    let e = Machine::new(&root, "installer", &remote);
    let secret = crate::crypt::passphrase(PASSPHRASE.to_owned());
    let signer = root.join("signer.pub");
    let mine = choice("new-host", &["gimp", "libreoffice"], &["blender"]);
    let out = root.join("staged");
    let report = e
        .engine()
        .copy_plain(&out, Some(&secret), Some(&signer), Some(&mine))
        .unwrap();
    assert_eq!(report.pushed, [DECLINED]);
    assert_eq!(report.choice, Some(ChoiceThere::Pushed));
    assert!(report.held.is_empty(), "{report:?}");
    assert!(report.changed.iter().any(|p| p == DECLINED), "{report:?}");

    // SIGNED ONE PAST, over the new file and what was signed before.
    let there = remote_file(&remote_dir, &root, DECLINED_THERE).unwrap();
    let manifest = remote_manifest(&remote_dir, &root, &key);
    assert_eq!(manifest.counter, before + 1);
    assert_eq!(
        manifest.files.get("person/software/declined.json"),
        Some(&sha256(&there))
    );
    assert_eq!(
        manifest.files.get("person/options.json"),
        Some(&sha256(OPTIONS.as_bytes())),
        "the new signature left a signed file out"
    );
    assert_eq!(
        remote_declines(&remote_dir, &root),
        [
            ("gimp".to_owned(), on(&["machine-alpha", "new-host"])),
            ("libreoffice".to_owned(), on(&["new-host"])),
        ]
    );

    // COPIED OUT AS PUSHED, with the signature that covers it.
    assert_eq!(
        std::fs::read(out.join("person/software/declined.json")).unwrap(),
        there
    );
    assert_eq!(
        std::fs::read(out.join("signed.json")).unwrap(),
        remote_file(&remote_dir, &root, "plain/signed.json").unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(&signer).unwrap().trim(),
        key.to_string()
    );

    // A MACHINE THAT PINS THE KEY, and the one that saw the older counter,
    // take it as signed.
    let report = plain(&b, false);
    assert!(report.held.is_empty(), "{report:?}");
    assert_eq!(
        b.read(DECLINED).map(String::into_bytes),
        Some(there.clone())
    );
    let report = plain(&a, false);
    assert!(report.held.is_empty(), "{report:?}");
    assert_eq!(
        a.read(DECLINED).map(String::into_bytes),
        Some(there.clone())
    );

    // THE SAME CHOICE AGAIN (an installation tried again): nothing pushed.
    let head = remote_head(&remote_dir);
    let again = root.join("staged-again");
    let report = e
        .engine()
        .copy_plain(&again, Some(&secret), Some(&signer), Some(&mine))
        .unwrap();
    assert!(report.pushed.is_empty(), "{report:?}");
    assert_eq!(report.choice, Some(ChoiceThere::AlreadyThere));
    assert_eq!(remote_head(&remote_dir), head, "a second run pushed");
    assert_eq!(
        std::fs::read(again.join("person/software/declined.json")).unwrap(),
        there
    );

    // TICKED NOW what the attempt before left out, and kept what the person
    // declined on every machine (another day, by hand): the first comes
    // back on this machine alone, the second stays, and is said.
    a.write(
        DECLINED,
        &String::from_utf8(there).unwrap().replace(
            "\n  ]\n}",
            ",\n    {\n      \"id\": \"vlc\",\n      \"machines\": null,\n      \"edited-at\": \"2026-10-21T10:00:00Z\",\n      \"edited-on\": \"machine-alpha\"\n    }\n  ]\n}",
        ),
    );
    a.sync();
    let ticked = choice("new-host", &["gimp"], &["libreoffice", "vlc"]);
    let report = e
        .engine()
        .copy_plain(
            &root.join("staged-ticked"),
            Some(&secret),
            Some(&signer),
            Some(&ticked),
        )
        .unwrap();
    assert_eq!(report.choice, Some(ChoiceThere::Pushed));
    assert_eq!(report.kept_everywhere, ["vlc"]);
    assert_eq!(
        remote_declines(&remote_dir, &root),
        [
            ("gimp".to_owned(), on(&["machine-alpha", "new-host"])),
            ("vlc".to_owned(), None),
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer signs its choice over a `declined.json` the
/// person did not sign — laundering it for every machine that pins the key
/// — or over one in a format this machine does not read, instead of
/// refusing with nothing pushed and nothing copied out.
#[test]
fn a_choice_over_a_file_not_as_signed_or_newer_is_refused() {
    let root = scratch("plain-decline-tampered");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write(DECLINED, &declined_on("machine-alpha", &["gimp"]));
    a.sync();
    let evil = declined_on("machine-alpha", &["gimp", "zsh"]);
    tamper(&remote_dir, &root, |w| {
        std::fs::write(w.join(DECLINED_THERE), &evil).unwrap();
    });
    let e = Machine::new(&root, "installer", &remote);
    let refused = |e: &Machine, out: &Path| {
        e.engine()
            .copy_plain(
                out,
                Some(&crate::crypt::passphrase(PASSPHRASE.to_owned())),
                Some(&root.join("signer.pub")),
                Some(&choice("new-host", &["libreoffice"], &[])),
            )
            .unwrap_err()
            .why
    };
    let head = remote_head(&remote_dir);
    let out = root.join("staged");
    assert_eq!(refused(&e, &out), Why::Corrupt);
    assert_eq!(
        remote_head(&remote_dir),
        head,
        "pushed over a tampered file"
    );
    assert!(!out.exists(), "copied out after a refusal");

    // A NEWER FORMAT the person signed on a newer machine, in a repository
    // of its own: not read, not written over.
    let newer_dir = root.join("newer.git");
    let newer = bare(&newer_dir);
    let n = Machine::new(&root, "machine-newer", &newer);
    n.write(DECLINED, "{\"format\": 2, \"apps\": []}\n");
    n.sync();
    let e = Machine::new(&root, "installer-newer", &newer);
    let head = remote_head(&newer_dir);
    let out = root.join("staged-newer");
    assert_eq!(refused(&e, &out), Why::Newer);
    assert_eq!(remote_head(&newer_dir), head, "pushed over a newer file");
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's choice, beaten to the repository by another
/// machine's push of its own declines, is lost or pushed over it instead of
/// said on top of it and signed past its counter — or a race that never
/// ends is not refused after the tries.
#[test]
fn a_choice_that_loses_the_race_goes_again_on_top() {
    let root = scratch("plain-decline-race");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    a.write("state/person/options.json", OPTIONS);
    a.sync();
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    let key = b.pinned.clone().unwrap();
    let before = remote_manifest(&remote_dir, &root, &key).counter;
    let theirs = declined_on("machine-alpha", &["gimp", "libreoffice"]);
    let raced = Cell::new(0);
    let race = || {
        if raced.get() == 0 {
            a.write(DECLINED, &theirs);
            assert_eq!(plain(&a, true).pushed, [DECLINED]);
        }
        raced.set(raced.get() + 1);
    };
    let e = Machine::new(&root, "installer", &remote);
    let secret = crate::crypt::passphrase(PASSPHRASE.to_owned());
    let mut engine = e.engine();
    engine.before_push = Some(&race);
    let out = root.join("staged");
    let report = engine
        .copy_plain(
            &out,
            Some(&secret),
            Some(&root.join("signer.pub")),
            Some(&choice("new-host", &["libreoffice"], &[])),
        )
        .unwrap();
    assert_eq!(raced.get(), 2, "the installer did not try again");
    assert_eq!(report.pushed, [DECLINED]);
    let manifest = remote_manifest(&remote_dir, &root, &key);
    assert_eq!(
        manifest.counter,
        before + 2,
        "not signed past the other push"
    );
    assert_eq!(
        remote_declines(&remote_dir, &root),
        [
            ("gimp".to_owned(), on(&["machine-alpha"])),
            ("libreoffice".to_owned(), on(&["machine-alpha", "new-host"])),
        ],
        "the other machine's declines were lost, or ours"
    );
    assert_eq!(
        std::fs::read(out.join("person/software/declined.json")).unwrap(),
        remote_file(&remote_dir, &root, DECLINED_THERE).unwrap()
    );

    // A RACE THAT NEVER ENDS: refused after the tries, nothing copied out.
    let n = Cell::new(0);
    let forever = || {
        n.set(n.get() + 1);
        a.write(
            "config/brain-modules/my-tea/module.rs",
            &format!("// {}\n", n.get()),
        );
        plain(&a, true);
    };
    let mut engine = e.engine();
    engine.before_push = Some(&forever);
    let never = root.join("staged-never");
    let refused = engine
        .copy_plain(
            &never,
            Some(&secret),
            Some(&root.join("signer.pub")),
            Some(&choice("new-host", &["blender"], &[])),
        )
        .unwrap_err();
    assert_eq!(refused.why, Why::Raced);
    assert_eq!(n.get(), TRIES);
    assert!(!never.exists());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a push of the installer's choice that went in and was reported
/// failed — the round goes again and finds nothing to change — is reported
/// as a choice that never went up, rather than one the repository has.
#[test]
fn a_choice_whose_push_went_in_and_was_reported_failed_is_said_there() {
    let root = scratch("plain-decline-accepted");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::keeping(&root, "machine-alpha", &remote);
    a.write("state/person/options.json", OPTIONS);
    a.sync();
    let e = Machine::new(&root, "installer", &remote);
    // THE PUSH GOES IN — as the installer's own commit — and another
    // machine pushes on top before the answer comes back, so the
    // installer's push is refused and looks lost.
    let once = Cell::new(true);
    let went_in = || {
        if once.replace(false) {
            Repo::discover(&e.places.repo())
                .unwrap()
                .with_tools(tools())
                .output(
                    None,
                    "push",
                    &["-q", "origin", "HEAD:refs/heads/main"],
                    1 << 20,
                )
                .unwrap();
            a.write("state/person/machines/machine-alpha.json", RECORD);
            plain(&a, true);
        }
    };
    let mut engine = e.engine();
    engine.before_push = Some(&went_in);
    let out = root.join("staged");
    let report = engine
        .copy_plain(
            &out,
            Some(&crate::crypt::passphrase(PASSPHRASE.to_owned())),
            Some(&root.join("signer.pub")),
            Some(&choice("new-host", &["libreoffice"], &[])),
        )
        .unwrap();
    assert!(!once.get(), "the push was never tried");
    assert_eq!(report.choice, Some(ChoiceThere::AlreadyThere), "{report:?}");
    assert_eq!(
        remote_declines(&remote_dir, &root),
        [("libreoffice".to_owned(), on(&["new-host"]))]
    );
    assert!(out.join("person/software/declined.json").is_file());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the installer's choice reaches the network when it cannot be
/// said — a file that does not read, the old list of declines in its
/// place, or no passphrase to sign it with — or the passphrase outlives the
/// refusal.
#[test]
fn the_installers_choice_is_read_before_the_network() {
    let root = scratch("plain-decline-read");
    // NO REPOSITORY AT ALL: a refusal that reached the network would be
    // another one.
    let remote = format!("file://{}", root.join("none.git").display());
    let keys = root.join("keys");
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::write(
        keys.join(crate::places::DEPLOY_KEY),
        "a local remote asks for none",
    )
    .unwrap();
    let pass = root.join("pass");
    let signer = root.join("signer.pub");
    let chosen = root.join("choice.json");
    let out = root.join("staged");
    let copy = |with_passphrase: bool| {
        say_into(&pass, PASSPHRASE);
        let got = copy_out(
            &remote,
            &keys,
            &out,
            None,
            tools(),
            Opened {
                passphrase: with_passphrase.then_some(pass.as_path()),
                signer: with_passphrase.then_some(signer.as_path()),
                choice: Some(&chosen),
            },
        );
        if with_passphrase {
            assert!(!pass.exists(), "the passphrase outlived the refusal");
        }
        got.map(|_| ()).unwrap_err().why
    };
    std::fs::write(&chosen, "{\"format\": 1, \"machine\": ").unwrap();
    assert_eq!(copy(true), Why::ChoiceUnread, "a file that does not read");
    std::fs::write(&chosen, declined_on("new-host", &["libreoffice"])).unwrap();
    assert_eq!(
        copy(true),
        Why::ChoiceUnread,
        "declines in place of a choice"
    );
    std::fs::write(
        &chosen,
        ronoctua_person::to_json(&choice("new-host", &["libreoffice"], &[])),
    )
    .unwrap();
    assert_eq!(copy(false), Why::NoPassphrase, "a choice no key signs");
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the signature the installer's choice goes up with vouches for
/// a change of the repository's the person never signed — a file changed,
/// one added, one DELETED — so that a machine pinning the key takes it,
/// instead of every such change staying held as it was.
#[test]
fn a_choice_never_signs_what_the_person_did_not() {
    let root = scratch("plain-decline-launder");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    const DECLARED: &str = "state/person/software/declared.json";
    let mine = declarations(&[]);
    a.write("state/person/options.json", OPTIONS);
    a.write(DECLARED, &mine);
    a.sync();
    let mut b = Machine::new(&root, "machine-beta", &remote);
    b.pin_from(&remote_dir, &root);
    let key = b.pinned.clone().unwrap();
    assert!(plain(&b, false).held.is_empty());

    // WHOEVER CAN WRITE THE REPOSITORY deletes the options, changes a
    // declaration and adds a module.
    let evil = declarations(&[declared_app("evil", "6.6", "2026-10-11T10:00:00Z")]);
    tamper(&remote_dir, &root, |w| {
        std::fs::remove_file(w.join("plain/person/options.json")).unwrap();
        std::fs::write(w.join("plain/person/software/declared.json"), &evil).unwrap();
        std::fs::create_dir_all(w.join("plain/brain-modules/my-evil")).unwrap();
        std::fs::write(w.join("plain/brain-modules/my-evil/module.rs"), "evil()").unwrap();
    });
    let e = Machine::new(&root, "installer", &remote);
    let out = root.join("staged");
    let report = e
        .engine()
        .copy_plain(
            &out,
            Some(&crate::crypt::passphrase(PASSPHRASE.to_owned())),
            Some(&root.join("signer.pub")),
            Some(&choice("new-host", &["libreoffice"], &[])),
        )
        .unwrap();
    assert_eq!(report.pushed, [DECLINED]);
    let manifest = remote_manifest(&remote_dir, &root, &key);
    assert_eq!(
        manifest.files.get("person/options.json"),
        Some(&sha256(OPTIONS.as_bytes())),
        "the deletion was signed"
    );
    assert_eq!(
        manifest.files.get("person/software/declared.json"),
        Some(&sha256(mine.as_bytes())),
        "the changed declaration was signed"
    );
    assert!(
        !manifest
            .files
            .contains_key("brain-modules/my-evil/module.rs"),
        "the added module was signed"
    );
    assert!(manifest.files.contains_key("person/software/declined.json"));
    for held in [DECLARED, "config/brain-modules/my-evil/module.rs"] {
        assert_eq!(
            report.held.get(held),
            Some(&HeldWhy::NotAsSigned),
            "{report:?}"
        );
    }
    assert!(!out.join("brain-modules/my-evil/module.rs").exists());

    // A MACHINE THAT PINS THE KEY takes the choice, and holds the rest.
    let report = plain(&b, false);
    assert_eq!(
        b.read("state/person/options.json").as_deref(),
        Some(OPTIONS),
        "an unsigned deletion was taken"
    );
    assert_eq!(b.read(DECLARED).as_deref(), Some(mine.as_str()));
    assert_eq!(b.read("config/brain-modules/my-evil/module.rs"), None);
    assert!(
        b.read(DECLINED).is_some(),
        "the signed choice was not taken"
    );
    assert_eq!(
        report.held.get("state/person/options.json"),
        Some(&HeldWhy::NotAsSigned),
        "{report:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}
