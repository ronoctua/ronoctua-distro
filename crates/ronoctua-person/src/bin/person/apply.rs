//! `apply`: the staged tree, validated, becomes the machine's `person/` —
//! whole, or not at all.
//!
//! 0. The caller holds root's lock ([`ronoctua_person::root::Lock`]), so two
//!    applies, an apply and an approval, or an apply and the update's verify
//!    and rebuild never interleave — and keeps it until the verdict is
//!    written.
//! 1. The machine directory is opened (a folder, not a link, ours and
//!    writable by nobody else).
//! 2. Root's pin is read ([`crate::pin::read`]): the person's signing key
//!    this machine trusts, if one is pinned.
//! 3. The staging's nonce is read; what the person signed (the manifest and
//!    its signature); the staged folder is walked ([`crate::walk`]); the
//!    icons the declarations name are read by name; the nonce is read again
//!    — another one means the tree was staged again while it was read, and
//!    it is refused. What it held is assembled
//!    ([`ronoctua_person::tree::assemble`]) — in memory, before a byte is
//!    written. Any refusal ends here, and the machine's data is what it was:
//!    an older machine meeting a newer file keeps its old copy, and the
//!    update goes on with it. A brain module that could not be read is left
//!    out and the machine keeps its previous copy of it.
//! 4. With a key pinned, the manifest is opened with it: missing, not the
//!    key's, or of a counter lower than the last one this machine took, the
//!    whole staging is refused — and the pin says so, for the machine's
//!    report. What the signature holds, file by file, is what arrives
//!    ([`ronoctua_person::tree::hold_to_signature`]); with no key pinned,
//!    the declarations arrive and the options and the brain modules wait.
//!    The pin records the counter taken and what was kept back.
//! 5. If the machine already holds exactly that tree, nothing is written.
//! 6. Otherwise `person.next` is written, synced, and EXCHANGED with
//!    `person` in one `renameat2(RENAME_EXCHANGE)` — a reader sees the old
//!    tree or the new, never half of each — and the old one is removed.

use std::os::fd::OwnedFd;

use ronoctua_person::local::{SignatureRefused, Signer};
use ronoctua_person::tree::{self, Done, Outcome, Refused, Tree, TreeWhy, Trust};
use ronoctua_person::{Stamp, names, tree::staged_place};
use ronoctua_pq::sign::{Manifest, PublicKey};
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use rustix::io::Errno;

use crate::disk::{self, Whose};
use crate::walk;

/// The name the next tree is written under, beside `person`.
const NEXT: &str = "person.next";

/// The machine directory: a folder, not a link, ours, writable by nobody
/// else. (Who may touch it NOW is root's lock's question, not this folder's:
/// a lock on a folder anybody may read could be held by anybody.)
pub fn open_machine(machine: &str) -> Result<OwnedFd, Refused> {
    let me = Whose {
        owner: rustix::process::geteuid().as_raw(),
    };
    disk::open_root(machine, me).map_err(|r| match r.why {
        TreeWhy::NotOwned | TreeWhy::Writable => Refused::new(TreeWhy::Unwritable, r.at, r.detail),
        _ => r,
    })
}

/// The machine's `person/` as it is now (an empty tree when there is none).
pub fn current(machine: &OwnedFd) -> Result<Tree, Refused> {
    let me = Whose {
        owner: rustix::process::geteuid().as_raw(),
    };
    match rustix::fs::statat(machine, names::MACHINE_PERSON, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => return Ok(Tree::default()),
        Err(e) => return Err(disk::refusal_of(e, names::MACHINE_PERSON)),
        Ok(_) => {}
    }
    let fd = rustix::fs::openat(
        machine,
        names::MACHINE_PERSON,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| disk::refusal_of(e, names::MACHINE_PERSON))?;
    let read = walk::walk(&fd, &walk::everything, me)?;
    Ok(Tree {
        dirs: read.dirs,
        files: read.files,
    })
}

