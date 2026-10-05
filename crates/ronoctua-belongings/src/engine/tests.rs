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

use std::cell::Cell;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::fs::PermissionsExt as _;

use super::*;

/// A small scrypt, so a test does not pay a second per sync.
const WORK: Option<u8> = Some(10);

/// The passphrase both machines type.
const PASSPHRASE: &str = "a long passphrase, typed twice";

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
}

impl Machine {
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
        }
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
    assert!(clone.output().unwrap().status.success());
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

/// How many blobs the remote's head holds.
fn remote_blobs(remote_dir: &Path, root: &Path) -> usize {
    remote_files(remote_dir, root)
        .0
        .iter()
        .filter(|n| n.starts_with("blobs/"))
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

    // NOTHING IN THE CLEAR: the tree's names are ours, and no byte of a path,
    // a content or the machine's name is in any file of it.
    let (names, bytes) = remote_files(&remote_dir, &root);
    for name in &names {
        let fine = [FORMAT_FILE, KEY_FILE, INDEX_FILE].contains(&name.as_str())
            || name
                .strip_prefix("blobs/")
                .and_then(|n| n.strip_suffix(".age"))
                .is_some_and(|id| id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(fine, "the repository names {name}");
    }
    for needle in [
        &b"theme.json"[..],
        b"owl-secret-theme",
        b"tmdb",
        b"diary",
        b"plaintext-marker",
        b"notes",
        b"machine-alpha",
        b"config/",
    ] {
        for (name, content) in names.iter().zip(&bytes) {
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
    assert_eq!(remote_blobs(&remote_dir, &root), 40, "blobs went missing");
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
