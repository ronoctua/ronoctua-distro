//! The answer `ronoctua-git-askpass` gives git — pure but for reading the
//! token's file, so the rules are tested here and the binary is a wrapper.
//!
//! git runs `$GIT_ASKPASS "<prompt>"` and reads one line from its stdout.
//! It asks twice for HTTPS: `Username for 'https://host': ` and `Password
//! for 'https://user@host': `. The user name travels in the environment
//! ([`crate::USER_VARIABLE`]; it is not a secret); the token never does —
//! only the PATH of its file ([`crate::SECRET_VARIABLE`]).

use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

/// The largest token file read: a token, not a document.
pub const TOKEN_MOST_BYTES: u64 = 16 * 1024;

/// Why the helper answers nothing (git then fails with "could not read",
/// and [`crate::Why::NeedsCredentials`] says so on screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    /// A question that is neither a user name nor a password.
    UnknownQuestion,
    /// A user name was asked and none was given.
    NoUser,
    /// A password was asked and no token file was given.
    NoToken,
    /// The token's file could not be read, was empty, or too large.
    Unreadable,
    /// The token's file can be read by somebody else: it is not used.
    NotPrivate,
}

/// What to print for `prompt`: the user name for a user-name question, the
/// token (first line of the file) for a password one.
///
/// # Errors
///
/// A [`Silence`].
pub fn answer(
    prompt: &str,
    user: Option<&str>,
    token_file: Option<&Path>,
) -> Result<String, Silence> {
    let asked = prompt.trim_start().to_ascii_lowercase();
    if asked.starts_with("username") {
        return user
            .filter(|u| !u.is_empty())
            .map(str::to_owned)
            .ok_or(Silence::NoUser);
    }
    if !asked.starts_with("password") {
        return Err(Silence::UnknownQuestion);
    }
    let file = token_file.ok_or(Silence::NoToken)?;
    let meta = std::fs::metadata(file).map_err(|_| Silence::Unreadable)?;
    if !meta.is_file() || meta.len() > TOKEN_MOST_BYTES {
        return Err(Silence::Unreadable);
    }
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(Silence::NotPrivate);
    }
    let mut text = String::new();
    std::fs::File::open(file)
        .and_then(|f| f.take(TOKEN_MOST_BYTES).read_to_string(&mut text))
        .map_err(|_| Silence::Unreadable)?;
    let token = text.lines().next().unwrap_or_default().trim().to_owned();
    zeroize_string(&mut text);
    if token.is_empty() {
        return Err(Silence::Unreadable);
    }
    Ok(token)
}

/// Overwrites a string's bytes before it is freed — without a dependency:
/// the helper lives for one question.
fn zeroize_string(text: &mut String) {
    let len = text.len();
    text.clear();
    text.extend(std::iter::repeat_n('\0', len));
    text.clear();
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    use super::*;

    #[test]
    fn the_helper_answers_the_two_questions_and_nothing_else() {
        let dir = std::env::temp_dir().join(format!("ronoctua-askpass-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let token = dir.join("token");
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&token)
            .unwrap();
        f.write_all(b"ghp_example\n").unwrap();
        drop(f);

        assert_eq!(
            answer(
                "Username for 'https://example.org': ",
                Some("someone"),
                Some(&token)
            ),
            Ok("someone".to_owned())
        );
        assert_eq!(
            answer(
                "Password for 'https://someone@example.org': ",
                Some("someone"),
                Some(&token)
            ),
            Ok("ghp_example".to_owned())
        );
        assert_eq!(
            answer("Username for 'https://example.org': ", None, Some(&token)),
            Err(Silence::NoUser)
        );
        assert_eq!(
            answer("Enter passphrase for key '/x': ", None, Some(&token)),
            Err(Silence::UnknownQuestion)
        );
        assert_eq!(answer("Password: ", None, None), Err(Silence::NoToken));
        std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            answer("Password: ", None, Some(&token)),
            Err(Silence::NotPrivate)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
