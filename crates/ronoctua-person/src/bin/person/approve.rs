//! `approve`: the sudo door's unit — the staged request consumed, the
//! module's hash recomputed by `nix/person-hash.nix` (the single
//! implementation), and the approval recorded only when the two agree. The
//! decision is [`ronoctua_person::approve::decide`]'s; every way out has a
//! reason ([`Denied`]) for the verdict.
//!
//! And `request-approval`, the person's half: the request written where the
//! unit will read it.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

use ronoctua_person::approve::{self as decision, Denied, REQUEST, REQUEST_MOST_BYTES, Request};
use ronoctua_person::local::Approved;
use ronoctua_person::tree::{BRAIN, TreeWhy};
use ronoctua_person::{Document as _, Stamp, names};
use rustix::fs::AtFlags;
use rustix::io::Errno;

use crate::apply;
use crate::disk::{self, Whose};
use crate::nix::Nix;

/// An approval not recorded: why, what root computed (when it did), and the
/// detail for the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    /// Why.
    pub denied: Denied,
    /// The hash root computed, when it got that far.
    pub computed: Option<String>,
    /// What happened, English.
    pub detail: String,
}

impl Failed {
    fn new(denied: Denied, detail: impl Into<String>) -> Self {
        Self {
            denied,
            computed: None,
            detail: detail.into(),
        }
    }
}

/// The request staged in `dir` (the person's folder, owned by `owner`),
/// CONSUMED whatever it is: a request left behind would be approved again by
/// the next start of the unit, which turns one password into a standing one.
pub fn take_request(dir: &str, owner: u32) -> Result<Request, Failed> {
    let whose = Whose { owner };
    let refused = |r: ronoctua_person::tree::Refused| {
        let denied = match r.why {
            TreeWhy::NotOwned | TreeWhy::Writable => Denied::NotOwned,
            _ => Denied::InvalidRequest,
        };
        Failed::new(denied, r.to_string())
    };
    let folder = disk::open_root(dir, whose).map_err(refused)?;
    let name = disk::c_name(REQUEST).map_err(refused)?;
    let was = match rustix::fs::statat(&folder, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) => st,
        Err(Errno::NOENT) => {
            return Err(Failed::new(
                Denied::NoRequest,
                format!("{dir}/{REQUEST}: nothing was staged"),
            ));
        }
        Err(e) => return Err(refused(disk::refusal_of(e, REQUEST))),
    };
    let read = disk::read_at(&folder, &name, &was, whose, REQUEST_MOST_BYTES, REQUEST);
    // `unlinkat` takes a link away and never follows it.
    let _ = rustix::fs::unlinkat(&folder, REQUEST, AtFlags::empty());
    let bytes = read.map_err(refused)?;
    ronoctua_person::parse::<Request>(&bytes)
        .map_err(|r| Failed::new(Denied::InvalidRequest, format!("the request: {r}")))
}

/// The pin root's helper wrote at `path` ([`ronoctua_person::root::APPROVE_PIN`]):
/// a regular file of `owner`'s (root's; a test's own uid), `0600`, never
/// followed through a link — CONSUMED whatever it is, as the request is.
///
/// # Errors
///
/// [`Failed`] with [`Denied::NoRequest`] when there is none, and
/// [`Denied::InvalidRequest`] for one that is not root's private file or
/// does not read as an id and a hash.
pub fn take_pin(path: &str, owner: u32) -> Result<Request, Failed> {
    use std::io::Read as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(flags.bits()).unwrap_or(0))
        .open(path);
    let _ = std::fs::remove_file(path);
    let file = match opened {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Failed::new(
                Denied::NoRequest,
                format!("{path}: the sudo door pinned nothing"),
            ));
        }
        Err(e) => return Err(Failed::new(Denied::InvalidRequest, format!("{path}: {e}"))),
    };
    let meta = file
        .metadata()
        .map_err(|e| Failed::new(Denied::InvalidRequest, format!("{path}: {e}")))?;
    if !meta.is_file()
        || meta.uid() != owner
        || meta.mode() & 0o077 != 0
        || meta.len() > REQUEST_MOST_BYTES
    {
        return Err(Failed::new(
            Denied::InvalidRequest,
            format!("{path} is not root's private pin"),
        ));
    }
    let mut text = String::new();
    file.take(REQUEST_MOST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| Failed::new(Denied::InvalidRequest, format!("{path}: {e}")))?;
    decision::of_pin(&text).ok_or_else(|| {
        Failed::new(
            Denied::InvalidRequest,
            format!("{path} is not an id and a hash"),
        )
    })
}