/// Writes `tree` as `person.next` in `machine`: folders `0755`, files
/// `0644`, every file synced.
fn write_next(machine: &OwnedFd, tree: &Tree) -> Result<(), Refused> {
    disk::remove_tree(machine, NEXT)?;
    let next = disk::dir_in(machine, NEXT, 0o755)?;
    rustix::fs::fchmod(&next, Mode::from_raw_mode(0o755)).map_err(|e| disk::unwritable(NEXT, e))?;
    // Parents first: a sorted set puts `software` before `software/icons`.
    for dir in &tree.dirs {
        let fd = disk::dir_in(&next, dir, 0o755)?;
        rustix::fs::fchmod(&fd, Mode::from_raw_mode(0o755))
            .map_err(|e| disk::unwritable(dir, e))?;
    }
    for (path, bytes) in &tree.files {
        match path.rsplit_once('/') {
            None => disk::write_whole(&next, path, bytes, 0o644)?,
            Some((parent, name)) => {
                disk::write_whole(&disk::dir_in(&next, parent, 0o755)?, name, bytes, 0o644)?;
            }
        }
    }
    rustix::fs::fsync(&next).map_err(|e| disk::unwritable(NEXT, e))
}

/// Puts `person.next` in `person`'s place in one step, and removes the old.
fn swap(machine: &OwnedFd) -> Result<(), Refused> {
    let person = names::MACHINE_PERSON;
    match rustix::fs::statat(machine, person, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => {
            rustix::fs::renameat_with(machine, NEXT, machine, person, RenameFlags::EXCHANGE)
                .map_err(|e| disk::unwritable(person, e))?;
            // The OLD tree is `person.next` now. Failing to remove it leaves
            // the new data in place; the next apply clears it first.
            if let Err(r) = disk::remove_tree(machine, NEXT) {
                eprintln!("<4>the previous person data was not removed: {r}");
            }
        }
        Err(Errno::NOENT) => rustix::fs::renameat(machine, NEXT, machine, person)
            .map_err(|e| disk::unwritable(person, e))?,
        Err(e) => return Err(disk::unwritable(person, e)),
    }
    rustix::fs::fsync(machine).map_err(|e| disk::unwritable(person, e))
}

/// Applies the tree staged at `staged` (owned by `owner`) to the machine
/// directory `machine` at `at`: the nonce of the staging read (when it was
/// read), and what the apply came to.
pub fn apply(
    staged: &str,
    machine: &str,
    owner: u32,
    at: &Stamp,
) -> (Option<String>, Result<Done, Refused>) {
    let mut nonce = None;
    let done = applied(staged, machine, owner, at, &mut nonce);
    (nonce, done)
}

/// The manifest `signed` holds, opened with the pinned `key` and not rolled
/// back below what `signer` took.
///
/// # Errors
///
/// [`TreeWhy::NotSigned`], [`TreeWhy::BadSignature`],
/// [`TreeWhy::RolledBack`].
fn opened(
    signed: Option<&(Vec<u8>, Vec<u8>)>,
    key: &PublicKey,
    signer: &Signer,
) -> Result<Manifest, Refused> {
    let Some((json, signature)) = signed else {
        return Err(Refused::new(
            TreeWhy::NotSigned,
            tree::MANIFEST,
            "a key is pinned, and the staging carries no signature",
        ));
    };
    let manifest = ronoctua_pq::sign::open_manifest(json, signature, key).map_err(|r| {
        Refused::new(
            TreeWhy::BadSignature,
            tree::SIGNATURE,
            format!("not the pinned key's signature over the manifest ({r})"),
        )
    })?;
    if manifest.counter < signer.counter {
        return Err(Refused::new(
            TreeWhy::RolledBack,
            tree::MANIFEST,
            format!(
                "counter {}, and this machine took {}",
                manifest.counter, signer.counter
            ),
        ));
    }
    Ok(manifest)
}

