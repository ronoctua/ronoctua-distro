//! The belongings against a real git and a `file://` bare remote: two
//! machines, one repository.
//!
//! What these prove, each with the real `git` of this build
//! (`RONOCTUA_GIT`): a round trip restores content, modes and links; the
//! repository holds no plaintext path, name or content; a wrong passphrase is
//! refused and its file deleted; a change on both sides waits for the person
//! and each decision does what it says (the overwritten side set aside); a
//! deletion travels; a path a person stopped keeping is forgotten, not
//! deleted; a push another machine beat is tried again on top of it, and
//! three in a row are refused; a new passphrase opens what the old one did.
//!
//! And format 2's: no repository file reaches the chunk limit, an edit moves
//! few chunks, chunk names are keyed, a sync sweeps only what it stopped
//! naming, a large push goes in batches with the index last, a repository of
//! another format is refused, nothing is fetched or
//! pushed before the person let the host, and a compaction keeps the content
//! and loses no other machine's push.

use std::cell::Cell;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::fs::PermissionsExt as _;

use super::*;

// THE PLAIN PART, THE MERGES AND THE INSTALLER'S VERBS (the
// belongings-plain-merge package), on the helpers above.
mod plain;
// THE PREDEFINED TARGETS AND THE IDENTITY KEPT FOR THE SESSION (the
// belongings-targets-unlock package), on the helpers above.
mod targets;
// THE LISTING A DOOR DRAWS IS THE WALK A SYNC MAKES (the belongings-view
// package), on the helpers above.
mod listing;
// ONE SEALED FILE FOR THE INSTALLER, and the push token's sealed home (the
// person's binary cache, 2026-10-11), on the helpers above.
mod take;
// THE PERSON'S EMACS CONFIGURATION: one place travels, and what Emacs
// reads elsewhere moves nothing (2026-10-19), on the helpers above.
mod emacs;

/// A vault of a test's own: what a session keeps, in memory — never the
/// kernel's keyring of whoever runs the tests.
#[derive(Debug, Default)]
struct Memory(std::sync::Mutex<BTreeMap<String, String>>);

impl crate::unlock::Vault for Memory {
    fn keep(&self, description: &str, secret: &str, _seconds: u64) -> bool {
        self.0
            .lock()
            .unwrap()
            .insert(description.to_owned(), secret.to_owned());
        true
    }

    fn take(&self, description: &str) -> Option<zeroize::Zeroizing<String>> {
        self.0
            .lock()
            .unwrap()
            .get(description)
            .cloned()
            .map(zeroize::Zeroizing::new)
    }

    fn forget(&self, prefix: &str) -> usize {
        let mut kept = self.0.lock().unwrap();
        let before = kept.len();
        kept.retain(|d, _| !d.starts_with(prefix));
        before - kept.len()
    }
}

/// A small scrypt, so a test does not pay a second per sync.
const WORK: Option<u8> = Some(10);

/// The passphrase both machines type.
const PASSPHRASE: &str = "a long passphrase, typed twice";

/// The consent of a machine that let no host: a `file://` remote asks none.
fn nothing_let(host: &str) -> Result<ronoctua_consent::Pass, ronoctua_consent::NotLet> {
    Err(ronoctua_consent::NotLet {
        host: host.to_owned(),
    })
}

fn tools() -> Tools {
    Tools::from_env().expect(
        "no git for this build: these tests run the real one — enter the \
         environment (`nix develop`) or run them through the flake",
    )
}

/// A scratch folder of this test, empty.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ronoctua-belongings-engine-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A bare repository at `dir` — the remote both machines share.
fn bare(dir: &Path) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let mut init = ronoctua_git::base(&tools(), dir, None).unwrap();
    ronoctua_git::subcommand(&mut init, "init").args(["-q", "--bare", "-b", BRANCH]);
    assert!(init.output().unwrap().status.success());
    format!("file://{}", dir.display())
}

/// One machine: its places under `root/<name>`, its settings.
struct Machine {
    name: &'static str,
    places: Places,
    prefs: Belongings,
    /// Where its session keeps the identity; `None`: it keeps nothing.
    vault: Option<Memory>,
    /// The signing key root pinned on it; `None`: none is.
    pinned: Option<ronoctua_pq::sign::PublicKey>,
}

impl Machine {
    /// A machine whose session keeps the identity once a sync opened it
    /// (`keep_unlocked`, on by default): its plain runs sign.
    fn keeping(root: &Path, name: &'static str, remote: &str) -> Self {
        Self {
            vault: Some(Memory::default()),
            ..Self::new(root, name, remote)
        }
    }

    fn new(root: &Path, name: &'static str, remote: &str) -> Self {
        let places = Places::under(&root.join(name));
        std::fs::create_dir_all(&places.config).unwrap();
        std::fs::create_dir_all(&places.home).unwrap();
        Self {
            name,
            places,
            prefs: Belongings {
                repository: remote.to_owned(),
                ..Belongings::default()
            },
            vault: None,
            pinned: None,
        }
    }