/// The request staged in the person's folder, held to the pin: what the
/// sudo door was given is what is approved, and a request another process
/// of the person's swapped in meanwhile is refused.
///
/// # Errors
///
/// [`Denied::InvalidRequest`] when the two name another module or another
/// hash.
pub fn held_to_pin(request: Request, pin: &Request) -> Result<Request, Failed> {
    if request.id == pin.id && request.hash == pin.hash {
        Ok(request)
    } else {
        Err(Failed::new(
            Denied::InvalidRequest,
            format!(
                "the staged request ({} at {}) is not what the sudo door was given ({} at {})",
                request.id, request.hash, pin.id, pin.hash
            ),
        ))
    }
}

/// The hash `nix/person-hash.nix` gives the folder `module`, asked of `nix`
/// with the folder in the environment — never spliced into the expression.
pub fn hash_of(nix: &Nix, hasher: &str, module: &Path) -> Result<String, String> {
    let plain = hasher.starts_with('/')
        && hasher
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'+' | b'-'));
    if !plain {
        return Err(format!("the hasher's path {hasher:?} is not a plain path"));
    }
    let expr = format!("import {hasher} (builtins.getEnv \"RONOCTUA_MODULE_DIR\")");
    let out = Command::new(&nix.program)
        .args([
            "eval",
            "--raw",
            "--impure",
            "--expr",
            &expr,
            "--extra-experimental-features",
            "nix-command",
        ])
        .env("RONOCTUA_MODULE_DIR", module)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("nix could not be started: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "nix could not hash {} ({})",
            module.display(),
            out.status
        ));
    }
    String::from_utf8(out.stdout).map_err(|_| "nix printed a hash that is not text".to_owned())
}

/// The brain modules the machine's copy holds.
fn present(tree: &ronoctua_person::tree::Tree) -> BTreeSet<String> {
    tree.dirs
        .iter()
        .filter_map(|d| d.strip_prefix(&format!("{BRAIN}/")).map(str::to_owned))
        .filter(|id| !id.contains('/'))
        .collect()
}

/// Records `request` against the machine directory `machine` — the caller
/// holds root's lock — and answers the hash root computed.
///
/// # Errors
///
/// [`Failed`]: a module the machine does not hold, a hash that could not be
/// computed or is not the one the person was shown, a file not written.
pub fn record(
    request: &Request,
    machine: &str,
    nix: &Nix,
    hasher: &str,
    now: Stamp,
) -> Result<String, Failed> {
    let unwritable = |e: String| Failed::new(Denied::Unwritable, e);
    let machine_fd = apply::open_machine(machine).map_err(|r| unwritable(r.to_string()))?;
    let tree = apply::current(&machine_fd).map_err(|r| unwritable(r.to_string()))?;
    let here = present(&tree);
    if !here.contains(&request.id) {
        return Err(Failed::new(
            Denied::NoSuchModule,
            format!("the machine's copy holds no module {}", request.id),
        ));
    }
    let absolute =
        std::fs::canonicalize(machine).map_err(|e| unwritable(format!("{machine}: {e}")))?;
    let module = absolute
        .join(names::MACHINE_PERSON)
        .join(BRAIN)
        .join(&request.id);
    let computed = hash_of(nix, hasher, &module).map_err(|e| Failed::new(Denied::Unhashable, e))?;
    // A broken approvals file fails CLOSED: nothing in it counts, and a
    // module is imported again only once approved again.
    let before: Approved = match disk::read_local(&machine_fd, names::APPROVED) {
        Ok(None) => Approved::default(),
        Ok(Some(bytes)) => ronoctua_person::parse(&bytes).unwrap_or_else(|r| {
            eprintln!("<4>approved.json was refused ({r}); it starts empty");
            Approved::default()
        }),
        Err(e) => {
            eprintln!("<4>approved.json could not be read ({e}); it starts empty");
            Approved::default()
        }
    };
    let after = decision::decide(&before, &here, request, &computed, now).map_err(|d| Failed {
        denied: d,
        computed: Some(computed.clone()),
        detail: format!(
            "{} is not approved: {} (shown {}, computed {computed})",
            request.id,
            d.id(),
            request.hash
        ),
    })?;
    disk::write_local(
        &machine_fd,
        names::APPROVED,
        ronoctua_person::to_json(&after).as_bytes(),
    )
    .map_err(|e| Failed {
        denied: Denied::Unwritable,
        computed: Some(computed.clone()),
        detail: e,
    })?;
    Ok(computed)
}

