//! The real binary, with its answers in the environment — what the
//! terminal-less mode is given.
//!
//! The unit tests prove the command the installer starts itself again with;
//! these prove the process that comes out of it: its answers crossed, and
//! not one of them is left in its environment, where every child it starts
//! (`mkpasswd`, `lsblk`, `btrfs`, `systemd-creds`, `nixos-install`) would
//! read it from `/proc/<pid>/environ`.

use std::io::Read as _;
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ronoctua_installer::platform::{Platform, SYSROOT};

/// The installer, built with these tests.
const INSTALLER: &str = env!("CARGO_BIN_EXE_ronoctua-install");

/// Every answer the terminal-less mode needs, none of them a real one — on
/// a machine started from UEFI, with a TPM 2.0, whatever runs the tests.
fn answered() -> Command {
    let mut c = Command::new(INSTALLER);
    let sysroot =
        std::env::temp_dir().join(format!("ronoctua-answers-sysroot-{}", std::process::id()));
    Platform::default().plant(&sysroot).unwrap();
    c.env(SYSROOT, &sysroot);
    for (name, value) in [
        ("RONOCTUA_INST_DISK", "/dev/nvme0n1"),
        ("RONOCTUA_INST_HOSTNAME", "noctua-answers"),
        ("RONOCTUA_INST_PASSWORD", "Zq7-test-password"),
        ("RONOCTUA_INST_PASSPHRASE", "Zq7-test-passphrase"),
        (
            "RONOCTUA_INST_BELONGINGS",
            "git@example.org:someone/belongings.git",
        ),
        ("RONOCTUA_INST_BELONGINGS_ACCESS", "/run/keys/deploy"),
        ("RONOCTUA_INST_BELONGINGS_PASSPHRASE", "Zq7-belongings"),
    ] {
        c.env(name, value);
    }
    c.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

/// FAILS IF: the answers do not cross to the installer started again —
/// the dry run would refuse a hostname it was given.
#[test]
fn the_answers_cross_to_the_installer_started_again() {
    let out = answered().output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains("networking.hostName = \"noctua-answers\";"),
        "{said}"
    );
}

/// FAILS IF: an answer is still in the environment of the installer that
/// reaches its first check — read from `/proc/<pid>/environ` the moment it
/// knocks on a system bus this test owns, after it started itself again.
#[test]
fn no_answer_is_left_in_the_installers_environment() {
    let dir = std::env::temp_dir().join(format!("ronoctua-answers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bus = dir.join("system_bus_socket");
    let listener = UnixListener::bind(&bus).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = answered()
        .arg("--execute")
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            format!("unix:path={}", bus.display()),
        )
        // A MARK of this test's own, which must survive: proof that the
        // environment read is the installer's, and that only the answers
        // left it.
        .env("RONOCTUA_ANSWERS_TEST_MARK", "here")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait().unwrap() {
                    let mut err = String::new();
                    let _ = child.stderr.take().unwrap().read_to_string(&mut err);
                    panic!("the installer ended ({status}) before it asked the bus: {err}");
                }
                assert!(
                    Instant::now() < deadline,
                    "the installer never asked the bus"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("{e}"),
        }
    };
    // THE SAME PROCESS ID, after the exec: its environment as it is now.
    let environ = std::fs::read(format!("/proc/{}/environ", child.id())).unwrap();
    drop(stream);
    let names: Vec<String> = environ
        .split(|b| *b == 0)
        .filter_map(|v| std::str::from_utf8(v).ok())
        .filter_map(|v| v.split_once('=').map(|(n, _)| n.to_owned()))
        .collect();
    let status = child.wait().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        names.iter().any(|n| n == "RONOCTUA_ANSWERS_TEST_MARK"),
        "the environment read is not the installer's: {names:?}"
    );
    let left: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("RONOCTUA_INST_"))
        .collect();
    assert!(left.is_empty(), "answers left in the environment: {left:?}");
    // AND THE BUS THAT WENT AWAY IS NO NETWORK, which stops nothing: the
    // installer goes on to what it checks here — the access named, which no
    // machine running this holds (nor, as anybody but root, the scratch
    // folder under `/run`) — and that is what refuses, with the
    // repository's code.
    assert_eq!(
        status.code(),
        Some(i32::from(ronoctua_installer::reach::EXIT_REPOSITORY)),
        "{status}"
    );
}
