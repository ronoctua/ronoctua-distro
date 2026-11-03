//! The holder PROGRAM, as a window's spawner runs it — without
//! `systemd-run`, which a test never runs: the same arguments, the same
//! ready line, the same registry, and the same end.

use std::ffi::OsString;
use std::io::{BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ronoctua_term::ghostty::{self, Decoder, Format};
use ronoctua_terminals::registry::Registry;
use ronoctua_terminals::spawn::{self, Serve};
use ronoctua_terminals::wire::{self, Attach, Colors, Rgb, Scheme, Size, ToViewer};
use ronoctua_terminals::{GroupId, PROTOCOL, TerminalId};

/// `setsid`, by store path where the build gave one.
const SETSID: &str = match option_env!("RONOCTUA_SETSID") {
    Some(p) => p,
    None => "setsid",
};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ronoctua-holder-program-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The window's colours: the engine's own, read off a terminal nothing was
/// set on. A run of the program has no theme to resolve roles in, and a
/// colour table written here would be one this crate keeps (invariant 9).
fn colors() -> Colors {
    let vt = ghostty::Vt::new(1, 1, &ghostty::Config::for_version("9.9")).unwrap();
    let state = ghostty::RenderState::new().unwrap();
    state.update(&vt).unwrap();
    let own = state.frame().colors();
    let rgb = |c: ghostty::Rgb| Rgb([c.r, c.g, c.b]);
    Colors {
        foreground: rgb(own.foreground),
        background: rgb(own.background),
        cursor: own.cursor.map(rgb),
        palette: own.palette.iter().copied().map(rgb).collect(),
        scheme: Scheme::Dark,
    }
}

/// Waits for `done`, for at most 20 s.
fn eventually(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(20), "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_holder_program_serves_a_terminal_until_it_is_ended() {
    // FAILS IF the program the spawner starts does not say it is ready once it
    // can be attached to, keeps its socket and state anywhere but this
    // instance's registry, cannot be attached to over its socket, cannot start
    // its program from the session it leads (the slave taken as its own
    // controlling terminal), or — asked to end — leaves the program running
    // instead of hanging it up.
    let runtime = scratch("runtime");
    let registry = Registry::at(runtime.join("ronoctua-terms"));
    let pid_file = runtime.join("program.pid");
    let serve = Serve {
        id: TerminalId::fresh().unwrap(),
        group: GroupId::fresh().unwrap(),
        order: 0,
        size: Size {
            columns: 60,
            rows: 10,
            cell_width_px: 9,
            cell_height_px: 18,
        },
        cwd: Some(runtime.clone()),
        program: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            format!(
                "echo $$ > {}; echo served-here; exec sleep 300",
                pid_file.display()
            ),
        ],
        // THE PERSON'S CHOICES travel as the spawner writes them: the
        // program reads them back, and a flag it does not know would stop
        // it before it is ready.
        born: ronoctua_terminals::spawn::Born {
            scrollback_bytes: Some(1_000_000),
            cursor: Some((ronoctua_terminals::spawn::CursorShape::Bar, true)),
        },
    };
    // `setsid --fork`, the spawner's own first step: the holder outlives what
    // started it, the ready line is what is waited for, and the holder LEADS
    // a session with no controlling terminal — as it does in its scope, and
    // as a test that started it some other way would never see (the first
    // terminal such a process opens becomes its own).
    let mut argv: Vec<OsString> = vec![
        "/bin/sh".into(),
        "-c".into(),
        "export XDG_RUNTIME_DIR=\"$0\"; unset RONOCTUA_INSTANCE; exec \"$@\"".into(),
        runtime.clone().into_os_string(),
        SETSID.into(),
        "--fork".into(),
        env!("CARGO_BIN_EXE_ronoctua-term-holder").into(),
    ];
    argv.extend(serve.to_args());
    spawn::launch(&argv, Duration::from_secs(20)).expect("the holder said it was ready");

    let census = registry.census();
    assert_eq!(census.terminals.len(), 1, "{:?}", registry.dir());
    assert_eq!(census.terminals[0].id, serve.id);
    assert_eq!(census.away(), 1, "no window yet");

    let mut stream = registry
        .attach(
            &serve.id,
            &Attach {
                protocol: PROTOCOL,
                engine: ghostty::ENGINE.to_owned(),
                pid: std::process::id(),
                size: serve.size,
                colors: colors(),
                group: serve.group.clone(),
                order: 0,
            },
        )
        .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut snapshot, mut mirror) = (Vec::new(), None);
    let shows = |m: &Option<ghostty::Mirror>| {
        m.as_ref().is_some_and(|m: &ghostty::Mirror| {
            String::from_utf8_lossy(&m.format(Format::Plain).unwrap()).contains("served-here")
        })
    };
    while !shows(&mirror) {
        let (kind, payload) = wire::read_frame(&mut reader).unwrap().expect("a frame");
        match ToViewer::decode(kind, payload).unwrap() {
            ToViewer::Snapshot(b) => snapshot.extend(b),
            ToViewer::Ready => {
                mirror = Some(Decoder::decode(std::mem::take(&mut snapshot)).unwrap())
            }
            ToViewer::Bytes(b) => mirror.as_mut().unwrap().feed(&b),
            _ => {}
        }
    }
    let pid: u32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(Path::new(&format!("/proc/{pid}")).exists());

    registry.end(&serve.id).unwrap();
    eventually("the holder leaves its registry", || {
        !registry.state_file(&serve.id).exists() && !registry.socket(&serve.id).exists()
    });
    // Hung up by the kernel when the holder's master closed: `sleep` takes
    // SIGHUP's default and ends. (A zombie still has a `/proc` entry, but no
    // command line.)
    eventually("the program is hung up", || {
        std::fs::read(format!("/proc/{pid}/cmdline")).map_or(true, |c| c.is_empty())
    });
    let _ = stream.flush();
    let _ = std::fs::remove_dir_all(&runtime);
}
