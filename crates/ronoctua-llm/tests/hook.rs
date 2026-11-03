//! `ronoctua-llm hook`, run as Claude Code runs it: a child, the event on
//! stdin, nothing on stdout or stderr — an async hook's output joins the
//! conversation — and 0, always: exit code 2 is a blocking error.
//!
//! The mapping of every event is the library's (`hook::tests`); this is the
//! other side, against the real binary cargo built for this package.

use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PROGRAM: &str = env!("CARGO_BIN_EXE_ronoctua-llm");

const SESSION: &str = "5f0c1e2a-0b1c-4d2e-8f3a-9b8c7d6e5f40";

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-hook-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("runtime")).expect("a temp folder");
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn event(name: &str, extra: &str) -> Vec<u8> {
    format!(
        r#"{{"session_id":"{SESSION}","transcript_path":"/home/p/t.jsonl","cwd":"/home/p/work","hook_event_name":"{name}"{extra}}}"#
    )
    .into_bytes()
}

/// Runs the hook once in `dir`'s rooms, with `stdin`, and asks it answered
/// 0 and said nothing.
fn run(dir: &Path, args: &[&str], stdin: &[u8], env: &[(&str, &str)], what: &str) {
    let mut command = Command::new(PROGRAM);
    command
        .arg("hook")
        .args(args)
        .env_clear()
        .env("HOME", dir.join("home"))
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_STATE_HOME", dir.join("state"))
        .env("XDG_RUNTIME_DIR", dir.join("runtime"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        if value.is_empty() {
            command.env_remove(name);
        } else {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().expect("the hook runs");
    // A WRITE THE HOOK STOPPED READING (past its bound) is its right: the
    // broken pipe is the writer's, as it is Claude Code's.
    let _ = child.stdin.take().expect("stdin").write_all(stdin);
    let out = child.wait_with_output().expect("the hook ends");
    assert_eq!(out.status.code(), Some(0), "{what}: answered");
    assert!(
        out.stdout.is_empty(),
        "{what}: printed {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        out.stderr.is_empty(),
        "{what}: said {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn record(dir: &Path, folder: &str) -> Option<serde_json::Value> {
    let path = dir
        .join("runtime")
        .join(folder)
        .join(format!("{SESSION}.json"));
    let text = std::fs::read_to_string(&path).ok()?;
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600,
        "the record is its owner's"
    );
    Some(serde_json::from_str(&text).expect("a record is JSON"))
}

/// FAILS IF: the hook prints anything, answers anything but 0 — for an
/// input it cannot read, one past its bound, an argument it does not know,
/// a machine with no runtime folder — or does not keep a session's record,
/// private, in the folder of the instance it runs in.
#[test]
fn the_hook_never_prints_and_always_answers_0() {
    let dir = Dir::new("quiet");
    let d = dir.0.as_path();
    let mut huge = b"{\"session_id\":\"".to_vec();
    huge.extend(std::iter::repeat_n(b'a', 5 * 1024 * 1024));
    for (what, input) in [
        ("garbage", b"\x00\xffnot json at all".to_vec()),
        ("nothing", Vec::new()),
        ("past the bound", huge),
        (
            "a subagent's",
            event(
                "UserPromptSubmit",
                r#","agent_id":"a1","agent_type":"Explore""#,
            ),
        ),
        ("an event nobody maps", event("PreCompact", "")),
    ] {
        run(d, &[], &input, &[], what);
        assert!(record(d, "ronoctua-agents").is_none(), "{what}: kept");
    }
    run(
        d,
        &[],
        &event("SessionStart", r#","source":"startup""#),
        &[("XDG_RUNTIME_DIR", "")],
        "no runtime folder",
    );
    assert!(
        record(d, "ronoctua-agents").is_none(),
        "kept with no runtime folder"
    );

    run(
        d,
        &["--an-argument-from-later"],
        &event("UserPromptSubmit", r#","prompt":"hi""#),
        &[],
        "a prompt",
    );
    let kept = record(d, "ronoctua-agents").expect("a prompt is kept");
    assert_eq!(kept["status"], "working");
    assert_eq!(kept["folder"], "/home/p/work");
    assert_eq!(
        kept["account-dir"],
        d.join("home").join(".claude").to_str().unwrap()
    );
    let folder = d.join("runtime").join("ronoctua-agents");
    assert_eq!(
        std::fs::metadata(&folder).unwrap().permissions().mode() & 0o777,
        0o700
    );

    // THE DEVELOPMENT LOOP'S SESSIONS are its own folder's.
    run(
        d,
        &[],
        &event("PermissionRequest", r#","tool_name":"Bash""#),
        &[
            ("RONOCTUA_INSTANCE", "dev"),
            ("CLAUDE_CONFIG_DIR", "/home/p/.claude-2"),
        ],
        "a dev session",
    );
    let dev = record(d, "ronoctua-dev-agents").expect("a dev session is kept");
    assert_eq!(
        (dev["status"].as_str(), dev["account-dir"].as_str()),
        (Some("waiting"), Some("/home/p/.claude-2"))
    );
    assert_eq!(
        record(d, "ronoctua-agents").unwrap()["status"],
        "working",
        "the other instance's untouched"
    );

    run(
        d,
        &[],
        &event("SessionEnd", r#","reason":"other""#),
        &[],
        "an end",
    );
    assert_eq!(
        record(d, "ronoctua-agents").unwrap()["ended"],
        true,
        "a tombstone"
    );
}