fn applied(
    staged: &str,
    machine: &str,
    owner: u32,
    at: &Stamp,
    nonce: &mut Option<String>,
) -> Result<Done, Refused> {
    let machine_fd = open_machine(machine)?;
    // ROOT'S PIN FIRST: what this machine trusts is read before anything of
    // the person's is.
    let pin = crate::pin::read(&machine_fd)?;
    let whose = Whose { owner };
    let root = disk::open_root(staged, whose)?;
    *nonce = walk::read_nonce(&root, whose)?;
    let signed = walk::read_signed(&root, whose)?;
    let mut read = walk::walk(&root, &staged_place, whose)?;
    let named = tree::named_icons(&read)?;
    walk::read_icons(&root, &named, whose, &mut read)?;
    tree::same_staging(nonce.as_deref(), walk::read_nonce(&root, whose)?.as_deref())?;
    let mut assembled = tree::assemble(&read)?;
    // THE SIGNATURE, against the pin: a staging it does not hold is refused
    // whole — and the pin says why, for the machine's report.
    let manifest = match &pin {
        None => None,
        Some((signer, key)) => match opened(signed.as_ref(), key, signer) {
            Ok(m) => Some(m),
            Err(refused) => {
                let said = Signer {
                    refused: Some(SignatureRefused {
                        at: at.clone(),
                        why: refused.why,
                        detail: refused.detail.clone(),
                    }),
                    kept_back: Vec::new(),
                    ..signer.clone()
                };
                if let Err(e) = crate::pin::write(&machine_fd, &said) {
                    eprintln!("<4>the pin could not say why the staging was refused: {e}");
                }
                return Err(refused);
            }
        },
    };
    // The machine's copy is read for three things: the documents the
    // signature does not vouch for keep theirs, the modules left out keep
    // theirs, and an identical tree is not written again. A copy that cannot
    // be read back (somebody edited it by hand) keeps nothing and is simply
    // replaced.
    let now = current(&machine_fd);
    let none = Tree::default();
    let trust = manifest
        .as_ref()
        .map_or(Trust::Unpinned, |m| Trust::Signed(&m.files));
    tree::hold_to_signature(
        &mut assembled,
        &read,
        now.as_ref().unwrap_or(&none),
        trust,
        &ronoctua_pq::sign::sha256_hex,
    );
    if let Ok(now) = &now {
        tree::carry_modules(&mut assembled.tree, now, &mut assembled.left_out);
    }
    // THE PIN REMEMBERS what this apply took — before the swap, so a counter
    // is never behind a tree the machine holds.
    if let (Some((signer, _)), Some(m)) = (&pin, &manifest) {
        let took = Signer {
            counter: signer.counter.max(m.counter),
            refused: None,
            kept_back: assembled.kept_back.clone(),
            ..signer.clone()
        };
        if took != *signer {
            crate::pin::write(&machine_fd, &took)?;
        }
    }
    let outcome = if now.is_ok_and(|now| now == assembled.tree) {
        Outcome::Unchanged
    } else {
        write_next(&machine_fd, &assembled.tree)?;
        swap(&machine_fd)?;
        Outcome::Applied
    };
    Ok(Done {
        outcome,
        ignored: assembled.ignored,
        left_out: assembled.left_out,
        kept_back: assembled.kept_back,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Scratch;

    const ICON: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn me() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    fn staged_good(s: &Scratch) {
        s.file(
            "staged/person/software/declared.json",
            include_bytes!("../../../fixtures/good/declared.json"),
        );
        s.file(
            &format!("staged/person/software/icons/{ICON}.png"),
            b"\x89PNG\r\n\x1a\nicon",
        );
        s.file("staged/person/machines/vm.json", b"{}");
        s.file("staged/brain-modules/my-mod/module.nix", b"{ }\n");
        s.dir("machine");
    }

    fn at() -> Stamp {
        Stamp::parse("2026-10-11T12:00:00Z").unwrap()
    }

    /// Every file under `dir`, by its path relative to it.
    fn files_under(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                let rel = p.strip_prefix(dir).unwrap().display().to_string();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push((rel, std::fs::read(&p).unwrap()));
                }
            }
        }
        out
    }

    /// Signs the staging as the person would, with `key` at `counter`: the
    /// manifest of every file the signature covers, and its signature.
    fn sign_staged(s: &Scratch, key: &ronoctua_pq::sign::SigningKey, counter: u64) {
        let files = files_under(&s.0.join("staged"))
            .into_iter()
            .filter(|(rel, _)| tree::signed(rel))
            .map(|(rel, bytes)| (rel, ronoctua_pq::sign::sha256_hex(&bytes)))
            .collect();
        let manifest = Manifest {
            format: ronoctua_pq::sign::MANIFEST_FORMAT,
            counter,
            signed_at_s: 1_760_000_000,
            signer: String::new(),
            files,
        };
        let (json, signature) = ronoctua_pq::sign::sign_manifest(key, &manifest).unwrap();
        s.file(&format!("staged/{}", tree::MANIFEST), &json);
        s.file(&format!("staged/{}", tree::SIGNATURE), &signature);
    }

    /// Pins `key` in the machine directory.
    fn pin_key(s: &Scratch, key: &ronoctua_pq::sign::SigningKey) {
        crate::pin::pin(s.0.join("machine").to_str().unwrap(), key.public(), at()).unwrap();
    }

    /// A good staging, signed at `counter` by a key the machine pinned.
    fn signed_good(s: &Scratch) -> ronoctua_pq::sign::SigningKey {
        staged_good(s);
        let key = ronoctua_pq::sign::SigningKey::generate().unwrap();
        pin_key(s, &key);
        sign_staged(s, &key, 1);
        key
    }

    /// The machine's pin, read back.
    fn pin_of(s: &Scratch) -> Signer {
        let fd = open_machine(s.0.join("machine").to_str().unwrap()).unwrap();
        crate::pin::read(&fd).unwrap().unwrap().0
    }

    fn applied(s: &Scratch) -> Result<Done, Refused> {
        apply(
            s.0.join("staged").to_str().unwrap(),
            s.0.join("machine").to_str().unwrap(),
            me(),
            &at(),
        )
        .1
    }

    /// FAILS IF: a good tree is not swapped in whole, a second apply of the
    /// same tree writes again, or `person.next` is left behind.
    #[test]
    fn a_good_tree_is_swapped_in_and_a_second_apply_changes_nothing() {
        let s = Scratch::new("apply-good");
        let key = signed_good(&s);
        let done = applied(&s).unwrap();
        assert_eq!(done.outcome, Outcome::Applied);
        assert_eq!(done.ignored, ["person/machines"]);
        assert!(done.left_out.is_empty());
        let person = s.0.join("machine/person");
        assert_eq!(
            std::fs::read(person.join("software/declared.json")).unwrap(),
            include_bytes!("../../../fixtures/good/declared.json")
        );
        assert!(person.join(format!("software/icons/{ICON}.png")).is_file());
        assert!(person.join("brain/my-mod/module.nix").is_file());
        assert!(!s.0.join("machine/person.next").exists());
        assert_eq!(applied(&s).unwrap().outcome, Outcome::Unchanged);

        // An entry forgotten: the swap takes the old tree away whole.
        s.file(
            "staged/person/software/declared.json",
            br#"{"format": 1, "apps": []}"#,
        );
        sign_staged(&s, &key, 2);
        assert_eq!(applied(&s).unwrap().outcome, Outcome::Applied);
        assert_eq!(pin_of(&s).counter, 2, "the pin did not take the counter");
        assert!(
            !person.join("software/icons").exists(),
            "its icon left with it"
        );
        assert_eq!(
            std::fs::read_to_string(person.join("software/declared.json")).unwrap(),
            "{\n  \"format\": 1,\n  \"apps\": []\n}\n",
            "rewritten canonical"
        );
    }

    fn snapshot(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                let rel = p.strip_prefix(dir).unwrap().display().to_string();
                if p.is_dir() {
                    stack.push(p);
                    out.push((rel, Vec::new()));
                } else {
                    out.push((rel, std::fs::read(&p).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    /// FAILS IF: a refused tree touches the machine's data — a newer file, a
    /// link, an empty staging folder — or leaves a half-written tree.
    #[test]
    fn a_refused_tree_leaves_the_machines_data_as_it_was() {
        let s = Scratch::new("apply-refused");
        staged_good(&s);
        applied(&s).unwrap();
        let before = snapshot(&s.0.join("machine"));

        s.file(
            "staged/person/software/declared.json",
            include_bytes!("../../../fixtures/bad/declared-newer.json"),
        );
        assert_eq!(
            applied(&s).unwrap_err().why,
            TreeWhy::Document(ronoctua_person::Why::Newer)
        );
        assert_eq!(snapshot(&s.0.join("machine")), before);

        std::fs::remove_file(s.0.join("staged/person/software/declared.json")).unwrap();
        std::os::unix::fs::symlink(
            "/etc/passwd",
            s.0.join("staged/person/software/declared.json"),
        )
        .unwrap();
        assert_eq!(applied(&s).unwrap_err().why, TreeWhy::Link);
        assert_eq!(snapshot(&s.0.join("machine")), before);

        let empty = Scratch::new("apply-empty");
        empty.dir("staged");
        empty.dir("machine/person/software");
        empty.file("machine/person/software/declared.json", b"kept");
        assert_eq!(
            apply(
                empty.0.join("staged").to_str().unwrap(),
                empty.0.join("machine").to_str().unwrap(),
                me(),
                &at(),
            )
            .1
            .unwrap_err()
            .why,
            TreeWhy::NothingStaged
        );
        assert_eq!(
            std::fs::read(empty.0.join("machine/person/software/declared.json")).unwrap(),
            b"kept"
        );
    }

    /// FAILS IF: a machine copy somebody edited by hand — with a link in it
    /// — stops the apply, or the link is followed when the old copy goes.
    #[test]
    fn a_copy_edited_by_hand_is_replaced_and_its_link_never_followed() {
        let s = Scratch::new("apply-hand");
        staged_good(&s);
        s.file("target", b"not root's to remove");
        s.dir("machine/person/software");
        std::os::unix::fs::symlink(s.0.join("target"), s.0.join("machine/person/software/x"))
            .unwrap();
        assert_eq!(applied(&s).unwrap().outcome, Outcome::Applied);
        assert!(s.0.join("machine/person/software/declared.json").is_file());
        assert!(std::fs::symlink_metadata(s.0.join("machine/person/software/x")).is_err());
        assert_eq!(
            std::fs::read(s.0.join("target")).unwrap(),
            b"not root's to remove"
        );
    }

    /// FAILS IF: an icon no entry names — a link, or over its cap — is
    /// opened and refuses the tree (it must never be read at all).
    #[test]
    fn an_icon_nobody_names_is_never_opened() {
        let s = Scratch::new("apply-stale-icon");
        staged_good(&s);
        s.file(
            &format!("staged/person/software/icons/{}.png", "a".repeat(64)),
            &vec![0; 300_000],
        );
        std::os::unix::fs::symlink(
            "/etc/shadow",
            s.0.join(format!(
                "staged/person/software/icons/{}.png",
                "b".repeat(64)
            )),
        )
        .unwrap();
        let done = applied(&s).unwrap();
        assert_eq!(done.outcome, Outcome::Applied);
        let icons: Vec<_> = std::fs::read_dir(s.0.join("machine/person/software/icons"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(icons, [format!("{ICON}.png")], "only the named one");
    }

    /// FAILS IF: a link inside one brain module stops the declarations, or
    /// takes the machine's copy of that module away.
    #[test]
    fn a_module_that_cannot_be_read_stays_as_the_machine_had_it() {
        let s = Scratch::new("apply-module");
        let key = signed_good(&s);
        applied(&s).unwrap();
        s.file("staged/brain-modules/my-mod/module.nix", b"{ changed }\n");
        std::os::unix::fs::symlink("/etc/passwd", s.0.join("staged/brain-modules/my-mod/x"))
            .unwrap();
        s.file(
            "staged/brain-modules/my-mod/.module.nix.swp",
            b"vim was here",
        );
        s.file(
            "staged/person/software/declared.json",
            br#"{"format": 1, "apps": []}"#,
        );
        sign_staged(&s, &key, 2);
        let done = applied(&s).unwrap();
        assert_eq!(done.outcome, Outcome::Applied, "the declarations arrived");
        assert_eq!(done.left_out.len(), 1);
        assert_eq!(done.left_out[0].module, "my-mod");
        assert_eq!(done.left_out[0].why, TreeWhy::Link);
        assert!(done.left_out[0].kept);
        let person = s.0.join("machine/person");
        assert_eq!(
            std::fs::read(person.join("brain/my-mod/module.nix")).unwrap(),
            b"{ }\n",
            "the copy the machine had"
        );
        assert!(!person.join("software/icons").exists());

        // The dropping alone does not leave a module out.
        std::fs::remove_file(s.0.join("staged/brain-modules/my-mod/x")).unwrap();
        sign_staged(&s, &key, 3);
        let done = applied(&s).unwrap();
        assert!(done.left_out.is_empty(), "{:?}", done.left_out);
        assert!(
            done.ignored
                .contains(&"brain-modules/my-mod/.module.nix.swp".to_owned())
        );
        assert_eq!(
            std::fs::read(person.join("brain/my-mod/module.nix")).unwrap(),
            b"{ changed }\n"
        );
        assert!(!person.join("brain/my-mod/.module.nix.swp").exists());
    }

    /// FAILS IF: the verdict does not carry the staging's nonce, or a nonce
    /// that is not one is let through.
    #[test]
    fn the_staging_says_which_it_is() {
        let s = Scratch::new("apply-nonce");
        staged_good(&s);
        s.file("staged/nonce", b"stager-0001\n");
        let (nonce, done) = apply(
            s.0.join("staged").to_str().unwrap(),
            s.0.join("machine").to_str().unwrap(),
            me(),
            &at(),
        );
        assert_eq!(nonce.as_deref(), Some("stager-0001"));
        assert_eq!(done.unwrap().outcome, Outcome::Applied);
        s.file("staged/nonce", b"$(reboot)");
        let (nonce, done) = apply(
            s.0.join("staged").to_str().unwrap(),
            s.0.join("machine").to_str().unwrap(),
            me(),
            &at(),
        );
        assert_eq!(nonce, None);
        assert_eq!(done.unwrap_err().why, TreeWhy::NotANonce);
    }

    /// FAILS IF: a staging the pinned key did not sign — unsigned, signed by
    /// another key, or with one byte of the manifest changed — touches the
    /// machine's data, or the pin does not say why for the machine's report.
    #[test]
    fn a_tree_not_signed_by_the_pinned_key_is_refused() {
        let s = Scratch::new("apply-not-signed");
        let key = signed_good(&s);
        applied(&s).unwrap();
        let before = snapshot(&s.0.join("machine/person"));
        s.file(
            "staged/person/software/declared.json",
            br#"{"format": 1, "apps": []}"#,
        );

        let stranger = ronoctua_pq::sign::SigningKey::generate().unwrap();
        sign_staged(&s, &stranger, 2);
        assert_eq!(applied(&s).unwrap_err().why, TreeWhy::BadSignature);
        assert_eq!(snapshot(&s.0.join("machine/person")), before);
        let said = pin_of(&s).refused.unwrap();
        assert_eq!(said.why, TreeWhy::BadSignature);

        sign_staged(&s, &key, 2);
        let manifest = s.0.join(format!("staged/{}", tree::MANIFEST));
        let text = std::fs::read_to_string(&manifest).unwrap();
        s.file(
            &format!("staged/{}", tree::MANIFEST),
            text.replacen("\"counter\": 2", "\"counter\": 3", 1)
                .as_bytes(),
        );
        assert_eq!(applied(&s).unwrap_err().why, TreeWhy::BadSignature);

        std::fs::remove_file(s.0.join(format!("staged/{}", tree::SIGNATURE))).unwrap();
        assert_eq!(applied(&s).unwrap_err().why, TreeWhy::NotSigned);
        assert_eq!(snapshot(&s.0.join("machine/person")), before);
        assert_eq!(pin_of(&s).refused.unwrap().why, TreeWhy::NotSigned);

        // SIGNED AGAIN BY THE PINNED KEY: it arrives, and the refusal goes.
        sign_staged(&s, &key, 2);
        assert_eq!(applied(&s).unwrap().outcome, Outcome::Applied);
        assert_eq!(pin_of(&s).refused, None);
    }

    /// FAILS IF: an older tree the person really signed, played again with
    /// a lower counter than the machine took, is applied.
    #[test]
    fn a_rollback_is_refused() {
        let s = Scratch::new("apply-rollback");
        let key = signed_good(&s);
        sign_staged(&s, &key, 5);
        applied(&s).unwrap();
        assert_eq!(pin_of(&s).counter, 5);
        let before = snapshot(&s.0.join("machine/person"));
        s.file(
            "staged/person/software/declared.json",
            br#"{"format": 1, "apps": []}"#,
        );
        sign_staged(&s, &key, 4);
        let r = applied(&s).unwrap_err();
        assert_eq!(r.why, TreeWhy::RolledBack);
        assert_eq!(snapshot(&s.0.join("machine/person")), before);
        assert_eq!(pin_of(&s).counter, 5);
        // THE SAME COUNTER is not lower: two machines may sign one each.
        sign_staged(&s, &key, 5);
        assert_eq!(applied(&s).unwrap().outcome, Outcome::Applied);
    }

    /// FAILS IF: a brain module the person did not sign reaches the
    /// machine's copy (what the brain is compiled from) — on a machine with
    /// a key pinned, or on one with none.
    #[test]
    fn an_unsigned_module_never_reaches_the_machine() {
        let s = Scratch::new("apply-unsigned-module");
        let key = signed_good(&s);
        s.file(
            "staged/brain-modules/my-evil/module.rs",
            b"pub fn evil() {}",
        );
        let done = applied(&s).unwrap();
        assert!(
            done.left_out
                .iter()
                .any(|l| l.module == "my-evil" && l.why == TreeWhy::NotAsSigned),
            "{:?}",
            done.left_out
        );
        let person = s.0.join("machine/person");
        assert!(
            person.join("brain/my-mod/module.nix").is_file(),
            "the signed one"
        );
        assert!(
            !person.join("brain/my-evil").exists(),
            "an unsigned module arrived"
        );
        // ONE FILE OF A SIGNED MODULE changed after the signature.
        s.file(
            "staged/brain-modules/my-mod/module.nix",
            b"{ evil = true; }\n",
        );
        let done = applied(&s).unwrap();
        assert!(done.left_out.iter().any(|l| l.module == "my-mod" && l.kept));
        assert_eq!(
            std::fs::read(person.join("brain/my-mod/module.nix")).unwrap(),
            b"{ }\n"
        );
        let _ = key;

        // NO KEY PINNED: no module at all.
        let bare = Scratch::new("apply-unpinned-module");
        staged_good(&bare);
        let done = applied(&bare).unwrap();
        assert!(done.left_out.iter().any(|l| l.why == TreeWhy::NoKeyPinned));
        assert!(!bare.0.join("machine/person/brain").exists());
        assert!(
            bare.0
                .join("machine/person/software/declared.json")
                .is_file()
        );
    }

    /// FAILS IF: a document changed after the person signed is taken, or
    /// takes the rest of the staging with it, or the verdict and the pin do
    /// not say what was kept back.
    #[test]
    fn a_document_not_as_signed_is_kept_back() {
        let s = Scratch::new("apply-kept-back");
        let key = signed_good(&s);
        s.file(
            "staged/person/options.json",
            br#"{"format": 1, "ssh-signing": null}"#,
        );
        sign_staged(&s, &key, 2);
        applied(&s).unwrap();
        // A PIN MOVED WHILE THE IDENTITY WAS LOCKED: the declaration is not
        // what was signed; the options are.
        s.file(
            "staged/person/software/declared.json",
            br#"{"format": 1, "apps": []}"#,
        );
        let done = applied(&s).unwrap();
        assert_eq!(done.kept_back.len(), 1);
        assert_eq!(done.kept_back[0].at, "person/software/declared.json");
        assert_eq!(
            std::fs::read(s.0.join("machine/person/software/declared.json")).unwrap(),
            include_bytes!("../../../fixtures/good/declared.json"),
            "the unsigned declaration was taken"
        );
        assert!(s.0.join("machine/person/options.json").is_file());
        assert_eq!(pin_of(&s).kept_back, done.kept_back);
    }

    #[test]
    fn a_staged_tree_of_somebody_else_is_refused() {
        let s = Scratch::new("apply-owner");
        staged_good(&s);
        let r = apply(
            s.0.join("staged").to_str().unwrap(),
            s.0.join("machine").to_str().unwrap(),
            me() + 1,
            &at(),
        )
        .1
        .unwrap_err();
        assert_eq!(r.why, TreeWhy::NotOwned);
        assert!(!s.0.join("machine/person").exists());
    }
}
