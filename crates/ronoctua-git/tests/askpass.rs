//! The helper itself, as git runs it: a prompt in argv, one line on stdout.

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::process::Command;

fn ask(prompt: &str, user: Option<&str>, token: Option<&std::path::Path>) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ronoctua-git-askpass"));
    command
        .arg(prompt)
        .env_remove(ronoctua_git::USER_VARIABLE)
        .env_remove(ronoctua_git::SECRET_VARIABLE);
    if let Some(user) = user {
        command.env(ronoctua_git::USER_VARIABLE, user);
    }
    if let Some(token) = token {
        command.env(ronoctua_git::SECRET_VARIABLE, token);
    }
    let out = command.output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn the_helper_prints_the_user_or_the_token_and_only_that() {
    let dir = std::env::temp_dir().join(format!("ronoctua-askpass-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let token = dir.join("token");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&token)
        .unwrap();
    f.write_all(b"glpat-example\n").unwrap();
    drop(f);

    assert_eq!(
        ask(
            "Username for 'https://example.org': ",
            Some("someone"),
            Some(&token)
        ),
        (true, "someone\n".to_owned())
    );
    assert_eq!(
        ask(
            "Password for 'https://someone@example.org': ",
            Some("someone"),
            Some(&token)
        ),
        (true, "glpat-example\n".to_owned())
    );
    let (ok, printed) = ask("Password for 'https://example.org': ", None, None);
    assert!(
        !ok && printed.is_empty(),
        "no token: nothing printed, a failure"
    );
    let (ok, printed) = ask(
        "Are you sure you want to continue connecting?",
        None,
        Some(&token),
    );
    assert!(
        !ok && printed.is_empty(),
        "a question it does not know is not answered"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A web server that asks every request "401, who are you?" and writes down
/// the credentials each one brought.
/// Served until `stop` is set (and one more connection wakes it).
fn refuser(
    seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> (u16, std::thread::JoinHandle<()>) {
    use std::io::Read as _;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let thread = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stop.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&buffer[..n]),
                }
            }
            let text = String::from_utf8_lossy(&request).into_owned();
            seen.lock().unwrap().extend(
                text.lines()
                    .filter(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                    .map(str::to_owned),
            );
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\n\
                  Content-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    (port, thread)
}

/// THE WHOLE WAY, with the real git: a remote over HTTP that wants a name
/// and a password gets the user and the token through this helper, and the
/// token was never in git's argv or environment — only the file's path.
#[test]
fn git_hands_the_token_through_the_helper() {
    let mut tools = ronoctua_git::Tools::from_env()
        .expect("RONOCTUA_GIT is not set: run inside `nix develop` or the tests derivation");
    tools.askpass = Some(env!("CARGO_BIN_EXE_ronoctua-git-askpass").into());
    let dir = std::env::temp_dir().join(format!("ronoctua-askpass-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let token = dir.join("token");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&token)
        .unwrap();
    f.write_all(b"glpat-example\n").unwrap();
    drop(f);

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (port, server) = refuser(seen.clone(), stop.clone());
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec![
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/x.git"),
        ],
    ] {
        let done = Command::new(&tools.git)
            .arg("-C")
            .arg(&work)
            .args(&args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap();
        assert!(done.success(), "git {args:?}");
    }
    let repo = ronoctua_git::Repo::discover(&work)
        .unwrap()
        .with_tools(tools);
    // NOT NARROWED: this remote is plain HTTP, where no TLS group is agreed
    // (`belongings-https-is-pq-only` runs a narrowed token against TLS).
    let credentials = ronoctua_git::Credentials::Token {
        file: token,
        user: Some("someone".to_owned()),
        pq_only: false,
    };
    let refused = repo.fetch(Some(&credentials)).unwrap_err();
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = std::net::TcpStream::connect(("127.0.0.1", port));
    server.join().unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        ["Authorization: Basic c29tZW9uZTpnbHBhdC1leGFtcGxl"],
        "base64 of `someone:glpat-example`: the first request brought none, the second these"
    );
    assert_eq!(
        refused.why,
        ronoctua_git::Why::Refused,
        "and the 401 after it is a refusal"
    );
    let _ = std::fs::remove_dir_all(dir);
}
