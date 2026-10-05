//! `ronoctua-git-askpass` — what git runs as `GIT_ASKPASS` for a remote over
//! HTTPS with a token: it prints the user name or the token, read from the
//! `0600` file whose PATH is `RONOCTUA_GIT_SECRET`. Nothing else is ever
//! printed; a question it cannot answer ends it with a failure, and git then
//! says it could not read the credential. See `ronoctua_git::askpass`.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let prompt = std::env::args().nth(1).unwrap_or_default();
    let user = std::env::var(ronoctua_git::USER_VARIABLE).ok();
    let token = std::env::var_os(ronoctua_git::SECRET_VARIABLE)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    match ronoctua_git::askpass::answer(&prompt, user.as_deref(), token.as_deref()) {
        Ok(answer) => {
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{answer}")
                .and_then(|()| out.flush())
                .is_err()
            {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(silence) => {
            // The kind of silence, never the secret.
            eprintln!("ronoctua-git-askpass: {silence:?}");
            ExitCode::FAILURE
        }
    }
}
