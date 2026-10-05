//! The `ronoctua-tags` PROGRAM, run as the footer runs it: a child, words on
//! the command line, edits on stdin, one JSON `Answer` on stdout.
//!
//! The shell's side of the protocol is driven by stand-ins in its own tests
//! (`inspection_edit`); this is the other side, against the real binary cargo
//! built for this package, on a real FLAC: a sheet, a write, the sheet again,
//! and a write that names a stamp the file no longer has.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ronoctua_tags::{Answer, Edit, Field, Stamp, Why};

const PROGRAM: &str = env!("CARGO_BIN_EXE_ronoctua-tags");

/// `fLaC`, a STREAMINFO block (44.1 kHz, two channels, 16 bits) and a frame
/// sync after it: the smallest file lofty reads as a FLAC. (The crate's own
/// `fixture::flac` is test-private to the library; this is the same bytes.)
fn flac() -> Vec<u8> {
    let mut out = b"fLaC".to_vec();
    out.extend_from_slice(&[0x80, 0x00, 0x00, 34]);
    out.extend_from_slice(&4096_u16.to_be_bytes());
    out.extend_from_slice(&4096_u16.to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let packed: u64 = (44_100_u64 << 44) | (1 << 41) | (15 << 36);
    out.extend_from_slice(&packed.to_be_bytes());
    out.extend_from_slice(&[0_u8; 16]);
    out.extend_from_slice(&[0xFF, 0xF8, 0x69, 0x08, 0x00, 0x00, 0x00, 0x00]);
    out
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-tags-program-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temp folder");
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the program once and reads its one answer.
fn run(words: &[&str], stdin: Option<&[u8]>) -> Answer {
    let mut child = Command::new(PROGRAM)
        .args(words)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the program starts");
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(bytes).expect("stdin is written");
    }
    let out = child.wait_with_output().expect("the program ends");
    assert!(
        out.status.success(),
        "{words:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "ONE line of JSON"
    );
    serde_json::from_slice(&out.stdout).expect("the answer is an Answer")
}

fn sheet(path: &Path) -> ronoctua_tags::Sheet {
    match run(&["sheet", path.to_str().expect("a UTF-8 path")], None) {
        Answer::Sheet { sheet } => *sheet,
        other => panic!("a sheet was expected, not {other:?}"),
    }
}

fn apply(path: &Path, stamp: Stamp, edits: &[Edit]) -> Answer {
    let stamp = serde_json::to_string(&stamp).expect("a stamp serialises");
    let edits = serde_json::to_vec(edits).expect("edits serialise");
    run(
        &[
            "apply",
            path.to_str().expect("a UTF-8 path"),
            "--stamp",
            &stamp,
        ],
        Some(&edits),
    )
}

#[test]
fn a_tag_written_through_the_program_is_read_back_through_it() {
    let dir = Dir::new("write");
    let song = dir.0.join("aquarela.flac");
    std::fs::write(&song, flac()).expect("the fixture is written");

    let before = sheet(&song);
    let title = before.field(Field::Title).expect("a FLAC has a title row");
    assert!(title.value.is_none(), "{title:?}");

    let edits = [Edit::Set {
        field: Field::Title,
        value: "Aquarela".to_owned(),
    }];
    let Answer::Written { stamp } = apply(&song, before.stamp, &edits) else {
        panic!("the write was refused");
    };
    let after = sheet(&song);
    assert_eq!(after.stamp, stamp, "the stamp handed back is the file's");
    assert_eq!(
        after.field(Field::Title).and_then(|r| r.value.as_deref()),
        Some("Aquarela")
    );

    // THE STAMP THE FIRST SHEET HAD is no longer the file's: a write naming
    // it is refused, and the file keeps what the first write put there.
    assert_eq!(
        apply(
            &song,
            before.stamp,
            &[Edit::Set {
                field: Field::Title,
                value: "Outra".to_owned(),
            }]
        ),
        Answer::Refused {
            why: Why::ChangedSince
        }
    );
    assert_eq!(
        sheet(&song)
            .field(Field::Title)
            .and_then(|r| r.value.as_deref()),
        Some("Aquarela")
    );
}

#[test]
fn a_file_that_is_not_there_is_a_refusal_on_stdout_and_status_zero() {
    assert_eq!(
        run(&["sheet", "/nowhere/at/all.flac"], None),
        Answer::Refused { why: Why::Gone }
    );
}