    fn engine(&self) -> Engine<'_> {
        Engine {
            places: &self.places,
            prefs: &self.prefs,
            tools: tools(),
            credentials: None,
            machine: self.name.to_owned(),
            work: WORK,
            now_s: now_s(),
            before_push: None,
            consent: &nothing_let,
            built_s: Some(1_700_000_000),
            push_most_bytes: kept::PUSH_MOST_BYTES,
            vault: self.vault.as_ref().map(|v| v as &dyn crate::unlock::Vault),
            pinned: self.pinned.clone(),
        }
    }

    /// Pins on this machine the signing key `key.age` of `remote_dir` holds
    /// — what the installer, or the person through the sudo door, does.
    fn pin_from(&mut self, remote_dir: &Path, root: &Path) {
        let (names, bytes) = remote_files(remote_dir, root);
        let at = names.iter().position(|n| n == KEY_FILE).expect("a key.age");
        let ring = Keyring::open(&bytes[at], &crate::crypt::passphrase(PASSPHRASE.to_owned()))
            .expect("the passphrase opens it");
        self.pinned = Some(ring.signing().public().clone());
    }

    /// Writes the passphrase's file the way a door does: `create_new`, `0600`.
    fn say(&self, passphrase: &str) {
        say_into(&self.places.passphrase(), passphrase);
    }

    fn write(&self, logical: &str, text: &str) {
        let p = self.places.resolve(logical).unwrap();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn read(&self, logical: &str) -> Option<String> {
        std::fs::read_to_string(self.places.resolve(logical).unwrap()).ok()
    }

    /// A whole sync with no conflict expected.
    fn sync(&self) -> Outcome {
        self.say(PASSPHRASE);
        match self.engine().prepare(&mut |_| {}) {
            Ok(Done::Synced(o)) => o,
            other => panic!("{}: {other:?}", self.name),
        }
    }

    /// A whole sync whose only questions are the repository's changes to
    /// signed files the signature did not vouch for (`Conflict::held`), each
    /// asked with no suggestion and taken here — the person's decision,
    /// which no run takes for them.
    fn sync_taking_held(&self) -> Outcome {
        self.say(PASSPHRASE);
        let plan = match self.engine().prepare(&mut |_| {}) {
            Ok(Done::Synced(o)) => return o,
            Ok(Done::Waiting(plan)) => plan,
            other => panic!("{}: {other:?}", self.name),
        };
        assert!(
            plan.conflicts
                .iter()
                .all(|c| c.held.is_some() && c.suggested.is_none()),
            "{}: {plan:?}",
            self.name
        );
        let file = crate::plan::DecisionsFile::of_paths(
            plan.conflicts
                .iter()
                .map(|c| (c.path.clone(), Decision::TakeRemote))
                .collect(),
        );
        write_private(
            &self.places.decisions(),
            &serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
        match self.engine().apply(&mut |_| {}) {
            Ok(Done::Synced(o)) => o,
            other => panic!("{}: {other:?}", self.name),
        }
    }
}

fn say_into(path: &Path, text: &str) {
    make_private_dir(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(text.as_bytes()).unwrap();
}

/// Every file of the remote's head, and every name in its tree.
fn remote_files(remote_dir: &Path, root: &Path) -> (Vec<String>, Vec<Vec<u8>>) {
    let look = root.join("look");
    let _ = std::fs::remove_dir_all(&look);
    std::fs::create_dir_all(&look).unwrap();
    let mut clone = ronoctua_git::base(&tools(), &look, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(remote_dir)
        .arg(look.join("w"));
    let cloned = clone.output().unwrap();
    assert!(
        cloned.status.success(),
        "{}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let w = look.join("w");
    let repo = Repo::discover(&w).unwrap().with_tools(tools());
    let names = repo
        .output(None, "ls-tree", &["-r", "--name-only", "HEAD"], 1 << 20)
        .unwrap();
    let names: Vec<String> = String::from_utf8_lossy(&names)
        .lines()
        .map(str::to_owned)
        .collect();
    let bytes = names
        .iter()
        .map(|n| std::fs::read(w.join(n)).unwrap())
        .collect();
    (names, bytes)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// How many chunks the remote's head holds.
fn remote_chunks(remote_dir: &Path, root: &Path) -> usize {
    remote_files(remote_dir, root)
        .0
        .iter()
        .filter(|n| crate::chunks::id_of(n).is_some())
        .count()
}

/// FAILS IF: what one machine keeps does not come back whole on another —
/// content, a private file's `0600`, a link as a link, a folder the person
/// added — or the repository holds a path, a name, a content or the
/// machine's name in the clear.
#[test]
fn a_round_trip_restores_everything_and_the_repository_holds_nothing_in_the_clear() {
    let root = scratch("round");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl-secret-theme\"}");
    a.write("config/player-keys/tmdb", "tmdb-token-plaintext");
    std::fs::set_permissions(
        a.places.resolve("config/player-keys/tmdb").unwrap(),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    a.write("home/notes/diary.md", "dear diary, plaintext-marker");
    std::os::unix::fs::symlink("diary.md", a.places.home.join("notes/latest.md")).unwrap();
    a.write("config/column-player.pid", "12");
    assert!(a.prefs.keep("home/notes"));

    let outcome = a.sync();
    let Outcome::Synced { pushed, kept, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(pushed, 4, "theme, key, diary and the link");
    assert_eq!(kept, 4);
    assert!(
        !a.places.passphrase().exists(),
        "the passphrase outlived the sync"
    );
    assert!(!a.places.session().exists());

    // NOTHING IN THE CLEAR: the tree's names are ours — the three plain files,
    // the key, the index's head and keyed chunk names, and the plain part's
    // signature (its manifest names plain paths only) — and no byte of a
    // path, a content or the machine's name is in any sealed file of it.
    let (names, bytes) = remote_files(&remote_dir, &root);
    let plain: [(&str, String); 3] = [
        (FORMAT_FILE, format!("{}\n", crate::FORMAT)),
        (RECOVER_FILE, RECOVER.to_owned()),
        (ATTRIBUTES_FILE, ATTRIBUTES.to_owned()),
    ];
    let signature = ["plain/signed.json", "plain/signed.sig", "plain/signer.pub"];
    for name in &names {
        let fine = [KEY_FILE, INDEX_FILE].contains(&name.as_str())
            || plain.iter().any(|(p, _)| p == name)
            || signature.contains(&name.as_str())
            || crate::chunks::id_of(name).is_some();
        assert!(fine, "the repository names {name}");
    }
    for (name, text) in &plain {
        let at = names.iter().position(|n| n == name).expect(name);
        assert_eq!(
            bytes[at],
            text.as_bytes(),
            "{name} is not what this build writes"
        );
    }
    for (name, content) in names.iter().zip(&bytes) {
        if crate::chunks::id_of(name).is_some() || [KEY_FILE, INDEX_FILE].contains(&name.as_str()) {
            let kind = ronoctua_pq::sniff(content);
            let sealed = if name == KEY_FILE {
                ronoctua_pq::Kind::Passphrase
            } else {
                ronoctua_pq::Kind::Pq
            };
            assert_eq!(kind, sealed, "{name} is not sealed post-quantum");
        }
    }
    // EIGHT BYTES OR MORE, every needle: a sealed chunk is bytes that look
    // random, and a four-byte word turns up in a few megabytes of them by
    // chance (`tmdb` did, in the `most` of 2026-10-24) — eight bytes, never
    // in any repository a test makes.
    for needle in [
        &b"theme.json"[..],
        b"owl-secret-theme",
        b"player-keys/tmdb",
        b"tmdb-token-plaintext",
        b"diary.md",
        b"dear diary",
        b"plaintext-marker",
        b"home/notes",
        b"machine-alpha",
        b"config/theme",
    ] {
        for (name, content) in names.iter().zip(&bytes) {
            if plain.iter().any(|(p, _)| p == name) {
                continue;
            }
            assert!(
                !contains(content, needle),
                "{name} carries `{}` in the clear",
                String::from_utf8_lossy(needle)
            );
        }
    }
    let repo = Repo::discover(&root.join("look/w"))
        .unwrap()
        .with_tools(tools());
    let log = repo
        .output(None, "log", &["--format=%an|%ae|%cn|%s"], 1 << 20)
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&log).trim(),
        format!("{AUTHOR_NAME}|{AUTHOR_EMAIL}|{AUTHOR_NAME}|belongings"),
        "a commit names more than the belongings"
    );

    // THE OTHER MACHINE, EMPTY, TAKES IT ALL BACK.
    let b = Machine::new(&root, "machine-beta", &remote);
    let outcome = b.sync();
    assert!(
        matches!(outcome, Outcome::Synced { pulled: 4, .. }),
        "{outcome:?}"
    );
    assert_eq!(
        b.read("config/theme.json").as_deref(),
        Some("{\"theme\":\"owl-secret-theme\"}")
    );
    let key = b.places.resolve("config/player-keys/tmdb").unwrap();
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600,
        "a private file came back readable"
    );
    let link = b.places.home.join("notes/latest.md");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("diary.md"));
    assert_eq!(
        b.read("home/notes/diary.md").as_deref(),
        Some("dear diary, plaintext-marker"),
        "a folder another machine added did not travel"
    );
    assert!(b.read("config/column-player.pid").is_none());
    let last = Last::read(&b.places.last()).unwrap();
    assert!(last.entries["home/notes/diary.md"].added);
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a wrong passphrase opens anything, is not said as such, or its
/// file outlives the refusal.
#[test]
fn a_wrong_passphrase_is_refused_and_forgotten() {
    let root = scratch("wrong");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{}");
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.say("not the passphrase");
    assert_eq!(
        b.engine().prepare(&mut |_| {}).unwrap_err().why,
        Why::WrongPassphrase
    );
    assert!(!b.places.passphrase().exists());
    assert!(
        b.read("config/theme.json").is_none(),
        "something was written"
    );
    let status = Status::read(&b.places.status()).unwrap();
    assert_eq!(
        status.outcome,
        Outcome::Refused {
            why: Why::WrongPassphrase
        }
    );
    // AND A PASSPHRASE SOMEBODY ELSE CAN READ is not used at all.
    let file = b.places.passphrase();
    say_into(&file, PASSPHRASE);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        b.engine().prepare(&mut |_| {}).unwrap_err().why,
        Why::NoPassphrase
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a change on both sides is taken without asking, the conflict
/// does not carry its text's other side and diff, a decision does not do
/// what it says, or what it overwrote is not set aside.
#[test]
fn both_changed_waits_for_the_person_and_each_decision_does_what_it_says() {
    let root = scratch("conflict");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}\n");
    a.write("config/player.json", "{\"volume\":1}\n");
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();

    a.write("config/theme.json", "{\"theme\":\"cyber\"}\n");
    a.write("config/player.json", "{\"volume\":2}\n");
    a.sync();
    b.write("config/theme.json", "{\"theme\":\"paper\"}\n");
    b.write("config/player.json", "{\"volume\":3}\n");

    b.say(PASSPHRASE);
    let plan = match b.engine().prepare(&mut |_| {}) {
        Ok(Done::Waiting(plan)) => plan,
        other => panic!("{other:?}"),
    };
    let paths: Vec<&str> = plan.conflicts.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["config/player.json", "config/theme.json"]);
    let theme = &plan.conflicts[1];
    assert!(theme.text);
    assert_eq!(theme.machine, "machine-alpha");
    let theirs = b.places.theirs(theme.theirs.as_deref().unwrap());
    assert_eq!(
        std::fs::read_to_string(&theirs).unwrap(),
        "{\"theme\":\"cyber\"}\n"
    );
    assert_eq!(
        std::fs::metadata(&theirs).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let diff = std::fs::read_to_string(
        b.places
            .theirs(&format!("{}.diff", theme.theirs.as_deref().unwrap())),
    )
    .unwrap();
    assert!(diff.contains("-{\"theme\":\"paper\"}"), "{diff}");
    assert!(diff.contains("+{\"theme\":\"cyber\"}"), "{diff}");
    // THE DIFF OF THE PATH, never of the session's files: git's header named
    // them, under the runtime folder.
    assert!(diff.starts_with("@@"), "{diff}");
    for leak in ["session", "theirs", ".here", "diff --git", "index "] {
        assert!(!diff.contains(leak), "the diff says `{leak}`: {diff}");
    }
    assert!(
        b.places.passphrase().exists(),
        "the passphrase left before apply"
    );
    assert_eq!(
        Status::read(&b.places.status()).unwrap().outcome,
        Outcome::Waiting { conflicts: 2 }
    );
    // NOTHING WAS WRITTEN while it waits.
    assert_eq!(
        b.read("config/theme.json").as_deref(),
        Some("{\"theme\":\"paper\"}\n")
    );

    // UNDECIDED IS REFUSED.
    let mut decisions = Decisions::new();
    decisions.insert("config/theme.json".to_owned(), Decision::TakeRemote);
    write_private(
        &b.places.decisions(),
        &serde_json::to_vec(&decisions).unwrap(),
    )
    .unwrap();
    assert_eq!(
        b.engine().apply(&mut |_| {}).unwrap_err().why,
        Why::Undecided
    );
    decisions.insert("config/player.json".to_owned(), Decision::KeepLocal);
    write_private(
        &b.places.decisions(),
        &serde_json::to_vec(&decisions).unwrap(),
    )
    .unwrap();
    let outcome = match b.engine().apply(&mut |_| {}) {
        Ok(Done::Synced(o)) => o,
        other => panic!("{other:?}"),
    };
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                pulled: 1,
                pushed: 1,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        b.read("config/theme.json").as_deref(),
        Some("{\"theme\":\"cyber\"}\n"),
        "take the repository's did not"
    );
    assert_eq!(
        b.read("config/player.json").as_deref(),
        Some("{\"volume\":3}\n")
    );
    assert!(!b.places.session().exists());
    assert!(!b.places.passphrase().exists());
    assert!(
        !b.places.decisions().exists(),
        "the decisions outlived the sync"
    );
    // WHAT IT OVERWROTE, SET ASIDE.
    let replaced: Vec<PathBuf> = std::fs::read_dir(b.places.replaced())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(replaced.len(), 1);
    assert_eq!(
        std::fs::read_to_string(replaced[0].join("config/theme.json")).unwrap(),
        "{\"theme\":\"paper\"}\n"
    );
    // AND THE OTHER MACHINE TAKES THE ONE KEPT HERE.
    a.sync();
    assert_eq!(
        a.read("config/player.json").as_deref(),
        Some("{\"volume\":3}\n")
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a deletion does not travel, the deleted file is not set aside,
/// or a path a person stopped keeping is deleted anywhere.
#[test]
fn a_deletion_travels_and_what_stops_being_kept_is_only_forgotten() {
    let root = scratch("deletion");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/old.json", "{\"old\":true}");
    a.write("home/notes/a.md", "a");
    assert!(a.prefs.keep("home/notes"));
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    assert_eq!(b.read("home/notes/a.md").as_deref(), Some("a"));

    std::fs::remove_file(a.places.resolve("config/old.json").unwrap()).unwrap();
    let outcome = a.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                deleted_there: 1,
                ..
            }
        ),
        "{outcome:?}"
    );
    let outcome = b.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                deleted_here: 1,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert!(b.read("config/old.json").is_none());
    let aside: Vec<PathBuf> = std::fs::read_dir(b.places.replaced())
        .unwrap()
        .flatten()
        .map(|e| e.path().join("config/old.json"))
        .filter(|p| p.is_file())
        .collect();
    assert_eq!(aside.len(), 1, "the deleted file was not set aside");

    // STOP KEEPING the notes on A — the view's row and the file manager's
    // `C-m k` both reach `stop_keeping` — and write in them afterwards: out
    // of the repository, forgotten on B, whose file stays as it was.
    assert!(a.prefs.stop_keeping("home/notes"));
    a.write("home/notes/a.md", "written after stop keeping");
    let outcome = a.sync();
    assert!(
        matches!(outcome, Outcome::Synced { pushed: 0, .. }),
        "A sent what it stopped keeping: {outcome:?}"
    );
    assert_eq!(
        a.read("home/notes/a.md").as_deref(),
        Some("written after stop keeping"),
        "A deleted its own file"
    );
    assert!(
        !Last::read(&a.places.last())
            .unwrap()
            .entries
            .contains_key("home/notes/a.md"),
        "A still follows it"
    );
    let outcome = b.sync();
    assert!(
        matches!(outcome, Outcome::Synced { pulled: 0, .. }),
        "B took something: {outcome:?}"
    );
    assert_eq!(
        b.read("home/notes/a.md").as_deref(),
        Some("a"),
        "B deleted it, or took what A wrote after stopping"
    );
    assert!(
        !Last::read(&b.places.last())
            .unwrap()
            .entries
            .contains_key("home/notes/a.md"),
        "B still follows it"
    );
    // AND B, which follows nothing of it now, sends nothing of it either.
    b.write("home/notes/b.md", "new on B");
    assert!(matches!(b.sync(), Outcome::Synced { pushed: 0, .. }));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a file created inside a folder a person added on ANOTHER machine
/// is not kept — the folder must travel as a folder, not as the files it had.
#[test]
fn an_added_folder_travels_as_a_folder() {
    let root = scratch("roots");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.write("home/notes/a.md", "a");
    assert!(a.prefs.keep("home/notes"));
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    assert_eq!(b.read("home/notes/a.md").as_deref(), Some("a"));
    assert!(
        Last::read(&b.places.last())
            .unwrap()
            .roots
            .contains("home/notes"),
        "B does not follow the folder"
    );
    b.write("home/notes/from-b.md", "written on B");
    let outcome = b.sync();
    assert!(
        matches!(outcome, Outcome::Synced { pushed: 1, .. }),
        "{outcome:?}"
    );
    a.sync();
    assert_eq!(
        a.read("home/notes/from-b.md").as_deref(),
        Some("written on B"),
        "a file new in an added folder did not travel"
    );
    // A FILE DELETED IN IT travels as a deletion, not as a "forget".
    std::fs::remove_file(a.places.resolve("home/notes/from-b.md").unwrap()).unwrap();
    a.sync();
    let outcome = b.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                deleted_here: 1,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert!(b.read("home/notes/from-b.md").is_none());
    // B STOPS KEEPING IT: out of the repository, since B followed it — and A,
    // which added it ITSELF, puts it back. Then it stays still: B, which no
    // longer follows it, does not take it out again, nor take it in.
    let mut b = b;
    assert!(b.prefs.stop_keeping("home/notes"));
    let outcome = b.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                deleted_there: 1,
                ..
            }
        ),
        "{outcome:?}"
    );
    let outcome = a.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                pushed: 1,
                deleted_here: 0,
                ..
            }
        ),
        "A did not put back what it added: {outcome:?}"
    );
    for (machine, said) in [(&b, "B"), (&a, "A")] {
        let outcome = machine.sync();
        assert!(
            matches!(
                outcome,
                Outcome::Synced {
                    pulled: 0,
                    pushed: 0,
                    deleted_here: 0,
                    deleted_there: 0,
                    ..
                }
            ),
            "{said} keeps moving it: {outcome:?}"
        );
    }
    assert_eq!(b.read("home/notes/a.md").as_deref(), Some("a"));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a folder this machine cannot read is taken for deleted — its
