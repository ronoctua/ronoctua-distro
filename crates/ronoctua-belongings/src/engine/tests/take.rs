//! One sealed file for the installer (`take`), and the push token's sealed
//! home — against a real git and a `file://` bare remote.
//!
//! What these prove: the person's push token (`ronoctua_cachix::token`)
//! lies in the SEALED part — kept by the `config` tree, never under a plain
//! root, never taken out by a `never` — so it travels encrypted and the
//! repository holds no byte of it in the clear; and `take` gives the
//! installer that one file, `0600`, before `nixos-install`, with the
//! passphrase, which it deletes however it ends — `false` for a file the
//! repository does not keep, and a refusal for a wrong passphrase.

use std::os::unix::fs::PermissionsExt as _;

use super::*;

/// A token as Cachix hands one out (a JWT's shape).
const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJqdGkiOiJ0YWtlIn0.c2lnbmF0dXJlLW9mLXRoZS10b2tlbg";

/// FAILS IF: the push token is kept anywhere but the sealed part — in
/// clear under a plain root, taken out by a `never`, or not kept at all.
#[test]
fn belongings_token_is_sealed() {
    let logical = ronoctua_cachix::token::logical();
    assert_eq!(logical, "config/binary-cache-keys/push-token");
    assert!(
        matches!(kept::by_default(&logical), kept::Verdict::Kept(_)),
        "{:?}",
        kept::by_default(&logical)
    );
    assert!(!kept::is_plain(&logical), "the token would travel in clear");
    assert!(kept::plain_root(&logical).is_none());
}

/// FAILS IF: `take` gives the installer anything but the one file it names,
/// whole and `0600` — or leaves the passphrase behind, or takes a file the
/// repository does not keep, or opens with a wrong passphrase.
#[test]
fn take_gives_the_installer_one_sealed_file_and_forgets_the_passphrase() {
    let root = scratch("take");
    let remote_dir = root.join("remote.git");
    let remote = bare(&remote_dir);
    let a = Machine::new(&root, "machine-alpha", &remote);
    let logical = ronoctua_cachix::token::logical();
    a.write(&logical, TOKEN);
    std::fs::set_permissions(
        a.places.resolve(&logical).unwrap(),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    a.write("config/theme.json", "{\"theme\":\"owl\"}");
    let outcome = a.sync();
    assert!(matches!(outcome, Outcome::Synced { .. }), "{outcome:?}");
    // NOT A BYTE OF IT IN THE CLEAR.
    let (_, bytes) = remote_files(&remote_dir, &root);
    assert!(bytes.iter().all(|b| !contains(b, TOKEN.as_bytes())));

    // THE INSTALLER'S SCRATCH HOME: nothing but the settings.
    let b = Machine::new(&root, "installer", &remote);
    let to = root.join("scratch").join("binary-cache").join("token");
    b.say(PASSPHRASE);
    assert_eq!(b.engine().take(&logical, &to, &mut |_| {}), Ok(true));
    assert_eq!(std::fs::read_to_string(&to).unwrap(), TOKEN);
    assert_eq!(
        std::fs::metadata(&to).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(
        !b.places.passphrase().exists(),
        "the passphrase outlived take"
    );
    assert!(
        b.read(&logical).is_none(),
        "take restored into the home: it writes only where it is told"
    );

    // A FILE THE REPOSITORY DOES NOT KEEP.
    b.say(PASSPHRASE);
    assert_eq!(
        b.engine().take(
            "config/binary-cache-keys/none",
            &root.join("none"),
            &mut |_| {}
        ),
        Ok(false)
    );
    assert!(!root.join("none").exists());

    // A WRONG PASSPHRASE: refused, nothing written, the file forgotten.
    let again = root.join("again");
    b.say("not the passphrase");
    let refused = b.engine().take(&logical, &again, &mut |_| {});
    assert_eq!(refused.map_err(|r| r.why), Err(Why::WrongPassphrase));
    assert!(!again.exists());
    assert!(!b.places.passphrase().exists());
    let _ = std::fs::remove_dir_all(&root);
}