/// The person's half: writes the request for `id` at `hash` into `into`
/// (their own approval folder), `0600`, whole.
pub fn request(into: &str, id: &str, hash: &str) -> Result<(), String> {
    let request = Request::new(id, hash);
    request
        .validate()
        .map_err(|r| format!("the request: {r}"))?;
    let me = Whose {
        owner: rustix::process::geteuid().as_raw(),
    };
    let folder = disk::open_root(into, me).map_err(|r| r.to_string())?;
    disk::write_whole(
        &folder,
        REQUEST,
        ronoctua_person::to_json(&request).as_bytes(),
        0o600,
    )
    .map_err(|r| r.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Scratch;

    fn me() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    /// A `nix` whose `eval --impure` answers the hash of the folder in
    /// `RONOCTUA_MODULE_DIR` as the file `hash` inside it says — so the test
    /// controls what root "computes".
    fn fake_nix(s: &Scratch) -> Nix {
        Nix {
            program: s.program(
                "nix",
                "#!/bin/sh\ncase \"$*\" in\n  *--impure*) cat \"$RONOCTUA_MODULE_DIR/hash\" ;;\n  *) exit 2 ;;\nesac\n",
            ),
        }
    }

    fn now() -> Stamp {
        Stamp::parse("2026-10-05T12:00:00Z").unwrap()
    }

    /// What the unit does, as `main` runs it under the lock: the request
    /// taken, then recorded.
    fn approved(s: &Scratch, nix: &Nix) -> Result<String, Failed> {
        let request = take_request(s.0.join("approve").to_str().unwrap(), me())?;
        record(
            &request,
            s.0.join("machine").to_str().unwrap(),
            nix,
            "/nix/store/0000000000000000000000000000000a-person-hash.nix",
            now(),
        )
    }

    /// FAILS IF: an approval is recorded at a hash root did not compute, a
    /// request survives its use, a module the machine lacks is approved — or
    /// any of them is refused without its reason.
    #[test]
    fn the_approval_is_the_hash_root_computes_and_the_request_is_spent() {
        let s = Scratch::new("approve");
        s.dir("approve");
        s.file("machine/person/brain/my-mod/hash", b"sha256-now");
        let nix = fake_nix(&s);
        let stage = |id: &str, hash: &str| {
            request(s.0.join("approve").to_str().unwrap(), id, hash).unwrap();
        };

        stage("my-mod", "sha256-then");
        let e = approved(&s, &nix).unwrap_err();
        assert_eq!(e.denied, Denied::Changed, "{e:?}");
        assert_eq!(e.computed.as_deref(), Some("sha256-now"));
        assert!(
            !s.0.join("approve/request.json").exists(),
            "spent even when refused"
        );
        assert!(!s.0.join("machine/local/approved.json").exists());

        stage("my-other", "sha256-now");
        assert_eq!(approved(&s, &nix).unwrap_err().denied, Denied::NoSuchModule);

        stage("my-mod", "sha256-now");
        assert_eq!(approved(&s, &nix).unwrap(), "sha256-now");
        let approved_file: Approved = ronoctua_person::parse(
            &std::fs::read(s.0.join("machine/local/approved.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(approved_file.modules["my-mod"].hash, "sha256-now");

        assert_eq!(
            approved(&s, &nix).unwrap_err().denied,
            Denied::NoRequest,
            "nothing staged now"
        );
        s.file("approve/request.json", b"{\"format\": 1}");
        assert_eq!(
            approved(&s, &nix).unwrap_err().denied,
            Denied::InvalidRequest
        );

        let broken = Nix {
            program: s.program("broken-nix", "#!/bin/sh\nexit 1\n"),
        };
        stage("my-mod", "sha256-now");
        assert_eq!(
            approved(&s, &broken).unwrap_err().denied,
            Denied::Unhashable
        );
    }

    /// FAILS IF: an approval goes on when the request in the person's folder
    /// is not the one the sudo door was given — another process of theirs
    /// swapped it while the password was being checked — or when root's pin
    /// is missing, readable by others, or a link; or a pin survives its use.
    #[test]
    fn the_request_is_held_to_what_the_door_was_given() {
        use std::os::unix::fs::PermissionsExt as _;
        let s = Scratch::new("approve-pin");
        let pin = s.0.join("approve-pin");
        let pin_at = pin.to_str().unwrap();
        let write_pin = |id: &str, hash: &str, mode: u32| {
            std::fs::write(&pin, ronoctua_person::approve::pin_text(id, hash)).unwrap();
            std::fs::set_permissions(&pin, std::fs::Permissions::from_mode(mode)).unwrap();
        };

        write_pin("my-mod", "sha256-now", 0o600);
        let pinned = take_pin(pin_at, me()).unwrap();
        assert!(!pin.exists(), "a pin outlived its use");
        let swapped = Request::new("my-evil", "sha256-evil");
        let e = held_to_pin(swapped, &pinned).unwrap_err();
        assert_eq!(e.denied, Denied::InvalidRequest, "{e:?}");
        let rehashed = Request::new("my-mod", "sha256-other");
        assert!(held_to_pin(rehashed, &pinned).is_err());
        assert_eq!(
            held_to_pin(Request::new("my-mod", "sha256-now"), &pinned).unwrap(),
            Request::new("my-mod", "sha256-now")
        );

        assert_eq!(
            take_pin(pin_at, me()).unwrap_err().denied,
            Denied::NoRequest
        );
        write_pin("my-mod", "sha256-now", 0o644);
        assert_eq!(
            take_pin(pin_at, me()).unwrap_err().denied,
            Denied::InvalidRequest
        );
        assert!(!pin.exists(), "a refused pin outlived its use");
        write_pin("my-mod", "sha256-now", 0o600);
        assert_eq!(
            take_pin(pin_at, me().wrapping_add(1)).unwrap_err().denied,
            Denied::InvalidRequest,
            "a pin root did not write"
        );
        s.file("elsewhere", b"my-mod\nsha256-now\n");
        std::os::unix::fs::symlink(s.0.join("elsewhere"), &pin).unwrap();
        assert_eq!(
            take_pin(pin_at, me()).unwrap_err().denied,
            Denied::InvalidRequest
        );
        assert!(s.0.join("elsewhere").exists(), "the target is untouched");
        assert!(std::fs::symlink_metadata(&pin).is_err());
    }

    #[test]
    fn a_linked_request_is_refused_and_removed_never_followed() {
        let s = Scratch::new("approve-link");
        s.dir("approve");
        s.file(
            "elsewhere.json",
            br#"{"format":1,"id":"my-mod","hash":"x"}"#,
        );
        std::os::unix::fs::symlink(s.0.join("elsewhere.json"), s.0.join("approve/request.json"))
            .unwrap();
        let e = take_request(s.0.join("approve").to_str().unwrap(), me()).unwrap_err();
        assert_eq!(e.denied, Denied::InvalidRequest);
        assert!(e.detail.starts_with("link"), "{e:?}");
        assert!(
            s.0.join("elsewhere.json").exists(),
            "the target is untouched"
        );
        assert!(std::fs::symlink_metadata(s.0.join("approve/request.json")).is_err());
    }

    #[test]
    fn the_hasher_is_never_an_expression() {
        let s = Scratch::new("approve-hasher");
        let nix = fake_nix(&s);
        for bad in [
            "x.nix",
            "/nix/store/a b.nix",
            "/nix/store/a;b.nix",
            "/nix/store/a\".nix",
        ] {
            assert!(hash_of(&nix, bad, Path::new("/")).is_err(), "{bad}");
        }
    }
}