/// files would be deleted from the repository and then on every machine.
#[test]
fn a_folder_this_machine_cannot_read_is_deleted_nowhere() {
    let root = scratch("unreadable");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/studio/scenes.json", "{\"scene\":1}");
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    let studio = a.places.resolve("config/studio").unwrap();
    std::fs::set_permissions(&studio, std::fs::Permissions::from_mode(0o000)).unwrap();
    // ROOT READS EVERYTHING: the test says so instead of passing blind.
    let blind = std::fs::read_dir(&studio).is_err();
    let outcome = a.sync();
    std::fs::set_permissions(&studio, std::fs::Permissions::from_mode(0o700)).unwrap();
    if blind {
        let Outcome::Synced {
            deleted_there,
            unreadable,
            ..
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        assert_eq!(deleted_there, 0, "A deleted what it could not read");
        assert_eq!(unreadable, ["config/studio"]);
    }
    let outcome = b.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                deleted_here: 0,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        b.read("config/studio/scenes.json").as_deref(),
        Some("{\"scene\":1}"),
        "B lost a file A could not read"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: two runs of one machine at once both go — each cleans the
/// other's new blobs out of the clone, and the index pushed names blobs that
/// are not there — or the one refused touches the other's secrets or status.
#[test]
fn two_runs_at_once_are_one_run_and_a_refusal() {
    let root = scratch("concurrent");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    for n in 0..40 {
        a.write(&format!("config/many/f{n}.json"), &format!("{{\"n\":{n}}}"));
    }
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    for n in 0..40 {
        a.write(
            &format!("config/many/f{n}.json"),
            &format!("{{\"n\":{n},\"r\":1}}"),
        );
    }
    // THE SECOND RUN STARTS WHILE THE FIRST IS ABOUT TO PUSH, its new blobs
    // in the clone — the press of `s v s` twice. A lock is per open file, so
    // one process is two runs here as two processes are.
    let second: std::cell::RefCell<Option<Why>> = std::cell::RefCell::new(None);
    let untouched = Cell::new(false);
    let during = || {
        if second.borrow().is_some() {
            return;
        }
        let status = std::fs::read(a.places.status()).ok();
        let other = a.engine().prepare(&mut |_| {});
        *second.borrow_mut() = Some(other.map_or_else(|r| r.why, |_| Why::Failed));
        untouched
            .set(std::fs::read(a.places.status()).ok() == status && a.places.passphrase().exists());
    };
    let mut engine = a.engine();
    engine.before_push = Some(&during);
    a.say(PASSPHRASE);
    let first = engine.prepare(&mut |_| {});
    assert!(matches!(first, Ok(Done::Synced(_))), "{first:?}");
    assert_eq!(
        *second.borrow(),
        Some(Why::Busy),
        "the second run was not refused as busy"
    );
    assert!(
        untouched.get(),
        "the refused run touched the status or the passphrase of the one in flight"
    );
    assert!(
        remote_chunks(&remote_dir, &root) > 40,
        "chunks went missing"
    );
    let outcome = b.sync();
    let Outcome::Synced {
        pulled, unwritten, ..
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!((pulled, unwritten.len()), (40, 0));
    assert_eq!(
        b.read("config/many/f7.json").as_deref(),
        Some("{\"n\":7,\"r\":1}")
    );
    assert!(!a.places.passphrase().exists(), "the first run left it");
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a refusal that comes before a run starts — no access, an
/// address that is not plain, nothing to apply — leaves the passphrase the
/// door wrote in the runtime folder, deletes the one a waiting sync needs, or
/// a run refused as busy touches the secrets or the status of the run in
/// flight.
#[test]
fn every_refusal_forgets_what_was_typed_for_it() {
    let root = scratch("refusals");
    let places = Places::under(&root);
    std::fs::create_dir_all(&places.config).unwrap();
    let set = |text: &str| {
        std::fs::write(places.prefs(), text).unwrap();
    };
    let typed = || say_into(&places.passphrase(), PASSPHRASE);
    let run = |op: Op| order(op, &places, &mut |_| {}).unwrap_err().why;

    set(r#"{"repository":"https://example.org/someone/b.git","auth":"token"}"#);
    typed();
    assert_eq!(run(Op::Prepare), Why::NeedsAccess);
    assert!(!places.passphrase().exists(), "kept after a missing token");
    assert_eq!(
        Status::read(&places.status()).unwrap().outcome,
        Outcome::Refused {
            why: Why::NeedsAccess
        }
    );

    set(r#"{"repository":"file:///srv/b.git"}"#);
    typed();
    say_into(&places.new_passphrase(), "new");
    assert_eq!(run(Op::Rekey), Why::NoRepository);
    assert!(!places.passphrase().exists() && !places.new_passphrase().exists());

    set(r#"{"repository":"https://example.org/someone/b.git","auth":"token"}"#);
    say_into(&places.token(), "token");
    typed();
    assert_eq!(run(Op::Apply), Why::NoSession);
    assert!(!places.passphrase().exists(), "kept with nothing to apply");

    // A WAITING SYNC keeps its passphrase through a refusal of another run.
    write_private(&places.plan(), b"{}").unwrap();
    std::fs::remove_file(places.token()).unwrap();
    typed();
    assert_eq!(run(Op::Prepare), Why::NeedsAccess);
    assert!(
        places.passphrase().exists(),
        "the waiting sync lost its passphrase"
    );

    // BUSY: another run holds the lock, and owns the secrets and the status.
    let before = std::fs::read(places.status()).unwrap();
    let held = crate::running::hold(&places).unwrap();
    assert_eq!(run(Op::Prepare), Why::Busy);
    assert_eq!(run(Op::Abandon), Why::Busy);
    assert!(
        places.passphrase().exists(),
        "a busy refusal took the secret"
    );
    assert_eq!(std::fs::read(places.status()).unwrap(), before);
    drop(held);
    assert_eq!(order(Op::Abandon, &places, &mut |_| {}), Ok(0));
    assert!(!places.passphrase().exists());
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a push another machine beat is lost or forced instead of being
/// made again on top of the other's, or a race that never ends is not
/// refused after three.
#[test]
fn a_push_race_is_won_on_top_of_the_other_and_refused_after_three() {
    let root = scratch("race");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{}");
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();

    // A PUSHES ONCE, right before B does.
    b.write("config/player.json", "{\"from\":\"b\"}");
    let raced = Cell::new(0);
    let race = || {
        if raced.get() == 0 {
            a.write("config/audio.json", "{\"from\":\"a\"}");
            a.sync();
        }
        raced.set(raced.get() + 1);
    };
    let mut engine = b.engine();
    engine.before_push = Some(&race);
    b.say(PASSPHRASE);
    let outcome = match engine.prepare(&mut |_| {}) {
        Ok(Done::Synced(o)) => o,
        other => panic!("{other:?}"),
    };
    assert_eq!(raced.get(), 2, "B did not try again");
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                pulled: 1,
                pushed: 1,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        b.read("config/audio.json").as_deref(),
        Some("{\"from\":\"a\"}")
    );
    a.sync();
    assert_eq!(
        a.read("config/player.json").as_deref(),
        Some("{\"from\":\"b\"}")
    );

    // A RACE THAT NEVER ENDS: refused after three.
    b.write("config/player.json", "{\"from\":\"b again\"}");
    let n = Cell::new(0);
    let forever = || {
        n.set(n.get() + 1);
        a.write("config/audio.json", &format!("{{\"n\":{}}}", n.get()));
        a.sync();
    };
    let mut engine = b.engine();
    engine.before_push = Some(&forever);
    b.say(PASSPHRASE);
    assert_eq!(engine.prepare(&mut |_| {}).unwrap_err().why, Why::Raced);
    assert_eq!(n.get(), TRIES);
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a new passphrase does not open what the old one did, or the old
/// one still does.
#[test]
fn a_new_passphrase_opens_what_the_old_one_did() {
    let root = scratch("rekey");
    let remote = bare(&root.join("remote.git"));
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{}");
    a.sync();
    a.say(PASSPHRASE);
    say_into(&a.places.new_passphrase(), "the new passphrase");
    a.engine().rekey(&mut |_| {}).unwrap();
    assert!(!a.places.new_passphrase().exists() && !a.places.passphrase().exists());
    let b = Machine::new(&root, "machine-beta", &remote);
    b.say(PASSPHRASE);
    assert_eq!(
        b.engine().prepare(&mut |_| {}).unwrap_err().why,
        Why::WrongPassphrase
    );
    b.say("the new passphrase");
    assert!(matches!(
        b.engine().prepare(&mut |_| {}),
        Ok(Done::Synced(_))
    ));
    assert_eq!(b.read("config/theme.json").as_deref(), Some("{}"));
    // AN EMPTY REPOSITORY has no passphrase to change.
    let empty = bare(&root.join("empty.git"));
    let c = Machine::new(&root, "machine-gamma", &empty);
    c.say(PASSPHRASE);
    say_into(&c.places.new_passphrase(), "x");
    assert_eq!(c.engine().rekey(&mut |_| {}).unwrap_err().why, Why::Empty);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_repository_that_is_not_ours_is_refused() {
    let root = scratch("foreign");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    // SOMEBODY ELSE'S REPOSITORY: one commit, no FORMAT.
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let mut init = ronoctua_git::base(&tools(), &work, None).unwrap();
    ronoctua_git::subcommand(&mut init, "init").args(["-q", "-b", BRANCH]);
    assert!(init.output().unwrap().status.success());
    std::fs::write(work.join("README"), "hello").unwrap();
    let repo = Repo::discover(&work).unwrap().with_tools(tools());
    for args in [
        &["config", "user.name", "x"][..],
        &["config", "user.email", "x@example.org"],
        &["config", "commit.gpgsign", "false"],
    ] {
        repo.output(None, args[0], &args[1..], 1 << 20).unwrap();
    }
    repo.output(None, "add", &["-A"], 1 << 20).unwrap();
    repo.output(None, "commit", &["-q", "-m", "x"], 1 << 20)
        .unwrap();
    repo.output(
        None,
        "push",
        &["-q", remote.as_str(), "HEAD:refs/heads/main"],
        1 << 20,
    )
    .unwrap();
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.say(PASSPHRASE);
    assert_eq!(
        a.engine().prepare(&mut |_| {}).unwrap_err().why,
        Why::NotOurs
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_stamps_sort_as_text_and_three_are_kept() {
    assert_eq!(stamp_of(0), "19700101-000000");
    assert_eq!(stamp_of(1_727_712_000), "20240930-160000");
    assert_eq!(stamp_of(951_782_400), "20000229-000000", "a leap day");
    let root = scratch("rotate");
    for s in [5, 1, 4, 2, 3] {
        std::fs::create_dir_all(root.join(stamp_of(s * 86_400))).unwrap();
    }
    rotate_replaced(&root);
    let mut left: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            stamp_of(3 * 86_400),
            stamp_of(4 * 86_400),
            stamp_of(5 * 86_400)
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the deploy key is not made `0600` with its public half, or a
/// second call makes another over it.
#[test]
fn the_deploy_key_is_made_once_and_private() {
    let root = scratch("keygen");
    let places = Places::under(&root);
    assert!(
        std::env::var_os(KEYGEN_VARIABLE).is_some() || option_env!("RONOCTUA_SSH_KEYGEN").is_some(),
        "RONOCTUA_SSH_KEYGEN is not set: this test runs the real ssh-keygen"
    );
    assert!(keygen(&places).unwrap());
    let key = places.deploy_key();
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(places.keys())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let public = std::fs::read_to_string(places.deploy_public()).unwrap();
    assert!(public.starts_with("ssh-ed25519 "), "{public}");
    let first = std::fs::read(&key).unwrap();
    assert!(!keygen(&places).unwrap(), "a second key over the first");
    assert_eq!(std::fs::read(&key).unwrap(), first);
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the belongings reach their repository over SSH without asking
/// for a post-quantum key exchange, or a server that offers none is read as
/// anything but `NotPostQuantum` — the sentence would blame the network.
/// The negotiation itself is proved against a real `sshd` in `ronoctua-git`
/// (`belongings-ssh-is-pq-only`).
#[test]
fn the_belongings_ssh_is_post_quantum_only() {
    let root = scratch("pq-only");
    let places = Places::under(&root);
    crate::secret::make_private_dir(&places.keys()).unwrap();
    crate::secret::write_private(&places.deploy_key(), b"a key").unwrap();
    let prefs = Belongings {
        auth: Auth::Ssh,
        ..Belongings::default()
    };
    match credentials_of(&places, &prefs).unwrap() {
        Credentials::Ssh { pq_only, .. } => assert!(pq_only, "the deploy key's session"),
        other @ Credentials::Token { .. } => panic!("{other:?}"),
    }
    assert_eq!(
        why_of(ronoctua_git::Refusal {
            why: ronoctua_git::Why::NotPostQuantum
        }),
        Why::NotPostQuantum
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the belongings reach their repository over HTTPS with a token
/// without asking for post-quantum TLS — git's curl would agree a classical
/// key with any server. The narrowing itself is proved against a real TLS
/// server in `ronoctua-git` (`belongings-https-is-pq-only`).
#[test]
fn the_belongings_https_is_post_quantum_only() {
    let root = scratch("pq-only-https");
    let places = Places::under(&root);
    crate::secret::make_private_dir(&places.keys()).unwrap();
    crate::secret::write_private(&places.token(), b"a token").unwrap();
    let prefs = Belongings {
        auth: Auth::Token,
        ..Belongings::default()
    };
    match credentials_of(&places, &prefs).unwrap() {
        Credentials::Token { pq_only, .. } => assert!(pq_only, "the token's session"),
        other @ Credentials::Ssh { .. } => panic!("{other:?}"),
    }
    assert_eq!(
        why_of(ronoctua_git::Refusal {
            why: ronoctua_git::Why::NoCommonKeyExchange
        }),
        Why::NotPostQuantum,
        "the belongings always ask for post-quantum"
    );
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------------------
// FORMAT 2 (the belongings-sealed package)
// ---------------------------------------------------------------------------

/// Bytes no chunker finds a pattern in and no compressor shrinks.
fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed | 1;
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.extend_from_slice(&x.to_le_bytes());
    }
    out.truncate(len);
    out
}

/// Writes `len` bytes of noise at `path`, a MiB at a time: a large file is
/// never held whole by the test either.
fn write_noise(path: &Path, seed: u64, len: usize) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = std::fs::File::create(path).unwrap();
    let mut left = len;
    let mut n = seed;
    while left > 0 {
        let take = left.min(1 << 20);
        f.write_all(&noise(n, take)).unwrap();
        n = n.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        left -= take;
    }
}

/// Every file of the remote's head and its size — read from the disk, never
/// into memory (a test's repository can be hundreds of MiB).
fn remote_sizes(remote_dir: &Path, root: &Path) -> Vec<(String, u64)> {
    let look = root.join("look");
    let _ = std::fs::remove_dir_all(&look);
    std::fs::create_dir_all(&look).unwrap();
    let mut clone = ronoctua_git::base(&tools(), &look, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(remote_dir)
        .arg(look.join("w"));
    assert!(clone.output().unwrap().status.success());
    let w = look.join("w");
    let repo = Repo::discover(&w).unwrap().with_tools(tools());
    let names = repo
        .output(None, "ls-tree", &["-r", "--name-only", "HEAD"], 1 << 24)
        .unwrap();
    String::from_utf8_lossy(&names)
        .lines()
        .map(|n| (n.to_owned(), std::fs::metadata(w.join(n)).unwrap().len()))
        .collect()
}

/// The SHA-256 of a file, streamed.
fn sha_of(path: &Path) -> String {
    walk::sha256_of(&mut std::fs::File::open(path).unwrap()).unwrap()
}

/// The names the remote's last commit added or changed (never those it
/// deleted: a sweep's), and every commit's subject, newest first.
fn remote_log(remote_dir: &Path, root: &Path) -> (Vec<String>, Vec<String>) {
    let _ = remote_files(remote_dir, root);
    let repo = Repo::discover(&root.join("look/w"))
        .unwrap()
        .with_tools(tools());
    let added = if repo
        .output(None, "rev-parse", &["--verify", "-q", "HEAD~1"], 1 << 20)
        .is_ok()
    {
        repo.output(
            None,
            "diff",
            &["--name-only", "--diff-filter=AM", "HEAD~1", "HEAD"],
            1 << 24,
        )
        .unwrap()
    } else {
        repo.output(None, "ls-tree", &["-r", "--name-only", "HEAD"], 1 << 24)
            .unwrap()
    };
    let subjects = repo.output(None, "log", &["--format=%s"], 1 << 20).unwrap();
    let lines = |b: &[u8]| {
        String::from_utf8_lossy(b)
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    (lines(&added), lines(&subjects))
}

/// FAILS IF: a file larger than a chunk becomes a repository file larger
/// than a chunk (GitHub refuses one over 100 MB), or does not come back
/// byte for byte on another machine.
fn no_file_reaches_the_limit_with(name: &str, len: usize) {
    let root = scratch(name);
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    let big = a.places.resolve("config/big.bin").unwrap();
    write_noise(&big, 3, len);
    let outcome = a.sync();
    assert!(
        matches!(outcome, Outcome::Synced { pushed: 1, .. }),
        "{outcome:?}"
    );
    let sizes = remote_sizes(&remote_dir, &root);
    let limit = crate::chunks::sealed_most_bytes(kept::CHUNK_MAX_BYTES);
    let (largest_name, largest) = sizes.iter().max_by_key(|(_, b)| *b).unwrap();
    assert!(
        *largest <= limit,
        "{largest_name} is {largest} bytes, past {limit}"
    );
    let chunks = sizes
        .iter()
        .filter(|(n, _)| crate::chunks::id_of(n).is_some())
        .count() as u64;
    assert!(
        chunks >= len as u64 / kept::CHUNK_MAX_BYTES,
        "{chunks} chunks for {len} bytes"
    );
    let b = Machine::new(&root, "machine-beta", &remote);
    let outcome = b.sync();
    assert!(
        matches!(outcome, Outcome::Synced { pulled: 1, .. }),
        "{outcome:?}"
    );
    assert_eq!(
        sha_of(&b.places.resolve("config/big.bin").unwrap()),
        sha_of(&big),
        "the file did not come back whole"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_file_past_a_chunk_is_chunks_under_the_limit() {
    no_file_reaches_the_limit_with("limit-small", 20 << 20);
}

/// The design's own size: 300 MiB. Heavy, so `belongings-no-file-reaches-
/// the-limit` runs it on its own (`--include-ignored`).
#[test]
#[ignore = "300 MiB: run by the check belongings-no-file-reaches-the-limit"]
fn no_file_reaches_the_limit() {
    no_file_reaches_the_limit_with("limit", 300 << 20);
}

/// FAILS IF: one byte changed in a large file sends more than the chunks
/// around it (and the index's), or one content kept twice is two sets of
/// chunks.
fn an_edit_moves_few_chunks_with(name: &str, len: usize) {
    let root = scratch(name);
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    let big = a.places.resolve("config/big.bin").unwrap();
    write_noise(&big, 5, len);
    a.sync();
    let before = remote_chunks(&remote_dir, &root);

    // ONE BYTE, in the middle.
    let mut f = std::fs::OpenOptions::new().write(true).open(&big).unwrap();
    std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(len as u64 / 2)).unwrap();
    f.write_all(b"!").unwrap();
    drop(f);
    a.sync();
    let (added, _) = remote_log(&remote_dir, &root);
    let new_chunks = added
        .iter()
        .filter(|n| crate::chunks::id_of(n).is_some())
        .count();
    assert!(
        (1..=3).contains(&new_chunks),
        "one byte wrote {new_chunks} chunk files: {added:?}"
    );
    // WHAT THE EDIT STOPPED NAMING was swept: the count stays put.
    let after = remote_chunks(&remote_dir, &root);
    assert!(
        after <= before + 1 && after + 1 >= before,
        "{before} chunks became {after}"
    );

    // THE SAME CONTENT TWICE is one set of chunks.
    std::fs::copy(&big, a.places.resolve("config/big-copy.bin").unwrap()).unwrap();
    a.sync();
    let (added, _) = remote_log(&remote_dir, &root);
    let data_chunks = added
        .iter()
        .filter(|n| crate::chunks::id_of(n).is_some())
        .count();
    assert!(
        data_chunks <= 2,
        "a copy wrote {data_chunks} chunk files (only the index's may move)"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_edit_moves_few_chunks_in_eight_mib() {
    an_edit_moves_few_chunks_with("edit-small", 8 << 20);
}

/// The design's own size: 64 MiB, run by `belongings-an-edit-moves-few-chunks`.
#[test]
#[ignore = "64 MiB: run by the check belongings-an-edit-moves-few-chunks"]
fn an_edit_moves_few_chunks() {
    an_edit_moves_few_chunks_with("edit", 64 << 20);
}

/// FAILS IF: two people keeping the same file share a chunk name, or a name
/// is the content's own SHA-256 — either says what a file is to anybody
/// holding a copy of it.
#[test]
fn chunk_ids_are_keyed() {
    let root = scratch("keyed");
    let one = bare(&root.join("one.git"));
    let two = bare(&root.join("two.git"));
    let a = Machine::new(&root, "machine-alpha", &one);
    let b = Machine::new(&root, "machine-beta", &two);
    let theme = "{\"theme\":\"owl\"}\n";
    for m in [&a, &b] {
        m.write("config/theme.json", theme);
        write_noise(&m.places.resolve("config/big.bin").unwrap(), 9, 3 << 20);
        m.sync();
    }
    let ids = |dir: &Path| -> BTreeSet<String> {
        remote_files(dir, &root)
            .0
            .iter()
            .filter_map(|n| crate::chunks::id_of(n).map(str::to_owned))
            .collect()
    };
    let (first, second) = (ids(&root.join("one.git")), ids(&root.join("two.git")));
    assert!(!first.is_empty());
    assert!(
        first.is_disjoint(&second),
        "two identities named a chunk alike"
    );
    // A FILE SMALLER THAN A CHUNK is one chunk, whose name is not its hash.
    let sha = walk::sha256_of(&mut theme.as_bytes()).unwrap();
    assert!(!first.contains(&sha) && !second.contains(&sha));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: new chunks past the push budget go up in one push, the index
/// goes up before every chunk it names is there, or the result does not
/// restore.
#[test]
fn a_large_push_goes_in_batches_and_the_index_last() {
    let root = scratch("batches");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    write_noise(&a.places.resolve("config/big.bin").unwrap(), 13, 12 << 20);
    let mut engine = a.engine();
    engine.push_most_bytes = 4 << 20;
    a.say(PASSPHRASE);
    assert!(matches!(engine.prepare(&mut |_| {}), Ok(Done::Synced(_))));
    let (_, subjects) = remote_log(&remote_dir, &root);
    assert!(
        subjects.len() >= 3,
        "12 MiB under a 4 MiB budget went up in {} pushes",
        subjects.len()
    );
    assert_eq!(subjects[0], COMMIT_MESSAGE, "the index did not go last");
    assert!(subjects[1..].iter().all(|s| s == CHUNKS_MESSAGE));
    // EVERY BATCH BUT THE LAST CARRIES CHUNKS ONLY.
    let repo = Repo::discover(&root.join("look/w"))
        .unwrap()
        .with_tools(tools());
    for back in 1..subjects.len() {
        let touched = repo
            .output(
                None,
                "diff-tree",
                &[
                    "--root",
                    "--no-commit-id",
                    "--name-only",
                    "-r",
                    &format!("HEAD~{back}"),
                ],
                1 << 20,
            )
            .unwrap();
        for name in String::from_utf8_lossy(&touched).lines() {
            assert!(
                crate::chunks::id_of(name).is_some(),
                "a batch carried {name}"
            );
        }
    }
    let b = Machine::new(&root, "machine-beta", &remote);
    assert!(matches!(b.sync(), Outcome::Synced { pulled: 1, .. }));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a sync deletes a chunk it did not name before — another
/// machine's, pushed for an index it never pushed — or keeps one it stopped
/// naming.
#[test]
fn a_sync_sweeps_only_what_it_stopped_naming() {
    let root = scratch("sweep");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"v\":1}");
    a.sync();
    let orphan = plant_orphan(&remote_dir, &root);
    a.write("config/theme.json", "{\"v\":2}");
    a.sync();
    let (names, _) = remote_files(&remote_dir, &root);
    assert!(
        names.contains(&orphan),
        "a sync swept another machine's chunk"
    );
    let ids: Vec<&String> = names
        .iter()
        .filter(|n| crate::chunks::id_of(n).is_some())
        .collect();
    // THE THEME'S ONE CHUNK, THE INDEX'S ONE, AND THE ORPHAN — the first
    // theme's chunk and the first index's were swept.
    assert_eq!(ids.len(), 3, "{ids:?}");
    let _ = std::fs::remove_dir_all(root);
}

/// Pushes a chunk no index names, the way a machine whose push of the
/// chunks went up and whose index never did leaves one. Its path.
fn plant_orphan(remote_dir: &Path, root: &Path) -> String {
    let work = root.join("orphan");
    let _ = std::fs::remove_dir_all(&work);
    let mut clone = ronoctua_git::base(&tools(), root, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(remote_dir)
        .arg(&work);
    assert!(clone.output().unwrap().status.success());
    let id = "ab".repeat(32);
    let rel = crate::chunks::relative(&id).unwrap();
    std::fs::create_dir_all(work.join(&rel).parent().unwrap()).unwrap();
    std::fs::write(
        work.join(&rel),
        b"sealed by a machine that never pushed its index",
    )
    .unwrap();
    commit_all(&work, "belongings: chunks");
    let repo = Repo::discover(&work).unwrap().with_tools(tools());
    repo.output(
        None,
        "push",
        &["-q", "origin", "HEAD:refs/heads/main"],
        1 << 20,
    )
    .unwrap();
    rel
}

/// Commits everything in `work` as a person's git would.
fn commit_all(work: &Path, message: &str) {
    let repo = Repo::discover(work).unwrap().with_tools(tools());
    for args in [
        &["config", "user.name", "x"][..],
        &["config", "user.email", "x@example.org"],
        &["config", "commit.gpgsign", "false"],
        &["add", "-A"],
        &["commit", "-q", "-m", message],
    ] {
        repo.output(None, args[0], &args[1..], 1 << 20).unwrap();
    }
}

/// FAILS IF: a repository whose `FORMAT` is not this build's is synced,
/// compacted or restored — a newer one is not refused as newer (update this
/// machine), or a lower one (format 1 was never deployed) is read as if it
/// were this one instead of refused as not ours.
#[test]
fn a_format_not_its_own_is_refused() {
    let root = scratch("formats");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{}");
    assert!(matches!(a.sync(), Outcome::Synced { .. }));
    let work = root.join("bump");
    let mut clone = ronoctua_git::base(&tools(), &root, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(&remote_dir)
        .arg(&work);
    assert!(clone.output().unwrap().status.success());
    for (n, (format, why)) in [("3", Why::Newer), ("1", Why::NotOurs), ("0", Why::NotOurs)]
        .into_iter()
        .enumerate()
    {
        std::fs::write(
            work.join(FORMAT_FILE),
            format!("ronoctua-belongings {format}\n"),
        )
        .unwrap();
        commit_all(&work, &format!("belongings {n}"));
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
        a.say(PASSPHRASE);
        assert_eq!(
            a.engine().prepare(&mut |_| {}).unwrap_err().why,
            why,
            "a sync of format {format}"
        );
        assert_eq!(
            Status::read(&a.places.status()).unwrap().outcome,
            Outcome::Refused { why },
            "the sentence of format {format}"
        );
        a.say(PASSPHRASE);
        assert_eq!(
            a.engine()
                .compact(kept::KEEP_DAYS, &mut |_| {})
                .unwrap_err()
                .why,
            why,
            "a compaction of format {format}"
        );
        let engine = a.engine();
        let ground = engine.ground().unwrap();
        engine
            .checkout(&ground.repo, ground.head.as_deref().unwrap())
            .unwrap();
        assert_eq!(
            engine
                .open_repository(&crate::crypt::passphrase(PASSPHRASE.to_owned()))
                .unwrap_err()
                .why,
            why,
            "a restore's read of format {format}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: anything is fetched from or pushed to a host the person did
/// not let — or a host let is not asked.
#[test]
fn a_host_not_let_is_never_asked() {
    let root = scratch("consent");
    let remote_dir = root.join("remote.git");
    let local = bare(&remote_dir);
    // A FIRST MACHINE fills the repository through the folder itself: an
    // empty repository would make the next ones start their clone over, and
    // the `insteadOf` below with it.
    let first = Machine::new(&root, "machine-first", &local);
    first.write("config/theme.json", "{\"theme\":\"owl\"}");
    first.sync();
    // THE ADDRESS HAS A HOST, and git reaches it in a folder of the test's:
    // the clone's `insteadOf` turns the one into the other.
    let address = "https://belongings.example/someone/b.git";
    let store = ronoctua_consent::store_at(Some(root.join("consent")));
    let mint = |host: &str| ronoctua_consent::pass_in(&store, host);
    let machine = |name: &'static str| {
        let m = Machine::new(&root, name, address);
        m.engine().fresh_clone().unwrap();
        let repo = Repo::discover(&m.places.repo())
            .unwrap()
            .with_tools(tools());
        repo.output(
            None,
            "config",
            &[&format!("url.{local}.insteadOf"), address],
            1 << 20,
        )
        .unwrap();
        m
    };
    let fetched = |m: &Machine| {
        Repo::discover(&m.places.repo())
            .unwrap()
            .with_tools(tools())
            .output(None, "for-each-ref", &["refs/remotes/"], 1 << 20)
            .unwrap()
    };
    // THE BARE REMOTE'S BRANCHES, asked of it alone (`-C`): a discovery
    // from a bare folder would walk up into whatever repository holds it.
    let remote_head = || {
        let mut refs = ronoctua_git::base(&tools(), &remote_dir, None).unwrap();
        ronoctua_git::subcommand(&mut refs, "for-each-ref").arg("refs/heads/");
        refs.output().unwrap().stdout
    };
    let a = machine("machine-alpha");
    a.write("config/player.json", "{\"volume\":1}");
    let before = remote_head();

    // NOT LET: refused before anything is asked.
    a.say(PASSPHRASE);
    let mut engine = a.engine();
    engine.consent = &mint;
    assert_eq!(engine.prepare(&mut |_| {}).unwrap_err().why, Why::NotLet);
    assert!(fetched(&a).is_empty(), "something was fetched");
    assert_eq!(remote_head(), before, "something was pushed");
    assert!(
        a.read("config/theme.json").is_none(),
        "something was written"
    );
    assert!(!a.places.passphrase().exists());
    assert_eq!(
        Status::read(&a.places.status()).unwrap().outcome,
        Outcome::Refused { why: Why::NotLet }
    );

    // LET: the same sync goes, both ways.
    ronoctua_consent::let_hosts_in(&store, &["belongings.example"], true).unwrap();
    a.say(PASSPHRASE);
    let synced = engine.prepare(&mut |_| {});
    assert!(matches!(synced, Ok(Done::Synced(_))), "{synced:?}");
    assert_ne!(remote_head(), before, "the host let was not asked");
    assert_eq!(
        a.read("config/theme.json").as_deref(),
        Some("{\"theme\":\"owl\"}")
    );

    // TAKEN BACK: the next machine asks nothing, and nothing is written there.
    ronoctua_consent::let_hosts_in(&store, &["belongings.example"], false).unwrap();
    let b = machine("machine-beta");
    b.say(PASSPHRASE);
    let mut engine = b.engine();
    engine.consent = &mint;
    assert_eq!(engine.prepare(&mut |_| {}).unwrap_err().why, Why::NotLet);
    assert!(fetched(&b).is_empty());
    assert!(b.read("config/theme.json").is_none());
    // AND A COMPACTION ASKS THE SAME.
    b.say(PASSPHRASE);
    assert_eq!(engine.compact(0, &mut |_| {}).unwrap_err().why, Why::NotLet);
    assert!(fetched(&b).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

/// The clone's weight, in bytes — the engine's own measure.
fn clone_weight(m: &Machine) -> u64 {
    let repo = Repo::discover(&m.places.repo())
        .unwrap()
        .with_tools(tools());
    crate::size::history_bytes(&crate::size::objects_of(&repo).unwrap())
}

/// FAILS IF: a compaction changes a byte of what is kept, leaves the old
/// history, keeps an orphan — or another machine, its `last.json` from
/// before and an edit of its own, deletes anything, meets a conflict, loses
/// its edit or keeps the old history in its clone.
#[test]
fn belongings_compaction_is_safe() {
    let root = scratch("compact");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    let big = a.places.resolve("config/big.bin").unwrap();
    write_noise(&big, 21, 2 << 20);
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    // A HISTORY: four versions of 2 MiB of noise, and an orphan.
    for n in 0..4 {
        write_noise(&big, 100 + n, 2 << 20);
        a.sync();
    }
    let orphan = plant_orphan(&remote_dir, &root);
    a.sync();
    b.sync();
    let heavy = clone_weight(&b);
    assert!(heavy > 8 << 20, "the history is only {heavy} bytes");
    // B EDITS, and does not sync before A compacts.
    b.write("config/player.json", "{\"volume\":3}");

    a.say(PASSPHRASE);
    let Compaction::Done {
        before_bytes: before,
        after_bytes: after,
    } = a.engine().compact(0, &mut |_| {}).unwrap()
    else {
        panic!("a history of five versions was not compacted");
    };
    assert!(after < before / 2, "{before} → {after}");
    assert!(matches!(
        Status::read(&a.places.status()).unwrap().outcome,
        Outcome::Compacted { .. }
    ));
    let (names, _) = remote_files(&remote_dir, &root);
    assert!(
        !names.contains(&orphan),
        "the orphan survived the compaction"
    );
    let (_, subjects) = remote_log(&remote_dir, &root);
    assert!(subjects.len() <= 2, "{subjects:?}");

    // B: nothing deleted, no conflict, its edit sent, its clone lighter.
    let outcome = b.sync();
    assert!(
        matches!(
            outcome,
            Outcome::Synced {
                pushed: 1,
                deleted_here: 0,
                deleted_there: 0,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        sha_of(&b.places.resolve("config/big.bin").unwrap()),
        sha_of(&big)
    );
    let light = clone_weight(&b);
    assert!(
        light < heavy / 2,
        "B's clone kept the old history: {heavy} → {light}"
    );
    a.sync();
    assert_eq!(
        a.read("config/player.json").as_deref(),
        Some("{\"volume\":3}")
    );
    // A NEW MACHINE gets it all from the compacted history.
    let c = Machine::new(&root, "machine-gamma", &remote);
    assert!(matches!(c.sync(), Outcome::Synced { pulled: 3, .. }));
    assert_eq!(
        sha_of(&c.places.resolve("config/big.bin").unwrap()),
        sha_of(&big)
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a push another machine makes between a compaction's fetch and
/// its forced push is lost — the lease must refuse, and the compaction start
/// over on top of it.
#[test]
fn belongings_compaction_loses_no_race() {
    let root = scratch("compact-race");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"v\":1}");
    a.sync();
    a.write("config/theme.json", "{\"v\":2}");
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    b.write("config/audio.json", "{\"from\":\"b\"}");
    let raced = Cell::new(0);
    let race = || {
        if raced.get() == 0 {
            b.sync();
        }
        raced.set(raced.get() + 1);
    };
    let mut engine = a.engine();
    engine.before_push = Some(&race);
    a.say(PASSPHRASE);
    engine.compact(0, &mut |_| {}).unwrap();
    assert_eq!(raced.get(), 2, "the compaction did not start over");
    let c = Machine::new(&root, "machine-gamma", &remote);
    c.sync();
    assert_eq!(
        c.read("config/audio.json").as_deref(),
        Some("{\"from\":\"b\"}"),
        "the push made during the compaction was lost"
    );
    assert_eq!(c.read("config/theme.json").as_deref(), Some("{\"v\":2}"));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a machine whose push lost to another machine's compaction
/// keeps the old history in its clone — its push's own fetch moved the
/// remote's ref to the compacted head, and the retry must still see a
/// history rewritten under it.
#[test]
fn a_clone_shrinks_after_a_compaction_beat_its_push() {
    let root = scratch("compact-beaten");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    let big = a.places.resolve("config/big.bin").unwrap();
    write_noise(&big, 31, 2 << 20);
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    for n in 0..4 {
        write_noise(&big, 200 + n, 2 << 20);
        a.sync();
    }
    b.sync();
    let heavy = clone_weight(&b);
    assert!(heavy > 8 << 20, "the history is only {heavy} bytes");
    b.write("config/player.json", "{\"volume\":3}");
    // A COMPACTS right before B's push: B's push is refused, and the fetch
    // that tells a lost race from a failure brings the compacted head.
    let compacted = Cell::new(false);
    let compact = || {
        if !compacted.get() {
            compacted.set(true);
            a.say(PASSPHRASE);
            a.engine().compact(0, &mut |_| {}).unwrap();
        }
    };
    let mut engine = b.engine();
    engine.before_push = Some(&compact);
    b.say(PASSPHRASE);
    let done = engine.prepare(&mut |_| {});
    assert!(compacted.get());
    assert!(
        matches!(
            done,
            Ok(Done::Synced(Outcome::Synced {
                pushed: 1,
                deleted_here: 0,
                deleted_there: 0,
                ..
            }))
        ),
        "{done:?}"
    );
    let light = clone_weight(&b);
    assert!(
        light < heavy / 2,
        "B's clone kept the old history: {heavy} → {light}"
    );
    let c = Machine::new(&root, "machine-gamma", &remote);
    c.sync();
    assert_eq!(
        c.read("config/player.json").as_deref(),
        Some("{\"volume\":3}")
    );
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: a compaction that finds nothing to fold says it compacted, or
/// pushes anything — or the budget of a young history says a compaction
/// would fold it, which keeps a recommendation on screen that no compaction
/// can clear.
#[test]
fn nothing_to_compact_is_said_and_never_recommended() {
    let root = scratch("compact-nothing");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    for n in 0..3 {
        a.write("config/theme.json", &format!("{{\"v\":{n}}}"));
        let synced = a.sync();
        let Outcome::Synced {
            budget: Some(budget),
            ..
        } = synced
        else {
            panic!("{synced:?}");
        };
        assert!(!budget.foldable, "a history younger than keepDays folds");
        assert_ne!(
            kept::weight(budget.tree_bytes, budget.history_bytes, budget.foldable),
            kept::Weight::Compact
        );
    }
    let (_, before) = remote_log(&remote_dir, &root);
    a.say(PASSPHRASE);
    assert_eq!(
        a.engine().compact(kept::KEEP_DAYS, &mut |_| {}).unwrap(),
        Compaction::Nothing
    );
    assert_eq!(
        Status::read(&a.places.status()).unwrap().outcome,
        Outcome::NothingToCompact {
            keep_days: kept::KEEP_DAYS
        }
    );
    assert!(!a.places.passphrase().exists());
    let (_, after) = remote_log(&remote_dir, &root);
    assert_eq!(before, after, "a compaction with nothing to fold pushed");
    // AND WITH NOTHING KEPT, the same history folds.
    a.say(PASSPHRASE);
    assert!(matches!(
        a.engine().compact(0, &mut |_| {}).unwrap(),
        Compaction::Done { .. }
    ));
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: the person's recipient is left for an identity the repository
/// never received — a first push refused or beaten — so that "to myself"
/// seals files to a key that exists nowhere.
#[test]
fn a_first_push_that_fails_leaves_no_recipient() {
    let root = scratch("recipient");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    // THE REMOTE GOES AWAY between the fetch and the push.
    let away = root.join("remote.away");
    let gone = || {
        if remote_dir.exists() {
            std::fs::rename(&remote_dir, &away).unwrap();
        }
    };
    let mut engine = a.engine();
    engine.before_push = Some(&gone);
    a.say(PASSPHRASE);
    assert!(engine.prepare(&mut |_| {}).is_err());
    assert!(
        !a.places.recipient().exists(),
        "a recipient was left for an identity nobody has"
    );
    std::fs::rename(&away, &remote_dir).unwrap();
    a.sync();
    let remembered = std::fs::read_to_string(a.places.recipient()).unwrap();
    let (names, bytes) = remote_files(&remote_dir, &root);
    let key = &bytes[names.iter().position(|n| n == KEY_FILE).unwrap()];
    let ring = Keyring::open(key, &crate::crypt::passphrase(PASSPHRASE.to_owned())).unwrap();
    assert_eq!(
        remembered.trim(),
        ring.recipient().to_string(),
        "the recipient is not the repository's identity"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// The repository `belongings-recover-without-the-distro` restores with Go
/// `age`, written into `$RONOCTUA_RECOVER_FIXTURE`: `repo/` (a clone),
/// `passphrase`, `expected/<logical path>` — every file as it was, a link as
/// a link — `expected.modes` (`<logical path> <octal mode>` of every file:
/// the store keeps no mode) and `damage` (`<chunk> <logical path>`: one
/// file's only chunk, which the check damages to see the script name that
/// file and go on).
#[test]
#[ignore = "writes the fixture of the check belongings-recover-without-the-distro"]
fn a_repository_for_the_recovery_check() {
    let out = std::env::var_os("RONOCTUA_RECOVER_FIXTURE")
        .map(PathBuf::from)
        .expect("RONOCTUA_RECOVER_FIXTURE names where the fixture goes");
    let root = scratch("recover");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}\n");
    a.write("config/player-keys/tmdb", "tmdb-token");
    std::fs::set_permissions(
        a.places.resolve("config/player-keys/tmdb").unwrap(),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    write_noise(
        &a.places.resolve("home/notes/big.bin").unwrap(),
        33,
        5 << 20,
    );
    std::fs::write(a.places.resolve("home/notes/empty.md").unwrap(), b"").unwrap();
    std::os::unix::fs::symlink("big.bin", a.places.home.join("notes/latest")).unwrap();
    assert!(a.prefs.keep("home/notes"));
    a.sync();
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let mut clone = ronoctua_git::base(&tools(), &out, None).unwrap();
    ronoctua_git::subcommand(&mut clone, "clone")
        .args(["-q"])
        .arg(&remote_dir)
        .arg(out.join("repo"));
    assert!(clone.output().unwrap().status.success());
    std::fs::write(out.join("passphrase"), PASSPHRASE).unwrap();
    let mut modes = String::new();
    for logical in Last::read(&a.places.last()).unwrap().entries.keys() {
        let from = a.places.resolve(logical).unwrap();
        let to = out.join("expected").join(logical);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        let meta = std::fs::symlink_metadata(&from).unwrap();
        if meta.file_type().is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&from).unwrap(), &to).unwrap();
        } else {
            std::fs::copy(&from, &to).unwrap();
            std::fs::set_permissions(&to, meta.permissions()).unwrap();
            modes.push_str(&format!(
                "{logical} {:o}\n",
                meta.permissions().mode() & 0o7777
            ));
        }
    }
    std::fs::write(out.join("expected.modes"), modes).unwrap();
    // THE CHUNK TO DAMAGE: the theme's only one, which no other file shares.
    let engine = a.engine();
    let ground = engine.ground().unwrap();
    engine
        .checkout(&ground.repo, ground.head.as_deref().unwrap())
        .unwrap();
    let (_, index) = engine
        .open_repository(&crate::crypt::passphrase(PASSPHRASE.to_owned()))
        .unwrap();
    let theme = index
        .entries
        .iter()
        .find(|b| b.path == "config/theme.json")
        .unwrap();
    assert_eq!(theme.chunks.len(), 1);
    let chunk = crate::chunks::relative(&theme.chunks[0].id).unwrap();
    std::fs::write(out.join("damage"), format!("{chunk} config/theme.json\n")).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

/// FAILS IF: "keep both" loses the repository's side. It is pulled beside
/// this machine's AFTER the push, and by then the new index names this
/// machine's side alone — its chunks are swept, and must wait aside.
#[test]
fn keeping_both_lands_the_other_side_beside() {
    let root = scratch("both");
    let remote = bare(&root.join("remote.git"));
    let mut a = Machine::new(&root, "machine-alpha", &remote);
    a.write("home/notes/a.md", "first\n");
    assert!(a.prefs.keep("home/notes"));
    a.sync();
    let b = Machine::new(&root, "machine-beta", &remote);
    b.sync();
    a.write("home/notes/a.md", "from alpha\n");
    a.sync();
    b.write("home/notes/a.md", "from beta\n");
    b.say(PASSPHRASE);
    let plan = match b.engine().prepare(&mut |_| {}) {
        Ok(Done::Waiting(plan)) => plan,
        other => panic!("{other:?}"),
    };
    assert!(Decision::KeepBoth.allowed(&plan.conflicts[0]));
    let mut decisions = Decisions::new();
    decisions.insert("home/notes/a.md".to_owned(), Decision::KeepBoth);
    write_private(
        &b.places.decisions(),
        &serde_json::to_vec(&decisions).unwrap(),
    )
    .unwrap();
    let outcome = match b.engine().apply(&mut |_| {}) {
        Ok(Done::Synced(o)) => o,
        other => panic!("{other:?}"),
    };
    let Outcome::Synced { unwritten, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert!(unwritten.is_empty(), "{unwritten:?}");
    assert_eq!(b.read("home/notes/a.md").as_deref(), Some("from beta\n"));
    assert_eq!(
        b.read(&beside("home/notes/a.md", "machine-alpha"))
            .as_deref(),
        Some("from alpha\n"),
        "the repository's side did not land beside"
    );
    assert!(
        !b.engine().aside().exists(),
        "the aside folder outlived the sync"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A store's file for [`a_sync_says_which_files_wait_for_their_repairs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Waiting {
    #[default]
    Calm,
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Waits {
    mode: Waiting,
}

/// FAILS IF: a sync's outcome does not name the person's files whose
/// repairs wait for an answer (`Synced.repaired`, by logical path) — the
/// one place a sync tells the page there is something to decide — or names
/// one the person already answered.
#[test]
fn a_sync_says_which_files_wait_for_their_repairs() {
    let root = scratch("repaired");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    a.write("config/waits.json", "{\"mode\":\"frantic\"}");
    a.write("config/left.json", "{\"mode\":\"later\"}");
    let store = |file: &'static str| {
        ronoctua_store::Store::<Waits>::at(Some(a.places.config.clone()), file)
            .with_roots(Some(a.places.config.clone()), Some(a.places.state.clone()))
    };
    let _ = store("waits.json").read();
    let _ = store("left.json").read();
    let left = store("left.json").ledger().expect("a ledger");
    ronoctua_store::repair::dismiss(
        &a.places.repairs(),
        &a.places.config.join("left.json"),
        &left,
    )
    .unwrap();
    let outcome = a.sync();
    let Outcome::Synced { repaired, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(repaired, ["config/waits.json"]);
    let _ = std::fs::remove_dir_all(root);
}
