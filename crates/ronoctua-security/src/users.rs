//! Who the helper acts for.
//!
//! Through the door, sudo names the person in `SUDO_UID` (sudo sets it after
//! resetting the environment, so the caller cannot choose it). The helper
//! turns it into a name from `/etc/passwd` — a real file on NixOS — and
//! refuses uid 0: root asking through sudo is nobody's gesture. From a unit or
//! the dispatcher there is no `SUDO_UID` at all.
//!
//! A name is used in a sudoers line, a PAM list and an fprintd claim, so it is
//! CHECKED and never escaped: [`plain`] or nothing.

use crate::{Refusal, Why, refused};

/// Where the names are.
pub const PASSWD: &str = "/etc/passwd";
/// The variable sudo sets to the invoking user's uid.
pub const SUDO_UID: &str = "SUDO_UID";

/// The longest user name the helper acts for.
const USER_MOST: usize = 32;

/// The person the helper acts for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// Their uid.
    pub uid: u32,
    /// Their name, [`plain`].
    pub name: String,
}

/// A user name a line may carry: lowercase letters, digits, `_` and `-`, not
/// starting with a digit or `-`, at most 32 bytes.
#[must_use]
pub fn plain(user: &str) -> bool {
    let mut chars = user.chars();
    let first_fits = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    let rest_fits =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    user.len() <= USER_MOST && first_fits && rest_fits
}

/// The name of `uid` in a passwd file's text; `None` when no line has it.
/// Pure.
#[must_use]
pub fn name_of(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        let id: u32 = fields.next()?.parse().ok()?;
        (id == uid).then(|| name.to_owned())
    })
}

/// The uid of `name` in a passwd file's text; `None` when the name is not
/// [`plain`] or no line has it. Pure.
#[must_use]
pub fn uid_of(passwd: &str, name: &str) -> Option<u32> {
    if !plain(name) {
        return None;
    }
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let this = fields.next()?;
        let _password = fields.next()?;
        let id: u32 = fields.next()?.parse().ok()?;
        (this == name).then_some(id)
    })
}

/// The uid of a plain user name, read from `/etc/passwd`: for a unit that acts
/// for the one person it was GIVEN BY NAME (`nix/modules/vpn.nix` bakes the
/// name into the unit), with no `SUDO_UID` to name them.
///
/// # Errors
///
/// [`Why::Invalid`] for a name that is not [`plain`] or has no line;
/// [`Why::NotAllowed`] for uid 0 (root stages nothing for itself);
/// [`Why::Unreadable`] when `/etc/passwd` cannot be read.
pub fn uid_named(name: &str) -> Result<u32, Refusal> {
    let passwd = std::fs::read_to_string(PASSWD).map_err(|_| refused(Why::Unreadable))?;
    let uid = uid_of(&passwd, name).ok_or(refused(Why::Invalid))?;
    if uid == 0 {
        return Err(refused(Why::NotAllowed));
    }
    Ok(uid)
}

/// The person of a `SUDO_UID` value and a passwd text. Pure.
///
/// # Errors
///
/// [`Why::NotAllowed`] for uid 0; [`Why::Invalid`] for a value that is not a
/// uid, a uid with no name, or a name that is not [`plain`].
pub fn person_of(sudo_uid: &str, passwd: &str) -> Result<Person, Refusal> {
    let uid: u32 = sudo_uid.trim().parse().map_err(|_| refused(Why::Invalid))?;
    if uid == 0 {
        return Err(refused(Why::NotAllowed));
    }
    let name = name_of(passwd, uid).ok_or(refused(Why::Invalid))?;
    if !plain(&name) {
        return Err(refused(Why::Invalid));
    }
    Ok(Person { uid, name })
}

/// Who asked through the door: `None` when nobody did (a unit, the
/// dispatcher).
///
/// # Errors
///
/// What [`person_of`] refuses, and [`Why::Unreadable`] when `/etc/passwd`
/// cannot be read.
pub fn invoking() -> Result<Option<Person>, Refusal> {
    let Some(value) = std::env::var_os(SUDO_UID) else {
        return Ok(None);
    };
    let value = value.to_str().ok_or(refused(Why::Invalid))?;
    let passwd = std::fs::read_to_string(PASSWD).map_err(|_| refused(Why::Unreadable))?;
    person_of(value, &passwd).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWD_TEXT: &str = "root:x:0:0:System administrator:/root:/run/current-system/sw/bin/bash\n\
         messagebus:x:4:4:D-Bus system message bus daemon user:/run/dbus:/run/current-system/sw/bin/nologin\n\
         someone:x:1000:100::/home/someone:/run/current-system/sw/bin/zsh\n\
         Odd Name:x:1001:100::/home/odd:/bin/sh\n";

    #[test]
    fn the_person_is_the_one_sudo_names() {
        assert_eq!(
            person_of("1000", PASSWD_TEXT),
            Ok(Person {
                uid: 1000,
                name: "someone".to_owned()
            })
        );
        assert_eq!(
            person_of("0", PASSWD_TEXT).unwrap_err().why,
            Why::NotAllowed
        );
        assert_eq!(
            person_of("1001", PASSWD_TEXT).unwrap_err().why,
            Why::Invalid
        );
        assert_eq!(
            person_of("4242", PASSWD_TEXT).unwrap_err().why,
            Why::Invalid
        );
        assert_eq!(person_of("-1", PASSWD_TEXT).unwrap_err().why, Why::Invalid);
        assert_eq!(person_of("", PASSWD_TEXT).unwrap_err().why, Why::Invalid);
        assert_eq!(name_of(PASSWD_TEXT, 4).as_deref(), Some("messagebus"));
    }

    /// FAILS IF: a plain name does not resolve to its uid, an absent name
    /// resolves to anything, or a name that is not plain is looked up at all.
    #[test]
    fn a_name_resolves_to_its_uid() {
        assert_eq!(uid_of(PASSWD_TEXT, "someone"), Some(1000));
        assert_eq!(uid_of(PASSWD_TEXT, "root"), Some(0));
        assert_eq!(uid_of(PASSWD_TEXT, "nobody-here"), None);
        // "Odd Name" is not a plain name, so it is never even searched for.
        assert_eq!(uid_of(PASSWD_TEXT, "Odd Name"), None);
    }

    #[test]
    fn only_plain_names_are_written_anywhere() {
        for good in ["someone", "_svc", "a-b_c9"] {
            assert!(plain(good), "{good}");
        }
        for bad in [
            "",
            "ALL",
            "root ALL",
            "a\nb",
            "-x",
            "9lives",
            "x,y",
            "a:b",
            &"a".repeat(33),
        ] {
            assert!(!plain(bad), "{bad:?}");
        }
    }
}
