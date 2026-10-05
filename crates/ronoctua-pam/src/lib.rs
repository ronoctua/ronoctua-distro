//! The person's system password, checked through PAM — the one crate that
//! speaks PAM.
//!
//! # What it is for
//!
//! The player's adult catalogue opens only after the person types the
//! password of their system account: [`verify`] asks PAM, under the fixed
//! service [`SERVICE`] (declared in NixOS by `nix/modules/unlock.nix` as
//! `security.pam.services.ronoctua-unlock`: `pam_unix` through the setuid
//! `unix_chkpwd`, a delay after a failure, and nothing else that could answer
//! for the password), and answers yes or a [`Refusal`]. It is a lock on the
//! doors, not encryption — the page says so.
//!
//! # The secret
//!
//! The password arrives as a [`Secret`] (zeroed when dropped). The
//! conversation hands libpam a `malloc`'d copy of it for each hidden prompt,
//! which libpam overwrites and frees; a copy the conversation made and could
//! not hand over is zeroed before it is freed. There is no other copy: no
//! `CString` of it, never argv, a log line or the wire.
//!
//! # A service that is not there
//!
//! Linux-PAM runs a missing service under `/etc/pam.d/other`, which on NixOS
//! denies everything — so a machine without the service would answer every
//! password "wrong", and the person would retype a right one for ever. The
//! file is looked for first, and its absence is [`Refusal::NoService`], said
//! in a sentence (the `.#dev` loop on a host that never declared it).
//!
//! # Blocks
//!
//! PAM sleeps after a wrong password (`failDelay`): call it on a thread.

mod ffi;

use std::ffi::{CString, c_int};
use std::path::Path;

use ronoctua_consent::keys::Secret;

/// The PAM service every check runs under. Fixed: a caller cannot pick
/// another stack (`pam-service-is-declared` compares it with NixOS).
pub const SERVICE: &str = "ronoctua-unlock";

/// Where Linux-PAM looks for a service's stack first, and where NixOS writes
/// every one.
pub const CONFIG_DIR: &str = "/etc/pam.d";

/// Why the password was not accepted, as a closed set
/// (`player-adult-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// PAM said no: a wrong password, an unknown user, an expired or locked
    /// account.
    Wrong,
    /// There is no `/etc/pam.d/ronoctua-unlock` (the `.#dev` loop on a host
    /// without the service): said in a sentence, never a silent failure.
    NoService,
    /// PAM could not be asked at all: it failed to start, a module could not
    /// be loaded, the helper could not run, memory ran out.
    Unavailable,
}

impl Refusal {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 3] = [Self::Wrong, Self::NoService, Self::Unavailable];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Wrong => "wrong",
            Self::NoService => "no-service",
            Self::Unavailable => "unavailable",
        }
    }
}

/// A refusal, and what PAM said about it — `pam_strerror`'s words and the
/// lines its modules sent — for a log. The words are PAM's (data, like a
/// journal line); what a person reads is the refusal's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    /// Which refusal.
    pub refusal: Refusal,
    /// PAM's words.
    pub said: String,
}

/// Whether `secret` is `user`'s system password (authentication, then
/// account management), under [`SERVICE`]. Blocks.
///
/// # Errors
///
/// [`Refusal::Wrong`], [`Refusal::NoService`], [`Refusal::Unavailable`].
pub fn verify(user: &str, secret: &Secret) -> Result<(), Refusal> {
    verify_saying(user, secret).map_err(|told| told.refusal)
}

/// [`verify`], with PAM's words beside the refusal.
///
/// # Errors
///
/// As [`verify`].
pub fn verify_saying(user: &str, secret: &Secret) -> Result<(), Told> {
    verify_in(Path::new(CONFIG_DIR), user, secret)
}

/// [`verify_saying`], with the service looked for in `config_dir` — the test's
/// way to say "there is no service" without a machine that lacks one.
fn verify_in(config_dir: &Path, user: &str, secret: &Secret) -> Result<(), Told> {
    if !declared_in(config_dir) {
        return Err(Told {
            refusal: Refusal::NoService,
            said: format!("{} is not there", config_dir.join(SERVICE).display()),
        });
    }
    // A NUL cannot be in a user's name or a password PAM compares: C would
    // cut the text there and check a prefix. Refused before PAM is asked.
    let (Ok(user), false) = (CString::new(user), secret.expose().contains('\0')) else {
        return Err(Told {
            refusal: Refusal::Wrong,
            said: "a NUL in the user or the password".to_owned(),
        });
    };
    let service = CString::new(SERVICE).map_err(|_| Told {
        refusal: Refusal::Unavailable,
        said: "the service's name".to_owned(),
    })?;
    ffi::verify_with_pam(&service, &user, secret.expose().as_bytes()).map_err(|ended| Told {
        refusal: refusal_of(ended.step, ended.code),
        said: ended.words,
    })
}

/// Whether the stack [`SERVICE`] names is declared in `config_dir`.
fn declared_in(config_dir: &Path) -> bool {
    config_dir.join(SERVICE).is_file()
}

/// Which refusal a PAM status is, by the call it came from. Pure.
///
/// What the person can change by typing again is [`Refusal::Wrong`]: a wrong
/// password, an unknown user, too many tries, and an account PAM will not let
/// in (expired, locked, a password that must be changed first). Everything
/// else — PAM not starting, a module that will not load, the setuid helper
/// that cannot run, a conversation that failed, memory — is
/// [`Refusal::Unavailable`]: typing again would not help, and saying "wrong"
/// would make the person doubt a right password.
fn refusal_of(step: ffi::Step, code: c_int) -> Refusal {
    use ffi::{
        PAM_ABORT, PAM_ACCT_EXPIRED, PAM_AUTH_ERR, PAM_AUTHINFO_UNAVAIL, PAM_BUF_ERR, PAM_CONV_ERR,
        PAM_CRED_INSUFFICIENT, PAM_MAXTRIES, PAM_NEW_AUTHTOK_REQD, PAM_OPEN_ERR, PAM_PERM_DENIED,
        PAM_SERVICE_ERR, PAM_SYMBOL_ERR, PAM_SYSTEM_ERR, PAM_USER_UNKNOWN, Step,
    };
    match (step, code) {
        (
            Step::Authenticate | Step::Account,
            PAM_AUTH_ERR | PAM_USER_UNKNOWN | PAM_MAXTRIES | PAM_CRED_INSUFFICIENT
            | PAM_PERM_DENIED,
        )
        | (Step::Account, PAM_ACCT_EXPIRED | PAM_NEW_AUTHTOK_REQD) => Refusal::Wrong,
        // NAMED, so the mapping reads as a decision and not as a fall-through:
        // a module or the library failing, the helper unable to answer, the
        // conversation or memory giving out.
        (
            Step::Authenticate | Step::Account,
            PAM_OPEN_ERR | PAM_SYMBOL_ERR | PAM_SERVICE_ERR | PAM_SYSTEM_ERR | PAM_BUF_ERR
            | PAM_AUTHINFO_UNAVAIL | PAM_CONV_ERR | PAM_ABORT,
        )
        // Whatever `pam_start` said, nothing was asked of the person.
        | (Step::Start, _) => Refusal::Unavailable,
        // A status this does not know is not the person's password: saying
        // "wrong" would make them doubt a right one.
        _ => Refusal::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-pam-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_service_is_fixed_and_every_refusal_has_a_name() {
        assert_eq!(SERVICE, "ronoctua-unlock");
        let ids: Vec<&str> = Refusal::ALL.iter().map(|r| r.id()).collect();
        assert_eq!(ids, ["wrong", "no-service", "unavailable"]);
    }

    #[test]
    fn a_missing_service_is_said_and_pam_is_never_asked() {
        let dir = scratch("absent");
        let told = verify_in(&dir, "someone", &Secret::new("x")).unwrap_err();
        assert_eq!(told.refusal, Refusal::NoService);
        assert!(told.said.contains("ronoctua-unlock"));
        // A DIRECTORY of that name is not a stack either.
        std::fs::create_dir(dir.join(SERVICE)).unwrap();
        assert_eq!(
            verify_in(&dir, "someone", &Secret::new("x"))
                .unwrap_err()
                .refusal,
            Refusal::NoService
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_nul_is_refused_before_pam() {
        let dir = scratch("nul");
        std::fs::write(dir.join(SERVICE), "auth required pam_deny.so\n").unwrap();
        assert_eq!(
            verify_in(&dir, "some\0one", &Secret::new("x"))
                .unwrap_err()
                .refusal,
            Refusal::Wrong
        );
        assert_eq!(
            verify_in(&dir, "someone", &Secret::new("pass\0word"))
                .unwrap_err()
                .refusal,
            Refusal::Wrong
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn what_typing_again_can_change_is_wrong_and_the_rest_unavailable() {
        use ffi::*;
        for code in [
            PAM_AUTH_ERR,
            PAM_USER_UNKNOWN,
            PAM_MAXTRIES,
            PAM_CRED_INSUFFICIENT,
            PAM_PERM_DENIED,
        ] {
            assert_eq!(
                refusal_of(Step::Authenticate, code),
                Refusal::Wrong,
                "{code}"
            );
            assert_eq!(refusal_of(Step::Account, code), Refusal::Wrong, "{code}");
        }
        for code in [PAM_ACCT_EXPIRED, PAM_NEW_AUTHTOK_REQD] {
            assert_eq!(refusal_of(Step::Account, code), Refusal::Wrong, "{code}");
        }
        for code in [
            PAM_OPEN_ERR,
            PAM_SYMBOL_ERR,
            PAM_SERVICE_ERR,
            PAM_SYSTEM_ERR,
            PAM_BUF_ERR,
            PAM_AUTHINFO_UNAVAIL,
            PAM_CONV_ERR,
            PAM_ABORT,
        ] {
            assert_eq!(
                refusal_of(Step::Authenticate, code),
                Refusal::Unavailable,
                "{code}"
            );
            assert_eq!(
                refusal_of(Step::Account, code),
                Refusal::Unavailable,
                "{code}"
            );
        }
        // Whatever `pam_start` said, nothing was asked of the person.
        assert_eq!(refusal_of(Step::Start, PAM_AUTH_ERR), Refusal::Unavailable);
        assert_eq!(refusal_of(Step::Start, PAM_BUF_ERR), Refusal::Unavailable);
    }

    // NO TEST RUNS A WHOLE TRANSACTION. Linux-PAM reads the stack from
    // `/etc/pam.d`, which a test cannot write, and a transaction under the
    // machine's `other` stack would log a warning into the machine's journal
    // (`pam_warn`) on every run — a test touching the machine. The right and
    // wrong passwords and the absent service are answered for real by the VM
    // test; here the conversation (`ffi::tests`), the mapping and the refusals
    // before PAM are tested in isolation, and `pam_strerror` proves the link.
}
